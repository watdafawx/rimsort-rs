//! Incremental reader for RimWorld's `Player.log`.

use crate::Result;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// First read only takes this much of the file's tail.
const INITIAL_TAIL: u64 = 400 * 1024;
/// Never return more than this in one chunk.
const MAX_CHUNK: u64 = 4 * 1024 * 1024;

/// `Player.log` sits next to the `Config` folder.
pub fn player_log_path(config_folder: &str) -> Option<PathBuf> {
    Path::new(config_folder)
        .parent()
        .map(|p| p.join("Player.log"))
}

pub struct Chunk {
    pub text: String,
    /// Byte offset to pass to the next read.
    pub offset: u64,
    /// True when `text` replaces everything shown so far (first read, or the file was rewritten).
    pub reset: bool,
}

/// Read new bytes since `offset`. `None` = first read (tail). A shrunken file (game restarted) resets.
pub fn read_from(path: &Path, offset: Option<u64>) -> Result<Chunk> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let (start, reset) = match offset {
        None => (len.saturating_sub(INITIAL_TAIL), true),
        Some(o) if o > len => (len.saturating_sub(INITIAL_TAIL), true),
        Some(o) => (o, false),
    };
    let end = len.min(start + MAX_CHUNK);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0; (end - start) as usize];
    f.read_exact(&mut buf)?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    // A tail read can start mid-line; drop the partial first line.
    if start > 0
        && reset
        && let Some(i) = text.find('\n')
    {
        text.drain(..=i);
    }
    Ok(Chunk {
        text,
        offset: end,
        reset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tails_follows_and_resets() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("Player.log");
        std::fs::write(&p, "one\ntwo\n").unwrap();
        let c = read_from(&p, None).unwrap();
        assert!(c.reset && c.text == "one\ntwo\n");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&p)
            .unwrap()
            .write_all(b"three\n")
            .unwrap();
        let c2 = read_from(&p, Some(c.offset)).unwrap();
        assert!(!c2.reset && c2.text == "three\n");
        std::fs::write(&p, "new\n").unwrap(); // game restarted: file shrank
        let c3 = read_from(&p, Some(c2.offset)).unwrap();
        assert!(c3.reset && c3.text == "new\n");
        assert_eq!(
            player_log_path("C:/x/Config").unwrap().file_name().unwrap(),
            "Player.log"
        );
    }
}
