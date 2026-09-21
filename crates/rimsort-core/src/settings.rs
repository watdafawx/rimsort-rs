//! Settings + instances. The JSON layout mirrors RimSort's `settings.json` (instances embedded,
//! unknown keys preserved) so a RimSort file can be imported as-is.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const DEFAULT_INSTANCE: &str = "Default";

/// Keys never carried over when importing from RimSort.
const SECRET_KEYS: &[&str] = &["github_token", "steam_apikey", "rentry_auth_code"];

/// Marker file next to the executable that switches to portable mode (data in `<exe dir>/data`).
pub const PORTABLE_MARKER: &str = "portable.txt";

/// Where our own settings live: `RIMSORT_RS_DATA_DIR` (tests/dev), else `<exe dir>/data` when a
/// `portable.txt` sits next to the executable, else the per-user local data folder.
pub fn data_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    data_dir_for(
        std::env::var_os("RIMSORT_RS_DATA_DIR").map(PathBuf::from),
        exe_dir,
    )
}

pub fn is_portable() -> bool {
    std::env::var_os("RIMSORT_RS_DATA_DIR").is_none()
        && std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(PORTABLE_MARKER)))
            .is_some_and(|m| m.is_file())
}

fn data_dir_for(env: Option<PathBuf>, exe_dir: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = env {
        return dir;
    }
    if let Some(exe) = exe_dir.filter(|d| d.join(PORTABLE_MARKER).is_file()) {
        return exe.join("data");
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("RimSort-rs")
}

/// Where the Python RimSort keeps its settings, for import.
pub fn rimsort_settings_path() -> Option<PathBuf> {
    Some(
        dirs::data_local_dir()?
            .join("RimSort")
            .join("settings.json"),
    )
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Instance {
    pub name: String,
    pub game_folder: String,
    pub config_folder: String,
    pub local_folder: String,
    pub workshop_folder: String,
    pub run_args: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub sorting_algorithm: String,
    #[serde(rename = "use_moddependencies_as_loadTheseBefore")]
    pub use_moddependencies_as_load_these_before: bool,
    pub use_alternative_package_ids_as_satisfying_dependencies: bool,
    pub check_dependencies_on_sort: bool,
    pub prefer_versioned_about_tags: bool,
    pub current_instance: String,
    pub instances: BTreeMap<String, Instance>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sorting_algorithm: "Topological".into(),
            use_moddependencies_as_load_these_before: false,
            use_alternative_package_ids_as_satisfying_dependencies: true,
            check_dependencies_on_sort: true,
            prefer_versioned_about_tags: true,
            current_instance: DEFAULT_INSTANCE.into(),
            instances: BTreeMap::new(),
            extra: Map::new(),
        }
    }
}

impl Settings {
    pub fn current(&self) -> Option<&Instance> {
        self.instances.get(&self.current_instance)
    }

    /// Guarantee at least one instance and a valid `current_instance`.
    pub fn normalize(&mut self) {
        for (k, i) in self.instances.iter_mut() {
            if i.name.is_empty() {
                i.name = k.clone();
            }
        }
        if self.instances.is_empty() {
            self.instances.insert(
                DEFAULT_INSTANCE.into(),
                Instance {
                    name: DEFAULT_INSTANCE.into(),
                    ..Default::default()
                },
            );
        }
        if !self.instances.contains_key(&self.current_instance) {
            self.current_instance = self.instances.keys().next().cloned().unwrap_or_default();
        }
    }

    pub fn from_rimsort_json(text: &str) -> Result<Self> {
        let mut s: Settings = serde_json::from_str(text)?;
        for k in SECRET_KEYS {
            s.extra.remove(*k);
        }
        s.normalize();
        Ok(s)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        atomic_write(path, serde_json::to_string_pretty(self)?.as_bytes())
    }
}

pub struct Loaded {
    pub settings: Settings,
    /// Set when the file was unreadable and replaced by defaults.
    pub warning: Option<String>,
    /// True when no settings file existed yet.
    pub first_run: bool,
}

/// Load settings; a corrupt file is moved aside (`.corrupt-<unix>`) and defaults returned with a warning.
pub fn load(path: &Path) -> Loaded {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                settings: Settings::default(),
                warning: None,
                first_run: true,
            };
        }
        Err(e) => {
            return Loaded {
                settings: Settings::default(),
                warning: Some(format!("Could not read settings ({e}); using defaults.")),
                first_run: false,
            };
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(mut s) => {
            s.normalize();
            Loaded {
                settings: s,
                warning: None,
                first_run: false,
            }
        }
        Err(e) => {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let backup = path.with_extension(format!("json.corrupt-{secs}"));
            let moved = fs::rename(path, &backup).is_ok();
            Loaded {
                settings: Settings::default(),
                warning: Some(format!(
                    "Settings file was corrupt ({e}); {} Using defaults.",
                    if moved {
                        format!("backed up to {}.", backup.display())
                    } else {
                        "could not back it up.".into()
                    }
                )),
                first_run: true,
            }
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Other(format!("JSON: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_gets_defaults_and_keeps_unknown_keys() {
        let s = Settings::from_rimsort_json(
            r#"{"current_instance":"X","instances":{"X":{"game_folder":"G:/rw","weird":1}},"theme_name":"dark","github_token":"secret"}"#,
        )
        .unwrap();
        assert_eq!(s.current().unwrap().game_folder, "G:/rw");
        assert_eq!(s.current().unwrap().name, "X");
        assert_eq!(s.sorting_algorithm, "Topological");
        assert!(s.extra.contains_key("theme_name") && !s.extra.contains_key("github_token"));
        let out = serde_json::to_string(&s).unwrap();
        assert!(out.contains("\"weird\":1") && out.contains("theme_name"));
    }

    #[test]
    fn corrupt_file_is_backed_up() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("settings.json");
        fs::write(&p, "{ not json").unwrap();
        let l = load(&p);
        assert!(l.warning.is_some() && l.first_run && !p.exists());
        assert!(
            fs::read_dir(t.path())
                .unwrap()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().contains("corrupt"))
        );
        assert!(load(&p).first_run); // missing file -> first run, no warning
    }

    #[test]
    fn atomic_save_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("sub/settings.json");
        let mut s = Settings::default();
        s.normalize();
        s.save(&p).unwrap();
        s.save(&p).unwrap(); // overwrite
        let l = load(&p);
        assert!(!l.first_run && l.warning.is_none() && l.settings.current().is_some());
        assert!(!p.with_extension("tmp").exists());
    }

    #[test]
    fn data_dir_env_beats_portable_beats_default() {
        let tmp = std::env::temp_dir().join(format!("rs-portable-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        // No marker: default location.
        assert!(data_dir_for(None, Some(tmp.clone())).ends_with("RimSort-rs"));
        std::fs::write(tmp.join(PORTABLE_MARKER), "").unwrap();
        assert_eq!(data_dir_for(None, Some(tmp.clone())), tmp.join("data"));
        // The override always wins (tests/dev must never touch a portable folder by accident).
        let over = tmp.join("elsewhere");
        assert_eq!(data_dir_for(Some(over.clone()), Some(tmp.clone())), over);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
