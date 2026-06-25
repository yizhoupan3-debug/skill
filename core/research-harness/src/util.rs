//! Shared JSON value helpers for the research harness.
//!
//! Consolidates duplicated helper functions from state, render, smoke,
//! claims, search, and CLI modules.
//!
//! NOTE: Canonical helpers live here; local duplicates in claims/lifecycle.rs
//! and search modules import from this module.

use serde_json::Value;

/// Extract a string field, returning `""` if missing.
pub(crate) fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}


/// Extract a string field with a custom default.
pub(crate) fn str_field_default<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
}

/// Get an array slice, returning `&[]` if missing.
pub(crate) fn arr<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// Get a mutable array, inserting `[]` if missing.
pub(crate) fn arr_mut<'a>(value: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    #[allow(clippy::expect_used)]
    obj_mut(value)
        .entry(key.to_string())
        .or_insert_with(|| serde_json::json!([]))
        // safety: or_insert_with(json!([])) guarantees the entry is an array;
        // as_array_mut() cannot return None here.
        .as_array_mut()
        .expect("expected array")
}

/// Get the underlying mutable object map, coercing non-objects to `{}`.
pub(crate) fn obj_mut(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    #[allow(clippy::expect_used)]
    // safety: obj_mut just coerced this to `{}` above; as_object_mut() always succeeds.
    value.as_object_mut().expect("obj_mut: value must be object after coercion")
}

/// Insert a key-value pair into a mutable object.
pub(crate) fn set_key(value: &mut Value, key: &str, child: Value) {
    obj_mut(value).insert(key.to_string(), child);
}

/// Get the `novelty_gate` sub-object (immutable).
pub(crate) fn novelty_gate(state: &Value) -> &Value {
    state.get("novelty_gate").unwrap_or(&Value::Null)
}

/// Get the `novelty_gate` sub-object (mutable), inserting `{}` if missing.
pub(crate) fn novelty_gate_mut(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    #[allow(clippy::expect_used)]
    // safety: or_insert_with(json!({})) guarantees the entry is an object;
    // as_object_mut() cannot return None here.
    obj_mut(value)
        .entry("novelty_gate".to_string())
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("novelty_gate must be object")
}

/// Extract a list of strings from a JSON array field.
pub(crate) fn value_as_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Render a Value as a string (string values pass through, numbers become text).
pub(crate) fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}

// ── HTTP Client Factories ──

/// Build a blocking HTTP client with a bounded timeout.
pub(crate) fn blocking_client(timeout_secs: u64) -> anyhow::Result<reqwest::blocking::Client> {
    use anyhow::Context;
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs.clamp(3, 120)))
        .build()
        .context("failed to build blocking HTTP client")
}

// ── Hash Helpers ──
