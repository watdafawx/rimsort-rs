//! Update git-cloned mods with the system `git` (fast-forward only, never rewrites local work).

use crate::{Error, Result};
use std::{path::Path, process::Command};

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0") // never hang waiting for credentials
        .output()
        .map_err(|e| Error::Other(format!("could not run git: {e} (is git installed?)")))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_owned();
    if out.status.success() {
        Ok(text)
    } else {
        Err(Error::Other(format!("git {}: {text}", args.join(" "))))
    }
}

/// Only the repository root counts (a mod folder nested inside some other repo is not a git mod).
pub fn is_repo_root(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// `git pull --ff-only`; refuses when there are local modifications so nothing is lost.
pub fn update(dir: &Path) -> Result<String> {
    if !is_repo_root(dir) {
        return Err(Error::Other("Not a git repository".into()));
    }
    if !git(dir, &["status", "--porcelain"])?.is_empty() {
        return Err(Error::Other(
            "The mod has local changes; commit or discard them first".into(),
        ));
    }
    git(dir, &["pull", "--ff-only"])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        git(dir, args).unwrap();
    }

    #[test]
    fn pulls_new_commits_and_refuses_dirty_trees() {
        if Command::new("git").arg("--version").output().is_err() {
            return; // git not installed on this machine
        }
        let t = tempfile::tempdir().unwrap();
        let (origin, clone) = (t.path().join("origin"), t.path().join("clone"));
        std::fs::create_dir(&origin).unwrap();
        run(&origin, &["init", "-q", "-b", "main"]);
        run(&origin, &["config", "user.email", "t@t"]);
        run(&origin, &["config", "user.name", "t"]);
        std::fs::write(origin.join("a.txt"), "1").unwrap();
        run(&origin, &["add", "."]);
        run(&origin, &["commit", "-q", "-m", "one"]);
        run(
            t.path(),
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );

        assert!(update(&clone).unwrap().contains("up to date") || update(&clone).is_ok());
        std::fs::write(origin.join("b.txt"), "2").unwrap();
        run(&origin, &["add", "."]);
        run(&origin, &["commit", "-q", "-m", "two"]);
        update(&clone).unwrap();
        assert!(clone.join("b.txt").exists());

        std::fs::write(clone.join("a.txt"), "dirty").unwrap();
        assert!(
            update(&clone)
                .unwrap_err()
                .to_string()
                .contains("local changes")
        );
        assert!(!is_repo_root(t.path().join("nope").as_path()));
    }
}
