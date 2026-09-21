//! Search inside mod folders (RimSort's "file search"): parallel, line-based, capped.

use crate::{Error, ModId, Result, mods::ModIndex};
use rayon::prelude::*;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

const MAX_HITS: usize = 1000;
const MAX_HITS_PER_MOD: usize = 60;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 12;
const MAX_LINE_CHARS: usize = 240;

#[derive(Debug, Clone, Deserialize, Type)]
pub struct SearchQuery {
    pub text: String,
    pub regex: bool,
    pub case_sensitive: bool,
    /// Extensions to look in, without dots; empty means every (text) file.
    pub extensions: Vec<String>,
    /// Also match file names, not just contents.
    pub file_names: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SearchHit {
    pub mod_id: ModId,
    pub mod_name: String,
    /// Absolute path of the file.
    pub path: String,
    /// Path relative to the mod folder.
    pub rel: String,
    /// 1-based line; 0 for a file-name match.
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    /// Stopped early at the overall or per-mod hit cap.
    pub truncated: bool,
    pub files_searched: u32,
    pub ms: u32,
}

pub fn search(index: &ModIndex, q: &SearchQuery) -> Result<SearchResult> {
    if q.text.is_empty() {
        return Err(Error::Other("Enter something to search for".into()));
    }
    let pattern = if q.regex {
        q.text.clone()
    } else {
        regex::escape(&q.text)
    };
    let re = RegexBuilder::new(&pattern)
        .case_insensitive(!q.case_sensitive)
        .size_limit(4 << 20)
        .build()
        .map_err(|e| Error::Other(format!("Invalid pattern: {e}")))?;
    let exts: Vec<String> = q
        .extensions
        .iter()
        .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|e| !e.is_empty())
        .collect();

    let started = Instant::now();
    let total = AtomicUsize::new(0);
    let files = AtomicUsize::new(0);
    let truncated = AtomicBool::new(false);

    let mut per_mod: Vec<Vec<SearchHit>> = index
        .mods
        .par_iter()
        .map(|m| {
            let mut hits = Vec::new();
            let mut stack = vec![(m.path.clone(), 0usize)];
            while let Some((dir, depth)) = stack.pop() {
                let Ok(rd) = fs::read_dir(&dir) else { continue };
                let mut entries: Vec<_> = rd.flatten().collect();
                entries.sort_by_key(|e| e.file_name());
                for e in entries {
                    if total.load(Ordering::Relaxed) >= MAX_HITS {
                        truncated.store(true, Ordering::Relaxed);
                        return hits;
                    }
                    let Ok(ft) = e.file_type() else { continue };
                    let path = e.path();
                    if ft.is_dir() {
                        if depth < MAX_DEPTH && e.file_name() != ".git" {
                            stack.push((path, depth + 1));
                        }
                        continue;
                    }
                    if !ft.is_file() || !ext_ok(&path, &exts) {
                        continue;
                    }
                    let rel = path
                        .strip_prefix(&m.path)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let mut push = |line: u32, text: &str| {
                        if hits.len() >= MAX_HITS_PER_MOD {
                            truncated.store(true, Ordering::Relaxed);
                            return;
                        }
                        total.fetch_add(1, Ordering::Relaxed);
                        hits.push(SearchHit {
                            mod_id: m.id,
                            mod_name: m.name.clone(),
                            path: path.to_string_lossy().into_owned(),
                            rel: rel.clone(),
                            line,
                            text: clip(text),
                        });
                    };
                    if q.file_names && re.is_match(&e.file_name().to_string_lossy()) {
                        push(0, &rel);
                    }
                    let Some(content) = read_text(&path) else {
                        continue;
                    };
                    files.fetch_add(1, Ordering::Relaxed);
                    for (i, line) in content.lines().enumerate() {
                        if re.is_match(line) {
                            push(i as u32 + 1, line.trim());
                        }
                    }
                }
            }
            hits
        })
        .collect();
    per_mod.retain(|h| !h.is_empty());
    per_mod.sort_by_cached_key(|h| h[0].mod_name.to_lowercase());
    let mut hits: Vec<SearchHit> = per_mod.into_iter().flatten().collect();
    if hits.len() > MAX_HITS {
        hits.truncate(MAX_HITS);
        truncated.store(true, Ordering::Relaxed);
    }
    Ok(SearchResult {
        hits,
        truncated: truncated.load(Ordering::Relaxed),
        files_searched: files.load(Ordering::Relaxed) as u32,
        ms: started.elapsed().as_millis() as u32,
    })
}

fn ext_ok(path: &Path, exts: &[String]) -> bool {
    if exts.is_empty() {
        return true;
    }
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|e| exts.contains(&e))
}

/// File contents as text, or None for oversized/binary files.
fn read_text(path: &Path) -> Option<String> {
    if fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if bytes[..bytes.len().min(4096)].contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn clip(s: &str) -> String {
    match s.char_indices().nth(MAX_LINE_CHARS) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::{Mod, ModType};

    fn index_with(dir: &Path) -> ModIndex {
        let m = Mod {
            id: ModId::from_path(dir),
            path: dir.into(),
            folder: "m".into(),
            mod_type: ModType::Local,
            valid: true,
            invalid_reason: None,
            package_id: "a.b".into(),
            name: "Mod".into(),
            authors: vec![],
            description: String::new(),
            url: String::new(),
            mod_version: String::new(),
            supported_versions: vec![],
            published_file_id: None,
            steam_app_id: None,
            rules: Default::default(),
            community: Default::default(),
            user: Default::default(),
            mtime: 0,
            added: 0,
            csharp: false,
        };
        ModIndex::new(vec![m], String::new(), 0)
    }

    fn q(text: &str) -> SearchQuery {
        SearchQuery {
            text: text.into(),
            regex: false,
            case_sensitive: false,
            extensions: vec![],
            file_names: false,
        }
    }

    #[test]
    fn finds_lines_filters_extensions_and_skips_binaries() {
        let dir = std::env::temp_dir().join(format!("rs-search-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Defs")).unwrap();
        fs::write(
            dir.join("Defs/a.xml"),
            "<x>\n  <defName>Needle</defName>\n</x>\n",
        )
        .unwrap();
        fs::write(dir.join("notes.txt"), "a needle here").unwrap();
        fs::write(dir.join("bin.dat"), b"needle\0\0").unwrap();
        let idx = index_with(&dir);

        let r = search(&idx, &q("needle")).unwrap();
        let mut rels: Vec<_> = r.hits.iter().map(|h| (h.rel.as_str(), h.line)).collect();
        rels.sort();
        assert_eq!(rels, [("Defs/a.xml", 2), ("notes.txt", 1)]);

        let mut only_xml = q("needle");
        only_xml.extensions = vec![".XML".into()];
        assert_eq!(search(&idx, &only_xml).unwrap().hits.len(), 1);

        let mut cased = q("needle");
        cased.case_sensitive = true;
        assert_eq!(search(&idx, &cased).unwrap().hits.len(), 1);

        let mut rx = q(r"<defName>\w+</defName>");
        rx.regex = true;
        assert_eq!(search(&idx, &rx).unwrap().hits.len(), 1);
        rx.text = "(".into();
        assert!(search(&idx, &rx).is_err());

        let mut names = q("a.xml");
        names.file_names = true;
        assert_eq!(search(&idx, &names).unwrap().hits[0].line, 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
