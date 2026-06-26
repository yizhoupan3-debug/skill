//! Shared `serde_json::Value` field extractors and small string utilities.
//!
//! Moved from `framework-runtime` to `framework-kernel` (B0) so all layers
//! can depend on these without introducing circular crate dependencies.

use core_errors::FrameworkError;
use serde_json::Value;
use std::collections::HashSet;

pub fn join_lines(values: &[String]) -> String {
    values
        .iter()
        .filter(|item| !item.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" / ")
}

pub fn safe_slug(value: &str) -> String {
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

pub fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.len() == text.len() {
                text.clone()
            } else {
                trimmed.to_string()
            }
        }
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

pub fn nonempty_string(value: Option<&Value>) -> Option<String> {
    let text = value_text(value);
    if text.is_empty() { None } else { Some(text) }
}

pub fn value_bool_or_none(value: Option<&Value>) -> Option<bool> {
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

pub fn value_string_list(value: Option<&Value>) -> Vec<String> {
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

pub fn first_nonempty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

pub fn stable_line_items(items: Vec<String>) -> Vec<String> {
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

// ── Payload key-based extractors ──

pub fn required_non_empty_string(
    payload: &Value,
    key: &str,
    context: &str,
) -> Result<String, FrameworkError> {
    let text = payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| FrameworkError::validation(format!("{context} requires non-empty {key}")))?;
    Ok(text.to_string())
}

pub fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
    let text = payload
        .get(key)
        .and_then(Value::as_str)?
        .trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

pub fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

/// Build a task ID from a label and optional creation timestamp.
/// Format: `{slug}-{timestamp_without_colons}` where slug comes from [`safe_slug`].
pub fn build_task_id(label: &str, created_at: Option<&str>) -> String {
    let stamp = created_at
        .unwrap_or(&crate::time::current_local_timestamp())
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    let slug = safe_slug(label);
    let suffix = if stamp.len() >= 14 {
        &stamp[stamp.len() - 14..]
    } else {
        &stamp
    };
    if slug.is_empty() {
        format!("task-{suffix}")
    } else {
        format!("{slug}-{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── join_lines ──

    #[test]
    fn join_lines_basic() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(join_lines(&items), "a / b / c");
    }

    #[test]
    fn join_lines_filters_empty() {
        let items = vec!["a".into(), "  ".into(), "b".into()];
        assert_eq!(join_lines(&items), "a / b");
    }

    #[test]
    fn join_lines_empty_vec() {
        let items: Vec<String> = vec![];
        assert_eq!(join_lines(&items), "");
    }

    #[test]
    fn join_lines_all_empty() {
        let items = vec!["".into(), "  ".into()];
        assert_eq!(join_lines(&items), "");
    }

    // ── safe_slug ──

    #[test]
    fn safe_slug_alphanumeric() {
        assert_eq!(safe_slug("hello123"), "hello123");
    }

    #[test]
    fn safe_slug_replaces_spaces() {
        assert_eq!(safe_slug("hello world"), "hello-world");
    }

    #[test]
    fn safe_slug_replaces_special_chars() {
        assert_eq!(safe_slug("foo@bar!baz"), "foo-bar-baz");
    }

    #[test]
    fn safe_slug_preserves_allowed_chars() {
        assert_eq!(safe_slug("a_b.c-d"), "a_b.c-d");
    }

    #[test]
    fn safe_slug_trims_leading_trailing() {
        assert_eq!(safe_slug("--hello--"), "hello");
    }

    #[test]
    fn safe_slug_empty() {
        assert_eq!(safe_slug(""), "");
    }

    #[test]
    fn safe_slug_no_consecutive_dashes() {
        assert_eq!(safe_slug("a  b"), "a-b");
    }

    // ── value_text ──

    #[test]
    fn value_text_string() {
        assert_eq!(value_text(Some(&json!("hello"))), "hello");
    }

    #[test]
    fn value_text_string_trimmed() {
        assert_eq!(value_text(Some(&json!("  hello  "))), "hello");
    }

    #[test]
    fn value_text_number() {
        assert_eq!(value_text(Some(&json!(42))), "42");
    }

    #[test]
    fn value_text_bool() {
        assert_eq!(value_text(Some(&json!(true))), "true");
    }

    #[test]
    fn value_text_null() {
        assert_eq!(value_text(Some(&json!(null))), "");
    }

    #[test]
    fn value_text_none() {
        assert_eq!(value_text(None), "");
    }

    #[test]
    fn value_text_object() {
        let val = json!({"key": "value"});
        let text = value_text(Some(&val));
        assert!(text.contains("key"));
    }

    // ── nonempty_string ──

    #[test]
    fn nonempty_string_some_nonempty() {
        assert_eq!(
            nonempty_string(Some(&json!("hello"))),
            Some("hello".into())
        );
    }

    #[test]
    fn nonempty_string_some_empty() {
        assert_eq!(nonempty_string(Some(&json!(""))), None);
    }

    #[test]
    fn nonempty_string_none() {
        assert_eq!(nonempty_string(None), None);
    }

    #[test]
    fn nonempty_string_whitespace_only() {
        assert_eq!(nonempty_string(Some(&json!("   "))), None);
    }

    // ── value_bool_or_none ──

    #[test]
    fn value_bool_or_none_bool_true() {
        assert_eq!(value_bool_or_none(Some(&json!(true))), Some(true));
    }

    #[test]
    fn value_bool_or_none_bool_false() {
        assert_eq!(value_bool_or_none(Some(&json!(false))), Some(false));
    }

    #[test]
    fn value_bool_or_none_string_true() {
        assert_eq!(
            value_bool_or_none(Some(&json!("true"))),
            Some(true)
        );
    }

    #[test]
    fn value_bool_or_none_string_yes() {
        assert_eq!(
            value_bool_or_none(Some(&json!("yes"))),
            Some(true)
        );
    }

    #[test]
    fn value_bool_or_none_string_1() {
        assert_eq!(value_bool_or_none(Some(&json!("1"))), Some(true));
    }

    #[test]
    fn value_bool_or_none_string_false() {
        assert_eq!(
            value_bool_or_none(Some(&json!("false"))),
            Some(false)
        );
    }

    #[test]
    fn value_bool_or_none_string_no() {
        assert_eq!(
            value_bool_or_none(Some(&json!("no"))),
            Some(false)
        );
    }

    #[test]
    fn value_bool_or_none_string_garbage() {
        assert_eq!(
            value_bool_or_none(Some(&json!("maybe"))),
            None
        );
    }

    #[test]
    fn value_bool_or_none_number() {
        assert_eq!(value_bool_or_none(Some(&json!(42))), None);
    }

    #[test]
    fn value_bool_or_none_null() {
        assert_eq!(value_bool_or_none(Some(&json!(null))), None);
    }

    #[test]
    fn value_bool_or_none_none() {
        assert_eq!(value_bool_or_none(None), None);
    }

    // ── value_string_list ──

    #[test]
    fn value_string_list_array() {
        let val = json!(["a", "b", "c"]);
        assert_eq!(value_string_list(Some(&val)), vec!["a", "b", "c"]);
    }

    #[test]
    fn value_string_list_empty_array() {
        let val = json!([]);
        assert!(value_string_list(Some(&val)).is_empty());
    }

    #[test]
    fn value_string_list_filters_empty_items() {
        let val = json!(["a", "", "b"]);
        assert_eq!(value_string_list(Some(&val)), vec!["a", "b"]);
    }

    #[test]
    fn value_string_list_non_array() {
        let val = json!("not an array");
        assert!(value_string_list(Some(&val)).is_empty());
    }

    #[test]
    fn value_string_list_none() {
        assert!(value_string_list(None).is_empty());
    }

    #[test]
    fn value_string_list_mixed_types() {
        let val = json!(["hello", 42, true, null]);
        // value_text converts numbers/bools/null to strings; empty ones are filtered.
        let result = value_string_list(Some(&val));
        assert!(result.contains(&"hello".to_string()));
        assert!(result.contains(&"42".to_string()));
        assert!(result.contains(&"true".to_string()));
        // null → "" → filtered out
        assert!(!result.contains(&"".to_string()));
    }

    // ── first_nonempty ──

    #[test]
    fn first_nonempty_basic() {
        let values = vec!["".into(), "hello".into(), "world".into()];
        assert_eq!(first_nonempty(&values), "hello");
    }

    #[test]
    fn first_nonempty_all_empty() {
        let values = vec!["".into(), "  ".into()];
        assert_eq!(first_nonempty(&values), "");
    }

    #[test]
    fn first_nonempty_empty_vec() {
        let values: Vec<String> = vec![];
        assert_eq!(first_nonempty(&values), "");
    }

    #[test]
    fn first_nonempty_first_is_nonempty() {
        let values = vec!["a".into(), "b".into()];
        assert_eq!(first_nonempty(&values), "a");
    }

    // ── stable_line_items ──

    #[test]
    fn stable_line_items_deduplicates() {
        let items = vec!["a".into(), "b".into(), "a".into()];
        assert_eq!(stable_line_items(items), vec!["a", "b"]);
    }

    #[test]
    fn stable_line_items_filters_empty() {
        let items = vec!["".into(), "a".into(), "  ".into()];
        assert_eq!(stable_line_items(items), vec!["a"]);
    }

    #[test]
    fn stable_line_items_trims_whitespace() {
        let items = vec!["  hello  ".into(), "hello".into()];
        assert_eq!(stable_line_items(items), vec!["hello"]);
    }

    #[test]
    fn stable_line_items_empty() {
        let items: Vec<String> = vec![];
        assert!(stable_line_items(items).is_empty());
    }

    // ── required_non_empty_string ──

    #[test]
    fn required_non_empty_string_ok() {
        let payload = json!({"name": "Alice"});
        let result =
            required_non_empty_string(&payload, "name", "test").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn required_non_empty_string_missing_key() {
        let payload = json!({});
        let result = required_non_empty_string(&payload, "name", "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires non-empty name"));
    }

    #[test]
    fn required_non_empty_string_empty_value() {
        let payload = json!({"name": ""});
        let result = required_non_empty_string(&payload, "name", "test");
        assert!(result.is_err());
    }

    #[test]
    fn required_non_empty_string_whitespace_only() {
        let payload = json!({"name": "  "});
        let result = required_non_empty_string(&payload, "name", "test");
        assert!(result.is_err());
    }

    // ── optional_non_empty_string ──

    #[test]
    fn optional_non_empty_string_some() {
        let payload = json!({"k": "val"});
        assert_eq!(
            optional_non_empty_string(&payload, "k"),
            Some("val".into())
        );
    }

    #[test]
    fn optional_non_empty_string_empty() {
        let payload = json!({"k": ""});
        assert_eq!(optional_non_empty_string(&payload, "k"), None);
    }

    #[test]
    fn optional_non_empty_string_missing() {
        let payload = json!({});
        assert_eq!(optional_non_empty_string(&payload, "k"), None);
    }

    #[test]
    fn optional_non_empty_string_trimmed() {
        let payload = json!({"k": "  hello  "});
        assert_eq!(
            optional_non_empty_string(&payload, "k"),
            Some("hello".into())
        );
    }

    // ── optional_bool ──

    #[test]
    fn optional_bool_true() {
        let payload = json!({"flag": true});
        assert_eq!(optional_bool(&payload, "flag"), Some(true));
    }

    #[test]
    fn optional_bool_false() {
        let payload = json!({"flag": false});
        assert_eq!(optional_bool(&payload, "flag"), Some(false));
    }

    #[test]
    fn optional_bool_missing() {
        let payload = json!({});
        assert_eq!(optional_bool(&payload, "flag"), None);
    }

    #[test]
    fn optional_bool_not_a_bool() {
        let payload = json!({"flag": "yes"});
        assert_eq!(optional_bool(&payload, "flag"), None);
    }
}
