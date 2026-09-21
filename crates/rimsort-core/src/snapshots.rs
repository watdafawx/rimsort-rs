//! Named load-order snapshots per instance ("Vanilla+QoL", "Big overhaul"): the ordered package ids of the
//! active list, kept in `<data dir>/snapshots/<instance>.json`.

use crate::{Error, Result, settings};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub name: String,
    pub created: u32,
    pub package_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SnapshotInfo {
    pub name: String,
    pub created: u32,
    pub count: u32,
}

fn path_for(instance: &str) -> PathBuf {
    // Instance names are user text; keep the file name portable.
    let safe: String = instance
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    settings::data_dir()
        .join("snapshots")
        .join(format!("{safe}.json"))
}

pub fn load(instance: &str) -> Vec<Snapshot> {
    fs::read(path_for(instance))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn store(instance: &str, list: &[Snapshot]) -> Result<()> {
    settings::atomic_write(&path_for(instance), &serde_json::to_vec_pretty(list)?)
}

pub fn info(instance: &str) -> Vec<SnapshotInfo> {
    let mut v: Vec<_> = load(instance)
        .into_iter()
        .map(|s| SnapshotInfo {
            name: s.name,
            created: s.created,
            count: s.package_ids.len() as u32,
        })
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.created));
    v
}

/// Save (or overwrite) a snapshot called `name`.
pub fn save(instance: &str, name: &str, package_ids: Vec<String>, now: u32) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("Give the snapshot a name".into()));
    }
    let mut list = load(instance);
    list.retain(|s| !s.name.eq_ignore_ascii_case(name));
    list.push(Snapshot {
        name: name.to_owned(),
        created: now,
        package_ids,
    });
    store(instance, &list)
}

pub fn delete(instance: &str, name: &str) -> Result<()> {
    let mut list = load(instance);
    list.retain(|s| s.name != name);
    store(instance, &list)
}

pub fn get(instance: &str, name: &str) -> Option<Snapshot> {
    load(instance).into_iter().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_overwrite_list_and_delete() {
        // Instance name doubles as the file name, so a unique one keeps this test isolated.
        let inst = format!("test inst/{}", std::process::id());
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        save(&inst, "A", ids(&["x", "y"]), 10).unwrap();
        save(&inst, "B", ids(&["z"]), 20).unwrap();
        save(&inst, "a", ids(&["q", "r", "s"]), 30).unwrap(); // same name (case-insensitive) overwrites
        let list = info(&inst);
        assert_eq!(
            list.iter()
                .map(|s| (s.name.as_str(), s.count))
                .collect::<Vec<_>>(),
            [("a", 3), ("B", 1)],
            "newest first"
        );
        assert_eq!(get(&inst, "B").unwrap().package_ids, ["z"]);
        delete(&inst, "B").unwrap();
        assert!(get(&inst, "B").is_none());
        assert!(save(&inst, "  ", vec![], 1).is_err());
        let _ = fs::remove_file(path_for(&inst));
    }
}
