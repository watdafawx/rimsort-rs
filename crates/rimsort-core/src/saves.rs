//! Latest RimWorld save: which mods it was made with, to mark mods that are new since (or that the save
//! still expects). The list is in the save's `<meta>` header, so only the start of the file is read.

use serde::Serialize;
use specta::Type;
use std::{fs, io::Read, path::Path, time::SystemTime};

/// The header lives at the top of the file; saves themselves can be tens of MB.
const HEAD_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize, Type)]
pub struct SaveInfo {
    /// File name without extension.
    pub name: String,
    /// Save file modification time (unix seconds).
    pub modified: u32,
    /// Lowercased package ids the save was made with.
    pub package_ids: Vec<String>,
}

/// Newest `*.rws` in `<config folder>/../Saves`, with its mtime.
pub fn latest(config_folder: &str) -> Option<(std::path::PathBuf, SystemTime)> {
    let saves = Path::new(config_folder).parent()?.join("Saves");
    fs::read_dir(saves)
        .ok()?
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("rws"))
        })
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .max_by_key(|(_, t)| *t)
}

pub fn read(path: &Path, mtime: SystemTime) -> Option<SaveInfo> {
    let mut head = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(HEAD_BYTES)
        .read_to_end(&mut head)
        .ok()?;
    let ids = package_ids(&crate::xml::decode_bytes(&head))?;
    Some(SaveInfo {
        name: path.file_stem()?.to_string_lossy().into_owned(),
        modified: mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs().min(u64::from(u32::MAX)) as u32),
        package_ids: ids,
    })
}

/// `<modIds><li>…</li></modIds>` from a save header; None when the header has no mod list.
fn package_ids(head: &str) -> Option<Vec<String>> {
    let start = head.find("<modIds>")? + "<modIds>".len();
    let end = start + head[start..].find("</modIds>")?;
    Some(
        head[start..end]
            .split("<li>")
            .skip(1)
            .filter_map(|c| c.split("</li>").next())
            .map(|p| crate::startup_impact::normalize(p.trim()))
            .filter(|p| !p.is_empty())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_mod_ids_from_the_header_only() {
        let dir = std::env::temp_dir().join(format!("rs-saves-{}", std::process::id()));
        let cfg = dir.join("Config");
        let saves = dir.join("Saves");
        fs::create_dir_all(&cfg).unwrap();
        fs::create_dir_all(&saves).unwrap();
        let body = "<savegame><meta><gameVersion>1.6</gameVersion><modIds><li>Brrainz.Harmony</li><li>ludeon.rimworld_steam</li></modIds></meta><game><modIds><li>later.ignored</li></modIds></game></savegame>";
        fs::write(saves.join("old.rws"), "<savegame></savegame>").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(saves.join("Colony1.rws"), body).unwrap();
        fs::write(saves.join("notes.txt"), "x").unwrap();

        let (path, t) = latest(cfg.to_str().unwrap()).unwrap();
        assert!(path.ends_with("Colony1.rws"));
        let info = read(&path, t).unwrap();
        assert_eq!(info.name, "Colony1");
        assert_eq!(
            info.package_ids,
            ["brrainz.harmony", "ludeon.rimworld", "later.ignored"][..2]
        );
        assert!(
            read(&saves.join("old.rws"), t).is_none(),
            "no mod list -> no comparison"
        );
        assert!(latest(dir.join("nope").join("Config").to_str().unwrap()).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
