use anyhow::{Context, Result};
use chrono::{DateTime, Timelike, Utc};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::constants::TEMPLATES_RELATIVE;

// ── time ──────────────────────────────────────────────────────────────

pub(crate) fn now_iso() -> String {
    Utc::now().with_nanosecond_zero().to_rfc3339()
}

trait NanosecondZero {
    fn with_nanosecond_zero(self) -> Self;
}

impl NanosecondZero for DateTime<Utc> {
    fn with_nanosecond_zero(self) -> Self {
        self.with_nanosecond(0).unwrap_or(self)
    }
}

pub(crate) fn parse_iso_timestamp(value: &str) -> Option<DateTime<Utc>> {
    if value.trim().is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(&value.replace('Z', "+00:00"))
        .ok()
        .map(|ts| ts.with_timezone(&Utc))
}

pub(crate) fn days_since(value: &str) -> Option<i64> {
    parse_iso_timestamp(value).map(|ts| (Utc::now() - ts).num_days().max(0))
}

// ── repo / templates ──────────────────────────────────────────────────

pub(crate) fn repo_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(root)
            .parent()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."))
            .to_path_buf());
    }
    let current = std::env::current_dir()?;
    for candidate in current.ancestors() {
        if candidate.join("AGENTS.md").exists() && candidate.join("skills").exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(current)
}

pub(crate) fn templates_dir() -> Result<PathBuf> {
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(manifest).join("templates"));
    }
    Ok(repo_root()?.join(TEMPLATES_RELATIVE))
}

pub(crate) fn template_path(name: &str) -> Result<PathBuf> {
    Ok(templates_dir()?.join(name))
}

pub(crate) fn load_template(name: &str) -> Result<String> {
    let path = template_path(name)?;
    fs::read_to_string(&path).with_context(|| format!("Missing template: {}", path.display()))
}

pub(crate) fn replace_placeholders(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in pairs {
        rendered = rendered.replace(&format!("{{{key}}}"), value);
    }
    rendered
}

pub(crate) fn slugify(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    let cleaned = Regex::new(r"[^a-z0-9]+")
        .unwrap()
        .replace_all(&lowered, "-")
        .to_string();
    let collapsed = Regex::new(r"-+")
        .unwrap()
        .replace_all(&cleaned, "-")
        .trim_matches('-')
        .to_string();
    if collapsed.is_empty() {
        "hypothesis".to_string()
    } else {
        collapsed
    }
}

// ── JSON value accessors ──────────────────────────────────────────────

pub(crate) fn obj_mut(value: &mut Value) -> &mut Map<String, Value> {
    value.as_object_mut().expect("state must be an object")
}

pub(crate) fn arr<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .expect("expected array after defaults")
}

pub(crate) fn arr_mut<'a>(value: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    obj_mut(value)
        .entry(key.to_string())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .expect("expected array")
}

pub(crate) fn str_key(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string()
}

pub(crate) fn str_field(value: &Value, key: &str) -> String {
    str_field_default(value, key, "-")
}

pub(crate) fn str_field_default(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

pub(crate) fn set_key(value: &mut Value, key: &str, child: Value) {
    obj_mut(value).insert(key.to_string(), child);
}

pub(crate) fn string_vec(values: &[String]) -> Value {
    json!(values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>())
}

pub(crate) fn optional_string(value: Option<&str>) -> Value {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(Value::from)
        .unwrap_or(Value::Null)
}

pub(crate) fn value_as_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.trim().is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn novelty_gate(value: &Value) -> &Map<String, Value> {
    value
        .get("novelty_gate")
        .and_then(Value::as_object)
        .expect("novelty gate defaults must exist")
}

pub(crate) fn novelty_gate_mut(value: &mut Value) -> &mut Map<String, Value> {
    obj_mut(value)
        .entry("novelty_gate".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("novelty_gate must be object")
}

pub(crate) fn novelty_arr<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    novelty_gate(value)
        .get(key)
        .and_then(Value::as_array)
        .expect("novelty array default missing")
}

pub(crate) fn novelty_value(value: &Value, key: &str) -> Value {
    novelty_gate(value).get(key).cloned().unwrap_or(Value::Null)
}

pub(crate) fn novelty_str(value: &Value, key: &str, default: &str) -> String {
    novelty_gate(value)
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

// ── formatting helpers ────────────────────────────────────────────────

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}

pub(crate) fn join_string_array(values: &[Value]) -> String {
    let joined = values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if joined.is_empty() {
        "_none_".into()
    } else {
        joined
    }
}

pub(crate) fn format_string_list(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        return empty.to_string();
    }
    values
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn escape_table_cell(value: &str) -> String {
    value.replace('|', "/")
}

pub(crate) fn format_overlap_risk(overlap: &str) -> String {
    match overlap {
        "low" => "\u{1f7e2} low".into(),
        "medium" => "\u{1f7e1} medium".into(),
        "high" => "\u{1f534} high".into(),
        _ => overlap.into(),
    }
}

pub(crate) fn markdown_link(value: Option<&str>) -> String {
    value
        .filter(|item| !item.trim().is_empty())
        .map(|item| format!("[link]({})", item.trim()))
        .unwrap_or_else(|| "-".into())
}

pub(crate) fn normalize_limit(limit: usize) -> usize {
    limit.clamp(1, 20)
}

pub(crate) fn xml_text_between(raw: &str, tag: &str) -> Option<String> {
    let pattern = Regex::new(&format!(r"(?s)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>")).ok()?;
    let captures = pattern.captures(raw)?;
    Some(decode_xml_entities(captures.get(1)?.as_str().trim()))
}

pub(crate) fn decode_xml_entities(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ── system helpers ────────────────────────────────────────────────────

pub(crate) fn command_output(args: &[&str], cwd: &Path) -> Option<String> {
    let (program, rest) = args.split_first()?;
    let output = Command::new(program)
        .args(rest)
        .current_dir(cwd)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

// ── NLP helpers ───────────────────────────────────────────────────────

pub(crate) fn stopwords() -> &'static [&'static str] {
    &[
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "in", "into", "is",
        "it", "of", "on", "or", "reduce", "research", "that", "the", "this", "to", "use", "using",
        "with",
    ]
}

pub(crate) fn compact_words(text: &str, limit: usize) -> Vec<String> {
    let re = Regex::new(r"[A-Za-z0-9][A-Za-z0-9_-]*").unwrap();
    let stops: std::collections::HashSet<&str> = stopwords().iter().copied().collect();
    let mut filtered = Vec::new();
    for cap in re.find_iter(&text.to_lowercase()) {
        let word = cap.as_str();
        if word.len() <= 2 || stops.contains(word) {
            continue;
        }
        if !filtered.iter().any(|item| item == word) {
            filtered.push(word.to_string());
        }
        if filtered.len() >= limit {
            break;
        }
    }
    filtered
}

pub(crate) fn merge_string_array(existing: &Value, additions: &[String]) -> Value {
    let mut merged = existing
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.trim().is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for item in additions
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
    {
        if !merged.iter().any(|existing| existing == item) {
            merged.push(item.to_string());
        }
    }
    json!(merged)
}
