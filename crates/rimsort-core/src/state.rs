//! Shared app state: settings, instances, the mod index, and the active list.

use crate::{
    Error, ModId, Result, TaskId, TaskManager,
    dto::{
        ExportFormat, ImportResult, InstanceDto, ListsView, LogChunk, ModDetail, ModRow,
        ModRulesView, OptionsDto, SaveResult, SettingsView, SortResultDto, UserRuleDto,
    },
    meta, modlist_io,
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
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Session {
    index: Arc<ModIndex>,
    meta: meta::MetaMap,
    meta_path: PathBuf,
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
    watcher: Mutex<Option<crate::watch::FsWatch>>,
    /// When we last wrote ModsConfig.xml, so our own save isn't reported as an external change.
    last_save: Arc<Mutex<Option<Instant>>>,
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
            watcher: Mutex::new(None),
            last_save: Arc::default(),
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
            launch_via_steam: i
                .extra
                .get("launch_via_steam_protocol")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }

    pub fn settings_view(&self) -> SettingsView {
        let s = self.settings.read().unwrap();
        let current = s.current().cloned().unwrap_or_default();
        SettingsView {
            current_instance: s.current_instance.clone(),
            instances: s.instances.values().map(Self::instance_dto).collect(),
            sorting_algorithm: s.sorting_algorithm.clone(),
            options: OptionsDto {
                dependencies_as_load_after: s.use_moddependencies_as_load_these_before,
                use_alternative_ids: s.use_alternative_package_ids_as_satisfying_dependencies,
                prefer_versioned: s.prefer_versioned_about_tags,
            },
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
            inst.extra.insert(
                "launch_via_steam_protocol".into(),
                serde_json::Value::Bool(dto.launch_via_steam),
            );
            Ok(())
        })
    }

    pub fn update_options(&self, o: OptionsDto) -> Result<()> {
        self.mutate_settings(|s| {
            s.use_moddependencies_as_load_these_before = o.dependencies_as_load_after;
            s.use_alternative_package_ids_as_satisfying_dependencies = o.use_alternative_ids;
            s.prefer_versioned_about_tags = o.prefer_versioned;
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
    ///
    /// `keep_active`: rescan disk but keep the current in-memory active list (unsaved edits survive)
    /// instead of re-reading `ModsConfig.xml`.
    pub fn start_scan(self: &Arc<Self>, keep_active: bool) -> Result<TaskId> {
        let inst = self.current_instance()?;
        if inst.game_folder.is_empty() {
            return Err(Error::Other(
                "Game folder is not set (Settings → Locations)".into(),
            ));
        }
        let settings = self.settings.read().unwrap().clone();
        let prefer_versioned = settings.prefer_versioned_about_tags;
        self.restart_watch(&inst);
        let kept: Option<Vec<String>> = keep_active.then(|| {
            let s = self.session.read().unwrap();
            modsconfig::config_ids(&s.index, &s.active)
        });
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
            let active = match (&kept, &config) {
                (Some(ids), _) if !ids.is_empty() => modsconfig::resolve_active(&index, ids),
                (_, Some(c)) => modsconfig::resolve_active(&index, &c.active),
                _ => Default::default(),
            };
            // Personal metadata: our file, else a one-time import from RimSort's aux DB.
            let meta_path = meta::meta_path(&inst.name);
            let meta = if meta_path.exists() {
                meta::load(&meta_path)
            } else {
                let m = meta::import_rimsort(&inst.name, &index);
                if !m.is_empty() {
                    let _ = meta::save(&meta_path, &m);
                }
                m
            };
            let mut rules = rules;
            Arc::make_mut(&mut rules).workshop =
                crate::steamacf::load(std::path::Path::new(&inst.workshop_folder));
            Arc::make_mut(&mut rules).ignored.extend(
                meta.iter()
                    .filter(|(_, m)| m.ignore)
                    .map(|(p, _)| p.clone()),
            );
            *this.session.write().unwrap() = Session {
                index,
                meta,
                meta_path,
                rules,
                active,
                config,
            };
            Ok(())
        }))
    }

    /// (Re)start watching the instance's mods and config folders.
    fn restart_watch(&self, inst: &Instance) {
        let dirs: Vec<PathBuf> = [&inst.local_folder, &inst.workshop_folder]
            .into_iter()
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .collect();
        let cfg = (!inst.config_folder.is_empty()).then(|| PathBuf::from(&inst.config_folder));
        let sink = self.tasks.sink();
        let last_save = self.last_save.clone();
        let watch = crate::watch::start(dirs, cfg, move |c| {
            let what = match c {
                crate::watch::Change::Mods => "mods",
                crate::watch::Change::Config => {
                    let ours = last_save
                        .lock()
                        .unwrap()
                        .is_some_and(|t| t.elapsed() < Duration::from_secs(5));
                    if ours {
                        return;
                    }
                    "config"
                }
            };
            sink.emit(crate::TaskEvent::FsChanged { what: what.into() });
        });
        match watch {
            Ok(w) => *self.watcher.lock().unwrap() = Some(w),
            Err(e) => tracing::warn!("file watching unavailable: {e}"),
        }
    }

    // ── lists ───────────────────────────────────────────────────────────

    fn row(m: &mods::Mod, game_version: &str, rules: &RuleSources, meta: &meta::MetaMap) -> ModRow {
        let md = meta.get(&m.package_id);
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
            modified: m.mtime.clamp(0, i64::from(u32::MAX)) as u32,
            color: md.and_then(|d| d.color.clone()),
            tags: md.map(|d| d.tags.clone()).unwrap_or_default(),
            has_note: md.is_some_and(|d| !d.note.is_empty()),
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
                .map(|m| Self::row(m, &s.index.game_version, &s.rules, &s.meta))
                .collect(),
            inactive: s
                .index
                .mods
                .iter()
                .filter(|m| !active_set.contains(&m.id))
                .map(|m| Self::row(m, &s.index.game_version, &s.rules, &s.meta))
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
            color: s.meta.get(&m.package_id).and_then(|d| d.color.clone()),
            tags: s
                .meta
                .get(&m.package_id)
                .map(|d| d.tags.clone())
                .unwrap_or_default(),
            note: s
                .meta
                .get(&m.package_id)
                .map(|d| d.note.clone())
                .unwrap_or_default(),
            workshop_updated: m
                .published_file_id
                .as_ref()
                .and_then(|p| s.rules.workshop.get(p))
                .map(|t| t.updated)
                .filter(|u| *u > 0),
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

    /// Required-but-inactive packages for the active list.
    pub fn missing_dependencies(&self) -> Vec<validate::MissingDep> {
        let use_alt = self
            .settings
            .read()
            .unwrap()
            .use_alternative_package_ids_as_satisfying_dependencies;
        let s = self.session.read().unwrap();
        validate::missing_dependencies(&s.index, &s.active.ids, &s.rules, use_alt)
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
            let details = sort::explain_cycles(&s.index, &out.cycles);
            return SortResultDto {
                ok: false,
                cycles: out
                    .cycles
                    .into_iter()
                    .zip(details)
                    .map(|(members, rules)| crate::dto::CycleDto { members, rules })
                    .collect(),
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
        *self.last_save.lock().unwrap() = Some(Instant::now());
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

    /// Replace the active list from a mod list file (any supported format). Caller refetches lists.
    pub fn import_modlist(&self, path: &str) -> Result<ImportResult> {
        let text = crate::xml::decode_bytes(&std::fs::read(path)?);
        let parsed = modlist_io::parse_list(&text)?;
        let mut s = self.session.write().unwrap();
        let active = modsconfig::resolve_active(&s.index, &parsed.package_ids);
        let result = ImportResult {
            imported: active.ids.len() as u32,
            missing: active.missing.clone(),
        };
        s.active = active;
        Ok(result)
    }

    /// Render the active list in `format`.
    pub fn export_text(&self, format: ExportFormat) -> String {
        let s = self.session.read().unwrap();
        let ids = modsconfig::config_ids(&s.index, &s.active);
        let version = s.index.game_version.clone();
        let known = s
            .config
            .as_ref()
            .map(|c| c.known_expansions.clone())
            .unwrap_or_default();
        match format {
            ExportFormat::PackageIds => ids.join(
                "
",
            ),
            ExportFormat::Json => modlist_io::to_json(&version, &ids, &known),
            ExportFormat::Xml => {
                let mut c = s
                    .config
                    .clone()
                    .unwrap_or_else(|| ModsConfig::new(&version));
                c.active = ids;
                if !version.is_empty() {
                    c.version = version;
                }
                c.to_xml()
            }
            ExportFormat::Report => {
                let mut out = format!(
                    "Created with RimSort-rs
RimWorld game version this list was created for: {version}
Total # of mods: {}
",
                    s.active.ids.len()
                );
                for m in s.active.ids.iter().filter_map(|id| s.index.get(*id)) {
                    let url = if !m.url.is_empty() {
                        m.url.clone()
                    } else if let Some(p) = &m.published_file_id {
                        format!("https://steamcommunity.com/sharedfiles/filedetails/?id={p}")
                    } else {
                        "No url specified".into()
                    };
                    out.push_str(&format!(
                        "
{} [{}][{url}]",
                        m.name, m.package_id
                    ));
                }
                out
            }
        }
    }

    pub fn export_modlist(&self, path: &str, format: ExportFormat) -> Result<()> {
        std::fs::write(path, self.export_text(format))?;
        Ok(())
    }

    /// Read Player.log incrementally (`None` = tail of the file).
    pub fn read_player_log(&self, offset: Option<u32>) -> Result<LogChunk> {
        let inst = self.current_instance()?;
        let path = crate::logs::player_log_path(&inst.config_folder)
            .ok_or_else(|| Error::Other("Config folder is not set".into()))?;
        let c = crate::logs::read_from(&path, offset.map(u64::from)).map_err(|_| {
            Error::Other(format!(
                "No Player.log at {} (run the game once)",
                path.display()
            ))
        })?;
        Ok(LogChunk {
            path: path.to_string_lossy().into_owned(),
            text: c.text,
            offset: c.offset.min(u64::from(u32::MAX)) as u32,
            reset: c.reset,
        })
    }

    /// Move a mod's folder to the OS trash and drop it from the index and lists.
    /// Base game / DLC can't be deleted. Nothing is removed if the trash operation fails.
    pub fn delete_mod(&self, id: ModId) -> Result<()> {
        let mut s = self.session.write().unwrap();
        let m = s
            .index
            .get(id)
            .ok_or_else(|| Error::Other("Unknown mod".into()))?;
        if m.mod_type == mods::ModType::Ludeon {
            return Err(Error::Other(
                "The base game and expansions can't be deleted here".into(),
            ));
        }
        let path = m.path.clone();
        trash::delete(&path).map_err(|e| {
            Error::Other(format!(
                "Could not move {} to the recycle bin: {e}",
                path.display()
            ))
        })?;
        tracing::info!("moved {} to trash", path.display());
        let mods: Vec<mods::Mod> = s
            .index
            .mods
            .iter()
            .filter(|m| m.id != id)
            .cloned()
            .collect();
        s.index = Arc::new(ModIndex::new(
            mods,
            s.index.game_version.clone(),
            s.index.scan_ms,
        ));
        s.active.ids.retain(|a| *a != id);
        s.active.steam_suffix.remove(&id);
        Ok(())
    }

    /// `git pull --ff-only` in a git-cloned mod's folder. Blocking: call from a worker thread.
    pub fn update_git_mod(&self, id: ModId) -> Result<String> {
        let path = {
            let s = self.session.read().unwrap();
            let m = s
                .index
                .get(id)
                .ok_or_else(|| Error::Other("Unknown mod".into()))?;
            m.path.clone()
        };
        crate::gitmods::update(&path)
    }

    /// Copy a mod's folder into the local mods folder so it can be edited (background task).
    /// The copy's name is the original folder name with a `-local` suffix; refuses to overwrite.
    pub fn create_local_copy(self: &Arc<Self>, id: ModId) -> Result<TaskId> {
        let inst = self.current_instance()?;
        if inst.local_folder.is_empty() {
            return Err(Error::Other(
                "Local mods folder is not set (Settings → Locations)".into(),
            ));
        }
        let (src, folder, name) = {
            let s = self.session.read().unwrap();
            let m = s
                .index
                .get(id)
                .ok_or_else(|| Error::Other("Unknown mod".into()))?;
            (m.path.clone(), m.folder.clone(), m.name.clone())
        };
        let dest = PathBuf::from(&inst.local_folder).join(format!("{folder}-local"));
        if dest.exists() {
            return Err(Error::Other(format!("{} already exists", dest.display())));
        }
        Ok(self.tasks.spawn(move |ctx| {
            ctx.progress(0, 1, format!("Copying {name}"));
            if let Err(e) = crate::steamcmd::copy_dir(&src, &dest) {
                let _ = std::fs::remove_dir_all(&dest); // no half copies
                return Err(e);
            }
            ctx.progress(1, 1, "Done");
            Ok(())
        }))
    }

    /// Clone GitHub repositories into the instance's local mods folder (background task).
    pub fn clone_git_mods(self: &Arc<Self>, urls: Vec<String>) -> Result<TaskId> {
        let inst = self.current_instance()?;
        if inst.local_folder.is_empty() {
            return Err(Error::Other(
                "Local mods folder is not set (Settings → Locations)".into(),
            ));
        }
        Ok(self.tasks.spawn(move |ctx| {
            crate::gitmods::clone_github(&urls, std::path::Path::new(&inst.local_folder), ctx)
                .map(|_| ())
        }))
    }

    /// Download Workshop items with SteamCMD into the instance's local mods folder (background task).
    pub fn download_mods(self: &Arc<Self>, ids: Vec<String>) -> Result<TaskId> {
        let inst = self.current_instance()?;
        if inst.local_folder.is_empty() {
            return Err(Error::Other(
                "Local mods folder is not set (Settings → Locations)".into(),
            ));
        }
        Ok(self.tasks.spawn(move |ctx| {
            let failed = crate::steamcmd::download(
                &ids,
                std::path::Path::new(&inst.local_folder),
                &crate::steamcmd::install_dir(),
                ctx,
            )?;
            if failed.is_empty() {
                Ok(())
            } else {
                Err(Error::Other(format!(
                    "Steam could not provide: {}",
                    failed.join(", ")
                )))
            }
        }))
    }

    /// Automatic ModsConfig.xml backups for the current instance, newest first.
    pub fn list_backups(&self) -> Result<Vec<crate::dto::BackupInfo>> {
        let path = Self::mods_config_path(&self.current_instance()?)?;
        Ok(modsconfig::list_backups(&path)
            .into_iter()
            .map(|b| crate::dto::BackupInfo {
                path: b.path.to_string_lossy().into_owned(),
                unix: b.unix.min(u64::from(u32::MAX)) as u32,
                count: b.count as u32,
            })
            .collect())
    }

    /// Package ids installed more than once, with which copy is active.
    pub fn duplicates(&self) -> Vec<crate::dto::DupGroup> {
        use crate::dto::{DupCopy, DupGroup};
        let s = self.session.read().unwrap();
        let active: HashSet<ModId> = s.active.ids.iter().copied().collect();
        let mut groups: std::collections::BTreeMap<&str, Vec<&mods::Mod>> = Default::default();
        for m in s.index.mods.iter().filter(|m| m.valid) {
            groups.entry(m.package_id.as_str()).or_default().push(m);
        }
        let mut out: Vec<DupGroup> = groups
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(pid, v)| DupGroup {
                package_id: pid.to_owned(),
                name: v[0].name.clone(),
                copies: v
                    .iter()
                    .map(|m| DupCopy {
                        id: m.id,
                        path: m.path.to_string_lossy().into_owned(),
                        mod_type: m.mod_type,
                        published_file_id: m.published_file_id.clone(),
                        mod_version: m.mod_version.clone(),
                        active: active.contains(&m.id),
                    })
                    .collect(),
            })
            .collect();
        out.sort_by_key(|g| g.name.to_lowercase());
        out
    }

    // ── rules editing ───────────────────────────────────────────────────

    pub fn mod_rules(&self, id: ModId) -> Option<ModRulesView> {
        let s = self.session.read().unwrap();
        let m = s.index.get(id)?;
        Some(ModRulesView {
            package_id: m.package_id.clone(),
            name: m.name.clone(),
            about_load_after: m.rules.load_after.clone(),
            about_load_before: m.rules.load_before.clone(),
            community_load_after: m.community.load_after.clone(),
            community_load_before: m.community.load_before.clone(),
            community_top: m.community.load_top,
            community_bottom: m.community.load_bottom,
            user: UserRuleDto {
                load_after: m.user.load_after.clone(),
                load_before: m.user.load_before.clone(),
                load_top: m.user.load_top,
                load_bottom: m.user.load_bottom,
            },
            ignored: s.rules.ignored.contains(&m.package_id),
        })
    }

    /// Save user rules for a mod: persisted to our `userRules.json` and applied immediately.
    pub fn set_user_rule(&self, id: ModId, rule: UserRuleDto) -> Result<()> {
        let clean = |v: Vec<String>, own: &str| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for x in v
                .into_iter()
                .map(|x| x.trim().to_lowercase())
                .filter(|x| !x.is_empty() && x != own)
            {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
            out
        };
        let mut s = self.session.write().unwrap();
        let pid = s
            .index
            .get(id)
            .map(|m| m.package_id.clone())
            .ok_or_else(|| Error::Other("Unknown mod".into()))?;
        let ext = crate::rules::ExtRule {
            load_after: clean(rule.load_after, &pid),
            load_before: clean(rule.load_before, &pid),
            load_top: rule.load_top,
            load_bottom: rule.load_bottom,
        };
        let name_of = |p: &str| {
            s.index
                .by_package(p)
                .next()
                .map_or_else(|| p.to_owned(), |m| m.name.clone())
        };
        let text = crate::rules::write_user_rule(
            crate::rules::read_db_text("userRules.json").as_deref(),
            &pid,
            &ext,
            name_of,
        )?;
        settings::atomic_write(
            &crate::rules::own_db_path("userRules.json"),
            text.as_bytes(),
        )?;

        // Apply without a rescan (a rescan would discard unsaved list edits).
        let mut mods = s.index.mods.clone();
        for m in mods.iter_mut().filter(|m| m.package_id == pid) {
            m.user = ext.clone();
        }
        s.index = Arc::new(ModIndex::new(
            mods,
            s.index.game_version.clone(),
            s.index.scan_ms,
        ));
        let rules = Arc::make_mut(&mut s.rules);
        rules
            .user
            .get_or_insert_with(Default::default)
            .set(&pid, ext);
        Ok(())
    }

    /// Suppress (or restore) warnings for a mod (stored with its personal metadata).
    pub fn set_ignored(&self, id: ModId, ignored: bool) -> Result<()> {
        let mut s = self.session.write().unwrap();
        let pid = s
            .index
            .get(id)
            .map(|m| m.package_id.clone())
            .ok_or_else(|| Error::Other("Unknown mod".into()))?;
        s.meta.entry(pid.clone()).or_default().ignore = ignored;
        meta::save(&s.meta_path, &s.meta)?;
        let rules = Arc::make_mut(&mut s.rules);
        if ignored {
            rules.ignored.insert(pid);
        } else {
            rules.ignored.remove(&pid);
        }
        Ok(())
    }

    /// Set a mod's color, tags and note.
    pub fn set_mod_meta(&self, id: ModId, dto: crate::dto::MetaDto) -> Result<()> {
        let mut s = self.session.write().unwrap();
        let pid = s
            .index
            .get(id)
            .map(|m| m.package_id.clone())
            .ok_or_else(|| Error::Other("Unknown mod".into()))?;
        let color = dto.color.map(|c| c.trim().to_lowercase()).filter(|c| {
            c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit())
        });
        let mut tags: Vec<String> = Vec::new();
        for t in dto
            .tags
            .into_iter()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
        {
            if !tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                tags.push(t);
            }
        }
        let m = s.meta.entry(pid).or_default();
        (m.color, m.tags, m.note) = (color, tags, dto.note.trim().to_owned());
        meta::save(&s.meta_path, &s.meta)
    }

    pub fn launch_game(&self) -> Result<()> {
        crate::launch::launch(&self.current_instance()?)
    }

    pub fn autodetect(&self) -> paths::DetectedPaths {
        paths::autodetect()
    }
}
