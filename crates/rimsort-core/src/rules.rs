//! External rule databases: Community Rules, user rules, ignore list, NoVersionWarning.
//! All are read from RimSort's on-disk formats; missing/unreadable files just mean "no rules".

use crate::{Result, settings::Settings, xml};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// Rules for one package id, from a community or user rules file.
#[derive(Debug, Clone, Default)]
pub struct ExtRule {
    pub load_after: Vec<String>,
    pub load_before: Vec<String>,
    /// Sort into tier 1 (load as early as possible after the base game).
    pub load_top: bool,
    /// Sort into tier 3 (load last).
    pub load_bottom: bool,
}

#[derive(Debug, Default)]
pub struct ExternalRules {
    pub timestamp: i64,
    rules: HashMap<String, ExtRule>,
}

impl ExternalRules {
    pub fn get(&self, package_id: &str) -> Option<&ExtRule> {
        self.rules.get(package_id)
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// `{"timestamp": n, "rules": {"pkg.id": {"loadAfter": {"other": {...}}, "loadTop": {"value": true}}}}`.
    /// Tolerant: unknown keys and odd shapes are skipped; ids are lowercased.
    pub fn parse(text: &str) -> Result<Self> {
        let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
        let keys = |v: Option<&Value>| -> Vec<String> {
            v.and_then(Value::as_object)
                .map(|o| o.keys().map(|k| k.to_lowercase()).collect())
                .unwrap_or_default()
        };
        let flag = |v: Option<&Value>| {
            v.and_then(|v| v.get("value"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        let rules = v
            .get("rules")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .map(|(pid, r)| {
                        (
                            pid.to_lowercase(),
                            ExtRule {
                                load_after: keys(r.get("loadAfter")),
                                load_before: keys(r.get("loadBefore")),
                                load_top: flag(r.get("loadTop")),
                                load_bottom: flag(r.get("loadBottom")),
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            timestamp: v.get("timestamp").and_then(Value::as_i64).unwrap_or(0),
            rules,
        })
    }

    pub fn read(path: &Path) -> Result<Self> {
        Self::parse(&xml::decode_bytes(&fs::read(path)?))
    }
}

/// Everything external that influences sorting and validation.
#[derive(Debug, Default)]
pub struct RuleSources {
    pub community: Option<ExternalRules>,
    pub user: Option<ExternalRules>,
    /// Package ids whose warnings are suppressed (`ignore.json` -> `ignored_mods`).
    pub ignored: HashSet<String>,
    /// Package ids that should never get a version-mismatch warning.
    pub no_version_warning: HashSet<String>,
}

/// Folders searched for database files: ours first, then RimSort's (read-only reuse).
fn db_dirs() -> Vec<PathBuf> {
    let mut d = vec![crate::settings::data_dir().join("dbs")];
    if let Some(p) =
        crate::settings::rimsort_settings_path().and_then(|p| p.parent().map(|p| p.join("dbs")))
    {
        d.push(p);
    }
    d
}

/// Mirror of RimSort's `_resolve_db_path`: disabled -> None; explicit file path; otherwise
/// `<dbs>/<repo name>/<file>` for git/URL sources.
fn resolve(s: &Settings, prefix: &str, repo_key: &str, file: &str) -> Option<PathBuf> {
    let get = |k: String| {
        s.extra
            .get(&k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let source = get(format!("{prefix}_metadata_source"));
    if matches!(source.as_str(), "None" | "Disabled") {
        return None;
    }
    if source == "Configured file path" {
        let p = get(format!("{prefix}_file_path"));
        return (!p.is_empty()).then(|| PathBuf::from(p));
    }
    let repo = get(repo_key.to_owned());
    let name = repo
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_owned();
    if name.is_empty() {
        return None;
    }
    db_dirs()
        .into_iter()
        .map(|d| d.join(&name).join(file))
        .find(|p| p.exists())
}

fn find_in_dbs(file: &str) -> Option<PathBuf> {
    db_dirs()
        .into_iter()
        .map(|d| d.join(file))
        .find(|p| p.exists())
}

impl RuleSources {
    pub fn load(settings: &Settings, game_version: &str) -> Self {
        let mut out = Self::default();

        // No explicit settings (fresh install with no RimSort) -> default community repo layout.
        let community = resolve(
            settings,
            "external_community_rules",
            "external_community_rules_repo",
            "communityRules.json",
        )
        .or_else(|| {
            settings
                .extra
                .get("external_community_rules_metadata_source")
                .is_none()
                .then(|| find_in_dbs("Community-Rules-Database/communityRules.json"))
                .flatten()
        });
        match community.as_ref().map(|p| ExternalRules::read(p)) {
            Some(Ok(r)) => {
                tracing::info!("community rules: {} entries", r.len());
                out.community = Some(r);
            }
            Some(Err(e)) => tracing::warn!("community rules unreadable: {e}"),
            None => tracing::info!("community rules: none configured/found"),
        }

        if let Some(p) = find_in_dbs("userRules.json") {
            match ExternalRules::read(&p) {
                Ok(r) => out.user = Some(r),
                Err(e) => tracing::warn!("user rules unreadable: {e}"),
            }
        }

        if let Some(v) = find_in_dbs("ignore.json")
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        {
            out.ignored = v
                .get("ignored_mods")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(str::to_lowercase))
                .collect();
        }

        // ModIdsToFix.xml, possibly in a per-version subfolder.
        if let Some(base) = resolve(
            settings,
            "external_no_version_warning",
            "external_no_version_warning_repo_path",
            "ModIdsToFix.xml",
        ) {
            let mm = game_version
                .split('.')
                .take(2)
                .collect::<Vec<_>>()
                .join(".");
            let versioned = base.parent().map(|p| p.join(&mm).join("ModIdsToFix.xml"));
            let path = if base.exists() {
                Some(base)
            } else {
                versioned.filter(|p| p.exists())
            };
            if let Some(text) = path.and_then(|p| fs::read(p).ok()) {
                let root = xml::parse(&xml::decode_bytes(&text));
                out.no_version_warning = root
                    .child("ModIdsToFix")
                    .map(|n| n.li_texts().into_iter().map(|s| s.to_lowercase()).collect())
                    .unwrap_or_default();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_community_shapes() {
        let r = ExternalRules::parse(
            r#"{"timestamp": 5, "rules": {
              "A.B": {"loadBefore": {"X.Y": {"name": ["a","b"]}}, "loadAfter": {"c.d": {"name": "C", "comment": "hi"}},
                      "loadBottom": {"value": true, "comment": "last"}},
              "e.f": {"loadTop": {"value": false}, "incompatibleWith": {"z": {}}, "weird": 3}
            }}"#,
        )
        .unwrap();
        assert_eq!(r.timestamp, 5);
        let a = r.get("a.b").unwrap();
        assert_eq!(
            (a.load_before.as_slice(), a.load_after.as_slice()),
            (&["x.y".to_string()][..], &["c.d".to_string()][..])
        );
        assert!(a.load_bottom && !a.load_top);
        assert!(!r.get("e.f").unwrap().load_top);
        assert!(ExternalRules::parse("not json").is_err());
    }
}
