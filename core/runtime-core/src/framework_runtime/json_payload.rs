//! JSON payload field extractors shared by B3 runtime control-plane modules.

use serde_json::Value;

pub fn required_non_empty_string(
    payload: &Value,
    key: &str,
    context: &str,
) -> Result<String, String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("{context} requires non-empty {key}"))
}

pub fn optional_non_empty_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

pub fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

pub fn nested_value<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = payload;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub fn nested_non_empty_string(payload: &Value, path: &[&str]) -> Option<String> {
    nested_value(payload, path)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

pub fn nested_bool(payload: &Value, path: &[&str]) -> Option<bool> {
    nested_value(payload, path).and_then(Value::as_bool)
}
