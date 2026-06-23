//! Shared `serde_json::Value` field extractors and small string utilities for `framework_runtime`.
//!
//! The portable pub functions have been moved to `framework-kernel` (single source of truth).
//! This module re-exports them and keeps only crate-private helpers.

pub use framework_kernel::json_value::*;

use serde_json::Value;

pub(crate) fn nested_value<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = payload;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub(crate) fn nested_non_empty_string(payload: &Value, path: &[&str]) -> Option<String> {
    let text = nested_value(payload, path)
        .and_then(Value::as_str)?
        .trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

pub(crate) fn nested_bool(payload: &Value, path: &[&str]) -> Option<bool> {
    nested_value(payload, path).and_then(Value::as_bool)
}
