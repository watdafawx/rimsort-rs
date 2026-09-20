//! Download Workshop items with SteamCMD (installed on demand into our data folder).
//! Blocking + cancellable: run inside a task.

use crate::{Error, Result, TaskCtx, paths::RIMWORLD_APPID, settings};
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

#[cfg(windows)]
const ARCHIVE_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip";
#[cfg(target_os = "linux")]
const ARCHIVE_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz";
#[cfg(target_os = "macos")]
const ARCHIVE_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_osx.tar.gz";

pub fn install_dir() -> PathBuf {
    settings::data_dir().join("steamcmd")
}

fn exe(dir: &Path) -> PathBuf {
    dir.join(if cfg!(windows) {
        "steamcmd.exe"
    } else {
        "steamcmd.sh"
    })
}

fn extract(bytes: &[u8], dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(windows)]
    {
        zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .and_then(|mut z| z.extract(dir)) // rejects `..` / absolute entry names
            .map_err(|e| Error::Other(format!("could not unpack SteamCMD: {e}")))?;
    }
    #[cfg(not(windows))]
    {
        tar::Archive::new(flate2::read::GzDecoder::new(bytes))
            .unpack(dir)
            .map_err(|e| Error::Other(format!("could not unpack SteamCMD: {e}")))?;
    }
    Ok(())
}

/// Run SteamCMD with `args`, streaming stdout lines to `on_line`; kills it when the task is cancelled.
fn run(dir: &Path, args: &[String], ctx: &TaskCtx, mut on_line: impl FnMut(&str)) -> Result<()> {
    let mut child = Command::new(exe(dir))
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| Error::Other(format!("could not start SteamCMD: {e}")))?;
    let (tx, rx) = mpsc::channel();
    let out = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        // SteamCMD prints progress with bare \r; split on both.
        for chunk in BufReader::new(out).split(b'\n').flatten() {
            for line in String::from_utf8_lossy(&chunk).split('\r') {
                if tx.send(line.to_owned()).is_err() {
                    return;
                }
            }
        }
    });
    loop {
        if ctx.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Cancelled);
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(l) => on_line(l.trim()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.wait(); // first runs exit non-zero after self-updating; results are judged by output/files
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let target = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Download `ids` and copy each into `dest_mods/<id>` (replacing an older copy).
/// Returns the ids that could not be downloaded.
pub fn download(
    ids: &[String],
    dest_mods: &Path,
    install: &Path,
    ctx: &TaskCtx,
) -> Result<Vec<String>> {
    if ids
        .iter()
        .any(|i| i.is_empty() || !i.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(Error::Other("Workshop ids must be numeric".into())); // also blocks arg injection
    }
    let total = ids.len() as u32 + 1;
    if !exe(install).exists() {
        ctx.progress(0, total, "Downloading SteamCMD");
        let bytes = reqwest::blocking::get(ARCHIVE_URL)
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map_err(|e| Error::Other(format!("could not download SteamCMD: {e}")))?;
        extract(&bytes, install)?;
        ctx.progress(0, total, "Updating SteamCMD (first run)");
        run(install, &["+quit".into()], ctx, |_| {})?;
    }

    let mut args: Vec<String> = vec![
        "+force_install_dir".into(),
        install.to_string_lossy().into_owned(),
        "+login".into(),
        "anonymous".into(),
    ];
    for id in ids {
        args.extend([
            "+workshop_download_item".into(),
            RIMWORLD_APPID.into(),
            id.clone(),
        ]);
    }
    args.push("+quit".into());

    let mut done = 0u32;
    ctx.progress(0, total, "Downloading from Steam");
    run(install, &args, ctx, |line| {
        if line.contains("Downloaded item") || line.starts_with("ERROR!") {
            done += 1;
        }
        if !line.is_empty() {
            tracing::debug!("steamcmd: {line}");
        }
        ctx.progress(
            done.min(ids.len() as u32),
            total,
            line.chars().take(80).collect::<String>(),
        );
    })?;

    let content = install
        .join("steamapps/workshop/content")
        .join(RIMWORLD_APPID);
    let mut failed = Vec::new();
    for id in ids {
        let src = content.join(id);
        if src.join("About").exists()
            || std::fs::read_dir(&src).is_ok_and(|mut d| d.next().is_some())
        {
            let dst = dest_mods.join(id);
            if dst.exists() {
                std::fs::remove_dir_all(&dst)?;
            }
            copy_dir(&src, &dst)?;
        } else {
            failed.push(id.clone());
        }
    }
    Ok(failed)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_non_numeric_ids_before_running_anything() {
        use crate::{TaskEvent, TaskManager, TaskSink};
        struct Null;
        impl TaskSink for Null {
            fn emit(&self, _: TaskEvent) {}
        }
        let tm = TaskManager::new(Null);
        let (tx, rx) = std::sync::mpsc::channel();
        tm.spawn(move |ctx| {
            let r = super::download(
                &["12; rm -rf".into()],
                std::path::Path::new("."),
                std::path::Path::new("."),
                ctx,
            );
            tx.send(r.is_err()).ok();
            Ok(())
        });
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap());
    }
}
