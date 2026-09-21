//! Per-mod load timings written by the "Loading Progress" mod to `StartupImpactData.xml` (next to the
//! game's `Config` folder). All timings in the file are milliseconds; absent elements mean zero.

use crate::xml;
use std::{collections::HashMap, fs, path::Path, time::SystemTime};

const FILE: &str = "StartupImpactData.xml";

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Impact {
    pub total_ms: f32,
    pub off_thread_ms: f32,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Lowercased package id (no `_steam` suffix) → impact.
    by_package: HashMap<String, Impact>,
    /// Lowercased display name → impact, for reports written before package ids were recorded.
    by_name: HashMap<String, Impact>,
}

pub fn normalize(package_id: &str) -> String {
    let p = package_id.to_lowercase();
    p.strip_suffix("_steam").unwrap_or(&p).to_owned()
}

impl Report {
    pub fn parse(src: &str) -> Option<Self> {
        let root = xml::parse(src);
        let session = root.child("StartupImpactSession")?;
        let mods = session.child("sessionData")?.child("mods")?;
        let ms = |n: &xml::Node, k: &str| -> f32 {
            n.child_text(k)
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or(0.0)
        };
        let mut out = Self::default();
        for li in mods.children_named("li") {
            let Some(name) = li.child_text("modName").filter(|n| !n.trim().is_empty()) else {
                continue;
            };
            let impact = Impact {
                total_ms: ms(li, "totalImpact"),
                off_thread_ms: ms(li, "offThreadTotalImpact"),
            };
            match li
                .child_text("modPackageId")
                .filter(|p| !p.trim().is_empty())
            {
                Some(pid) => out.by_package.insert(normalize(pid.trim()), impact),
                None => out.by_name.insert(name.trim().to_lowercase(), impact),
            };
        }
        Some(out)
    }

    /// Package id wins over display name, as in RimSort.
    pub fn find(&self, package_id: &str, name: &str) -> Option<Impact> {
        self.by_package
            .get(&normalize(package_id))
            .or_else(|| self.by_name.get(&name.to_lowercase()))
            .copied()
    }
}

/// The report for an instance (`<Config>/../StartupImpactData.xml`) plus its mtime, for caching.
pub fn load(config_folder: &str) -> Option<(SystemTime, Report)> {
    let path = Path::new(config_folder).parent()?.join(FILE);
    let mtime = fs::metadata(&path).and_then(|m| m.modified()).ok()?;
    let text = xml::decode_bytes(&fs::read(&path).ok()?);
    Some((mtime, Report::parse(&text)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<StartupImpactSession><sessionData><loadingTime>42500.5</loadingTime><mods>
      <li><modName>Some Mod</modName><modPackageId>Author.SomeMod_steam</modPackageId>
          <totalImpact>1230.5</totalImpact><offThreadTotalImpact>500</offThreadTotalImpact></li>
      <li><modName>Old Report Mod</modName><totalImpact>80</totalImpact></li>
      <li><modName></modName><totalImpact>9</totalImpact></li>
    </mods></sessionData></StartupImpactSession>"#;

    #[test]
    fn matches_by_package_id_then_name() {
        let r = Report::parse(SAMPLE).unwrap();
        let a = r.find("author.somemod", "whatever").unwrap();
        assert_eq!((a.total_ms, a.off_thread_ms), (1230.5, 500.0));
        assert_eq!(r.find("x.y", "OLD report MOD").unwrap().total_ms, 80.0);
        assert!(r.find("x.y", "Nope").is_none());
        assert!(Report::parse("<other/>").is_none());
    }
}
