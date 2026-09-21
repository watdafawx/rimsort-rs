//! Steam Workshop details (subscribers, tags, removal, dates…) for installed Workshop mods, fetched in the
//! background from Steam's public `GetPublishedFileDetails` endpoint (no API key) and cached on disk.
//! Requests are batched and spaced out, retried with backoff, and stop when the task is cancelled.

use crate::{Error, Result, TaskCtx, settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use std::{collections::HashMap, fs, path::PathBuf, time::Duration};

const ENDPOINT: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";
const BATCH: usize = 100;
/// Polite pause between batches.
const PACE: Duration = Duration::from_millis(1500);
/// Entries older than this are fetched again.
const MAX_AGE_SECS: u64 = 7 * 24 * 3600;
const MAX_DESCRIPTION: usize = 1200;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct WorkshopMeta {
    pub id: String,
    pub title: String,
    /// Steam's description (BBCode), cut to a preview.
    pub description: String,
    pub subscriptions: u32,
    pub favorited: u32,
    pub tags: Vec<String>,
    pub time_created: u32,
    pub time_updated: u32,
    pub file_size: u32,
    /// Steam no longer lists the item (deleted, or hidden by its author).
    pub removed: bool,
    pub banned: bool,
    /// When we fetched this (unix seconds).
    pub fetched: u32,
}

pub fn cache_path() -> PathBuf {
    settings::data_dir().join("workshop_cache.json")
}

pub fn load_cache() -> HashMap<String, WorkshopMeta> {
    fs::read(cache_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_cache(map: &HashMap<String, WorkshopMeta>) -> Result<()> {
    settings::atomic_write(&cache_path(), &serde_json::to_vec(map)?)
}

pub fn now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as u32)
}

/// Ids that are missing from the cache or stale.
pub fn needs_fetch(cache: &HashMap<String, WorkshopMeta>, ids: &[String], now: u32) -> Vec<String> {
    ids.iter()
        .filter(|id| {
            cache
                .get(*id)
                .is_none_or(|m| u64::from(now.saturating_sub(m.fetched)) > MAX_AGE_SECS)
        })
        .cloned()
        .collect()
}

/// One entry of `publishedfiledetails`.
fn parse_entry(v: &Value, fetched: u32) -> Option<WorkshopMeta> {
    let id = v.get("publishedfileid")?.as_str()?.to_owned();
    let num = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_u64().or_else(|| x.as_str()?.parse().ok()))
            .unwrap_or(0)
    };
    let result = num("result");
    let mut description: String = v
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(MAX_DESCRIPTION)
        .collect();
    description = description.trim().to_owned();
    Some(WorkshopMeta {
        id,
        title: v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        description,
        subscriptions: num("lifetime_subscriptions").max(num("subscriptions")) as u32,
        favorited: num("lifetime_favorited").max(num("favorited")) as u32,
        tags: v
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.get("tag")?.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        time_created: num("time_created") as u32,
        time_updated: num("time_updated") as u32,
        file_size: num("file_size") as u32,
        // result 1 = OK; 9 = not found; visibility != 0 = friends-only/private.
        removed: result != 1 || num("visibility") != 0,
        banned: num("banned") != 0,
        fetched,
    })
}

pub fn parse_response(body: &Value, fetched: u32) -> Vec<WorkshopMeta> {
    body.pointer("/response/publishedfiledetails")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|e| parse_entry(e, fetched)).collect())
        .unwrap_or_default()
}

fn fetch_batch(
    client: &reqwest::blocking::Client,
    ids: &[String],
    ctx: &TaskCtx,
) -> Result<Vec<WorkshopMeta>> {
    let mut body = format!("itemcount={}", ids.len());
    for (i, id) in ids.iter().enumerate() {
        body.push_str(&format!("&publishedfileids%5B{i}%5D={id}"));
    }
    let mut delay = Duration::from_secs(5);
    for attempt in 0..4 {
        ctx.check()?;
        let resp = client
            .post(ENDPOINT)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.clone())
            .send();
        match resp {
            Ok(r) if r.status().is_success() => {
                let bytes = r
                    .bytes()
                    .map_err(|e| Error::Other(format!("Steam sent an unreadable answer: {e}")))?;
                let json: Value = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::Other(format!("Steam sent an unreadable answer: {e}")))?;
                return Ok(parse_response(&json, now()));
            }
            // Rate limited or a Steam hiccup: back off and retry.
            Ok(r) if r.status().as_u16() == 429 || r.status().is_server_error() => {
                if attempt == 3 {
                    return Err(Error::Other(format!("Steam answered {}", r.status())));
                }
            }
            Ok(r) => return Err(Error::Other(format!("Steam answered {}", r.status()))),
            Err(e) if attempt == 3 => {
                return Err(Error::Other(format!("Steam is unreachable: {e}")));
            }
            Err(_) => {}
        }
        // Sleep in slices so cancel stays responsive.
        let until = std::time::Instant::now() + delay;
        while std::time::Instant::now() < until {
            ctx.check()?;
            std::thread::sleep(Duration::from_millis(200));
        }
        delay *= 2;
    }
    Err(Error::Other("Steam did not answer".into()))
}

/// Fetch details for `ids` (only those missing/stale) into the on-disk cache; returns the full cache.
/// `on_update` runs after each batch so the caller can publish partial results.
pub fn sync(
    ids: &[String],
    ctx: &TaskCtx,
    mut on_update: impl FnMut(&HashMap<String, WorkshopMeta>),
) -> Result<HashMap<String, WorkshopMeta>> {
    let mut cache = load_cache();
    let todo = needs_fetch(&cache, ids, now());
    if todo.is_empty() {
        return Ok(cache);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| Error::Other(e.to_string()))?;
    let total = todo.len() as u32;
    for (n, chunk) in todo.chunks(BATCH).enumerate() {
        ctx.progress((n * BATCH) as u32, total, "Fetching Steam details");
        for m in fetch_batch(&client, chunk, ctx)? {
            cache.insert(m.id.clone(), m);
        }
        // Ids Steam did not return at all are marked removed so they are not re-asked every launch.
        for id in chunk {
            cache.entry(id.clone()).or_insert_with(|| WorkshopMeta {
                id: id.clone(),
                removed: true,
                fetched: now(),
                ..Default::default()
            });
        }
        save_cache(&cache)?;
        on_update(&cache);
        // Pause between batches only.
        if (n + 1) * BATCH < todo.len() {
            let until = std::time::Instant::now() + PACE;
            while std::time::Instant::now() < until {
                ctx.check()?;
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    ctx.progress(total, total, "Fetching Steam details");
    Ok(cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_ok_removed_and_string_numbers() {
        let body = json!({"response": {"publishedfiledetails": [
            {"publishedfileid": "1", "result": 1, "title": "Harmony", "description": "  desc  ",
             "lifetime_subscriptions": "1500", "subscriptions": 10, "visibility": 0, "banned": 0,
             "time_updated": 1756914561, "file_size": "5729526", "tags": [{"tag": "Mod"}, {"tag": "1.6"}]},
            {"publishedfileid": "2", "result": 9},
            {"publishedfileid": "3", "result": 1, "visibility": 2}
        ]}});
        let m = parse_response(&body, 100);
        assert_eq!(m.len(), 3);
        assert_eq!(
            (m[0].subscriptions, m[0].time_updated, m[0].file_size),
            (1500, 1756914561, 5729526)
        );
        assert_eq!(m[0].description, "desc");
        assert_eq!(m[0].tags, ["Mod", "1.6"]);
        assert!(!m[0].removed);
        assert!(m[1].removed, "result 9 = not found");
        assert!(m[2].removed, "non-public items count as removed");
        assert!(parse_response(&json!({}), 1).is_empty());
    }

    #[test]
    fn only_missing_or_stale_ids_are_fetched() {
        let mut cache = HashMap::new();
        let fresh = WorkshopMeta {
            id: "a".into(),
            fetched: 1_000_000,
            ..Default::default()
        };
        let stale = WorkshopMeta {
            id: "b".into(),
            fetched: 1,
            ..Default::default()
        };
        cache.insert("a".into(), fresh);
        cache.insert("b".into(), stale);
        let ids: Vec<String> = ["a", "b", "c"].map(String::from).into();
        assert_eq!(needs_fetch(&cache, &ids, 1_000_100), ["b", "c"]);
    }
}
