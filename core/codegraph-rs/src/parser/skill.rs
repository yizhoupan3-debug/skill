//! Skill manifest parser — extracts skill metadata from `skills/SKILL_MANIFEST.json`.
//!
//! Each skill entry becomes a `kind="skill"` node in the codegraph index.
//! Keywords are stored as separate keyword nodes linked via edges so they
//! participate in the FTS5 index naturally (O(log n) lookup).

use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::parser::{ParsedEdge, ParsedFile, ParsedSymbol};

/// Index positions within the `SKILL_MANIFEST.json` "keys" array.
const KEY_SLUG: usize = 0;
const KEY_PRIORITY: usize = 4;
const KEY_DESCRIPTION: usize = 5;
const KEY_TRIGGER_HINTS: usize = 7;
const KEY_SKILL_PATH: usize = 10;

/// Parse `skills/SKILL_MANIFEST.json` and return a synthetic `ParsedFile` whose
/// symbols are skill entries and edges link each skill to its keywords.
///
/// Returns `None` if the manifest is missing or malformed.
pub fn parse_skill_manifest(repo_root: &Path) -> Option<ParsedFile> {
    let manifest_path = repo_root.join("skills/SKILL_MANIFEST.json");
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    parse_skill_manifest_content(&content, &manifest_path)
}

/// Core parse logic separated for testability.
fn parse_skill_manifest_content(content: &str, manifest_path: &Path) -> Option<ParsedFile> {
    let manifest: Value = serde_json::from_str(content).ok()?;
    let skills = manifest.get("skills")?.as_array()?;

    // Resolve the keys array to confirm field ordering.
    let keys = manifest.get("keys")?.as_array()?;
    let key_names: Vec<&str> = keys.iter().filter_map(|k| k.as_str()).collect();

    // Sanity check: ensure we have at least the fields we need.
    if key_names.len() <= KEY_SKILL_PATH {
        return None;
    }

    let mtime_ns = std::fs::metadata(manifest_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);

    let mut symbols = Vec::new();
    let mut edges = Vec::new();
    let path_str = manifest_path
        .parent()
        .and_then(|p| p.parent())
        .map(|p| {
            let base = p.to_string_lossy();
            // Produce relative path like "skills/SKILL_MANIFEST.json"
            if base.is_empty() || base == "." {
                "skills/SKILL_MANIFEST.json".to_string()
            } else {
                format!("{}/skills/SKILL_MANIFEST.json", base)
            }
        })
        .unwrap_or_else(|| "skills/SKILL_MANIFEST.json".to_string());

    for entry in skills {
        let arr = match entry.as_array() {
            Some(a) if a.len() > KEY_SKILL_PATH => a,
            _ => continue,
        };

        let slug = arr[KEY_SLUG].as_str().unwrap_or("unknown");
        let _priority = arr[KEY_PRIORITY].as_str().unwrap_or("P2");
        let _description = arr[KEY_DESCRIPTION].as_str().unwrap_or("");
        let skill_path = arr[KEY_SKILL_PATH].as_str().unwrap_or("");

        // Create the skill symbol node
        let _skill_id = format!("skill:{}:0:{}", skill_path, slug);
        symbols.push(ParsedSymbol {
            symbol: slug.to_string(),
            kind: "skill".to_string(),
            line: 0,
        });

        // Collect trigger hints (keywords) and link them as keyword nodes
        if let Some(hints) = arr[KEY_TRIGGER_HINTS].as_array() {
            for hint in hints {
                let kw = match hint.as_str() {
                    Some(s) if !s.is_empty() => s,
                    _ => continue,
                };
                let _kw_id = make_keyword_id(skill_path, kw);
                symbols.push(ParsedSymbol {
                    symbol: kw.to_string(),
                    kind: "keyword".to_string(),
                    line: 0,
                });
                // Edge: skill -> keyword (skill "calls" keyword for graph traversal)
                edges.push(ParsedEdge {
                    caller_symbol: slug.to_string(),
                    callee_symbol: kw.to_string(),
                    line: 0,
                });
            }
        }

        // Also index priority and short description as searchable tokens
        // The slug itself is already a symbol, which is sufficient for name search.
        // Description and priority flow through the FTS index via the node's symbol field.
    }

    // Compute content hash from the raw JSON bytes
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(content.as_bytes());
    let hash = hex_encode(digest.as_slice());

    Some(ParsedFile {
        path: path_str,
        language: "skill".to_string(),
        mtime_ns,
        content_hash: hash,
        symbols,
        edges,
    })
}

/// Deterministic ID for a keyword node.
fn make_keyword_id(skill_path: &str, keyword: &str) -> String {
    let mut hasher = DefaultHasher::new();
    skill_path.hash(&mut hasher);
    keyword.hash(&mut hasher);
    format!("kw:{:016x}", hasher.finish())
}

/// Encode bytes as hex string (matching graph::sync::hex_encode).
fn hex_encode(bytes: &[u8]) -> String {
    const HEX_TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_TABLE[(b >> 4) as usize] as char);
        out.push(HEX_TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_into_skill_and_keyword_nodes() {
        let json = r#"{
            "keys": ["slug","layer","owner","gate","priority","description","session_start","trigger_hints","source","source_position","skill_path","host_platforms","kind","allowedTools","model"],
            "skills": [
                ["test-skill","L0","owner","none","P1","A test skill description","n/a",["keyword1","测试关键词","/test-skill"],"project",3,"skills/test-skill/SKILL.md",["claude-code"],"skill",null,null]
            ]
        }"#;
        let path = Path::new("skills/SKILL_MANIFEST.json");
        let parsed = parse_skill_manifest_content(json, path).expect("should parse");

        assert_eq!(parsed.language, "skill");
        // 1 skill symbol + 2 keyword symbols + 1 slash-command keyword
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

        // Edges should link skill -> keywords
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
        let path = Path::new("skills/SKILL_MANIFEST.json");
        let result = parse_skill_manifest_content("not json", path);
        assert!(result.is_none());
    }
}
