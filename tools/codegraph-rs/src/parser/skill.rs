//! Skill manifest parser — extracts skill metadata from `skills/SKILL_MANIFEST.json`.
//!
//! Each skill entry becomes a `kind="skill"` node in the codegraph index.
//! Keywords are stored as separate keyword nodes linked via edges so they
//! participate in the FTS5 index naturally (O(log n) lookup).

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::parser::{ParsedEdge, ParsedFile, ParsedSymbol};

/// Index positions within the `SKILL_MANIFEST.json` "keys" array.
const KEY_SLUG: usize = 0;
const KEY_TRIGGER_HINTS: usize = 7;
const KEY_SKILL_PATH: usize = 10;

/// Relative path used in the codegraph index for the manifest.
pub const MANIFEST_REL_PATH: &str = "skills/SKILL_MANIFEST.json";

/// Parse `skills/SKILL_MANIFEST.json` and return a synthetic `ParsedFile` whose
/// symbols are skill entries and edges link each skill to its keywords.
///
/// Returns `None` if the manifest is missing or malformed.
pub fn parse_skill_manifest(repo_root: &Path) -> Option<ParsedFile> {
    let manifest_path = repo_root.join(MANIFEST_REL_PATH);
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let mtime_ns = std::fs::metadata(&manifest_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    let digest = Sha256::digest(content.as_bytes());
    let hash = super::common::hex_encode(digest.as_slice());
    parse_manifest_content(&content, mtime_ns, hash)
}

/// Alternative entry for callers that already have the content (avoids double-read).
pub fn parse_skill_manifest_with_content(
    content: &str,
    mtime_ns: i64,
    content_hash: String,
) -> Option<ParsedFile> {
    parse_manifest_content(content, mtime_ns, content_hash)
}

/// Core parse logic (extracted for testability and dual-entry reuse).
fn parse_manifest_content(content: &str, mtime_ns: i64, content_hash: String) -> Option<ParsedFile> {
    let manifest: Value = serde_json::from_str(content).ok()?;
    let skills = manifest.get("skills")?.as_array()?;

    let keys = manifest.get("keys")?.as_array()?;
    let key_names: Vec<&str> = keys.iter().filter_map(|k| k.as_str()).collect();
    if key_names.len() <= KEY_SKILL_PATH {
        return None;
    }

    let mut symbols = Vec::new();
    let mut edges = Vec::new();

    for entry in skills {
        let arr = match entry.as_array() {
            Some(a) if a.len() > KEY_SKILL_PATH => a,
            _ => continue,
        };

        let slug = arr[KEY_SLUG].as_str().unwrap_or("unknown");

        symbols.push(ParsedSymbol {
            symbol: slug.to_string(),
            kind: "skill".to_string(),
            line: 0,
        });

        if let Some(hints) = arr[KEY_TRIGGER_HINTS].as_array() {
            for hint in hints {
                let kw = match hint.as_str() {
                    Some(s) if !s.is_empty() => s,
                    _ => continue,
                };
                if kw == slug {
                    continue;
                }
                symbols.push(ParsedSymbol {
                    symbol: kw.to_string(),
                    kind: "keyword".to_string(),
                    line: 0,
                });
                edges.push(ParsedEdge {
                    caller_symbol: slug.to_string(),
                    callee_symbol: kw.to_string(),
                    line: 0,
                });
            }
        }
    }

    Some(ParsedFile {
        path: MANIFEST_REL_PATH.to_string(),
        language: "skill".to_string(),
        mtime_ns,
        content_hash,
        symbols,
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_into_skill_and_keyword_nodes() {
        let json = r#"{
            "keys": ["slug","layer","owner","gate","priority","description","session_start","trigger_hints","source","source_position","skill_path","host_platforms","kind","allowedTools","model"],
            "skills": [
                ["test-skill","L0","owner","none","P1","A test skill description","n/a",["keyword1","测试关键词","/test-skill"],"project",3,"skills/test-skill/SKILL.md",["claude"],"skill",null,null]
            ]
        }"#;
        let parsed = parse_manifest_content(json, 1, "hash".to_string()).expect("should parse");

        assert_eq!(parsed.language, "skill");
        assert!(!parsed.path.starts_with('/'), "path must be relative");
        assert_eq!(parsed.path, "skills/SKILL_MANIFEST.json");

        let skill_symbols: Vec<_> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "skill")
            .collect();
        assert_eq!(skill_symbols.len(), 1);
        assert_eq!(skill_symbols[0].symbol, "test-skill");

        let kw_symbols: Vec<_> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "keyword")
            .collect();
        assert!(kw_symbols.len() >= 2, "expected at least 2 keyword nodes");

        assert!(parsed.edges.len() >= 2);
        assert!(parsed.edges.iter().all(|e| e.caller_symbol == "test-skill"));
    }

    #[test]
    fn returns_none_for_missing_manifest() {
        let result = parse_skill_manifest(Path::new("/nonexistent/repo"));
        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_malformed_json() {
        let result = parse_manifest_content("not json", 1, "hash".to_string());
        assert!(result.is_none());
    }
}
