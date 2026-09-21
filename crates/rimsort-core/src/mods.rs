//! Mod model, About.xml parsing, parallel scan, and the in-memory index.

use crate::{
    ModId, Result, TaskCtx,
    rules::{ExtRule, RuleSources},
    xml::{self, Node},
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Instant,
};

pub const MISSING_PACKAGE_ID: &str = "missing.packageid";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub enum ModType {
    Local,
    SteamWorkshop,
    SteamCmd,
    Ludeon,
    Git,
    Unknown,
}

/// (appid, package id, display name, description) for base game + official DLC.
pub const DLC: &[(u32, &str, &str, &str)] = &[
    (294100, "ludeon.rimworld", "RimWorld", "Base game"),
    (
        1149640,
        "ludeon.rimworld.royalty",
        "RimWorld - Royalty",
        "DLC #1",
    ),
    (
        1392840,
        "ludeon.rimworld.ideology",
        "RimWorld - Ideology",
        "DLC #2",
    ),
    (
        1826140,
        "ludeon.rimworld.biotech",
        "RimWorld - Biotech",
        "DLC #3",
    ),
    (
        2380740,
        "ludeon.rimworld.anomaly",
        "RimWorld - Anomaly",
        "DLC #4",
    ),
    (
        3022790,
        "ludeon.rimworld.odyssey",
        "RimWorld - Odyssey",
        "DLC #5",
    ),
];

#[derive(Debug, Clone, Default)]
pub struct Dependency {
    pub package_id: String,
    pub display_name: String,
    pub workshop_url: String,
    pub alternatives: Vec<String>,
}

/// Load-order rules from About.xml (community/user rules join in step 12).
#[derive(Debug, Clone, Default)]
pub struct Rules {
    pub load_before: Vec<String>,
    pub load_after: Vec<String>,
    pub incompatible_with: Vec<String>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone)]
pub struct Mod {
    pub id: ModId,
    pub path: PathBuf,
    pub folder: String,
    pub mod_type: ModType,
    /// False when About.xml is missing/unparseable.
    pub valid: bool,
    pub invalid_reason: Option<String>,
    /// Lowercased.
    pub package_id: String,
    pub name: String,
    pub authors: Vec<String>,
    pub description: String,
    pub url: String,
    pub mod_version: String,
    pub supported_versions: Vec<String>,
    pub published_file_id: Option<String>,
    pub steam_app_id: Option<u32>,
    pub rules: Rules,
    /// Community / user rules for this package id (empty when none).
    pub community: ExtRule,
    pub user: ExtRule,
    /// Unix seconds of the folder mtime, -1 if unknown.
    pub mtime: i64,
}

/// Which folder a candidate mod directory came from.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Data,
    Local,
    Workshop,
}

#[derive(Debug, Clone, Default)]
pub struct ScanConfig {
    pub game_folder: PathBuf,
    pub local_folder: PathBuf,
    pub workshop_folder: PathBuf,
    /// Contents of `Version.txt`, e.g. `1.6.4871 rev590`.
    pub game_version: String,
    pub prefer_versioned: bool,
    pub rules: Arc<RuleSources>,
}

/// `1.6.4871 rev590` -> `("1", "6")`.
fn major_minor(version: &str) -> Option<(&str, &str)> {
    let mut it = version.trim().split('.');
    Some((
        it.next()?,
        it.next()?.split(|c: char| !c.is_ascii_digit()).next()?,
    ))
}

/// Find the `*ByVersion` child matching the game version (`v1.6`, `1.6`, or `v1.6.x` prefix).
fn match_by_version<'a>(by_version: &'a Node, game_version: &str) -> Option<&'a Node> {
    let (major, minor) = major_minor(game_version)?;
    let want = format!("v{major}.{minor}");
    let bare = format!("{major}.{minor}");
    by_version
        .children
        .iter()
        .find(|c| c.name == want || c.name == bare)
        .or_else(|| {
            by_version
                .children
                .iter()
                .find(|c| c.name.starts_with(&want))
        })
}

/// Base `<key>` list, replaced by `<keyByVersion>` when a version matches and `prefer_versioned`.
fn versioned_list(m: &Node, key: &str, cfg: &ScanConfig) -> Vec<String> {
    let mut list = m.child(key).map(Node::li_texts_rimsort).unwrap_or_default();
    if cfg.prefer_versioned
        && let Some(v) = m
            .child(&format!("{key}ByVersion"))
            .and_then(|bv| match_by_version(bv, &cfg.game_version))
    {
        list = v.li_texts_rimsort();
    }
    list
}

fn parse_dependency(li: &Node) -> Option<Dependency> {
    let package_id = li.child_text("packageId")?.to_lowercase();
    Some(Dependency {
        package_id,
        display_name: li.child_text("displayName").unwrap_or_default().into(),
        workshop_url: li
            .child_text("steamWorkshopUrl")
            .or_else(|| li.child_text("workshopUrl"))
            .unwrap_or_default()
            .into(),
        alternatives: li
            .child("alternativePackageIds")
            .map(Node::li_texts)
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect(),
    })
}

fn parse_rules(m: &Node, cfg: &ScanConfig) -> Rules {
    let mut deps_node = m.child("modDependencies");
    if cfg.prefer_versioned
        && let Some(v) = m
            .child("modDependenciesByVersion")
            .and_then(|bv| match_by_version(bv, &cfg.game_version))
    {
        deps_node = Some(v);
    }
    let mut dependencies: Vec<Dependency> = Vec::new();
    for dep in deps_node
        .into_iter()
        .flat_map(|n| n.children_named("li"))
        .filter_map(parse_dependency)
    {
        if !dependencies.iter().any(|d| d.package_id == dep.package_id) {
            dependencies.push(dep);
        }
    }

    let lower = |v: Vec<String>| v.into_iter().map(|s| s.to_lowercase()).collect::<Vec<_>>();
    let with_force = |key: &str, force: &str| {
        let mut v = versioned_list(m, key, cfg);
        v.extend(
            m.child(force)
                .map(Node::li_texts_rimsort)
                .unwrap_or_default(),
        ); // forceLoad* always applies
        lower(v)
    };
    Rules {
        load_before: with_force("loadBefore", "forceLoadBefore"),
        load_after: with_force("loadAfter", "forceLoadAfter"),
        incompatible_with: lower(versioned_list(m, "incompatibleWith", cfg)),
        dependencies,
    }
}

fn find_about_xml(mod_path: &Path) -> Option<PathBuf> {
    let about = fs::read_dir(mod_path)
        .ok()?
        .flatten()
        .find(|e| e.file_name().eq_ignore_ascii_case("about") && e.path().is_dir())?;
    fs::read_dir(about.path())
        .ok()?
        .flatten()
        .find(|e| e.file_name().eq_ignore_ascii_case("about.xml") && e.path().is_file())
        .map(|e| e.path())
}

fn read_published_file_id(path: &Path, folder: &str) -> Option<String> {
    if let Ok(bytes) = fs::read(path.join("About").join("PublishedFileId.txt")) {
        let s = xml::decode_bytes(&bytes);
        let s = s.trim();
        return (s.bytes().all(|b| b.is_ascii_digit()) && s.parse::<u64>().is_ok_and(|n| n > 0))
            .then(|| s.to_owned());
    }
    (folder.bytes().all(|b| b.is_ascii_digit()) && folder.parse::<u64>().is_ok_and(|n| n > 0))
        .then(|| folder.to_owned())
}

fn mod_type(source: Source, path: &Path, folder: &str) -> ModType {
    match source {
        Source::Data => ModType::Ludeon,
        Source::Workshop => ModType::SteamWorkshop,
        Source::Local => {
            let pfid_file = path.join("About").join("PublishedFileId.txt");
            if pfid_file.exists() && read_published_file_id(path, folder).as_deref() == Some(folder)
            {
                ModType::SteamCmd // SteamCMD names folders after the PublishedFileId
            } else if path.join(".git").exists() {
                ModType::Git
            } else if pfid_file.exists() {
                ModType::SteamCmd
            } else {
                ModType::Local
            }
        }
    }
}

/// Parse one mod directory. Never fails: bad mods come back `valid == false` with a reason.
fn parse_mod(path: &Path, source: Source, cfg: &ScanConfig) -> Mod {
    let folder = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(-1, |d| d.as_secs() as i64);
    let mut m = Mod {
        id: ModId::from_path(path),
        path: path.to_path_buf(),
        mod_type: mod_type(source, path, &folder),
        published_file_id: read_published_file_id(path, &folder),
        name: folder.clone(),
        folder,
        valid: false,
        invalid_reason: None,
        package_id: MISSING_PACKAGE_ID.into(),
        authors: vec![],
        description: String::new(),
        url: String::new(),
        mod_version: String::new(),
        supported_versions: vec![],
        steam_app_id: None,
        rules: Rules::default(),
        community: ExtRule::default(),
        user: ExtRule::default(),
        mtime,
    };

    let Some(about) = find_about_xml(path) else {
        m.invalid_reason = Some("No About/About.xml — likely a leftover folder.".into());
        return m;
    };
    let text = match fs::read(&about) {
        Ok(b) => xml::decode_bytes(&b),
        Err(e) => {
            m.invalid_reason = Some(format!("Cannot read About.xml: {e}"));
            return m;
        }
    };
    let root = xml::parse(&text);
    let Some(meta) = root.child("ModMetadata") else {
        m.invalid_reason = Some("About.xml has no <ModMetadata> element.".into());
        return m;
    };

    m.valid = true;
    match meta.child_text("packageId") {
        Some(pid) => m.package_id = pid.to_lowercase(),
        None => m.invalid_reason = Some("Missing <packageId>.".into()),
    }
    let dlc = DLC.iter().find(|d| d.1 == m.package_id);
    m.steam_app_id = meta
        .child_text("steamAppId")
        .and_then(|s| s.parse().ok())
        .or(dlc.map(|d| d.0));
    m.name = meta
        .child_text("name")
        .map(String::from)
        .or(dlc.map(|d| d.2.to_owned()))
        .unwrap_or_else(|| m.package_id.clone());
    m.description = meta
        .child_text("description")
        .map(String::from)
        .or(dlc.map(|d| d.3.to_owned()))
        .unwrap_or_default();
    if cfg.prefer_versioned
        && let Some(d) = meta
            .child("descriptionsByVersion")
            .and_then(|bv| match_by_version(bv, &cfg.game_version))
            .map(Node::trimmed)
            .filter(|s| !s.is_empty())
    {
        m.description = d.to_owned();
    }
    m.authors
        .extend(meta.child_text("author").map(String::from));
    m.authors.extend(
        meta.child("authors")
            .map(Node::li_texts)
            .unwrap_or_default(),
    );
    m.supported_versions = meta
        .child("supportedVersions")
        .map(Node::li_texts)
        .unwrap_or_default();
    m.url = meta.child_text("url").unwrap_or_default().into();
    m.mod_version = meta.child_text("modVersion").unwrap_or_default().into();
    m.rules = parse_rules(meta, cfg);
    m.community = cfg
        .rules
        .community
        .as_ref()
        .and_then(|r| r.get(&m.package_id))
        .cloned()
        .unwrap_or_default();
    m.user = cfg
        .rules
        .user
        .as_ref()
        .and_then(|r| r.get(&m.package_id))
        .cloned()
        .unwrap_or_default();
    m
}

impl Mod {
    /// loadAfter from About.xml + community + user rules.
    pub fn load_after_all(&self) -> impl Iterator<Item = &String> {
        self.rules
            .load_after
            .iter()
            .chain(&self.community.load_after)
            .chain(&self.user.load_after)
    }

    /// loadBefore from About.xml + community + user rules.
    pub fn load_before_all(&self) -> impl Iterator<Item = &String> {
        self.rules
            .load_before
            .iter()
            .chain(&self.community.load_before)
            .chain(&self.user.load_before)
    }

    pub fn load_first(&self) -> bool {
        self.community.load_top || self.user.load_top
    }

    pub fn load_last(&self) -> bool {
        self.community.load_bottom || self.user.load_bottom
    }
}

/// Immutable snapshot of everything found on disk.
#[derive(Debug, Default)]
pub struct ModIndex {
    /// Sorted by name (case-insensitive), then path.
    pub mods: Vec<Mod>,
    by_id: HashMap<ModId, usize>,
    by_package: HashMap<String, Vec<usize>>,
    pub game_version: String,
    pub scan_ms: u64,
}

impl ModIndex {
    pub fn new(mut mods: Vec<Mod>, game_version: String, scan_ms: u64) -> Self {
        mods.sort_by_cached_key(|m| (m.name.to_lowercase(), m.path.clone()));
        let by_id = mods.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
        let mut by_package: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, m) in mods.iter().enumerate().filter(|(_, m)| m.valid) {
            by_package.entry(m.package_id.clone()).or_default().push(i);
        }
        Self {
            mods,
            by_id,
            by_package,
            game_version,
            scan_ms,
        }
    }

    pub fn get(&self, id: ModId) -> Option<&Mod> {
        self.by_id.get(&id).map(|&i| &self.mods[i])
    }

    /// All valid mods sharing a package id (duplicates across sources).
    pub fn by_package(&self, package_id: &str) -> impl Iterator<Item = &Mod> {
        self.by_package
            .get(package_id)
            .into_iter()
            .flatten()
            .map(|&i| &self.mods[i])
    }

    pub fn duplicate_package_count(&self) -> usize {
        self.by_package.values().filter(|v| v.len() > 1).count()
    }
}

/// Scan game `Data`, local and workshop folders in parallel.
pub fn scan(cfg: &ScanConfig, ctx: &TaskCtx) -> Result<ModIndex> {
    let t0 = Instant::now();
    let mut dirs: Vec<(PathBuf, Source)> = Vec::new();
    for (root, source) in [
        (cfg.game_folder.join("Data"), Source::Data),
        (cfg.local_folder.clone(), Source::Local),
        (cfg.workshop_folder.clone(), Source::Workshop),
    ] {
        if root.as_os_str().is_empty() {
            continue;
        }
        if let Ok(rd) = fs::read_dir(&root) {
            dirs.extend(
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .map(|p| (p, source)),
            );
        }
    }
    // Same folder configured as both local and workshop shouldn't double-count.
    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    dirs.dedup_by(|a, b| a.0 == b.0);

    let total = dirs.len() as u32;
    let done = AtomicU32::new(0);
    ctx.progress(0, total, "Scanning mods");
    let mods: Vec<Mod> = dirs
        .par_iter()
        .filter_map(|(p, s)| {
            if ctx.is_cancelled() {
                return None;
            }
            let m = parse_mod(p, *s, cfg);
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n.is_multiple_of(32) || n == total {
                ctx.progress(n, total, "Scanning mods");
            }
            Some(m)
        })
        .collect();
    ctx.check()?;

    let index = ModIndex::new(
        mods,
        cfg.game_version.clone(),
        t0.elapsed().as_millis() as u64,
    );
    tracing::info!(
        "scanned {} mods ({} invalid, {} duplicate package ids) in {} ms",
        index.mods.len(),
        index.mods.iter().filter(|m| !m.valid).count(),
        index.duplicate_package_count(),
        index.scan_ms
    );
    Ok(index)
}

#[cfg(test)]
impl Mod {
    /// Minimal valid mod for tests.
    pub fn stub(package_id: &str, folder: &str) -> Mod {
        Mod {
            id: ModId::from_path(Path::new(folder)),
            path: PathBuf::from(folder),
            folder: folder.into(),
            mod_type: ModType::Local,
            valid: true,
            invalid_reason: None,
            package_id: package_id.into(),
            name: package_id.to_uppercase(),
            authors: vec![],
            description: String::new(),
            url: String::new(),
            mod_version: String::new(),
            supported_versions: vec![],
            published_file_id: None,
            steam_app_id: None,
            rules: Rules::default(),
            community: ExtRule::default(),
            user: ExtRule::default(),
            mtime: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_mod(root: &Path, folder: &str, about: &str) -> PathBuf {
        let p = root.join(folder);
        fs::create_dir_all(p.join("About")).unwrap();
        fs::write(p.join("About/About.xml"), about).unwrap();
        p
    }

    fn cfg(version: &str) -> ScanConfig {
        ScanConfig {
            game_version: version.into(),
            prefer_versioned: true,
            ..Default::default()
        }
    }

    #[test]
    fn parses_rules_and_versions() {
        let t = tempfile::tempdir().unwrap();
        let p = write_mod(
            t.path(),
            "m",
            r#"<?xml version="1.0"?><ModMetadata><name>My Mod</name><packageId>Author.MyMod</packageId>
            <author>A</author><supportedVersions><li>1.5</li><li>1.6</li></supportedVersions>
            <modDependencies><li><packageId>Brrainz.Harmony</packageId><displayName>Harmony</displayName></li></modDependencies>
            <modDependenciesByVersion><v1.5><li><packageId>old.dep</packageId></li></v1.5></modDependenciesByVersion>
            <loadAfter><li>A.B</li></loadAfter><forceLoadAfter><li>c.d</li></forceLoadAfter>
            <loadBeforeByVersion><v1.6><li>x.y</li></v1.6></loadBeforeByVersion></ModMetadata>"#,
        );
        let m = parse_mod(&p, Source::Local, &cfg("1.6.4871 rev590"));
        assert!(m.valid);
        assert_eq!(m.package_id, "author.mymod");
        assert_eq!(m.rules.dependencies[0].package_id, "brrainz.harmony");
        assert_eq!(m.rules.load_after, ["a.b", "c.d"]);
        assert_eq!(m.rules.load_before, ["x.y"]);
        let old = parse_mod(&p, Source::Local, &cfg("1.5.1"));
        assert_eq!(old.rules.dependencies[0].package_id, "old.dep");
    }

    #[test]
    fn malformed_is_invalid_not_error() {
        let t = tempfile::tempdir().unwrap();
        let p = write_mod(t.path(), "bad", "<<<not xml at all");
        let m = parse_mod(&p, Source::Local, &cfg("1.6"));
        assert!(!m.valid);
        let empty = t.path().join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(!parse_mod(&empty, Source::Workshop, &cfg("1.6")).valid);
    }

    #[test]
    fn about_xml_fuzz_never_panics() {
        use crate::fuzz::{ABOUT, Rng, mutate};
        let t = tempfile::tempdir().unwrap();
        let mut rng = Rng(0xABCDEF);
        for i in 0..400 {
            let p = t.path().join(format!("m{i}"));
            fs::create_dir_all(p.join("About")).unwrap();
            fs::write(p.join("About/About.xml"), mutate(&mut rng, ABOUT)).unwrap();
            let m = parse_mod(&p, Source::Workshop, &cfg("1.6.4871 rev590"));
            assert!(m.valid || m.invalid_reason.is_some()); // always a definite outcome
        }
    }

    #[test]
    fn workshop_pfid_from_folder() {
        let t = tempfile::tempdir().unwrap();
        let p = write_mod(
            t.path(),
            "12345",
            "<ModMetadata><packageId>a.b</packageId></ModMetadata>",
        );
        let m = parse_mod(&p, Source::Workshop, &cfg("1.6"));
        assert_eq!(m.published_file_id.as_deref(), Some("12345"));
        assert_eq!(m.mod_type, ModType::SteamWorkshop);
    }
}
