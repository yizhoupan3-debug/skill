//! Small JSON / text read helpers shared across `framework_runtime` submodules.

use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

pub(crate) fn read_json_strict(path: &Path) -> Result<Value, String> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read json failed for {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("parse json failed for {}: {err}", path.display()))
}

/// Best-effort read: missing file, read error, or parse error yields empty object.
pub(super) fn read_json_if_exists(path: &Path) -> Value {
    if !path.is_file() {
        return Value::Object(Map::new());
    }
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Map::new())),
        Err(_) => Value::Object(Map::new()),
    }
}

pub(super) fn read_text_if_exists(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}
