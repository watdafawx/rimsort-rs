//! External rule databases: Community Rules, user rules, ignore list, NoVersionWarning.
//! All are read from RimSort's on-disk formats; missing/unreadable files just mean "no rules".

use crate::{Result, settings::Settings, xml};
use serde_json::{Map, Value, json};
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

#[derive(Debug, Default, Clone)]
pub struct ExternalRules {
    pub timestamp: i64,
    rules: HashMap<String, ExtRule>,
}

impl ExternalRules {
    pub fn get(&self, package_id: &str) -> Option<&ExtRule> {
        self.rules.get(package_id)
    }

    /// Replace (or clear, when empty) the rule for one package.
    pub fn set(&mut self, package_id: &str, rule: ExtRule) {
        let empty = rule.load_after.is_empty()
            && rule.load_before.is_empty()
            && !rule.load_top
            && !rule.load_bottom;
        if empty {
            self.rules.remove(package_id);
        } else {
            self.rules.insert(package_id.to_owned(), rule);
        }
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

/// A recommended replacement ("Use This Instead" database), keyed by the old Workshop id.
#[derive(Debug, Clone, Default)]
pub struct Replacement {
    pub new_name: String,
    pub new_author: String,
    pub new_workshop_id: String,
    pub new_package_id: String,
}

/// Parse `{"rules": [{"oldWorkshopId": ..., "newName": ...}]}` (optionally gzipped bytes).
pub fn parse_replacements(bytes: &[u8]) -> Result<HashMap<String, Replacement>> {
    use std::io::Read;
    let text = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut s = String::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_string(&mut s)
            .map_err(|e| crate::Error::Other(format!("replacements gzip: {e}")))?;
        s
    } else {
        xml::decode_bytes(bytes)
    };
    let v: Value = serde_json::from_str(text.trim_start_matches('﻿'))?;
    let get = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    Ok(v.get("rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let old = get(r, "oldWorkshopId");
            (!old.is_empty()).then(|| {
                (
                    old,
                    Replacement {
                        new_name: get(r, "newName"),
                        new_author: get(r, "newAuthor"),
                        new_workshop_id: get(r, "newWorkshopId"),
                        new_package_id: get(r, "newPackageId"),
                    },
                )
            })
        })
        .collect())
}

/// Everything external that influences sorting and validation.
#[derive(Debug, Default, Clone)]
pub struct RuleSources {
    pub community: Option<ExternalRules>,
    pub user: Option<ExternalRules>,
    /// Package ids whose warnings are suppressed (`ignore.json` -> `ignored_mods`).
    pub ignored: HashSet<String>,
    /// Package ids that should never get a version-mismatch warning.
    pub no_version_warning: HashSet<String>,
    /// old Workshop id -> recommended replacement.
    pub replacements: HashMap<String, Replacement>,
    /// Workshop id -> update times from Steam's manifest (filled at scan time, needs the workshop folder).
    pub workshop: HashMap<String, crate::steamacf::WorkshopTimes>,
}

/// Our own writable copy of `file` (seeded from RimSort's on first write so existing rules carry over).
pub fn own_db_path(file: &str) -> PathBuf {
    crate::settings::data_dir().join("dbs").join(file)
}

/// Text of `file` from the first database folder that has it.
pub fn read_db_text(file: &str) -> Option<String> {
    find_in_dbs(file)
        .and_then(|p| fs::read(p).ok())
        .map(|b| xml::decode_bytes(&b))
}

/// Rewrite one package's entry in a RimSort-format `userRules.json`, keeping comments/names of
/// entries that stay and unknown top-level keys. An empty rule removes the entry.
pub fn write_user_rule(
    existing: Option<&str>,
    package_id: &str,
    rule: &ExtRule,
    name_of: impl Fn(&str) -> String,
) -> Result<String> {
    let mut root: Value = existing
        .and_then(|t| serde_json::from_str(t.trim_start_matches('﻿')).ok())
        .unwrap_or_else(|| json!({"timestamp": 0, "rules": {}}));
    if !root.is_object() {
        root = json!({"timestamp": 0, "rules": {}});
    }
    let rules = root
        .as_object_mut()
        .unwrap()
        .entry("rules")
        .or_insert_with(|| json!({}));
    if !rules.is_object() {
        *rules = json!({});
    }
    let rules = rules.as_object_mut().unwrap();
    // Find the existing key regardless of case so we edit in place.
    let key = rules
        .keys()
        .find(|k| k.eq_ignore_ascii_case(package_id))
        .cloned()
        .unwrap_or_else(|| package_id.to_owned());
    let mut entry = rules
        .get(&key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for (field, list) in [
        ("loadAfter", &rule.load_after),
        ("loadBefore", &rule.load_before),
    ] {
        let old = entry
            .get(field)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut new = Map::new();
        for id in list {
            let prior = old.iter().find(|(k, _)| k.eq_ignore_ascii_case(id));
            new.insert(
                prior.map_or_else(|| id.clone(), |(k, _)| k.clone()),
                prior.map_or_else(
                    || json!({"name": name_of(id), "comment": ""}),
                    |(_, v)| v.clone(),
                ),
            );
        }
        if new.is_empty() {
            entry.remove(field);
        } else {
            entry.insert(field.into(), Value::Object(new));
        }
    }
    for (field, on) in [("loadTop", rule.load_top), ("loadBottom", rule.load_bottom)] {
        if on {
            let comment = entry
                .get(field)
                .and_then(|v| v.get("comment"))
                .cloned()
                .unwrap_or(json!(""));
            entry.insert(field.into(), json!({"value": true, "comment": comment}));
        } else {
            entry.remove(field);
        }
    }
    if entry.is_empty() {
        rules.remove(&key);
    } else {
        rules.insert(key, Value::Object(entry));
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    root["timestamp"] = json!(secs);
    Ok(serde_json::to_string_pretty(&root)?)
}

/// Add/remove `package_id` in an `ignore.json` document (`ignored_mods` array), sorted.
pub fn write_ignore(existing: Option<&str>, package_id: &str, ignored: bool) -> Result<String> {
    let mut root: Value = existing
        .and_then(|t| serde_json::from_str(t.trim_start_matches('﻿')).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({"description": "Mods to ignore when checking for missing properties (identified by packageid)"}));
    let mut ids: Vec<String> = root
        .get("ignored_mods")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_lowercase))
        .filter(|i| i != package_id)
        .collect();
    if ignored {
        ids.push(package_id.to_lowercase());
    }
    ids.sort();
    ids.dedup();
    root["ignored_mods"] = json!(ids);
    Ok(serde_json::to_string_pretty(&root)?)
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

pub fn find_in_dbs(file: &str) -> Option<PathBuf> {
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

        // Fresh installs have no external_* settings: fall back to the default download layout.
        let unconfigured = |prefix: &str| {
            !settings
                .extra
                .contains_key(&format!("{prefix}_metadata_source"))
        };
        let disabled = |prefix: &str| {
            matches!(
                settings
                    .extra
                    .get(&format!("{prefix}_metadata_source"))
                    .and_then(Value::as_str),
                Some("None" | "Disabled")
            )
        };

        let replacements = resolve(
            settings,
            "external_use_this_instead",
            "external_use_this_instead_repo_path",
            "replacements.json.gz",
        )
        .or_else(|| {
            unconfigured("external_use_this_instead")
                .then(|| find_in_dbs("UseThisInstead/replacements.json.gz"))
                .flatten()
        });
        if let Some(p) = replacements
            && let Ok(bytes) = fs::read(&p)
        {
            match parse_replacements(&bytes) {
                Ok(r) => {
                    tracing::info!("use-this-instead: {} entries", r.len());
                    out.replacements = r;
                }
                Err(e) => tracing::warn!("use-this-instead unreadable: {e}"),
            }
        }

        // NoVersionWarning/ModIdsToFix.xml, preferring the per-game-version file.
        if !disabled("external_no_version_warning") {
            let mm = game_version
                .split('.')
                .take(2)
                .collect::<Vec<_>>()
                .join(".");
            let path = db_dirs()
                .into_iter()
                .flat_map(|d| {
                    let nv = d.join("NoVersionWarning");
                    [
                        nv.join(&mm).join("ModIdsToFix.xml"),
                        nv.join("ModIdsToFix.xml"),
                    ]
                })
                .find(|p| p.exists());
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
    fn user_rule_write_keeps_comments_and_removes_empty() {
        let base = r#"{"timestamp":1,"extra":true,"rules":{"a.b":{"loadAfter":{"x.y":{"name":"X","comment":"keep me"}},"loadTop":{"value":true,"comment":"c"}}}}"#;
        let rule = ExtRule {
            load_after: vec!["x.y".into(), "n.w".into()],
            load_top: true,
            ..Default::default()
        };
        let out = write_user_rule(Some(base), "a.b", &rule, |id| format!("Name of {id}")).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["extra"], true);
        assert_eq!(v["rules"]["a.b"]["loadAfter"]["x.y"]["comment"], "keep me");
        assert_eq!(v["rules"]["a.b"]["loadAfter"]["n.w"]["name"], "Name of n.w");
        assert_eq!(v["rules"]["a.b"]["loadTop"]["comment"], "c");
        let parsed = ExternalRules::parse(&out).unwrap();
        let mut after = parsed.get("a.b").unwrap().load_after.clone();
        after.sort();
        assert_eq!(after, ["n.w", "x.y"]);
        // clearing everything drops the entry
        let cleared =
            write_user_rule(Some(&out), "A.B", &ExtRule::default(), |_| String::new()).unwrap();
        assert!(ExternalRules::parse(&cleared).unwrap().get("a.b").is_none());
    }

    #[test]
    fn ignore_write_add_remove() {
        let a = write_ignore(None, "P.Q", true).unwrap();
        assert!(a.contains("p.q"));
        let b = write_ignore(Some(&a), "p.q", false).unwrap();
        assert!(!b.contains("p.q") && b.contains("description"));
    }

    #[test]
    fn parses_replacements_plain_and_gzip() {
        use std::io::Write;
        let json = r#"{"version":1,"rules":[{"oldWorkshopId":"111","newName":"New","newAuthor":"Me","newWorkshopId":"222","newPackageId":"a.b"},{"nope":1}]}"#;
        let plain = parse_replacements(json.as_bytes()).unwrap();
        assert_eq!(plain["111"].new_workshop_id, "222");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(json.as_bytes()).unwrap();
        let packed = gz.finish().unwrap();
        assert_eq!(parse_replacements(&packed).unwrap()["111"].new_name, "New");
        assert!(parse_replacements(b"garbage").is_err());
    }

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
