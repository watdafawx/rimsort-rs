//! Load-order sort. Port of RimSort's `Sorter` + topological sort:
//! tiers (0 = core/Harmony, 1 = frameworks, 2 = everything else, 3 = load-last), each tier
//! toposorted into levels, levels alphabetized by lowercase name.

use crate::{ModId, mods::ModIndex};
use petgraph::{algo::tarjan_scc, graphmap::DiGraphMap};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

type Graph = BTreeMap<String, BTreeSet<String>>;

pub const TIER_ZERO: &[&str] = &[
    "zetrith.prepatcher",
    "brrainz.harmony",
    "brrainz.visualexceptions",
    "ludeon.rimworld",
    "ludeon.rimworld.royalty",
    "ludeon.rimworld.ideology",
    "ludeon.rimworld.biotech",
    "ludeon.rimworld.anomaly",
    "ludeon.rimworld.odyssey",
];

pub const TIER_ONE: &[&str] = &[
    "adaptive.storage.framework",
    "aoba.framework",
    "aoba.exosuit.framework",
    "ebsg.framework",
    "imranfish.xmlextensions",
    "thesepeople.ritualattachableoutcomes",
    "ohno.asf.ab.local",
    "oskarpotocki.vanillafactionsexpanded.core",
    "owlchemist.cherrypicker",
    "redmattis.betterprerequisites",
    "smashphil.vehicleframework",
    "unlimitedhugs.hugslib",
    "vanillaexpanded.backgrounds",
];

#[derive(Debug, Clone, Copy)]
pub struct SortSettings {
    /// Treat `modDependencies` as implicit loadAfter edges.
    pub dependencies_as_load_after: bool,
    /// Let a dependency's alternative package ids satisfy it.
    pub use_alternative_ids: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SortOutcome {
    /// New active order (empty when `cycles` is non-empty).
    pub order: Vec<ModId>,
    /// Package-id cycles that made a sort impossible.
    pub cycles: Vec<Vec<String>>,
}

struct Compiled {
    deps: Graph,
    rev: Graph,
    tier_zero: BTreeSet<String>,
    tier_one: BTreeSet<String>,
    tier_three: BTreeSet<String>,
}

fn compile(index: &ModIndex, s: SortSettings) -> Compiled {
    let mut c = Compiled {
        deps: Graph::new(),
        rev: Graph::new(),
        tier_zero: TIER_ZERO.iter().map(|s| s.to_string()).collect(),
        tier_one: TIER_ONE.iter().map(|s| s.to_string()).collect(),
        tier_three: BTreeSet::new(),
    };
    let mods: Vec<_> = index.mods.iter().filter(|m| m.valid).collect();
    let all: HashSet<&str> = mods.iter().map(|m| m.package_id.as_str()).collect();
    let edge = |c: &mut Compiled, node: &str, before: &str| {
        c.deps
            .entry(node.to_owned())
            .or_default()
            .insert(before.to_owned());
        c.rev
            .entry(before.to_owned())
            .or_default()
            .insert(node.to_owned());
    };

    for m in &mods {
        let pid = m.package_id.as_str();
        for dep in m.load_after_all().filter(|d| all.contains(d.as_str())) {
            edge(&mut c, pid, dep);
        }
        for target in m.load_before_all().filter(|d| all.contains(d.as_str())) {
            edge(&mut c, target, pid);
        }
        if m.load_first() && !c.tier_zero.contains(pid) {
            c.tier_one.insert(pid.to_owned());
        }
        if m.load_last() {
            c.tier_three.insert(pid.to_owned());
        }
    }

    if s.dependencies_as_load_after {
        let excluded: HashSet<String> = c.tier_one.union(&c.tier_three).cloned().collect();
        for m in &mods {
            let pid = m.package_id.as_str();
            if excluded.contains(pid) {
                continue;
            }
            for dep in &m.rules.dependencies {
                let mut target = dep.package_id.as_str();
                if !all.contains(target) {
                    match s
                        .use_alternative_ids
                        .then(|| dep.alternatives.iter().find(|a| all.contains(a.as_str())))
                        .flatten()
                    {
                        Some(alt) => target = alt,
                        None => continue,
                    }
                }
                if excluded.contains(target) {
                    continue;
                }
                // Explicit rules win: skip an inferred edge that contradicts one.
                if c.deps.get(target).is_some_and(|d| d.contains(pid)) {
                    continue;
                }
                edge(&mut c, pid, target);
            }
        }
    }
    c
}

/// Everything reachable from `start` following `graph` edges (start excluded unless in a cycle).
fn reachable(start: &str, graph: &Graph) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(n) = stack.pop() {
        for next in graph.get(n).into_iter().flatten() {
            if seen.insert(next.clone()) {
                stack.push(next);
            }
        }
    }
    seen
}

fn subgraph(full: &Graph, members: &BTreeSet<String>) -> Graph {
    members
        .iter()
        .map(|m| {
            let deps = full
                .get(m)
                .map(|d| d.intersection(members).cloned().collect())
                .unwrap_or_default();
            (m.clone(), deps)
        })
        .collect()
}

/// Kahn levels: level N holds nodes whose deps are all in earlier levels. `Err` = cycles.
fn levels(graph: &Graph) -> Result<Vec<Vec<String>>, Vec<Vec<String>>> {
    let mut remaining: BTreeMap<&str, BTreeSet<&str>> = graph
        .iter()
        .map(|(k, v)| {
            (
                k.as_str(),
                v.iter().map(String::as_str).filter(|d| d != k).collect(),
            )
        })
        .collect();
    let mut out = Vec::new();
    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining
            .iter()
            .filter(|(_, d)| d.is_empty())
            .map(|(k, _)| *k)
            .collect();
        if ready.is_empty() {
            let mut g = DiGraphMap::<&str, ()>::new();
            for (n, deps) in &remaining {
                g.add_node(n);
                for d in deps {
                    g.add_edge(*n, *d, ());
                }
            }
            let mut cycles: Vec<Vec<String>> = tarjan_scc(&g)
                .into_iter()
                .filter(|c| c.len() > 1)
                .map(|c| {
                    let mut c: Vec<String> = c.into_iter().map(String::from).collect();
                    c.sort();
                    c
                })
                .collect();
            cycles.sort();
            return Err(cycles);
        }
        for r in &ready {
            remaining.remove(r);
        }
        for deps in remaining.values_mut() {
            for r in &ready {
                deps.remove(r);
            }
        }
        out.push(ready.into_iter().map(String::from).collect());
    }
    Ok(out)
}

/// Human-readable rules that form each cycle: "A must load after B  [mod|community|your rules]".
/// Only rules whose both ends are in the same cycle are listed.
pub fn explain_cycles(index: &ModIndex, cycles: &[Vec<String>]) -> Vec<Vec<String>> {
    let name = |pid: &str| {
        index
            .by_package(pid)
            .next()
            .map_or_else(|| pid.to_owned(), |m| format!("{} ({pid})", m.name))
    };
    cycles
        .iter()
        .map(|cycle| {
            let members: HashSet<&str> = cycle.iter().map(String::as_str).collect();
            let mut lines = Vec::new();
            for pid in cycle {
                let Some(m) = index.by_package(pid).next() else {
                    continue;
                };
                let sources: [(&str, &[String], &[String]); 3] = [
                    ("mod's About.xml", &m.rules.load_after, &m.rules.load_before),
                    (
                        "community rules",
                        &m.community.load_after,
                        &m.community.load_before,
                    ),
                    ("your rules", &m.user.load_after, &m.user.load_before),
                ];
                for (src, after, before) in sources {
                    for o in after
                        .iter()
                        .filter(|o| members.contains(o.as_str()) && *o != pid)
                    {
                        lines.push(format!(
                            "{} must load after {}  [{src}]",
                            name(pid),
                            name(o)
                        ));
                    }
                    for o in before
                        .iter()
                        .filter(|o| members.contains(o.as_str()) && *o != pid)
                    {
                        lines.push(format!(
                            "{} must load before {}  [{src}]",
                            name(pid),
                            name(o)
                        ));
                    }
                }
            }
            lines.sort();
            lines
        })
        .collect()
}

pub fn sort_active(index: &ModIndex, active: &[ModId], settings: SortSettings) -> SortOutcome {
    let c = compile(index, settings);

    // pid -> mod id (first occurrence in the current order wins), plus display names.
    let mut pid_to_id: HashMap<&str, ModId> = HashMap::new();
    let mut names: HashMap<&str, String> = HashMap::new();
    for id in active {
        if let Some(m) = index.get(*id).filter(|m| m.valid) {
            pid_to_id.entry(m.package_id.as_str()).or_insert(m.id);
            names
                .entry(m.package_id.as_str())
                .or_insert_with(|| m.name.to_lowercase());
        }
    }
    let active_pids: BTreeSet<String> = pid_to_id.keys().map(|s| s.to_string()).collect();

    let filter = |g: &Graph| -> Graph {
        g.iter()
            .filter(|(k, _)| active_pids.contains(*k))
            .map(|(k, v)| (k.clone(), v.intersection(&active_pids).cloned().collect()))
            .collect()
    };
    let active_deps = filter(&c.deps);
    let active_rev = filter(&c.rev);

    let expand = |known: &BTreeSet<String>, graph: &Graph| -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for m in known.iter().filter(|m| active_pids.contains(*m)) {
            out.insert(m.clone());
            out.extend(reachable(m, graph));
        }
        out.intersection(&active_pids).cloned().collect()
    };
    let t0 = expand(&c.tier_zero, &active_deps);
    let t1 = expand(&c.tier_one, &active_deps);
    let t3 = expand(&c.tier_three, &active_rev);
    let tiered: BTreeSet<String> = t0.iter().chain(&t1).chain(&t3).cloned().collect();
    let t2: BTreeSet<String> = active_pids.difference(&tiered).cloned().collect();

    let mut order: Vec<ModId> = Vec::with_capacity(active.len());
    let mut placed: HashSet<ModId> = HashSet::new();
    for tier in [&t0, &t1, &t2, &t3] {
        match levels(&subgraph(&active_deps, tier)) {
            Ok(lv) => {
                for mut level in lv {
                    level.sort_by(|a, b| {
                        names[a.as_str()]
                            .cmp(&names[b.as_str()])
                            .then_with(|| a.cmp(b))
                    });
                    for pid in level {
                        let id = pid_to_id[pid.as_str()];
                        if placed.insert(id) {
                            order.push(id);
                        }
                    }
                }
            }
            Err(cycles) => {
                return SortOutcome {
                    order: vec![],
                    cycles,
                };
            }
        }
    }
    // Entries the sort can't place (invalid mods, duplicate package ids) keep their relative order at the end.
    order.extend(active.iter().copied().filter(|id| placed.insert(*id)));
    SortOutcome {
        order,
        cycles: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::{Dependency, Mod, ModType, Rules};
    use std::path::Path;

    fn mk(pid: &str, name: &str, after: &[&str], before: &[&str], deps: &[&str]) -> Mod {
        Mod {
            id: ModId::from_path(Path::new(pid)),
            path: pid.into(),
            folder: pid.into(),
            mod_type: ModType::Local,
            valid: true,
            invalid_reason: None,
            package_id: pid.into(),
            name: name.into(),
            authors: vec![],
            description: String::new(),
            url: String::new(),
            mod_version: String::new(),
            supported_versions: vec![],
            published_file_id: None,
            steam_app_id: None,
            rules: Rules {
                load_after: after.iter().map(|s| s.to_string()).collect(),
                load_before: before.iter().map(|s| s.to_string()).collect(),
                incompatible_with: vec![],
                dependencies: deps
                    .iter()
                    .map(|d| Dependency {
                        package_id: d.to_string(),
                        ..Default::default()
                    })
                    .collect(),
            },
            community: Default::default(),
            user: Default::default(),
            mtime: 0,
        }
    }

    const S: SortSettings = SortSettings {
        dependencies_as_load_after: false,
        use_alternative_ids: true,
    };

    fn run(mods: Vec<Mod>, s: SortSettings) -> (Vec<String>, SortOutcome) {
        let idx = ModIndex::new(mods, String::new(), 0);
        let active: Vec<ModId> = idx.mods.iter().rev().map(|m| m.id).collect(); // scrambled input
        let out = sort_active(&idx, &active, s);
        (
            out.order
                .iter()
                .map(|id| idx.get(*id).unwrap().package_id.clone())
                .collect(),
            out,
        )
    }

    #[test]
    fn tiers_levels_and_alpha() {
        let (order, _) = run(
            vec![
                mk("zed.mod", "Zed", &["brrainz.harmony"], &[], &[]),
                mk("brrainz.harmony", "Harmony", &[], &[], &[]),
                mk("ludeon.rimworld", "RimWorld", &[], &[], &[]),
                mk("a.mod", "Alpha", &["zed.mod"], &[], &[]),
                mk("b.mod", "Beta", &[], &["a.mod"], &[]),
                mk("unlimitedhugs.hugslib", "HugsLib", &[], &[], &[]),
            ],
            S,
        );
        // tier0 (Harmony < RimWorld alphabetically), tier1 (HugsLib), then Beta/Zed level 0, Alpha level 1.
        assert_eq!(
            order,
            [
                "brrainz.harmony",
                "ludeon.rimworld",
                "unlimitedhugs.hugslib",
                "b.mod",
                "zed.mod",
                "a.mod"
            ]
        );
    }

    #[test]
    fn cycles_are_explained_with_their_sources() {
        let mut a = mk("a", "A", &["b"], &[], &[]);
        a.user.load_after = vec!["b".into()]; // same edge again, from the user's rules
        let idx = ModIndex::new(
            vec![
                a,
                mk("b", "B", &["a"], &[], &[]),
                mk("c", "C", &["a"], &[], &[]),
            ],
            String::new(),
            0,
        );
        let lines = &explain_cycles(&idx, &[vec!["a".into(), "b".into()]])[0];
        assert_eq!(
            lines,
            &[
                "A (a) must load after B (b)  [mod's About.xml]",
                "A (a) must load after B (b)  [your rules]",
                "B (b) must load after A (a)  [mod's About.xml]",
            ]
        ); // C is not in the cycle
    }

    #[test]
    fn cycle_reported() {
        let (order, out) = run(
            vec![
                mk("a", "A", &["b"], &[], &[]),
                mk("b", "B", &["a"], &[], &[]),
            ],
            S,
        );
        assert!(order.is_empty());
        assert_eq!(out.cycles, [["a", "b"]]);
    }

    #[test]
    fn dependencies_as_load_after_and_conflict_skip() {
        let s = SortSettings {
            dependencies_as_load_after: true,
            ..S
        };
        let (order, _) = run(
            vec![mk("a", "A", &[], &[], &["z"]), mk("z", "Z", &[], &[], &[])],
            s,
        );
        assert_eq!(order, ["z", "a"]);
        // explicit "z after a" beats inferred "a after z" -> no cycle
        let (order, out) = run(
            vec![
                mk("a", "A", &[], &[], &["z"]),
                mk("z", "Z", &["a"], &[], &[]),
            ],
            s,
        );
        assert!(out.cycles.is_empty());
        assert_eq!(order, ["a", "z"]);
    }
}
