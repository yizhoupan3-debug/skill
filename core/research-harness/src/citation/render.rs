//! BibTeX 渲染 — 将 JSON 条目转为 BibTeX 字符串。
//!
//! 从 citation_tool_rs 的渲染逻辑适配而来。

use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

static BIBTEX_FIELD_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(?m)^\s*(\w+)\s*=").expect("invalid BIBTEX_FIELD_NAME_RE regex")
});

/// Escape BibTeX special characters for proper LaTeX rendering.
///
/// In BibTeX field values wrapped in braces, these characters need escaping:
/// & % # _ ~ ^
fn escape_bibtex_value(val: &str) -> String {
    val.replace('&', "\\&")
        .replace('%', "\\%")
        .replace('#', "\\#")
        .replace('_', "\\_")
        .replace('~', "\\~")
        .replace('^', "\\^")
}

/// 将 JSON 格式的引用条目渲染为 BibTeX 字符串。
///
/// Expected JSON shape:
/// ```json
/// {
///   "entry_type": "article",
///   "key": "smith2024",
///   "fields": { "author": "...", "title": "...", ... }
/// }
/// ```
pub fn render_entry(entry: &serde_json::Value) -> Result<String> {
    let entry_type = entry
        .get("entry_type")
        .and_then(|v| v.as_str())
        .unwrap_or("misc");
    let key = entry
        .get("key")
        .and_then(|v| v.as_str())
        .context("missing 'key' field")?;
    let fields = entry
        .get("fields")
        .and_then(|v| v.as_object())
        .context("missing 'fields' object")?;

    let mut bibtex = format!("@{entry_type}{{{key},\n");
    let mut first = true;
    for (name, value) in fields {
        let val = value.as_str().unwrap_or("");
        if first {
            first = false;
        } else {
            bibtex.push_str(",\n");
        }
        bibtex.push_str(&format!("  {name} = {{{}}}", escape_bibtex_value(val)));
    }
    bibtex.push_str("\n}\n");
    Ok(bibtex)
}

/// 将 BibTeX 字符串解析为 JSON 条目列表。
///
/// 使用大括号深度计数正确处理嵌套大括号（如 `title = {A {Deep} Learning}`）。
pub fn parse_bibtex_to_json(text: &str) -> Vec<serde_json::Value> {
    let mut entries = Vec::new();
    // Find @type{key, ...} entries using brace-depth counting
    let mut pos = 0;
    while let Some(at_pos) = text[pos..].find('@') {
        let abs_at = pos + at_pos;
        // Match @type{ or @type(
        let after_at = &text[abs_at + 1..];
        let type_end = after_at
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after_at.len());
        if type_end == 0 {
            pos = abs_at + 1;
            continue;
        }
        let entry_type = after_at[..type_end].to_lowercase();
        let after_type = after_at[type_end..].trim_start();
        if !after_type.starts_with('{') && !after_type.starts_with('(') {
            pos = abs_at + 1;
            continue;
        }
        let open_char = if after_type.starts_with('{') {
            '{'
        } else {
            '('
        };
        let close_char = if open_char == '{' { '}' } else { ')' };
        let body_start = abs_at + 1 + (after_at.len() - after_type.len()) + 1;

        // Find key (first comma)
        let body_rest = &text[body_start..];
        let comma_pos = body_rest.find(',').unwrap_or(body_rest.len());
        let key = body_rest[..comma_pos].trim().to_string();

        // Find matching closing brace using depth counting
        let mut depth = 1;
        let mut body_end = body_start + comma_pos;
        for (i, ch) in text[body_start..].char_indices() {
            if ch == open_char {
                depth += 1;
            } else if ch == close_char {
                depth -= 1;
                if depth == 0 {
                    body_end = body_start + i;
                    break;
                }
            }
        }
        if depth != 0 {
            pos = body_start;
            continue;
        }
        let body = &text[body_start + comma_pos + 1..body_end];

        let mut fields = serde_json::Map::new();
        // Parse fields using brace-depth counting to handle nested braces
        for fcap in BIBTEX_FIELD_NAME_RE.captures_iter(body) {
            let field_name = fcap[1].trim().to_lowercase();
            let value_start = fcap.get(0).map(|m| m.end()).unwrap_or(0);
            // value_start already points past "=" in the regex match;
            // find the matching closing brace after optional whitespace
            if value_start >= body.len() {
                continue;
            }
            let after_eq = body[value_start..].trim_start();
            if !after_eq.starts_with('{') {
                continue;
            }
            let brace_start = body.len() - after_eq.len();
            let mut val_depth = 0;
            let val_start = brace_start + 1;
            let mut val_end = brace_start + 1;
            for (i, ch) in body[brace_start..].char_indices() {
                match ch {
                    '{' => val_depth += 1,
                    '}' => {
                        val_depth -= 1;
                        if val_depth == 0 {
                            val_end = brace_start + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if val_depth == 0 && val_end > val_start {
                fields.insert(
                    field_name,
                    serde_json::Value::String(body[val_start..val_end].trim().to_string()),
                );
            }
        }

        entries.push(serde_json::json!({
            "entry_type": entry_type,
            "key": key,
            "fields": fields,
        }));
        pos = body_end + 1;
    }
    entries
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn render_round_trip() {
        let entry = serde_json::json!({
            "entry_type": "article",
            "key": "smith2024",
            "fields": {
                "author": "Smith, Jane",
                "title": "A Great Paper",
                "journal": "JMLR",
                "year": "2024"
            }
        });

        let bibtex = render_entry(&entry).unwrap();
        assert!(bibtex.starts_with("@article{smith2024,"));
        assert!(bibtex.contains("author = {Smith, Jane}"));
        assert!(bibtex.contains("title = {A Great Paper}"));
    }

    #[test]
    fn render_minimal_entry() {
        let entry = serde_json::json!({
            "entry_type": "misc",
            "key": "note1",
            "fields": {}
        });
        let bibtex = render_entry(&entry).unwrap();
        assert!(bibtex.contains("@misc{note1,"));
    }

    #[test]
    fn parse_bibtex_basic() {
        let bibtex = r#"@article{smith2024,
  author = {Smith, Jane},
  title = {A Great Paper},
  year = {2024}
}"#;
        let entries = parse_bibtex_to_json(bibtex);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry["entry_type"], "article");
        assert_eq!(entry["key"], "smith2024");
        let fields = entry["fields"].as_object().unwrap();
        assert_eq!(
            fields.get("author").and_then(|v| v.as_str()),
            Some("Smith, Jane")
        );
        assert_eq!(
            fields.get("title").and_then(|v| v.as_str()),
            Some("A Great Paper")
        );
        assert_eq!(fields.get("year").and_then(|v| v.as_str()), Some("2024"));
    }

    #[test]
    fn parse_bibtex_non_ascii_author() {
        let bibtex = r#"@article{author2024,
  author = {Żołnierz, Jan and Nguyễn, ĩ}
}"#;
        let entries = parse_bibtex_to_json(bibtex);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        let fields = entry["fields"].as_object().unwrap();
        let author = fields.get("author").and_then(|v| v.as_str()).unwrap();
        assert!(
            author.contains("Żołnierz"),
            "Polish chars should survive ascii truncation bug: {author}"
        );
        assert!(
            author.contains("Nguyễn"),
            "Vietnamese chars should survive: {author}"
        );
    }

    #[test]
    fn parse_bibtex_nested_braces() {
        let bibtex = r#"@article{deep2024,
  title = {A {Deep} Learning Approach}
}"#;
        let entries = parse_bibtex_to_json(bibtex);
        assert_eq!(entries.len(), 1);
        let title = entries[0]["fields"]["title"].as_str().unwrap();
        assert_eq!(title, "A {Deep} Learning Approach");
    }

    #[test]
    fn parse_bibtex_multiple_entries() {
        let bibtex = r#"@article{a2024,
  author = {Alice}
}
@inproceedings{b2024,
  title = {Bob's Work}
}"#;
        let entries = parse_bibtex_to_json(bibtex);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_bibtex_empty_fields() {
        let bibtex = r#"@misc{empty2024,}"#;
        let entries = parse_bibtex_to_json(bibtex);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn parse_bibtex_doi_with_url_prefix() {
        let bibtex = r#"@article{doi2024,
  doi = {10.1234/example}
}"#;
        let entries = parse_bibtex_to_json(bibtex);
        assert_eq!(entries.len(), 1);
        let doi = entries[0]["fields"]["doi"].as_str().unwrap();
        assert_eq!(doi, "10.1234/example");
    }
}
