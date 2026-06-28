//! Shared `serde_json::Value` field extractors and small string utilities for `framework_runtime`.
//!
//! The portable pub functions have been moved to `framework-kernel` (single source of truth).
//! This module re-exports them and keeps only crate-private helpers.

pub use framework_kernel::json_value::*;

use serde_json::Value;

fn nested_value<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = payload;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub fn nested_non_empty_string(payload: &Value, path: &[&str]) -> Option<String> {
    let text = nested_value(payload, path).and_then(Value::as_str)?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

pub fn nested_bool(payload: &Value, path: &[&str]) -> Option<bool> {
    nested_value(payload, path).and_then(Value::as_bool)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nested_value_traverses_path() {
        let v = json!({"a": {"b": {"c": 42}}});
        assert_eq!(nested_value(&v, &["a", "b", "c"]).unwrap(), &json!(42));
    }

    #[test]
    fn nested_value_returns_none_on_missing() {
        let v = json!({"a": 1});
        assert!(nested_value(&v, &["a", "b"]).is_none());
    }

    #[test]
    fn nested_non_empty_string_extracts_trimmed() {
        let v = json!({"key": "  hello  "});
        assert_eq!(
            nested_non_empty_string(&v, &["key"]),
            Some("hello".to_string())
        );
    }

    #[test]
    fn nested_non_empty_string_returns_none_for_empty() {
        let v = json!({"key": "   "});
        assert!(nested_non_empty_string(&v, &["key"]).is_none());
    }

    #[test]
    fn nested_non_empty_string_returns_none_for_non_string() {
        let v = json!({"key": 42});
        assert!(nested_non_empty_string(&v, &["key"]).is_none());
    }

    #[test]
    fn nested_bool_extracts_true() {
        let v = json!({"flag": true});
        assert_eq!(nested_bool(&v, &["flag"]), Some(true));
    }

    #[test]
    fn nested_bool_returns_none_for_non_bool() {
        let v = json!({"flag": "yes"});
        assert!(nested_bool(&v, &["flag"]).is_none());
    }
}
