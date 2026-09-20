//! Per-mod personal metadata: color, tags, notes, "ignore warnings". Stored per instance in our own
//! JSON file (keyed by package id); first use imports from RimSort's aux SQLite DB (read-only).

use crate::{Result, mods::ModIndex, settings};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModMeta {
    /// `#rrggbb`.
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub note: String,
    /// Suppress warnings for this mod.
    pub ignore: bool,
}

impl ModMeta {
    pub fn is_empty(&self) -> bool {
        *self == ModMeta::default()
    }
}

/// Keyed by lowercase package id.
pub type MetaMap = HashMap<String, ModMeta>;

fn safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn meta_path(instance: &str) -> PathBuf {
    settings::data_dir()
        .join("instances")
        .join(safe(instance))
        .join("mod_meta.json")
}

pub fn load(path: &std::path::Path) -> MetaMap {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(path: &std::path::Path, map: &MetaMap) -> Result<()> {
    let clean: std::collections::BTreeMap<_, _> =
        map.iter().filter(|(_, m)| !m.is_empty()).collect();
    settings::atomic_write(path, serde_json::to_string_pretty(&clean)?.as_bytes())
}

/// RimSort's per-instance aux DB (`instances/<name>/aux_metadata.db`).
fn rimsort_aux_db(instance: &str) -> Option<PathBuf> {
    let base = settings::rimsort_settings_path()?
        .parent()?
        .join("instances")
        .join(instance);
    let p = base.join("aux_metadata.db");
    p.is_file().then_some(p)
}

/// Import colors, notes and ignore flags from RimSort. Rows are keyed by full path, which often
/// points at an old drive, so mods are matched by folder name (Workshop id / local folder).
pub fn import_rimsort(instance: &str, index: &ModIndex) -> MetaMap {
    let Some(db) = rimsort_aux_db(instance) else {
        return MetaMap::new();
    };
    match import_from(&db, index) {
        Ok(m) => {
            tracing::info!("imported metadata for {} mods from RimSort", m.len());
            m
        }
        Err(e) => {
            tracing::warn!("could not import RimSort metadata: {e}");
            MetaMap::new()
        }
    }
}

fn import_from(db: &std::path::Path, index: &ModIndex) -> rusqlite::Result<MetaMap> {
    let run = || -> rusqlite::Result<MetaMap> {
        let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut stmt = conn.prepare(
            "SELECT path, color_hex, user_notes, ignore_warnings FROM auxiliary_metadata
             WHERE (color_hex IS NOT NULL AND color_hex != '') OR user_notes != '' OR ignore_warnings = 1",
        )?;
        let by_folder: HashMap<String, &str> = index
            .mods
            .iter()
            .filter(|m| m.valid)
            .map(|m| (m.folder.to_lowercase(), m.package_id.as_str()))
            .collect();
        let mut out = MetaMap::new();
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, bool>(3)?,
            ))
        })? {
            let (path, color, note, ignore) = row?;
            let folder = path
                .replace('\\', "/")
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_lowercase();
            if let Some(pid) = by_folder.get(&folder) {
                out.insert(
                    (*pid).to_owned(),
                    ModMeta {
                        color: color
                            .filter(|c| !c.is_empty())
                            .map(|c| format!("#{}", c.trim_start_matches('#').to_lowercase())),
                        tags: vec![],
                        note,
                        ignore,
                    },
                );
            }
        }
        Ok(out)
    };
    run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_empty_entries_dropped() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("m/mod_meta.json");
        let mut m = MetaMap::new();
        m.insert(
            "a.b".into(),
            ModMeta {
                color: Some("#ff0000".into()),
                tags: vec!["qol".into()],
                note: "hi".into(),
                ignore: true,
            },
        );
        m.insert("c.d".into(), ModMeta::default());
        save(&p, &m).unwrap();
        let back = load(&p);
        assert_eq!(back.len(), 1);
        assert_eq!(back["a.b"].tags, ["qol"]);
        assert!(load(&t.path().join("nope.json")).is_empty());
        assert_eq!(safe("My Inst/ance"), "My_Inst_ance");
    }

    #[test]
    fn imports_from_sqlite_by_folder_name() {
        let t = tempfile::tempdir().unwrap();
        let db = t.path().join("aux.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE auxiliary_metadata (path VARCHAR PRIMARY KEY, user_notes VARCHAR DEFAULT '' NOT NULL, color_hex VARCHAR, ignore_warnings BOOLEAN DEFAULT 0 NOT NULL);
             INSERT INTO auxiliary_metadata VALUES ('E:/old/workshop/12345', 'my note', 'FF8800', 0);
             INSERT INTO auxiliary_metadata VALUES ('E:/old/workshop/99999', '', NULL, 1);
             INSERT INTO auxiliary_metadata VALUES ('E:/old/workshop/555', '', NULL, 0);",
        )
        .unwrap();
        drop(c);
        let mods = vec![
            crate::mods::Mod::stub("a.b", "12345"),
            crate::mods::Mod::stub("c.d", "99999"),
        ];
        let index = ModIndex::new(mods, String::new(), 0);
        let m = import_from(&db, &index).unwrap();
        assert_eq!(m["a.b"].color.as_deref(), Some("#ff8800"));
        assert_eq!(m["a.b"].note, "my note");
        assert!(m["c.d"].ignore);
        assert!(!m.contains_key("555"));
    }
}
