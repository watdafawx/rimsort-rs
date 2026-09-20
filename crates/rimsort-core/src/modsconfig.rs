//! `ModsConfig.xml` read/write (round-trip safe: unknown elements survive) and active-list resolution.

use crate::{
    Error, ModId, Result,
    mods::{Mod, ModIndex, ModType},
    xml::{self, Node},
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

const STEAM_SUFFIX: &str = "_steam";
const BACKUPS_KEPT: usize = 20;

#[derive(Debug, Clone)]
pub struct ModsConfig {
    pub version: String,
    /// Raw ids as in the file, lowercased (may carry a `_steam` suffix).
    pub active: Vec<String>,
    pub known_expansions: Vec<String>,
    /// The parsed `<ModsConfigData>` element; unknown children are preserved on write.
    doc: Node,
}

impl ModsConfig {
    pub fn new(version: &str) -> Self {
        Self {
            version: version.into(),
            active: vec!["ludeon.rimworld".into()],
            known_expansions: vec![],
            doc: Node::new("ModsConfigData"),
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let root = xml::parse(text);
        let doc = root
            .child("ModsConfigData")
            .ok_or_else(|| Error::Other("ModsConfig.xml has no <ModsConfigData> element".into()))?
            .clone();
        let ids = |key: &str| {
            doc.child(key)
                .map(Node::li_texts)
                .unwrap_or_default()
                .into_iter()
                .map(|s| s.to_lowercase())
                .collect()
        };
        Ok(Self {
            version: doc.child_text("version").unwrap_or_default().into(),
            active: ids("activeMods"),
            known_expansions: ids("knownExpansions"),
            doc,
        })
    }

    pub fn read(path: &Path) -> Result<Self> {
        Self::parse(&xml::decode_bytes(&fs::read(path)?))
    }

    pub fn to_xml(&self) -> String {
        let mut doc = self.doc.clone();
        let list = |name: &str, ids: &[String]| {
            let mut n = Node::new(name);
            n.children = ids
                .iter()
                .map(|id| {
                    let mut li = Node::new("li");
                    li.text = id.clone();
                    li
                })
                .collect();
            n
        };
        let mut set = |name: &str, node: Node| match doc.child_mut(name) {
            Some(existing) => *existing = node,
            None => doc.children.push(node),
        };
        let mut version = Node::new("version");
        version.text = self.version.clone();
        set("version", version);
        set("activeMods", list("activeMods", &self.active));
        set(
            "knownExpansions",
            list("knownExpansions", &self.known_expansions),
        );
        xml::write(&doc)
    }

    /// Atomic write; the previous file is first copied to `ModsConfig.xml.bak-<unix>` (last 20 kept).
    pub fn write(&self, path: &Path) -> Result<Option<PathBuf>> {
        let backup = if path.exists() {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let b = path.with_extension(format!("xml.bak-{secs}"));
            fs::copy(path, &b)?;
            prune_backups(path);
            Some(b)
        } else {
            None
        };
        crate::settings::atomic_write(path, self.to_xml().as_bytes())?;
        Ok(backup)
    }
}

fn prune_backups(path: &Path) {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let prefix = format!("{}.bak-", name.to_string_lossy());
    let mut backups: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&prefix))
        })
        .collect();
    backups.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
    while backups.len() > BACKUPS_KEPT {
        let _ = fs::remove_file(backups.remove(0));
    }
}

/// Ordered active list plus what couldn't be resolved.
#[derive(Debug, Clone, Default)]
pub struct ActiveList {
    pub ids: Vec<ModId>,
    /// Entries whose config id carried `_steam` (kept on save).
    pub steam_suffix: HashSet<ModId>,
    /// Package ids present in the config but not installed.
    pub missing: Vec<String>,
}

const PRIORITY_DEFAULT: [ModType; 5] = [
    ModType::Ludeon,
    ModType::Local,
    ModType::SteamCmd,
    ModType::Git,
    ModType::SteamWorkshop,
];
const PRIORITY_STEAM: [ModType; 4] = [
    ModType::SteamWorkshop,
    ModType::Local,
    ModType::SteamCmd,
    ModType::Git,
];

/// Map config package ids to installed mods, choosing among duplicate copies by source priority.
pub fn resolve_active(index: &ModIndex, config_ids: &[String]) -> ActiveList {
    let mut out = ActiveList::default();
    let mut seen = HashSet::new();
    for raw in config_ids {
        let raw = raw.to_lowercase();
        let (target, is_steam) = match raw.strip_suffix(STEAM_SUFFIX) {
            Some(t) => (t, true),
            None => (raw.as_str(), false),
        };
        let candidates: Vec<&Mod> = index.by_package(target).collect();
        let order: &[ModType] = if is_steam {
            &PRIORITY_STEAM
        } else {
            &PRIORITY_DEFAULT
        };
        let pick = match candidates.as_slice() {
            [] => None,
            [only] => Some(*only),
            many => order
                .iter()
                .find_map(|t| {
                    let mut of_type: Vec<&&Mod> =
                        many.iter().filter(|m| m.mod_type == *t).collect();
                    of_type.sort_by(|a, b| {
                        natord::compare(&a.path.to_string_lossy(), &b.path.to_string_lossy())
                    });
                    of_type.first().map(|m| **m)
                })
                .or(many.first().copied()),
        };
        match pick {
            Some(m) => {
                if seen.insert(m.id) {
                    out.ids.push(m.id);
                    if is_steam {
                        out.steam_suffix.insert(m.id);
                    }
                }
            }
            None if !out.missing.iter().any(|x| x == target) => out.missing.push(target.to_owned()),
            None => {}
        }
    }
    out
}

/// Ids to write back to `ModsConfig.xml`, in load order.
pub fn config_ids(index: &ModIndex, list: &ActiveList) -> Vec<String> {
    list.ids
        .iter()
        .filter_map(|id| index.get(*id))
        .map(|m| {
            if list.steam_suffix.contains(&m.id) {
                format!("{}{STEAM_SUFFIX}", m.package_id)
            } else {
                m.package_id.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::{Mod, Rules};

    fn mk(pid: &str, ty: ModType, path: &str) -> Mod {
        Mod {
            id: ModId::from_path(Path::new(path)),
            path: path.into(),
            folder: path.into(),
            mod_type: ty,
            valid: true,
            invalid_reason: None,
            package_id: pid.into(),
            name: pid.into(),
            authors: vec![],
            description: String::new(),
            url: String::new(),
            mod_version: String::new(),
            supported_versions: vec![],
            published_file_id: None,
            steam_app_id: None,
            rules: Rules::default(),
            community: Default::default(),
            user: Default::default(),
            mtime: 0,
        }
    }

    #[test]
    fn roundtrip_preserves_unknown_and_order() {
        let src = "<?xml version=\"1.0\" ?>\n<ModsConfigData>\n  <version>1.6.1 rev1</version>\n  <activeMods><li>B.b</li><li>a.a</li></activeMods>\n  <knownExpansions><li>ludeon.rimworld.royalty</li></knownExpansions>\n  <custom><x>1</x></custom>\n</ModsConfigData>";
        let cfg = ModsConfig::parse(src).unwrap();
        assert_eq!(cfg.active, ["b.b", "a.a"]);
        let again = ModsConfig::parse(&cfg.to_xml()).unwrap();
        assert_eq!(again.active, cfg.active);
        assert_eq!(again.known_expansions, cfg.known_expansions);
        assert!(cfg.to_xml().contains("<custom>"));
    }

    #[test]
    fn duplicates_missing_and_steam_suffix() {
        let idx = ModIndex::new(
            vec![
                mk("dup.mod", ModType::SteamWorkshop, "w/1"),
                mk("dup.mod", ModType::Local, "l/dup"),
                mk("solo.mod", ModType::SteamWorkshop, "w/2"),
            ],
            String::new(),
            0,
        );
        let ids: Vec<String> = ["dup.mod", "gone.mod", "solo.mod", "dup.mod"]
            .map(String::from)
            .into();
        let l = resolve_active(&idx, &ids);
        assert_eq!(l.missing, ["gone.mod"]);
        assert_eq!(l.ids.len(), 2); // duplicate entry collapsed
        assert_eq!(idx.get(l.ids[0]).unwrap().mod_type, ModType::Local); // default priority
        let s = resolve_active(&idx, &["dup.mod_steam".to_string()]);
        assert_eq!(idx.get(s.ids[0]).unwrap().mod_type, ModType::SteamWorkshop);
        assert_eq!(config_ids(&idx, &s), ["dup.mod_steam"]);
    }
}
