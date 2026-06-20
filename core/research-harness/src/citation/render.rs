//! BibTeX 渲染 — 将 JSON 条目转为 BibTeX 字符串。
//!
//! 从 citation_tool_rs 的渲染逻辑适配而来。

use anyhow::{Context, Result};

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
        bibtex.push_str(&format!("  {name} = {{{val}}}"));
    }
    bibtex.push_str("\n}\n");
    Ok(bibtex)
}

/// 将 BibTeX 字符串解析为 JSON 条目列表。
///
/// 简化解析器：提取 @type{key, fields...} 结构。
pub fn parse_bibtex_to_json(text: &str) -> Vec<serde_json::Value> {
    let re = regex::Regex::new(r"(?is)@(\w+)\s*[\({]\s*([^,\s]+)\s*,\s*(.+?)\s*[\)}]\s*$")
        .expect("static regex");
    let field_re = regex::Regex::new(r"(?m)^\s*(\w+)\s*=\s*\{([^}]*)\}").expect("static regex");

    let mut entries = Vec::new();
    for cap in re.captures_iter(text) {
        let entry_type = cap[1].to_lowercase();
        let key = cap[2].to_string();
        let body = &cap[3];

        let mut fields = serde_json::Map::new();
        for fcap in field_re.captures_iter(body) {
            fields.insert(
                fcap[1].to_lowercase(),
                serde_json::Value::String(fcap[2].trim().to_string()),
            );
        }

        entries.push(serde_json::json!({
            "entry_type": entry_type,
            "key": key,
            "fields": fields,
        }));
    }
    entries
}

#[cfg(test)]
mod tests {
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
}
