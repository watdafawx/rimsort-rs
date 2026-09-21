//! Steam Workshop subscribe/unsubscribe through the Steamworks client API (Steam must be running).
//!
//! `steam_api64.dll` is delay-loaded and shipped next to the exe (see `build.rs`); if it is missing
//! or Steam isn't running, calls fail with a message instead of taking the app down. The Steam
//! client is created per operation and dropped straight after, because while it is alive Steam
//! shows the user as "playing RimWorld".

use rimsort_core::{Error, Result, paths::RIMWORLD_APPID};
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
pub struct SteamOutcome {
    pub id: String,
    /// None on success.
    pub error: Option<String>,
}

#[cfg(windows)]
pub fn set_subscribed(ids: &[String], subscribe: bool) -> Result<Vec<SteamOutcome>> {
    use std::{sync::mpsc, time::Duration};
    use steamworks::{Client, PublishedFileId};

    let ids: Vec<u64> = ids
        .iter()
        .map(|i| i.parse::<u64>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error::Other("Workshop ids must be numeric".into()))?;
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    if !exe_dir.is_some_and(|d| d.join("steam_api64.dll").is_file()) {
        return Err(Error::Other(
            "steam_api64.dll is missing next to the app; reinstall RimSort-rs".into(),
        ));
    }
    let client =
        Client::init_app(RIMWORLD_APPID.parse::<u32>().unwrap_or(294_100)).map_err(|e| {
            Error::Other(format!(
                "Could not connect to Steam ({e}). Is the Steam client running and signed in?"
            ))
        })?;
    let ugc = client.ugc();
    let mut out = Vec::new();
    for id in ids {
        let (tx, rx) = mpsc::channel();
        let done = move |r: std::result::Result<(), steamworks::SteamError>| {
            let _ = tx.send(r);
        };
        if subscribe {
            ugc.subscribe_item(PublishedFileId(id), done);
        } else {
            ugc.unsubscribe_item(PublishedFileId(id), done);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let error = loop {
            client.run_callbacks();
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(())) => break None,
                Ok(Err(e)) => break Some(e.to_string()),
                Err(_) if std::time::Instant::now() > deadline => {
                    break Some("Steam did not answer in time".into());
                }
                Err(_) => {}
            }
        };
        out.push(SteamOutcome {
            id: id.to_string(),
            error,
        });
    }
    Ok(out)
}

#[cfg(not(windows))]
pub fn set_subscribed(_ids: &[String], _subscribe: bool) -> Result<Vec<SteamOutcome>> {
    Err(Error::Other(
        "Subscribing through Steam is only available on Windows for now".into(),
    ))
}
