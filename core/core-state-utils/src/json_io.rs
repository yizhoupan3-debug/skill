//! Canonical JSON/text file I/O helpers (B0).
//!
//! ADR-010 §9: single source of truth for `read_json`, `write_json`, `read_text`.
//! All crates MUST use these instead of rolling their own.

use core_errors::FrameworkError;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// Read a JSON file. Returns empty object if file is missing or unparseable.
pub fn read_json_if_exists(path: &Path) -> Value {
    if !path.is_file() {
        return Value::Object(Map::new());
    }
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!("failed to parse JSON from {}: {e}", path.display());
            Value::Object(Map::new())
        }),
        Err(e) => {
            tracing::warn!("failed to read {}: {e}", path.display());
            Value::Object(Map::new())
        }
    }
}

/// Strict JSON read. Returns error on missing file or parse failure.
pub fn read_json_strict(path: &Path) -> Result<Value, FrameworkError> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let text = fs::read_to_string(path)
        .map_err(|err| FrameworkError::validation(format!("read json failed for {}: {err}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|err| FrameworkError::validation(format!("parse json failed for {}: {err}", path.display())))
}

/// Read a text file; returns empty string if missing.
pub fn read_text_if_exists(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Write JSON value to file (pretty-printed). Returns true if file was actually written.
pub fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, FrameworkError> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| FrameworkError::validation(format!("serialize JSON payload failed: {err}")))?
    );
    write_text_if_changed(path, &serialized)
}

/// Write text to file only if content differs. Returns true if written.
pub fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, FrameworkError> {
    super::path_guard::reject_unsafe_path(path)?;
    let existing = read_text_if_exists(path);
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| FrameworkError::validation(format!("create parent directory failed: {err}")))?;
    }
    super::atomic_write::write_atomic_text(path, content)?;
    Ok(true)
}
