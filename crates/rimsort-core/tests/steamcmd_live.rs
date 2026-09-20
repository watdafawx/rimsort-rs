//! Live network test (installs SteamCMD, ~100 MB): `cargo test -p rimsort-core --test steamcmd_live -- --ignored --nocapture`
//! Downloads Harmony (2009463077) into `debug/steamcmd_test/mods`.

use rimsort_core::{TaskEvent, TaskManager, TaskSink, steamcmd};
use std::{path::PathBuf, sync::mpsc};

struct Log;
impl TaskSink for Log {
    fn emit(&self, e: TaskEvent) {
        if let TaskEvent::Progress {
            msg, done, total, ..
        } = e
        {
            println!("  [{done}/{total}] {msg}");
        }
    }
}

#[test]
#[ignore]
fn downloads_harmony() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../debug/steamcmd_test");
    let (install, mods) = (root.join("install"), root.join("mods"));
    std::fs::create_dir_all(&mods).unwrap();
    let tm = TaskManager::new(Log);
    let (tx, rx) = mpsc::channel();
    let (i, m) = (install.clone(), mods.clone());
    tm.spawn(move |ctx| {
        tx.send(steamcmd::download(
            &["2009463077".into(), "1".into()],
            &m,
            &i,
            ctx,
        ))
        .ok();
        Ok(())
    });
    let failed = rx.recv().unwrap().unwrap();
    println!("failed: {failed:?}");
    assert!(mods.join("2009463077/About/About.xml").exists());
    assert_eq!(failed, ["1"]); // id 1 doesn't exist -> reported, not fatal
}
