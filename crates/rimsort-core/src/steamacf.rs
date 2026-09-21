//! Steam's Workshop manifest (`appworkshop_294100.acf`): per-item update times, offline.

use crate::paths::{RIMWORLD_APPID, Vdf, parse_vdf};
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkshopTimes {
    /// When the installed version was published (unix seconds).
    pub updated: u32,
    /// Newest version Steam knows about; greater than `updated` means an update is pending.
    pub latest: u32,
}

impl WorkshopTimes {
    pub fn outdated(&self) -> bool {
        self.latest > self.updated && self.updated > 0
    }
}

pub fn parse(text: &str) -> HashMap<String, WorkshopTimes> {
    let root = parse_vdf(text);
    let Some(app) = root.get("AppWorkshop") else {
        return HashMap::new();
    };
    let num = |v: Option<&Vdf>, key: &str| -> u32 {
        v.and_then(|v| v.get(key))
            .and_then(Vdf::str)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    let mut out: HashMap<String, WorkshopTimes> = HashMap::new();
    if let Some(Vdf::Map(items)) = app.get("WorkshopItemsInstalled") {
        for (id, v) in items {
            out.entry(id.clone()).or_default().updated = num(Some(v), "timeupdated");
        }
    }
    if let Some(Vdf::Map(items)) = app.get("WorkshopItemDetails") {
        for (id, v) in items {
            let e = out.entry(id.clone()).or_default();
            e.latest = num(Some(v), "latest_timeupdated");
            if e.updated == 0 {
                e.updated = num(Some(v), "timeupdated");
            }
        }
    }
    out
}

/// `<library>/steamapps/workshop/content/294100` -> `<library>/steamapps/workshop/appworkshop_294100.acf`.
pub fn load(workshop_folder: &Path) -> HashMap<String, WorkshopTimes> {
    let acf = workshop_folder
        .parent()
        .and_then(Path::parent)
        .map(|w| w.join(format!("appworkshop_{RIMWORLD_APPID}.acf")));
    acf.and_then(|p| fs::read_to_string(p).ok())
        .map(|t| parse(&t))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installed_and_pending_updates() {
        let acf = r#""AppWorkshop" { "appid" "294100"
          "WorkshopItemsInstalled" { "111" { "size" "1" "timeupdated" "100" "manifest" "9" } "222" { "timeupdated" "200" } }
          "WorkshopItemDetails" { "111" { "timeupdated" "100" "latest_timeupdated" "150" } "222" { "timeupdated" "200" "latest_timeupdated" "200" } "333" { "latest_timeupdated" "5" } } }"#;
        let m = parse(acf);
        assert!(m["111"].outdated());
        assert!(!m["222"].outdated());
        assert!(!m["333"].outdated()); // never installed -> no update warning
        assert_eq!(m["111"].updated, 100);
        assert!(parse("garbage").is_empty());
    }
}
