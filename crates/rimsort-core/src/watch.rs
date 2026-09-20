//! Filesystem watching: tell the app when mods appear/disappear or `ModsConfig.xml` changes
//! outside of it. Debounced; never reads or mutates anything itself.

use crate::{Error, Result};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Something under a mods folder changed.
    Mods,
    /// `ModsConfig.xml` changed.
    Config,
}

/// Keeps the watcher alive; drop to stop.
pub struct FsWatch {
    _debouncer: Debouncer<RecommendedWatcher>,
}

fn classify(path: &Path, mods_dirs: &[PathBuf], config_dir: &Option<PathBuf>) -> Option<Change> {
    if path.components().any(|c| c.as_os_str() == ".git") {
        return None;
    }
    if let Some(cd) = config_dir
        && path.parent() == Some(cd.as_path())
        && path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("ModsConfig.xml"))
    {
        return Some(Change::Config);
    }
    mods_dirs
        .iter()
        .any(|d| path.starts_with(d))
        .then_some(Change::Mods)
}

/// Watch `mods_dirs` (recursively) and `config_dir` (for ModsConfig.xml only).
pub fn start(
    mods_dirs: Vec<PathBuf>,
    config_dir: Option<PathBuf>,
    on_change: impl Fn(Change) + Send + 'static,
) -> Result<FsWatch> {
    let (md, cd) = (mods_dirs.clone(), config_dir.clone());
    let mut debouncer = new_debouncer(
        Duration::from_millis(1500),
        move |res: DebounceEventResult| {
            let Ok(events) = res else { return };
            let mut kinds: Vec<Change> = events
                .iter()
                .filter_map(|e| classify(&e.path, &md, &cd))
                .collect();
            kinds.sort_by_key(|k| *k as u8);
            kinds.dedup();
            kinds.into_iter().for_each(&on_change);
        },
    )
    .map_err(|e| Error::Other(format!("watcher: {e}")))?;

    let w = debouncer.watcher();
    for d in mods_dirs.iter().filter(|d| d.is_dir()) {
        w.watch(d, RecursiveMode::Recursive)
            .map_err(|e| Error::Other(format!("watch {}: {e}", d.display())))?;
    }
    if let Some(c) = config_dir.filter(|c| c.is_dir()) {
        w.watch(&c, RecursiveMode::NonRecursive)
            .map_err(|e| Error::Other(format!("watch {}: {e}", c.display())))?;
    }
    Ok(FsWatch {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn classifies_paths() {
        let mods = vec![PathBuf::from("/m")];
        let cfg = Some(PathBuf::from("/c"));
        assert_eq!(
            classify(Path::new("/m/x/About/About.xml"), &mods, &cfg),
            Some(Change::Mods)
        );
        assert_eq!(
            classify(Path::new("/c/ModsConfig.xml"), &mods, &cfg),
            Some(Change::Config)
        );
        assert_eq!(classify(Path::new("/c/Other.xml"), &mods, &cfg), None);
        assert_eq!(classify(Path::new("/m/x/.git/index"), &mods, &cfg), None);
    }

    #[test]
    fn reports_new_mod_folder() {
        let t = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let _w = start(vec![t.path().to_path_buf()], None, move |c| {
            tx.send(c).ok();
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::create_dir(t.path().join("newmod")).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(6)).unwrap(),
            Change::Mods
        );
    }
}
