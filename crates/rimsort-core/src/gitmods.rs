//! Update git-cloned mods with the system `git` (fast-forward only, never rewrites local work).

use crate::{Error, Result, TaskCtx};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

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

/// `https://github.com/<owner>/<repo>[.git][/]` -> (owner, repo). Anything else is rejected, so
/// arbitrary URLs / `--upload-pack`-style options can never reach git.
pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let rest = url.trim().strip_prefix("https://github.com/")?;
    let mut it = rest.trim_end_matches('/').split('/');
    let (owner, repo) = (it.next()?, it.next()?);
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            && !s.starts_with('.')
    };
    (it.next().is_none_or(|rest| rest.is_empty()) && ok(owner) && ok(repo))
        .then(|| (owner.to_owned(), repo.to_owned()))
}

/// Clone `source` into `local/<repo>`; cancellable. Returns the new folder.
fn clone_into(source: &str, repo: &str, local: &Path, ctx: &TaskCtx) -> Result<PathBuf> {
    let dest = local.join(repo);
    if dest.exists() {
        return Err(Error::Other(format!("{} already exists", dest.display())));
    }
    let mut child = Command::new("git")
        .args(["clone", "--quiet", "--", source])
        .arg(&dest)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Other(format!("could not run git: {e} (is git installed?)")))?;
    let status = loop {
        if ctx.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&dest); // don't leave a half clone behind
            return Err(Error::Cancelled);
        }
        match child.try_wait()? {
            Some(s) => break s,
            None => std::thread::sleep(Duration::from_millis(200)),
        }
    };
    if !status.success() {
        let mut msg = String::new();
        if let Some(mut e) = child.stderr.take() {
            std::io::Read::read_to_string(&mut e, &mut msg).ok();
        }
        let _ = std::fs::remove_dir_all(&dest);
        return Err(Error::Other(format!("git clone failed: {}", msg.trim())));
    }
    Ok(dest)
}

/// Clone GitHub repositories into the local mods folder, one after another.
pub fn clone_github(urls: &[String], local: &Path, ctx: &TaskCtx) -> Result<Vec<PathBuf>> {
    let total = urls.len() as u32;
    let mut out = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        let (owner, repo) = parse_github_url(url)
            .ok_or_else(|| Error::Other(format!("Not a GitHub repository URL: {url}")))?;
        ctx.progress(i as u32, total, format!("Cloning {owner}/{repo}"));
        out.push(clone_into(
            &format!("https://github.com/{owner}/{repo}.git"),
            &repo,
            local,
            ctx,
        )?);
    }
    ctx.progress(total, total, "Done");
    Ok(out)
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
    fn github_url_parsing_is_strict() {
        let ok = |u: &str| parse_github_url(u);
        assert_eq!(
            ok("https://github.com/Zetrith/Prepatcher"),
            Some(("Zetrith".into(), "Prepatcher".into()))
        );
        assert_eq!(
            ok("https://github.com/a/b.git/"),
            Some(("a".into(), "b".into()))
        );
        assert_eq!(ok(" https://github.com/a/b/tree/main"), None); // deep links rejected
        for bad in [
            "http://github.com/a/b",
            "https://evil.com/a/b",
            "https://github.com/a",
            "https://github.com/../b",
            "https://github.com/-x/y z",
            "git@github.com:a/b.git",
            "--upload-pack=x",
        ] {
            assert_eq!(ok(bad), None, "{bad}");
        }
    }

    #[test]
    fn clones_a_local_repo_and_refuses_existing_dest() {
        use crate::{TaskEvent, TaskManager, TaskSink};
        struct Null;
        impl TaskSink for Null {
            fn emit(&self, _: TaskEvent) {}
        }
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let t = tempfile::tempdir().unwrap();
        let origin = t.path().join("origin");
        std::fs::create_dir(&origin).unwrap();
        run(&origin, &["init", "-q", "-b", "main"]);
        run(&origin, &["config", "user.email", "t@t"]);
        run(&origin, &["config", "user.name", "t"]);
        std::fs::write(origin.join("a.txt"), "1").unwrap();
        run(&origin, &["add", "."]);
        run(&origin, &["commit", "-q", "-m", "one"]);
        let (local, src) = (t.path().join("mods"), origin.to_string_lossy().into_owned());
        std::fs::create_dir(&local).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        TaskManager::new(Null).spawn(move |ctx| {
            let first = clone_into(&src, "cloned", &local, ctx);
            let again = clone_into(&src, "cloned", &local, ctx);
            tx.send((
                first.is_ok() && local.join("cloned/a.txt").exists(),
                again.is_err(),
            ))
            .ok();
            Ok(())
        });
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(30)).unwrap(),
            (true, true)
        );
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
