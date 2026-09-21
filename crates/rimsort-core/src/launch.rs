//! Start RimWorld: directly via its executable (default) or through the Steam protocol.

use crate::{Error, Result, paths::RIMWORLD_APPID, settings::Instance};
use std::{path::Path, process::Command};

/// Split a run-args string on whitespace, honoring double quotes.
pub fn split_args(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

pub fn launch(inst: &Instance) -> Result<()> {
    let game = Path::new(&inst.game_folder);
    let args = split_args(&inst.run_args);
    let via_steam = inst
        .extra
        .get("launch_via_steam_protocol")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut cmd = if via_steam {
        let url = format!("steam://rungameid/{RIMWORLD_APPID}");
        if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", &url]);
            c
        } else if cfg!(target_os = "macos") {
            let mut c = Command::new("open");
            c.arg(&url);
            c
        } else {
            let mut c = Command::new("xdg-open");
            c.arg(&url);
            c
        }
    } else if cfg!(windows) {
        Command::new(game.join("RimWorldWin64.exe"))
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(game.join("RimWorldMac.app")).arg("--args");
        c
    } else {
        Command::new(game.join("RimWorldLinux"))
    };
    if !via_steam {
        cmd.args(&args).current_dir(game);
    }
    cmd.spawn()
        .map_err(|e| Error::Other(format!("Could not start RimWorld: {e}")))?;
    tracing::info!("launched RimWorld (via_steam={via_steam}, args={args:?})");
    Ok(())
}

/// Whether a RimWorld process is running (RimWorld rewrites ModsConfig.xml when it exits).
pub fn game_running() -> bool {
    use sysinfo::{ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .any(|p| is_game_process(&p.name().to_string_lossy()))
}

fn is_game_process(name: &str) -> bool {
    ["rimworldwin64", "rimworldlinux", "rimworldmac"]
        .iter()
        .any(|n| name.to_ascii_lowercase().starts_with(n))
}

#[cfg(test)]
mod tests {
    #[test]
    fn recognises_game_process_names() {
        assert!(super::is_game_process("RimWorldWin64.exe"));
        assert!(super::is_game_process("RimWorldLinux"));
        assert!(!super::is_game_process("RimSort-rs.exe"));
    }

    #[test]
    fn splits_quotes() {
        assert_eq!(
            super::split_args(r#"-popupwindow -savedatafolder="C:\My Saves" "" x"#),
            ["-popupwindow", r"-savedatafolder=C:\My Saves", "", "x"]
        );
        assert!(super::split_args("   ").is_empty());
    }
}
