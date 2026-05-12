//! Shared `serde_json::Value` field extractors and small string utilities for `framework_runtime`.

use serde_json::Value;
use std::collections::HashSet;

pub(super) fn join_lines(values: &[String]) -> String {
    values
        .iter()
        .filter(|item| !item.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" / ")
}

pub(super) fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() || matches!(ch, '_' | '.' | '-') {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_string()
}

pub(super) fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

pub(super) fn nonempty_string(value: Option<&Value>) -> Option<String> {
    let text = value_text(value);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub(super) fn value_bool_or_none(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn value_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn first_nonempty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

pub(super) fn stable_line_items(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let value = item.trim().to_string();
        if value.is_empty() || !seen.insert(value.clone()) {
            continue;
        }
        result.push(value);
    }
    result
}
