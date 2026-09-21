//! todds texture optimizer (joseasoler/todds): downloaded on demand into our data folder and run
//! as a cancellable subprocess whose `Progress: n/m` output feeds task progress.

use crate::{Error, Result, TaskCtx, launch::split_args, settings};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

const VERSION: &str = "0.4.1";

fn archive_url() -> String {
    let asset = if cfg!(windows) {
        format!("todds_Windows_{VERSION}.zip")
    } else if cfg!(target_os = "macos") {
        let arch = if cfg!(target_arch = "aarch64") {
            "arm"
        } else {
            "i386"
        };
        format!("todds_Darwin_{arch}_{VERSION}.zip")
    } else {
        format!("todds_Linux_x86_64_{VERSION}.zip")
    };
    format!("https://github.com/joseasoler/todds/releases/download/{VERSION}/{asset}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Preset {
    /// BC1 / BC7 with mipmaps: what RimSort recommends.
    Optimized,
    /// Delete the `.dds` files todds would create.
    Clean,
    /// The user's own arguments.
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ToddsOptions {
    pub preset: Preset,
    pub dry_run: bool,
    pub overwrite: bool,
    pub custom_command: String,
    /// Only the active mods; otherwise every mod in the local and Workshop folders.
    pub active_mods_target: bool,
    /// Run the optimizer before every game launch.
    pub auto_before_launch: bool,
}

impl Default for ToddsOptions {
    fn default() -> Self {
        Self {
            preset: Preset::Optimized,
            dry_run: false,
            overwrite: false,
            custom_command: String::new(),
            active_mods_target: true,
            auto_before_launch: false,
        }
    }
}

impl Preset {
    /// Parse RimSort's stored preset name ("optimized", "clean", or anything containing "custom").
    pub fn from_setting(s: &str) -> Self {
        let s = s.to_ascii_lowercase();
        if s.contains("custom") {
            Self::Custom
        } else if s.contains("clean") {
            Self::Clean
        } else {
            Self::Optimized
        }
    }

    pub fn setting_name(self) -> &'static str {
        match self {
            Self::Optimized => "optimized",
            Self::Clean => "clean",
            Self::Custom => "custom",
        }
    }
}

pub fn install_dir() -> PathBuf {
    settings::data_dir().join("todds")
}

fn exe(dir: &Path) -> PathBuf {
    dir.join(if cfg!(windows) { "todds.exe" } else { "todds" })
}

pub fn installed() -> bool {
    exe(&install_dir()).exists()
}

/// The command line for `opts` (RimSort's presets), ending with the input path list.
pub fn build_args(opts: &ToddsOptions, input: &Path) -> Vec<String> {
    let mut args: Vec<String> = match opts.preset {
        Preset::Clean => ["-cl", "-o", "-ss", "Textures", "-p", "-t"]
            .map(String::from)
            .into(),
        Preset::Optimized => [
            "-f",
            "BC1",
            "-af",
            "BC7",
            if opts.overwrite { "-o" } else { "-on" },
            "-vf",
            "-fs",
            "-ss",
            "Textures",
            "-t",
            "-p",
        ]
        .map(String::from)
        .into(),
        Preset::Custom => split_args(&opts.custom_command),
    };
    if opts.dry_run {
        // A dry run only lists what would change: no progress bar, no timing.
        args.retain(|a| a != "-p" && a != "-t");
        args.extend(["-v".into(), "-dr".into()]);
    } else if opts.preset == Preset::Custom && !args.iter().any(|a| a == "-p") {
        args.push("-p".into());
    }
    args.push(input.to_string_lossy().into_owned());
    args
}

fn download(ctx: &TaskCtx) -> Result<()> {
    let dir = install_dir();
    ctx.progress(0, 0, "Downloading todds");
    let bytes = reqwest::blocking::get(archive_url())
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.bytes())
        .map_err(|e| Error::Other(format!("could not download todds: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .and_then(|mut z| z.extract(&dir)) // rejects `..` / absolute entry names
        .map_err(|e| Error::Other(format!("could not unpack todds: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = exe(&dir);
        let mut perm = std::fs::metadata(&p)?.permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&p, perm)?;
    }
    if !exe(&dir).exists() {
        return Err(Error::Other(
            "the todds archive did not contain todds".into(),
        ));
    }
    Ok(())
}

/// `Progress: 12/300` → (12, 300).
fn parse_progress(line: &str) -> Option<(u32, u32)> {
    let rest = line.trim().strip_prefix("Progress:")?;
    let (a, b) = rest.split_once('/')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// Run todds over `targets` (directories), installing it first if needed.
pub fn run(opts: &ToddsOptions, targets: &[PathBuf], ctx: &TaskCtx) -> Result<()> {
    if targets.is_empty() {
        return Err(Error::Other("No mod folders to process".into()));
    }
    if !installed() {
        download(ctx)?;
    }
    let dir = install_dir();
    let list = dir.join("todds.txt");
    let text: String = targets
        .iter()
        .map(|p| format!("{}\n", p.to_string_lossy()))
        .collect();
    std::fs::write(&list, text)?;

    let args = build_args(opts, &list);
    tracing::info!("todds {args:?} over {} folders", targets.len());
    let mut cmd = Command::new(exe(&dir));
    cmd.args(&args)
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Other(format!("could not start todds: {e}")))?;

    let (tx, rx) = mpsc::channel();
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            // Progress is redrawn with bare \r; split on both.
            for chunk in BufReader::new(stream).split(b'\n').flatten() {
                for line in String::from_utf8_lossy(&chunk).split('\r') {
                    if !line.trim().is_empty() && tx.send(line.trim().to_owned()).is_err() {
                        return;
                    }
                }
            }
        });
    }
    drop(tx);

    let mut last = String::new();
    loop {
        if ctx.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Cancelled);
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                if let Some((done, total)) = parse_progress(&line) {
                    ctx.progress(done, total, format!("todds: {done}/{total} textures"));
                } else {
                    ctx.progress(0, 0, format!("todds: {line}"));
                    last = line;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = child
        .wait()
        .map_err(|e| Error::Other(format!("todds: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Other(format!("todds failed ({status}): {last}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(preset: Preset) -> ToddsOptions {
        ToddsOptions {
            preset,
            ..Default::default()
        }
    }
    const IN: &str = "todds.txt";

    #[test]
    fn optimized_and_clean_presets() {
        let a = build_args(&opts(Preset::Optimized), Path::new(IN));
        assert_eq!(
            a.join(" "),
            "-f BC1 -af BC7 -on -vf -fs -ss Textures -t -p todds.txt"
        );
        let mut o = opts(Preset::Optimized);
        o.overwrite = true;
        assert!(build_args(&o, Path::new(IN)).contains(&"-o".to_owned()));
        assert_eq!(
            build_args(&opts(Preset::Clean), Path::new(IN)).join(" "),
            "-cl -o -ss Textures -p -t todds.txt"
        );
    }

    #[test]
    fn dry_run_drops_progress_and_lists_files() {
        let mut o = opts(Preset::Optimized);
        o.dry_run = true;
        let a = build_args(&o, Path::new(IN));
        assert!(!a.contains(&"-p".to_owned()) && !a.contains(&"-t".to_owned()));
        assert_eq!(&a[a.len() - 3..], ["-v", "-dr", IN]);
    }

    #[test]
    fn custom_gets_progress_and_input() {
        let mut o = opts(Preset::Custom);
        o.custom_command = r#"-f BC7 -ss "My Mods""#.into();
        assert_eq!(
            build_args(&o, Path::new(IN)),
            ["-f", "BC7", "-ss", "My Mods", "-p", IN]
        );
        o.custom_command = "-f BC7 -p".into();
        assert_eq!(build_args(&o, Path::new(IN)), ["-f", "BC7", "-p", IN]);
    }

    #[test]
    fn parses_progress_and_presets() {
        assert_eq!(parse_progress("Progress: 12/300"), Some((12, 300)));
        assert_eq!(parse_progress("Launching encoding pipeline."), None);
        assert_eq!(Preset::from_setting("Custom preset"), Preset::Custom);
        assert_eq!(Preset::from_setting("clean"), Preset::Clean);
        assert_eq!(Preset::from_setting(""), Preset::Optimized);
    }

    /// Downloads todds and runs it over generated PNGs: `cargo test -p rimsort-core -- --ignored todds_end_to_end`.
    #[test]
    #[ignore]
    fn todds_end_to_end() {
        use crate::{TaskEvent, TaskManager, TaskSink};
        struct Null;
        impl TaskSink for Null {
            fn emit(&self, _: TaskEvent) {}
        }
        let root = std::env::temp_dir().join(format!("rs-todds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let tex = root.join("mod/Textures");
        std::fs::create_dir_all(&tex).unwrap();
        // A valid 8x8 RGBA PNG.
        let png = |name: &str| {
            let mut raw = Vec::new();
            for _ in 0..8 {
                raw.push(0u8);
                raw.extend(std::iter::repeat_n([200u8, 100, 50, 255], 8).flatten());
            }
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            std::io::Write::write_all(&mut z, &raw).unwrap();
            let idat = z.finish().unwrap();
            let chunk = |t: &[u8], d: &[u8]| {
                let mut c = (d.len() as u32).to_be_bytes().to_vec();
                c.extend(t);
                c.extend(d);
                let mut h = flate2::Crc::new();
                h.update(t);
                h.update(d);
                c.extend(h.sum().to_be_bytes());
                c
            };
            let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
            let mut ihdr = 8u32.to_be_bytes().to_vec();
            ihdr.extend(8u32.to_be_bytes());
            ihdr.extend([8, 6, 0, 0, 0]);
            out.extend(chunk(b"IHDR", &ihdr));
            out.extend(chunk(b"IDAT", &idat));
            out.extend(chunk(b"IEND", b""));
            std::fs::write(tex.join(name), out).unwrap();
        };
        png("a.png");
        png("b.png");
        // SAFETY: only this ignored test touches the environment.
        unsafe { std::env::set_var("RIMSORT_RS_DATA_DIR", root.join("data")) };

        let tm = TaskManager::new(Null);
        let (tx, rx) = std::sync::mpsc::channel();
        let targets = vec![root.join("mod")];
        tm.spawn(move |ctx| {
            let a = run(&ToddsOptions::default(), &targets, ctx);
            let n = |e: &str| {
                std::fs::read_dir(targets[0].join("Textures"))
                    .unwrap()
                    .flatten()
                    .filter(|f| f.path().extension().is_some_and(|x| x == e))
                    .count()
            };
            let made = n("dds");
            let clean = ToddsOptions {
                preset: Preset::Clean,
                ..Default::default()
            };
            let b = run(&clean, &targets, ctx);
            tx.send((a.is_ok(), made, b.is_ok(), n("dds"))).ok();
            Ok(())
        });
        let r = rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(r, (true, 2, true, 0));
    }
}
