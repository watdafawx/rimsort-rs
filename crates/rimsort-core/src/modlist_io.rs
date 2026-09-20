//! Import/export of mod lists in the formats RimSort and RimWorld use.

use crate::{Error, Result, xml};
use serde_json::{Value, json};

#[derive(Debug, Default, PartialEq)]
pub struct ParsedList {
    pub package_ids: Vec<String>,
    pub game_version: Option<String>,
    pub known_expansions: Vec<String>,
}

fn lower_all(v: impl IntoIterator<Item = String>) -> Vec<String> {
    v.into_iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `Name [pkg.id][https://url]` (RimSort clipboard report) -> `pkg.id`.
fn report_line_id(line: &str) -> Option<&str> {
    let body = line.trim_end().strip_suffix(']')?;
    let (head, _url) = body.rsplit_once("][")?;
    let (_, id) = head.rsplit_once('[')?;
    (!id.is_empty()).then_some(id)
}

fn parse_text(text: &str) -> ParsedList {
    let package_ids = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            report_line_id(l)
                .or_else(|| (!l.contains(char::is_whitespace) && l.contains('.')).then_some(l))
        })
        .map(str::to_lowercase)
        .collect();
    ParsedList {
        package_ids,
        ..Default::default()
    }
}

/// Detect the format (JSON, ModsConfig/.rml/.rws XML, plain text) and parse.
pub fn parse_list(text: &str) -> Result<ParsedList> {
    let text = text.trim_start_matches('\u{feff}');
    let head = text.trim_start();

    if head.starts_with('{') {
        let v: Value = serde_json::from_str(text)?;
        let ids = v
            .get("activeMods")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::Other("JSON mod list needs 'version' and 'activeMods'".into()))?;
        let strs = |a: Option<&Value>| -> Vec<String> {
            a.and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        };
        return Ok(ParsedList {
            package_ids: lower_all(ids.iter().filter_map(|x| x.as_str().map(String::from))),
            game_version: v.get("version").and_then(Value::as_str).map(String::from),
            known_expansions: lower_all(strs(v.get("knownExpansions"))),
        });
    }

    if head.starts_with('<') {
        let root = xml::parse(text);
        if let Some(cfg) = root.child("ModsConfigData") {
            return Ok(ParsedList {
                package_ids: lower_all(
                    cfg.child("activeMods")
                        .map(xml::Node::li_texts)
                        .unwrap_or_default(),
                ),
                game_version: cfg.child_text("version").map(String::from),
                known_expansions: lower_all(
                    cfg.child("knownExpansions")
                        .map(xml::Node::li_texts)
                        .unwrap_or_default(),
                ),
            });
        }
        // .rws save (`savegame`) and .rml mod list (`savedModList`): ids live under meta/modIds.
        for top in ["savegame", "savedModList"] {
            if let Some(meta) = root.child(top).and_then(|n| n.child("meta")) {
                return Ok(ParsedList {
                    package_ids: lower_all(
                        meta.child("modIds")
                            .map(xml::Node::li_texts)
                            .unwrap_or_default(),
                    ),
                    game_version: meta.child_text("gameVersion").map(String::from),
                    known_expansions: vec![],
                });
            }
        }
        return Err(Error::Other(
            "Unrecognized XML mod list (expected ModsConfigData, savegame or savedModList)".into(),
        ));
    }

    let parsed = parse_text(text);
    if parsed.package_ids.is_empty() {
        return Err(Error::Other(
            "No mods found: expected RimSort JSON, RimWorld XML, or one package id per line".into(),
        ));
    }
    Ok(parsed)
}

pub fn to_json(version: &str, ids: &[String], known: &[String]) -> String {
    serde_json::to_string_pretty(
        &json!({"version": version, "activeMods": ids, "knownExpansions": known}),
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_every_format() {
        let json = parse_list(r#"{"version":"1.6.1","activeMods":["A.B","c.d"],"knownExpansions":["ludeon.rimworld.royalty"]}"#).unwrap();
        assert_eq!(json.package_ids, ["a.b", "c.d"]);
        assert_eq!(json.game_version.as_deref(), Some("1.6.1"));

        let cfg = parse_list("<?xml version=\"1.0\"?><ModsConfigData><version>1.6</version><activeMods><li>X.Y</li></activeMods></ModsConfigData>").unwrap();
        assert_eq!(cfg.package_ids, ["x.y"]);

        let rws = parse_list("<savegame><meta><gameVersion>1.5</gameVersion><modIds><li>p.q</li><li>r.s</li></modIds><modNames><li>P</li></modNames></meta></savegame>").unwrap();
        assert_eq!(rws.package_ids, ["p.q", "r.s"]);
        let rml =
            parse_list("<savedModList><meta><modIds><li>m.n</li></modIds></meta></savedModList>")
                .unwrap();
        assert_eq!(rml.package_ids, ["m.n"]);

        let report = "Created with RimSort 1.0\nRimWorld game version this list was created for: 1.6\nTotal # of mods: 2\n\nHarmony [brrainz.harmony][https://x.y/z]\nMy Mod [Auth.Mod][No url specified]\n";
        assert_eq!(
            parse_list(report).unwrap().package_ids,
            ["brrainz.harmony", "auth.mod"]
        );
        assert_eq!(
            parse_list("a.b\n  c.d \n# comment\n").unwrap().package_ids,
            ["a.b", "c.d"]
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_list("hello world").is_err());
        assert!(parse_list("{\"nope\":1}").is_err());
        assert!(parse_list("<html/>").is_err());
    }

    #[test]
    fn json_roundtrip() {
        let out = to_json("1.6", &["a.b".into()], &[]);
        assert_eq!(parse_list(&out).unwrap().package_ids, ["a.b"]);
    }
}
