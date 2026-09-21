//! Benchmark on a synthetic install: `node scripts/gen_mods.mjs 5000 debug/synth` then
//! `SYNTH_DIR=debug/synth cargo test -p rimsort-core --release --test synthetic -- --ignored --nocapture`.

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
fn synthetic_install() {
    let dir = std::path::PathBuf::from(std::env::var("SYNTH_DIR").expect("set SYNTH_DIR"));
    let (tx, rx) = mpsc::channel();
    let state = AppState::with_settings_path(
        TaskManager::new(Chan(Mutex::new(tx))),
        dir.join("settings.json"),
    );

    let scan = |label: &str| {
        let t = Instant::now();
        state.start_scan(false).unwrap();
        loop {
            match rx.recv().unwrap() {
                TaskEvent::Finished { .. } => break,
                TaskEvent::Failed { error, .. } => panic!("{}", error.message),
                _ => {}
            }
        }
        println!("{label}: scan + load config {:?}", t.elapsed());
    };
    scan("cold-ish");
    scan("warm");

    let t = Instant::now();
    let l = state.lists();
    println!(
        "lists: {} active, {} inactive, {} missing in {:?}",
        l.active.len(),
        l.inactive.len(),
        l.missing.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let v = state.validate();
    println!(
        "validate: {} errors, {} warnings in {:?}",
        v.errors,
        v.warnings,
        t.elapsed()
    );

    let t = Instant::now();
    let r = state.sort_active();
    let sort_ms = t.elapsed();
    println!(
        "sort: ok={} cycles={} changed={} in {:?}",
        r.ok,
        r.cycles.len(),
        r.changed,
        sort_ms
    );
    assert!(r.ok, "synthetic graph is a DAG by construction");

    let t = Instant::now();
    let v2 = state.validate();
    println!(
        "validate after sort: {} errors, {} warnings in {:?}",
        v2.errors,
        v2.warnings,
        t.elapsed()
    );
}
