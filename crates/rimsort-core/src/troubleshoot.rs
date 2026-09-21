//! Troubleshooting resets. Everything goes to the Recycle Bin, never straight to deletion, and
//! `ModsConfig.xml` is never touched (it belongs to the load-order editor).

use crate::{Error, Result, settings::Instance};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Fix {
    /// `Config/Mod_*.xml`: per-mod settings written by the game.
    ModSettings,
    /// `Prefs.xml` and `KeyPrefs.xml`.
    GameSettings,
    /// Steam's half-finished Workshop downloads (`workshop/downloads`).
    SteamDownloadCache,
}

/// Files a fix would move to the Recycle Bin.
pub fn preview(inst: &Instance, fix: Fix) -> Result<Vec<PathBuf>> {
    let mut out = match fix {
        Fix::ModSettings => list(&config_dir(inst)?, |n| {
            n.starts_with("Mod_") && n.ends_with(".xml")
        }),
        Fix::GameSettings => list(&config_dir(inst)?, |n| {
            n == "Prefs.xml" || n == "KeyPrefs.xml"
        }),
        Fix::SteamDownloadCache => {
            let dir = downloads_dir(inst)?;
            if dir.is_dir() {
                list(&dir, |_| true)
            } else {
                vec![]
            }
        }
    };
    out.sort();
    Ok(out)
}

pub fn apply(inst: &Instance, fix: Fix) -> Result<usize> {
    let files = preview(inst, fix)?;
    for f in &files {
        trash::delete(f).map_err(|e| {
            Error::Other(format!(
                "Could not move {} to the recycle bin: {e}",
                f.display()
            ))
        })?;
    }
    tracing::info!("troubleshoot {fix:?}: moved {} items to trash", files.len());
    Ok(files.len())
}

fn config_dir(inst: &Instance) -> Result<PathBuf> {
    if inst.config_folder.is_empty() {
        return Err(Error::Other(
            "Set the RimWorld config folder in Settings first".into(),
        ));
    }
    Ok(PathBuf::from(&inst.config_folder))
}

/// `<library>/steamapps/workshop/downloads`, derived from `<library>/steamapps/workshop/content/294100`.
fn downloads_dir(inst: &Instance) -> Result<PathBuf> {
    let w = Path::new(&inst.workshop_folder);
    let base = w
        .parent()
        .and_then(Path::parent)
        .filter(|_| !inst.workshop_folder.is_empty())
        .ok_or_else(|| Error::Other("Set the Steam Workshop folder in Settings first".into()))?;
    Ok(base.join("downloads"))
}

fn list(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| keep(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_only_the_targeted_files() {
        let dir = std::env::temp_dir().join(format!("rs-trouble-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cfg = dir.join("Config");
        fs::create_dir_all(&cfg).unwrap();
        for f in [
            "Mod_A.xml",
            "Mod_B.xml",
            "ModsConfig.xml",
            "Prefs.xml",
            "KeyPrefs.xml",
        ] {
            fs::write(cfg.join(f), "x").unwrap();
        }
        let inst = Instance {
            config_folder: cfg.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let names = |fix| -> Vec<String> {
            preview(&inst, fix)
                .unwrap()
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(names(Fix::ModSettings), ["Mod_A.xml", "Mod_B.xml"]);
        assert_eq!(names(Fix::GameSettings), ["KeyPrefs.xml", "Prefs.xml"]);
        assert!(preview(&Instance::default(), Fix::ModSettings).is_err());
        assert!(preview(&Instance::default(), Fix::SteamDownloadCache).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
