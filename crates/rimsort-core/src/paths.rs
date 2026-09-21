//! Locate RimWorld (game, config, local mods, workshop) and validate configured paths.

use crate::settings::Instance;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const RIMWORLD_APPID: &str = "294100";

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct DetectedPaths {
    pub game_folder: Option<String>,
    pub config_folder: Option<String>,
    pub local_folder: Option<String>,
    pub workshop_folder: Option<String>,
    /// Human-readable explanation of what was/wasn't found.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathCheck {
    pub kind: String,
    pub path: String,
    pub ok: bool,
    pub reason: String,
}

// ── minimal VDF/ACF (Valve KeyValues) parser ─────────────────────────────

#[derive(Debug, Clone)]
pub(crate) enum Vdf {
    Str(String),
    Map(Vec<(String, Vdf)>),
}

impl Vdf {
    pub(crate) fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Map(m) => m
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub(crate) fn str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Map(_) => None,
        }
    }
}

pub(crate) fn parse_vdf(text: &str) -> Vdf {
    #[derive(PartialEq)]
    enum Tok {
        S(String),
        Open,
        Close,
    }
    let mut toks = Vec::new();
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' => toks.push(Tok::Open),
            '}' => toks.push(Tok::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = it.next() {
                    match c {
                        '"' => break,
                        '\\' => match it.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(o) => s.push(o), // `\\` and `\"`
                            None => break,
                        },
                        c => s.push(c),
                    }
                }
                toks.push(Tok::S(s));
            }
            '/' if it.peek() == Some(&'/') => {
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    fn block(toks: &[Tok], i: &mut usize) -> Vdf {
        let mut out = Vec::new();
        while *i < toks.len() {
            match &toks[*i] {
                Tok::Close => {
                    *i += 1;
                    break;
                }
                Tok::S(key) => {
                    *i += 1;
                    match toks.get(*i) {
                        Some(Tok::S(v)) => {
                            out.push((key.clone(), Vdf::Str(v.clone())));
                            *i += 1;
                        }
                        Some(Tok::Open) => {
                            *i += 1;
                            out.push((key.clone(), block(toks, i)));
                        }
                        _ => {}
                    }
                }
                Tok::Open => *i += 1,
            }
        }
        Vdf::Map(out)
    }
    let mut i = 0;
    block(&toks, &mut i)
}

// ── detection ────────────────────────────────────────────────────────────

fn steam_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::*};
        let read = |hive, sub: &str, val: &str| {
            RegKey::predef(hive)
                .open_subkey(sub)
                .ok()
                .and_then(|k| k.get_value::<String, _>(val).ok())
        };
        roots.extend(
            read(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath").map(PathBuf::from),
        );
        roots.extend(
            read(
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\WOW6432Node\Valve\Steam",
                "InstallPath",
            )
            .map(PathBuf::from),
        );
        roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    }
    if let Some(home) = dirs::home_dir() {
        roots.extend([
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            home.join("snap/steam/common/.local/share/Steam"),
            home.join("Library/Application Support/Steam"),
        ]);
    }
    roots
}

fn config_folder_candidates() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    vec![
        home.join("AppData/LocalLow/Ludeon Studios/RimWorld by Ludeon Studios/Config"),
        home.join(".config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios/Config"),
        home.join("Library/Application Support/RimWorld/Config"),
    ]
}

/// Display string with consistent separators (`join` with "a/b" leaves mixed ones on Windows).
fn s(p: &Path) -> String {
    p.components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .into_owned()
}

/// Detect from explicit Steam roots and config candidates (testable core of `autodetect`).
pub fn detect_in(
    steam_roots: &[PathBuf],
    config_candidates: &[PathBuf],
    extra_game_dirs: &[PathBuf],
) -> DetectedPaths {
    let mut out = DetectedPaths::default();

    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in steam_roots.iter().filter(|r| r.is_dir()) {
        if !libraries.contains(root) {
            libraries.push(root.clone());
        }
        if let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf"))
            && let Some(Vdf::Map(entries)) = parse_vdf(&text).get("libraryfolders").cloned()
        {
            for (_, v) in entries {
                let path = v.get("path").and_then(Vdf::str).or_else(|| v.str());
                if let Some(p) = path.map(PathBuf::from).filter(|p| p.is_dir())
                    && !libraries.contains(&p)
                {
                    libraries.push(p);
                }
            }
        }
    }
    out.notes
        .push(format!("Steam libraries checked: {}", libraries.len()));

    for lib in &libraries {
        let manifest = lib.join(format!("steamapps/appmanifest_{RIMWORLD_APPID}.acf"));
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let dir = parse_vdf(&text)
            .get("AppState")
            .and_then(|a| a.get("installdir"))
            .and_then(Vdf::str)
            .map(|d| lib.join("steamapps/common").join(d));
        if let Some(game) = dir.filter(|d| d.is_dir()) {
            let workshop = lib.join(format!("steamapps/workshop/content/{RIMWORLD_APPID}"));
            if workshop.is_dir() {
                out.workshop_folder = Some(s(&workshop));
            }
            out.game_folder = Some(s(&game));
            out.notes.push(format!(
                "RimWorld found via Steam library {}",
                lib.display()
            ));
            break;
        }
    }
    if out.workshop_folder.is_none() {
        out.workshop_folder = libraries
            .iter()
            .map(|l| l.join(format!("steamapps/workshop/content/{RIMWORLD_APPID}")))
            .find(|p| p.is_dir())
            .map(|p| s(&p));
    }
    if out.game_folder.is_none() {
        out.game_folder = extra_game_dirs
            .iter()
            .find(|d| d.join("Data").is_dir())
            .map(|d| s(d));
        if out.game_folder.is_some() {
            out.notes
                .push("RimWorld found in a non-Steam location".into());
        } else {
            out.notes.push(
                "RimWorld not found in any Steam library; pick the game folder manually".into(),
            );
        }
    }
    if let Some(game) = &out.game_folder {
        let game = Path::new(game);
        let mac_mods = game.join("RimWorldMac.app/Mods");
        out.local_folder = Some(s(&if mac_mods.is_dir() {
            mac_mods
        } else {
            game.join("Mods")
        }));
    }
    out.config_folder = config_candidates.iter().find(|c| c.is_dir()).map(|c| s(c));
    if out.config_folder.is_none() {
        out.notes.push(
            "RimWorld config folder not found (run the game once, or pick it manually)".into(),
        );
    }
    out
}

pub fn autodetect() -> DetectedPaths {
    let extra: Vec<PathBuf> = [
        r"C:\GOG Games\RimWorld",
        r"C:\Program Files (x86)\GOG Galaxy\Games\RimWorld",
        r"C:\Program Files\RimWorld",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    detect_in(&steam_roots(), &config_folder_candidates(), &extra)
}

/// `Version.txt` contents (`1.6.4871 rev590`), empty if unreadable.
pub fn game_version(game_folder: &Path) -> String {
    fs::read_to_string(game_folder.join("Version.txt"))
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

/// Check each configured path and say why it's bad.
pub fn validate(inst: &Instance) -> Vec<PathCheck> {
    let check = |kind: &str, path: &str, must: &[&str]| {
        let (ok, reason) = if path.trim().is_empty() {
            (false, "not set".to_owned())
        } else if !Path::new(path).is_dir() {
            (false, "folder does not exist".to_owned())
        } else if let Some(missing) = must.iter().find(|m| !Path::new(path).join(m).exists()) {
            (false, format!("missing {missing}"))
        } else {
            (true, String::new())
        };
        PathCheck {
            kind: kind.into(),
            path: path.into(),
            ok,
            reason,
        }
    };
    vec![
        check("game", &inst.game_folder, &["Version.txt", "Data/Core"]),
        check("config", &inst.config_folder, &[]),
        check("local", &inst.local_folder, &[]),
        check("workshop", &inst.workshop_folder, &[]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vdf_libraryfolders_and_acf() {
        let v = parse_vdf(
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"apps\" { \"1\" \"2\" }\n\t}\n\t\"1\" \"D:\\\\Old\"\n}",
        );
        let Some(Vdf::Map(e)) = v.get("libraryfolders") else {
            panic!()
        };
        assert_eq!(
            e[0].1.get("path").unwrap().str().unwrap(),
            r"C:\Program Files (x86)\Steam"
        );
        assert_eq!(e[1].1.str().unwrap(), r"D:\Old");
    }

    #[test]
    fn detects_rimworld_in_second_library() {
        let t = tempfile::tempdir().unwrap();
        let (steam, lib2) = (t.path().join("Steam"), t.path().join("Lib2"));
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
                steam.to_string_lossy().replace('\\', "\\\\"),
                lib2.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        fs::create_dir_all(lib2.join("steamapps/common/RimWorld/Data/Core")).unwrap();
        fs::create_dir_all(lib2.join("steamapps/workshop/content/294100")).unwrap();
        fs::write(
            lib2.join("steamapps/appmanifest_294100.acf"),
            "\"AppState\" { \"appid\" \"294100\" \"installdir\" \"RimWorld\" }",
        )
        .unwrap();
        let cfg = t.path().join("Config");
        fs::create_dir(&cfg).unwrap();

        let d = detect_in(&[steam], std::slice::from_ref(&cfg), &[]);
        assert!(d.game_folder.unwrap().ends_with("RimWorld"));
        assert!(d.workshop_folder.unwrap().ends_with("294100"));
        assert!(d.local_folder.unwrap().ends_with("Mods"));
        assert_eq!(d.config_folder.unwrap(), s(&cfg));
    }

    #[test]
    fn nothing_found_explains_why() {
        let d = detect_in(&[], &[], &[]);
        assert!(d.game_folder.is_none() && d.notes.iter().any(|n| n.contains("not found")));
    }
}
