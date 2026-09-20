//! Per-mod warnings for the active list. Messages are formatted by the UI from `kind` + `other`.
//! Semantics follow RimSort's mods panel: errors = missing dependency / incompatibility,
//! warnings = load-order violation / game-version mismatch. Ignored mods get none.

use crate::{
    ModId,
    mods::{Mod, ModIndex},
    rules::RuleSources,
};
use serde::Serialize;
use specta::Type;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum WarningKind {
    /// `other` is a required package that isn't active.
    MissingDependency,
    /// `other` is an active mod declared incompatible (by either side).
    Incompatible,
    /// This mod should load before `other`, but comes after it.
    LoadBefore,
    /// This mod should load after `other`, but comes before it.
    LoadAfter,
    /// Mod doesn't list support for the running game version.
    VersionMismatch,
    /// A maintained replacement exists: `other` = its Workshop id, `other_name` = "Name by Author".
    UseThisInstead,
}

impl WarningKind {
    pub fn is_error(self) -> bool {
        matches!(self, Self::MissingDependency | Self::Incompatible)
    }
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct Warning {
    pub kind: WarningKind,
    pub other: String,
    pub other_name: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ModWarnings {
    pub id: ModId,
    pub warnings: Vec<Warning>,
}

/// A required package that isn't active, aggregated over everything that needs it.
#[derive(Debug, Clone, Serialize, Type)]
pub struct MissingDep {
    pub package_id: String,
    pub name: String,
    pub workshop_id: Option<String>,
    /// Names of active mods that require it.
    pub required_by: Vec<String>,
    /// An installed (but inactive) mod that satisfies it.
    pub installed: Option<ModId>,
}

/// Steam Workshop id from `steam://url/CommunityFilePage/<id>` or `…filedetails/?id=<id>` links.
pub fn workshop_id(url: &str) -> Option<String> {
    let i = ["CommunityFilePage/", "id="]
        .iter()
        .filter_map(|k| url.rfind(k).map(|p| p + k.len()))
        .max()?;
    let digits: String = url[i..].chars().take_while(char::is_ascii_digit).collect();
    (!digits.is_empty()).then_some(digits)
}

#[derive(Debug, Clone, Default, Serialize, Type)]
pub struct ValidationView {
    /// Only mods that have at least one warning.
    pub mods: Vec<ModWarnings>,
    /// Mods with an error-level warning.
    pub errors: u32,
    /// Mods with only warning-level issues.
    pub warnings: u32,
    /// Distinct packages required but not active.
    pub missing_dependencies: u32,
}

/// True when the mod lists supported versions but not the running `major.minor`.
pub fn version_mismatch(m: &Mod, game_version: &str, src: &RuleSources) -> bool {
    let mm = game_version
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");
    !mm.is_empty()
        && !m.supported_versions.is_empty()
        && !src.ignored.contains(&m.package_id)
        && !src.no_version_warning.contains(&m.package_id)
        && !m
            .supported_versions
            .iter()
            .any(|v| v.trim_start_matches('v') == mm)
}

pub fn validate(
    index: &ModIndex,
    active: &[ModId],
    src: &RuleSources,
    use_alternative_ids: bool,
) -> ValidationView {
    let mods: Vec<&Mod> = active
        .iter()
        .filter_map(|id| index.get(*id))
        .filter(|m| m.valid)
        .collect();
    let mut pos: HashMap<&str, usize> = HashMap::new();
    for (i, m) in mods.iter().enumerate() {
        pos.entry(m.package_id.as_str()).or_insert(i);
    }
    // Reverse incompatibility: who declares an incompatibility with me?
    let mut declared_by: HashMap<&str, Vec<&str>> = HashMap::new();
    for m in &mods {
        for other in &m.rules.incompatible_with {
            declared_by
                .entry(other.as_str())
                .or_default()
                .push(m.package_id.as_str());
        }
    }
    let name_of = |pid: &str, fallback: &str| -> String {
        index
            .by_package(pid)
            .next()
            .map(|m| m.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                if fallback.is_empty() {
                    pid.to_owned()
                } else {
                    fallback.to_owned()
                }
            })
    };

    let mut view = ValidationView::default();
    for (i, m) in mods.iter().enumerate() {
        let pid = m.package_id.as_str();
        if src.ignored.contains(pid) {
            continue;
        }
        let mut w: Vec<Warning> = Vec::new();
        let mut push = |kind, other: &str, fallback: &str| {
            w.push(Warning {
                kind,
                other: other.to_owned(),
                other_name: name_of(other, fallback),
            })
        };

        for dep in &m.rules.dependencies {
            let ok = pos.contains_key(dep.package_id.as_str())
                || (use_alternative_ids
                    && dep
                        .alternatives
                        .iter()
                        .any(|a| pos.contains_key(a.as_str())));
            if !ok {
                push(
                    WarningKind::MissingDependency,
                    &dep.package_id,
                    &dep.display_name,
                );
            }
        }

        let mut incompat: Vec<&str> = m
            .rules
            .incompatible_with
            .iter()
            .map(String::as_str)
            .filter(|o| *o != pid && pos.contains_key(o))
            .collect();
        incompat.extend(
            declared_by
                .get(pid)
                .into_iter()
                .flatten()
                .copied()
                .filter(|o| *o != pid),
        );
        incompat.sort_unstable();
        incompat.dedup();
        for o in incompat {
            push(WarningKind::Incompatible, o, "");
        }

        for o in m.load_before_all().filter(|o| o.as_str() != pid) {
            if pos.get(o.as_str()).is_some_and(|&j| i >= j) {
                push(WarningKind::LoadBefore, o, "");
            }
        }
        for o in m.load_after_all().filter(|o| o.as_str() != pid) {
            if pos.get(o.as_str()).is_some_and(|&j| i <= j) {
                push(WarningKind::LoadAfter, o, "");
            }
        }
        if version_mismatch(m, &index.game_version, src) {
            push(WarningKind::VersionMismatch, "", "");
        }
        if let Some(r) = m
            .published_file_id
            .as_ref()
            .and_then(|p| src.replacements.get(p))
        {
            w.push(Warning {
                kind: WarningKind::UseThisInstead,
                other: r.new_workshop_id.clone(),
                other_name: if r.new_author.is_empty() {
                    r.new_name.clone()
                } else {
                    format!("{} by {}", r.new_name, r.new_author)
                },
            });
        }

        if !w.is_empty() {
            if w.iter().any(|x| x.kind.is_error()) {
                view.errors += 1;
            } else {
                view.warnings += 1;
            }
            view.mods.push(ModWarnings {
                id: m.id,
                warnings: w,
            });
        }
    }
    // Dedup guard: same (kind, other) can arise from about + community duplicates.
    for mw in &mut view.mods {
        let mut seen = HashSet::new();
        mw.warnings
            .retain(|x| seen.insert((x.kind as u8, x.other.clone())));
    }
    view.missing_dependencies = view
        .mods
        .iter()
        .flat_map(|mw| &mw.warnings)
        .filter(|w| w.kind == WarningKind::MissingDependency)
        .map(|w| w.other.as_str())
        .collect::<HashSet<_>>()
        .len() as u32;
    view
}

/// Every required package that isn't active, grouped by package, with an installed-but-inactive
/// match when one exists (so the UI can offer "enable").
pub fn missing_dependencies(
    index: &ModIndex,
    active: &[ModId],
    src: &RuleSources,
    use_alternative_ids: bool,
) -> Vec<MissingDep> {
    let mods: Vec<&Mod> = active
        .iter()
        .filter_map(|id| index.get(*id))
        .filter(|m| m.valid)
        .collect();
    let active_pids: HashSet<&str> = mods.iter().map(|m| m.package_id.as_str()).collect();
    let mut out: std::collections::BTreeMap<String, MissingDep> = Default::default();
    for m in mods.iter().filter(|m| !src.ignored.contains(&m.package_id)) {
        for dep in &m.rules.dependencies {
            let satisfied = active_pids.contains(dep.package_id.as_str())
                || (use_alternative_ids
                    && dep
                        .alternatives
                        .iter()
                        .any(|a| active_pids.contains(a.as_str())));
            if satisfied {
                continue;
            }
            let entry = out.entry(dep.package_id.clone()).or_insert_with(|| {
                let installed = std::iter::once(&dep.package_id)
                    .chain(if use_alternative_ids {
                        dep.alternatives.iter()
                    } else {
                        [].iter()
                    })
                    .find_map(|pid| index.by_package(pid).next());
                MissingDep {
                    package_id: dep.package_id.clone(),
                    name: if dep.display_name.is_empty() {
                        installed.map_or_else(|| dep.package_id.clone(), |i| i.name.clone())
                    } else {
                        dep.display_name.clone()
                    },
                    workshop_id: workshop_id(&dep.workshop_url)
                        .or_else(|| installed.and_then(|i| i.published_file_id.clone())),
                    required_by: vec![],
                    installed: installed.map(|i| i.id),
                }
            });
            entry.required_by.push(m.name.clone());
        }
    }
    let mut v: Vec<MissingDep> = out.into_values().collect();
    v.sort_by_key(|d| d.name.to_lowercase());
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::{Dependency, ModType, Rules};
    use std::path::Path;

    fn mk(pid: &str, f: impl FnOnce(&mut Mod)) -> Mod {
        let mut m = Mod {
            id: ModId::from_path(Path::new(pid)),
            path: pid.into(),
            folder: pid.into(),
            mod_type: ModType::Local,
            valid: true,
            invalid_reason: None,
            package_id: pid.into(),
            name: pid.to_uppercase(),
            authors: vec![],
            description: String::new(),
            url: String::new(),
            mod_version: String::new(),
            supported_versions: vec![],
            published_file_id: None,
            steam_app_id: None,
            rules: Rules::default(),
            community: Default::default(),
            user: Default::default(),
            mtime: 0,
        };
        f(&mut m);
        m
    }

    fn run(mods: Vec<Mod>, order: &[&str], src: &RuleSources) -> HashMap<String, Vec<WarningKind>> {
        let idx = ModIndex::new(mods, "1.6.4871 rev590".into(), 0);
        let ids: Vec<ModId> = order
            .iter()
            .map(|p| ModId::from_path(Path::new(p)))
            .collect();
        validate(&idx, &ids, src, true)
            .mods
            .into_iter()
            .map(|mw| {
                (
                    idx.get(mw.id).unwrap().package_id.clone(),
                    mw.warnings.iter().map(|w| w.kind).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn each_kind() {
        let dep = |p: &str| Dependency {
            package_id: p.into(),
            ..Default::default()
        };
        let mods = vec![
            mk("needs", |m| m.rules.dependencies = vec![dep("absent")]),
            mk("alt", |m| {
                m.rules.dependencies = vec![Dependency {
                    package_id: "absent".into(),
                    alternatives: vec!["here".into()],
                    ..Default::default()
                }]
            }),
            mk("here", |_| {}),
            mk("a", |m| m.rules.load_after = vec!["b".into()]), // a before b -> violation
            mk("b", |_| {}),
            mk("x", |m| m.rules.incompatible_with = vec!["y".into()]),
            mk("y", |_| {}),
            mk("old", |m| m.supported_versions = vec!["1.4".into()]),
        ];
        let w = run(
            mods,
            &["needs", "alt", "here", "a", "b", "x", "y", "old"],
            &RuleSources::default(),
        );
        assert_eq!(w["needs"], [WarningKind::MissingDependency]);
        assert!(!w.contains_key("alt") && !w.contains_key("here"));
        assert_eq!(w["a"], [WarningKind::LoadAfter]);
        assert_eq!(w["x"], [WarningKind::Incompatible]);
        assert_eq!(w["y"], [WarningKind::Incompatible]); // reverse declaration
        assert_eq!(w["old"], [WarningKind::VersionMismatch]);
    }

    #[test]
    fn missing_dependency_report() {
        let dep = |p: &str, url: &str| Dependency {
            package_id: p.into(),
            display_name: "Lib".into(),
            workshop_url: url.into(),
            ..Default::default()
        };
        let mods = vec![
            mk("a", |m| {
                m.rules.dependencies = vec![
                    dep("lib.gone", "steam://url/CommunityFilePage/12345"),
                    dep("lib.here", ""),
                ]
            }),
            mk("b", |m| m.rules.dependencies = vec![dep("lib.gone", "")]),
            mk("lib.here", |_| {}),
        ];
        let idx = ModIndex::new(mods, String::new(), 0);
        let active = [
            ModId::from_path(Path::new("a")),
            ModId::from_path(Path::new("b")),
        ];
        let v = missing_dependencies(&idx, &active, &RuleSources::default(), true);
        assert_eq!(v.len(), 2);
        let gone = v.iter().find(|d| d.package_id == "lib.gone").unwrap();
        assert_eq!(
            (
                gone.workshop_id.as_deref(),
                gone.required_by.len(),
                gone.installed
            ),
            (Some("12345"), 2, None)
        );
        assert!(
            v.iter()
                .find(|d| d.package_id == "lib.here")
                .unwrap()
                .installed
                .is_some()
        );
        assert_eq!(
            workshop_id("https://steamcommunity.com/sharedfiles/filedetails/?id=99&x=1").as_deref(),
            Some("99")
        );
        assert_eq!(workshop_id("nothing"), None);
    }

    #[test]
    fn use_this_instead_warning() {
        let mut src = RuleSources::default();
        src.replacements.insert(
            "111".into(),
            crate::rules::Replacement {
                new_name: "New".into(),
                new_author: "Me".into(),
                new_workshop_id: "222".into(),
                ..Default::default()
            },
        );
        let mods = vec![
            mk("old", |m| m.published_file_id = Some("111".into())),
            mk("fine", |_| {}),
        ];
        let w = run(mods, &["old", "fine"], &src);
        assert_eq!(w["old"], [WarningKind::UseThisInstead]);
        assert!(!w.contains_key("fine"));
    }

    #[test]
    fn ignored_and_no_version_warning_suppress() {
        let mods = vec![mk("old", |m| m.supported_versions = vec!["1.4".into()])];
        let mut src = RuleSources::default();
        src.no_version_warning.insert("old".into());
        assert!(run(mods, &["old"], &src).is_empty());
    }
}
