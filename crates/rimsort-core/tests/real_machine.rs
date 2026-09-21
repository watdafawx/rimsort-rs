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
