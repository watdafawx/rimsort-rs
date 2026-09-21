//! RimSort's Steam Workshop database (`steamDB.json`): package id → Workshop id/name, used to
//! offer downloads for missing dependencies that don't carry a Workshop link themselves.

use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Default)]
pub struct SteamDb {
    /// Lowercased package id → every (workshop id, name) claiming it.
    by_package: HashMap<String, Vec<(String, String)>>,
}

#[derive(Deserialize)]
struct File {
    database: HashMap<String, Entry>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Entry {
    #[serde(rename = "packageId")]
    package_id: String,
    name: String,
    #[serde(rename = "steamName")]
    steam_name: String,
    unpublished: bool,
}

impl SteamDb {
    pub fn load(path: &Path) -> Option<Self> {
        let bytes = fs::read(path).ok()?;
        let file: File = serde_json::from_slice(&bytes)
            .map_err(|e| tracing::warn!("steam db unreadable: {e}"))
            .ok()?;
        let mut by_package: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (id, e) in file.database {
            // App entries carry "packageid" instead; skip anything without a mod package id.
            if e.package_id.is_empty() || e.unpublished {
                continue;
            }
            let name = if e.name.is_empty() {
                e.steam_name
            } else {
                e.name
            };
            by_package
                .entry(e.package_id.to_lowercase())
                .or_default()
                .push((id, name));
        }
        tracing::info!("steam db: {} package ids", by_package.len());
        Some(Self { by_package })
    }

    /// The Workshop item for a package id — only when it is unambiguous (alone, or the single
    /// candidate named `name_hint`), because a wrong guess would download an unrelated mod.
    pub fn lookup(&self, package_id: &str, name_hint: &str) -> Option<(&str, &str)> {
        let all = self.by_package.get(package_id)?;
        if let [(id, name)] = all.as_slice() {
            return Some((id, name));
        }
        let mut named = all
            .iter()
            .filter(|(_, n)| !name_hint.is_empty() && n.eq_ignore_ascii_case(name_hint));
        match (named.next(), named.next()) {
            (Some((id, name)), None) => Some((id, name)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_lookup_only() {
        let dir = std::env::temp_dir().join(format!("rs-steamdb-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("steamDB.json");
        fs::write(
            &p,
            r#"{"version":1,"database":{
              "1":{"packageId":"A.One","name":"One"},
              "2":{"packageId":"b.two","steamName":"Two A"},
              "3":{"packageId":"b.two","steamName":"Two B"},
              "4":{"packageId":"gone.mod","name":"Gone","unpublished":true},
              "294100":{"appid":true,"packageid":"ludeon.rimworld","name":"RimWorld"}}}"#,
        )
        .unwrap();
        let db = SteamDb::load(&p).unwrap();
        assert_eq!(db.lookup("a.one", ""), Some(("1", "One")));
        assert_eq!(
            db.lookup("b.two", ""),
            None,
            "ambiguous ids are not guessed"
        );
        assert_eq!(db.lookup("b.two", "two b"), Some(("3", "Two B")));
        assert_eq!(db.lookup("gone.mod", ""), None);
        assert_eq!(db.lookup("ludeon.rimworld", ""), None);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Against RimSort's real database, when installed: `cargo test -p rimsort-core -- --ignored steamdb`.
    #[test]
    #[ignore]
    fn real_database() {
        let Some(p) = crate::rules::find_in_dbs("Steam-Workshop-Database/steamDB.json") else {
            return;
        };
        let t = std::time::Instant::now();
        let db = SteamDb::load(&p).unwrap();
        println!("loaded in {:?}", t.elapsed());
        println!("harmony -> {:?}", db.lookup("brrainz.harmony", "Harmony"));
    }
}
