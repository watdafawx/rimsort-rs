//! Shared app state: settings, instances, the mod index, and the active list.

use crate::{
    Error, ModId, Result, TaskId, TaskManager,
    dto::{InstanceDto, ListsView, ModDetail, ModRow, SaveResult, SettingsView, SortResultDto},
    mods::{self, ModIndex, ScanConfig},
    modsconfig::{self, ActiveList, ModsConfig},
    paths,
    rules::RuleSources,
    settings::{self, Instance, Settings},
    sort::{self, SortSettings},
    validate::{self, ValidationView},
};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, RwLock},
};

#[derive(Default)]
struct Session {
    index: Arc<ModIndex>,
    rules: Arc<RuleSources>,
    active: ActiveList,
    /// The config as last read from disk (keeps unknown elements / known expansions).
    config: Option<ModsConfig>,
}

pub struct AppState {
    pub tasks: TaskManager,
    settings_path: PathBuf,
    settings: RwLock<Settings>,
    load_warning: Option<String>,
    session: RwLock<Session>,
}

impl AppState {
    pub fn new(tasks: TaskManager) -> Arc<Self> {
        Self::with_settings_path(tasks, settings::data_dir().join("settings.json"))
    }

    pub fn with_settings_path(tasks: TaskManager, settings_path: PathBuf) -> Arc<Self> {
        let loaded = settings::load(&settings_path);
        let mut settings = loaded.settings;
        let mut warning = loaded.warning;
        if loaded.first_run {
            // Prefer an existing RimSort setup; otherwise autodetect a fresh default instance.
            let imported = settings::rimsort_settings_path()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|t| Settings::from_rimsort_json(&t).ok())
                .filter(|s| s.current().is_some_and(|i| !i.game_folder.is_empty()));
            match imported {
                Some(s) => {
                    tracing::info!("first run: imported settings from RimSort");
                    settings = s;
                }
                None => {
                    settings.normalize();
                    let d = paths::autodetect();
                    if let Some(inst) = settings
                        .instances
                        .get_mut(&settings.current_instance.clone())
                    {
                        inst.game_folder = d.game_folder.unwrap_or_default();
                        inst.config_folder = d.config_folder.unwrap_or_default();
                        inst.local_folder = d.local_folder.unwrap_or_default();
                        inst.workshop_folder = d.workshop_folder.unwrap_or_default();
                    }
                    tracing::info!("first run: autodetected paths");
                }
            }
            if let Err(e) = settings.save(&settings_path) {
                warning.get_or_insert(format!("Could not save settings: {e}"));
            }
        }
        Arc::new(Self {
            tasks,
            settings_path,
            settings: RwLock::new(settings),
            load_warning: warning,
            session: RwLock::default(),
        })
    }

    // ── settings & instances ────────────────────────────────────────────

    fn instance_dto(i: &Instance) -> InstanceDto {
        InstanceDto {
            name: i.name.clone(),
            game_folder: i.game_folder.clone(),
            config_folder: i.config_folder.clone(),
            local_folder: i.local_folder.clone(),
            workshop_folder: i.workshop_folder.clone(),
            run_args: i.run_args.clone(),
        }
    }

    pub fn settings_view(&self) -> SettingsView {
        let s = self.settings.read().unwrap();
        let current = s.current().cloned().unwrap_or_default();
        SettingsView {
            current_instance: s.current_instance.clone(),
            instances: s.instances.values().map(Self::instance_dto).collect(),
            sorting_algorithm: s.sorting_algorithm.clone(),
            warning: self.load_warning.clone(),
            checks: paths::validate(&current),
            game_version: paths::game_version(std::path::Path::new(&current.game_folder)),
        }
    }

    fn mutate_settings(&self, f: impl FnOnce(&mut Settings) -> Result<()>) -> Result<()> {
        let mut s = self.settings.write().unwrap();
        f(&mut s)?;
        s.normalize();
        s.save(&self.settings_path)
    }

    pub fn save_instance(&self, dto: InstanceDto) -> Result<()> {
        if dto.name.trim().is_empty() {
            return Err(Error::Other("Instance name cannot be empty".into()));
        }
        self.mutate_settings(|s| {
            let inst = s.instances.entry(dto.name.clone()).or_default();
            inst.name = dto.name.clone();
            inst.game_folder = dto.game_folder;
            inst.config_folder = dto.config_folder;
            inst.local_folder = dto.local_folder;
            inst.workshop_folder = dto.workshop_folder;
            inst.run_args = dto.run_args;
            Ok(())
        })
    }

    pub fn switch_instance(&self, name: &str) -> Result<()> {
        self.mutate_settings(|s| {
            if !s.instances.contains_key(name) {
                return Err(Error::Other(format!("No such instance: {name}")));
            }
            s.current_instance = name.to_owned();
            Ok(())
        })?;
        *self.session.write().unwrap() = Session::default();
        Ok(())
    }

    /// New instance seeded with autodetected paths.
    pub fn create_instance(&self, name: &str) -> Result<()> {
        let name = name.trim();
        self.mutate_settings(|s| {
            if name.is_empty() || s.instances.contains_key(name) {
                return Err(Error::Other(format!(
                    "Instance name '{name}' is empty or already exists"
                )));
            }
            let d = paths::autodetect();
            s.instances.insert(
                name.to_owned(),
                Instance {
                    name: name.to_owned(),
                    game_folder: d.game_folder.unwrap_or_default(),
                    config_folder: d.config_folder.unwrap_or_default(),
                    local_folder: d.local_folder.unwrap_or_default(),
                    workshop_folder: d.workshop_folder.unwrap_or_default(),
                    ..Default::default()
                },
            );
            Ok(())
        })
    }

    pub fn delete_instance(&self, name: &str) -> Result<()> {
        let was_current = {
            let s = self.settings.read().unwrap();
            if s.instances.len() <= 1 {
                return Err(Error::Other("Cannot delete the last instance".into()));
            }
            s.current_instance == name
        };
        self.mutate_settings(|s| {
            s.instances.remove(name);
            Ok(())
        })?;
        if was_current {
            *self.session.write().unwrap() = Session::default();
        }
        Ok(())
    }

    fn current_instance(&self) -> Result<Instance> {
        self.settings
            .read()
            .unwrap()
            .current()
            .cloned()
            .ok_or_else(|| Error::Other("No instance configured".into()))
    }

    fn mods_config_path(inst: &Instance) -> Result<PathBuf> {
        if inst.config_folder.is_empty() {
            return Err(Error::Other(
                "Config folder is not set (Settings → Locations)".into(),
            ));
        }
        Ok(PathBuf::from(&inst.config_folder).join("ModsConfig.xml"))
    }

    // ── scan ────────────────────────────────────────────────────────────

    /// Scan disk and load `ModsConfig.xml` in the background; UI refetches lists when the task finishes.
    pub fn start_scan(self: &Arc<Self>) -> Result<TaskId> {
        let inst = self.current_instance()?;
        if inst.game_folder.is_empty() {
            return Err(Error::Other(
                "Game folder is not set (Settings → Locations)".into(),
            ));
        }
        let settings = self.settings.read().unwrap().clone();
        let prefer_versioned = settings.prefer_versioned_about_tags;
        let this = self.clone();
        Ok(self.tasks.spawn(move |ctx| {
            let game = PathBuf::from(&inst.game_folder);
            let game_version = paths::game_version(&game);
            let rules = Arc::new(RuleSources::load(&settings, &game_version));
            let cfg = ScanConfig {
                game_version,
                rules: rules.clone(),
                game_folder: game,
                local_folder: inst.local_folder.clone().into(),
                workshop_folder: inst.workshop_folder.clone().into(),
                prefer_versioned,
            };
            let index = Arc::new(mods::scan(&cfg, ctx)?);
            let config = match Self::mods_config_path(&inst).and_then(|p| ModsConfig::read(&p)) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!("could not read ModsConfig.xml: {e}");
                    None
                }
            };
            let active = config
                .as_ref()
                .map(|c| modsconfig::resolve_active(&index, &c.active))
                .unwrap_or_default();
            *this.session.write().unwrap() = Session {
                index,
                rules,
                active,
                config,
            };
            Ok(())
        }))
    }

    // ── lists ───────────────────────────────────────────────────────────

    fn row(m: &mods::Mod, game_version: &str, rules: &RuleSources) -> ModRow {
        let unsupported = validate::version_mismatch(m, game_version, rules);
        ModRow {
            id: m.id,
            name: m.name.clone(),
            authors: m.authors.join(", "),
            package_id: m.package_id.clone(),
            mod_type: m.mod_type,
            valid: m.valid,
            unsupported_version: unsupported,
            published_file_id: m.published_file_id.clone(),
        }
    }

    pub fn lists(&self) -> ListsView {
        let s = self.session.read().unwrap();
        let active_set: HashSet<ModId> = s.active.ids.iter().copied().collect();
        ListsView {
            active: s
                .active
                .ids
                .iter()
                .filter_map(|id| s.index.get(*id))
                .map(|m| Self::row(m, &s.index.game_version, &s.rules))
                .collect(),
            inactive: s
                .index
                .mods
                .iter()
                .filter(|m| !active_set.contains(&m.id))
                .map(|m| Self::row(m, &s.index.game_version, &s.rules))
                .collect(),
            missing: s.active.missing.clone(),
            game_version: s.index.game_version.clone(),
            scan_ms: s.index.scan_ms as u32,
            duplicate_package_ids: s.index.duplicate_package_count() as u32,
            community_rules: s.rules.community.as_ref().map_or(0, |r| r.len() as u32),
            user_rules: s.rules.user.as_ref().map_or(0, |r| r.len() as u32),
        }
    }

    pub fn mod_detail(&self, id: ModId) -> Option<ModDetail> {
        let s = self.session.read().unwrap();
        let m = s.index.get(id)?;
        Some(ModDetail {
            id: m.id,
            name: m.name.clone(),
            package_id: m.package_id.clone(),
            authors: m.authors.clone(),
            description: m.description.clone(),
            path: m.path.to_string_lossy().into_owned(),
            url: m.url.clone(),
            mod_version: m.mod_version.clone(),
            supported_versions: m.supported_versions.clone(),
            mod_type: m.mod_type,
            valid: m.valid,
            invalid_reason: m.invalid_reason.clone(),
            published_file_id: m.published_file_id.clone(),
            dependencies: m
                .rules
                .dependencies
                .iter()
                .map(|d| d.package_id.clone())
                .collect(),
            load_after: m.load_after_all().cloned().collect(),
            load_before: m.load_before_all().cloned().collect(),
            incompatible_with: m.rules.incompatible_with.clone(),
            preview: Some(m.path.join("About").join("Preview.png"))
                .filter(|p| p.is_file())
                .map(|p| p.to_string_lossy().into_owned()),
        })
    }

    /// Replace the active order. Unknown/invalid ids and duplicates are dropped.
    pub fn set_active(&self, ids: Vec<ModId>) {
        let mut s = self.session.write().unwrap();
        let mut seen = HashSet::new();
        let ids: Vec<ModId> = ids
            .into_iter()
            .filter(|id| s.index.get(*id).is_some_and(|m| m.valid) && seen.insert(*id))
            .collect();
        s.active.steam_suffix.retain(|id| seen.contains(id));
        s.active.ids = ids;
    }

    /// Warnings for the active list.
    pub fn validate(&self) -> ValidationView {
        let use_alt = self
            .settings
            .read()
            .unwrap()
            .use_alternative_package_ids_as_satisfying_dependencies;
        let s = self.session.read().unwrap();
        validate::validate(&s.index, &s.active.ids, &s.rules, use_alt)
    }

    // ── sort & save ─────────────────────────────────────────────────────

    pub fn sort_active(&self) -> SortResultDto {
        let settings = {
            let s = self.settings.read().unwrap();
            SortSettings {
                dependencies_as_load_after: s.use_moddependencies_as_load_these_before,
                use_alternative_ids: s.use_alternative_package_ids_as_satisfying_dependencies,
            }
        };
        let mut s = self.session.write().unwrap();
        let out = sort::sort_active(&s.index, &s.active.ids, settings);
        if !out.cycles.is_empty() {
            return SortResultDto {
                ok: false,
                cycles: out.cycles,
                changed: false,
            };
        }
        let changed = out.order != s.active.ids;
        s.active.ids = out.order;
        SortResultDto {
            ok: true,
            cycles: vec![],
            changed,
        }
    }

    pub fn save_mods_config(&self) -> Result<SaveResult> {
        let inst = self.current_instance()?;
        let path = Self::mods_config_path(&inst)?;
        let s = self.session.read().unwrap();
        let mut config = s
            .config
            .clone()
            .unwrap_or_else(|| ModsConfig::new(&s.index.game_version));
        config.active = modsconfig::config_ids(&s.index, &s.active);
        if !s.index.game_version.is_empty() {
            config.version = s.index.game_version.clone();
        }
        let backup = config.write(&path)?;
        tracing::info!(
            "saved {} active mods to {}",
            config.active.len(),
            path.display()
        );
        Ok(SaveResult {
            path: path.to_string_lossy().into_owned(),
            backup: backup.map(|b| b.to_string_lossy().into_owned()),
            count: config.active.len() as u32,
        })
    }

    pub fn launch_game(&self) -> Result<()> {
        crate::launch::launch(&self.current_instance()?)
    }

    pub fn autodetect(&self) -> paths::DetectedPaths {
        paths::autodetect()
    }
}
