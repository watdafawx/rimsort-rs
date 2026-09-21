//! Runs the full pipeline against this machine's real RimWorld install (read-only).
//! `cargo test -p rimsort-core --test real_machine -- --ignored --nocapture`

use rimsort_core::{AppState, TaskEvent, TaskManager, TaskSink};
use std::{sync::Mutex, sync::mpsc, time::Instant};

struct Chan(Mutex<mpsc::Sender<TaskEvent>>);
impl TaskSink for Chan {
    fn emit(&self, e: TaskEvent) {
        self.0.lock().unwrap().send(e).ok();
    }
}

#[test]
#[ignore]
fn scan_sort_real_install() {
    let tmp = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();
    let state = AppState::with_settings_path(
        TaskManager::new(Chan(Mutex::new(tx))),
        tmp.path().join("s.json"),
    );
    let sv = state.settings_view();
    println!("instance {} game {}", sv.current_instance, sv.game_version);
    for c in &sv.checks {
        println!("  {:8} ok={} {} {}", c.kind, c.ok, c.path, c.reason);
    }

    let t = Instant::now();
    state.start_scan(false).unwrap();
    loop {
        match rx.recv().unwrap() {
            TaskEvent::Finished { .. } => break,
            TaskEvent::Failed { error, .. } => panic!("{}", error.message),
            _ => {}
        }
    }
    println!("scan+config total {:?}", t.elapsed());

    let l = state.lists();
    println!(
        "active {} inactive {} missing {:?} dup-pids {} scan_ms {}",
        l.active.len(),
        l.inactive.len(),
        l.missing,
        l.duplicate_package_ids,
        l.scan_ms
    );
    println!(
        "invalid: {}",
        l.active
            .iter()
            .chain(&l.inactive)
            .filter(|r| !r.valid)
            .count()
    );

    println!(
        "rules: community {} user {}",
        l.community_rules, l.user_rules
    );
    let v = state.validate();
    println!(
        "validation before sort: {} errors, {} warnings; total flagged {}",
        v.errors,
        v.warnings,
        v.mods.len()
    );
    let all: Vec<_> = l.active.iter().chain(&l.inactive).collect();
    println!(
        "colored {} with-note {}",
        all.iter().filter(|r| r.color.is_some()).count(),
        all.iter().filter(|r| r.has_note).count()
    );
    let before: Vec<_> = l.active.iter().map(|r| r.package_id.clone()).collect();
    let t = Instant::now();
    let r = state.sort_active();
    println!(
        "sort ok={} cycles={} changed={} in {:?}",
        r.ok,
        r.cycles.len(),
        r.changed,
        t.elapsed()
    );
    for c in r.cycles.iter().take(5) {
        println!("  cycle: {}", c.members.join(" <-> "));
    }
    let after: Vec<_> = state
        .lists()
        .active
        .iter()
        .map(|r| r.package_id.clone())
        .collect();
    println!("first 12 after sort: {:?}", &after[..after.len().min(12)]);
    let same = before.iter().zip(&after).filter(|(a, b)| a == b).count();
    println!(
        "positions identical to saved order: {same}/{}",
        before.len()
    );
    if let Some(i) = before.iter().zip(&after).position(|(a, b)| a != b) {
        println!(
            "first divergence at {i}: saved {:?} vs ours {:?}",
            &before[i..(i + 4).min(before.len())],
            &after[i..(i + 4).min(after.len())]
        );
    }
    let v2 = state.validate();
    println!(
        "validation after sort: {} errors, {} warnings",
        v2.errors, v2.warnings
    );
    for mw in v2.mods.iter().take(6) {
        println!(
            "  {:?}",
            mw.warnings
                .iter()
                .map(|w| format!("{:?}:{}", w.kind, w.other))
                .collect::<Vec<_>>()
        );
    }
    if let Ok(p) = std::env::var("RUST_SORT_OUT") {
        std::fs::write(
            p,
            format!(
                "{{\"ok\": {}, \"before\": {:?}, \"sorted\": {:?}}}",
                r.ok, before, after
            ),
        )
        .unwrap();
    }
    if r.ok {
        assert_eq!(before.len(), after.len());
    }
}

/// Sort every `*.xml` ModsConfig in `GOLDEN_DIR` and write `<name>.rs.json` next to it, for
/// `tests/golden/multi.mjs` to compare against RimSort's Python (`GOLDEN_DIR` mode of py_sort.py).
#[test]
#[ignore]
fn sort_golden_configs() {
    let Some(dir) = std::env::var_os("GOLDEN_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();
    let state = AppState::with_settings_path(
        TaskManager::new(Chan(Mutex::new(tx))),
        tmp.path().join("s.json"),
    );
    state.start_scan(false).unwrap();
    loop {
        match rx.recv().unwrap() {
            TaskEvent::Finished { .. } => break,
            TaskEvent::Failed { error, .. } => panic!("{}", error.message),
            _ => {}
        }
    }
    if std::env::var_os("GOLDEN_ALPHA").is_some() {
        let mut o = state.settings_view().options;
        o.alphabetical_sort = true;
        state.update_options(o).unwrap();
    }
    let l = state.lists();
    let mut by_pid = std::collections::HashMap::new();
    for r in l.active.iter().chain(&l.inactive).filter(|r| r.valid) {
        by_pid.entry(r.package_id.clone()).or_insert(r.id);
    }
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "xml"))
        .collect();
    files.sort();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let ids: Vec<_> = text
            .split("<li>")
            .skip(1)
            .filter_map(|c| c.split("</li>").next())
            .filter_map(|p| by_pid.get(p.trim().to_lowercase().trim_end_matches("_steam")))
            .copied()
            .collect();
        state.set_active(ids);
        let before: Vec<_> = state
            .lists()
            .active
            .iter()
            .map(|r| r.package_id.clone())
            .collect();
        let r = state.sort_active();
        let after: Vec<_> = state
            .lists()
            .active
            .iter()
            .map(|r| r.package_id.clone())
            .collect();
        std::fs::write(
            f.with_extension("rs.json"),
            format!(
                "{{\"ok\": {}, \"before\": {before:?}, \"sorted\": {after:?}}}",
                r.ok
            ),
        )
        .unwrap();
        println!("{}: ok={} sorted={}", f.display(), r.ok, after.len());
    }
}
