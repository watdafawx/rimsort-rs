//! Download the external rule databases into our own `dbs` folder, using ETags so unchanged
//! files aren't fetched again. Blocking: run on a worker thread.

use crate::{Error, Result, settings};
use serde::Serialize;
use specta::Type;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Source {
    pub name: &'static str,
    pub url: String,
    /// Path relative to our `dbs` folder (matches the layout `rules::resolve` searches).
    pub dest: String,
}

/// The databases RimSort uses, from their default GitHub repositories.
pub fn sources(game_version: &str) -> Vec<Source> {
    let mm = game_version
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");
    let mut v = vec![
        Source {
            name: "Community rules",
            url: "https://raw.githubusercontent.com/RimSort/Community-Rules-Database/main/communityRules.json".into(),
            dest: "Community-Rules-Database/communityRules.json".into(),
        },
        Source {
            name: "Use This Instead",
            url: "https://raw.githubusercontent.com/emipa606/UseThisInstead/main/replacements.json.gz".into(),
            dest: "UseThisInstead/replacements.json.gz".into(),
        },
    ];
    if !mm.is_empty() {
        v.push(Source {
            name: "No version warning",
            url: format!("https://raw.githubusercontent.com/emipa606/NoVersionWarning/main/{mm}/ModIdsToFix.xml"),
            dest: format!("NoVersionWarning/{mm}/ModIdsToFix.xml"),
        });
    }
    v
}

#[derive(Debug, Clone, Serialize, Type)]
pub enum DbStatus {
    Updated,
    NotModified,
    Failed,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DbResult {
    pub name: String,
    pub status: DbStatus,
    /// Error text when `Failed`, else size info.
    pub detail: String,
}

fn etag_file(dbs: &Path) -> PathBuf {
    dbs.join("etags.json")
}

/// GET `url` into `dest` unless the server says it hasn't changed since `etags[url]`.
pub fn fetch_if_changed(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    etags: &mut HashMap<String, String>,
) -> Result<DbStatus> {
    let mut req = client.get(url);
    if dest.exists()
        && let Some(tag) = etags.get(url)
    {
        req = req.header("If-None-Match", tag);
    }
    let resp = req
        .send()
        .map_err(|e| Error::Other(format!("{url}: {e}")))?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(DbStatus::NotModified);
    }
    if !resp.status().is_success() {
        return Err(Error::Other(format!("{url}: HTTP {}", resp.status())));
    }
    let tag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let bytes = resp
        .bytes()
        .map_err(|e| Error::Other(format!("{url}: {e}")))?;
    settings::atomic_write(dest, &bytes)?;
    match tag {
        Some(t) => etags.insert(url.to_owned(), t),
        None => etags.remove(url),
    };
    Ok(DbStatus::Updated)
}

/// Update every source; one failing source doesn't stop the others.
pub fn update_all(game_version: &str) -> Vec<DbResult> {
    let dbs = settings::data_dir().join("dbs");
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("rimsort-rs/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return vec![DbResult {
                name: "HTTP client".into(),
                status: DbStatus::Failed,
                detail: e.to_string(),
            }];
        }
    };
    let mut etags: HashMap<String, String> = fs::read_to_string(etag_file(&dbs))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let results = sources(game_version)
        .into_iter()
        .map(|s| {
            let dest = dbs.join(&s.dest);
            match fetch_if_changed(&client, &s.url, &dest, &mut etags) {
                Ok(status) => {
                    let size = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                    DbResult {
                        name: s.name.into(),
                        status,
                        detail: format!("{} KB", size / 1024),
                    }
                }
                Err(e) => DbResult {
                    name: s.name.into(),
                    status: DbStatus::Failed,
                    detail: e.to_string(),
                },
            }
        })
        .collect();
    if let Ok(t) = serde_json::to_string_pretty(&etags) {
        let _ = settings::atomic_write(&etag_file(&dbs), t.as_bytes());
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    /// Tiny HTTP server: 200 + ETag first, 304 when If-None-Match matches, 404 for /missing.
    fn serve() -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                let reply = if req.starts_with("get /missing") {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_owned()
                } else if req.contains("if-none-match: \"v1\"") {
                    "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_owned()
                } else {
                    "HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello".to_owned()
                };
                let _ = s.write_all(reply.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn etag_roundtrip_and_errors() {
        let base = serve();
        let t = tempfile::tempdir().unwrap();
        let dest = t.path().join("sub/file.json");
        let client = reqwest::blocking::Client::new();
        let mut etags = HashMap::new();
        let url = format!("{base}/file");
        assert!(matches!(
            fetch_if_changed(&client, &url, &dest, &mut etags).unwrap(),
            DbStatus::Updated
        ));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "hello");
        assert!(matches!(
            fetch_if_changed(&client, &url, &dest, &mut etags).unwrap(),
            DbStatus::NotModified
        ));
        assert!(fetch_if_changed(&client, &format!("{base}/missing"), &dest, &mut etags).is_err());
        assert_eq!(sources("1.6.4871 rev590").len(), 3);
        assert_eq!(sources("").len(), 2);
    }
}
