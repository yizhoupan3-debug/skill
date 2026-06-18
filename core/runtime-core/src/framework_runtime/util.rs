//! Shared utility functions extracted from `mod.rs` for K6 module splitting.
//!
//! These are small helpers used across multiple framework_runtime submodules.
//! They do not depend on any other framework_runtime submodule.

use crate::atomic_write::write_atomic_text;
use super::constants::TASK_REGISTRY_SCHEMA_VERSION;
use super::json_io::read_text_if_exists;
use super::json_value::{
    first_nonempty, nonempty_string, safe_slug, value_bool_or_none, value_text,
};
use chrono::{Local, SecondsFormat};
use hex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Write text content to a path only if it differs from the existing content.
/// Returns `true` when the file was actually written.
pub fn write_text_if_changed_unlocked(path: &Path, content: &str) -> Result<bool, String> {
    crate::path_guard::reject_unsafe_path(path)?;
    let existing = read_text_if_exists(path);
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    write_atomic_text(path, content)?;
    Ok(true)
}

/// Compute SHA-256 hex digest of a file (used by integration tests across crates).
pub fn hash_file_for_test(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("read file failed for {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// Write a JSON value to a path, serializing to pretty-printed text first.
pub fn write_json_if_changed_unlocked(path: &Path, payload: &Value) -> Result<bool, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize JSON payload failed: {err}"))?
    );
    write_text_if_changed_unlocked(path, &serialized)
}

/// Local timestamp in RFC 3339 format (seconds precision).
pub fn current_local_timestamp() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// Extract and validate a required text field from a JSON payload.
pub fn required_payload_text(payload: &Value, key: &str, context: &str) -> Result<String, String> {
    let Some(v) = payload.get(key) else {
        return Err(format!("{context}: missing required field {key:?}"));
    };
    let s = value_text(Some(v));
    if s.trim().is_empty() {
        return Err(format!("{context}: required field {key:?} is empty"));
    }
    Ok(s)
}

/// Extract a text field with a fallback default.
pub fn defaulted_payload_text(payload: &Value, key: &str, fallback: &str) -> String {
    let s = payload
        .get(key)
        .map(|v| value_text(Some(v)))
        .unwrap_or_default();
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s
    }
}

/// Parse a session summary text (markdown list format) into a key-value map.
pub fn parse_session_summary(text: &str) -> Map<String, Value> {
    let mut result = Map::new();
    for line in text.lines() {
        if !line.starts_with("- ") {
            continue;
        }
        let body = &line[2..];
        let Some((key, value)) = body.split_once(':') else {
            continue;
        };
        result.insert(
            key.trim().to_string(),
            Value::String(value.trim().to_string()),
        );
    }
    result
}

/// Extract registry rows from a task payload.
pub fn registry_rows_from_payload(payload: &Value) -> Vec<Value> {
    let mut rows = Vec::new();
    if let Some(items) = payload.get("tasks").and_then(Value::as_array) {
        for item in items {
            let Some(row) = item.as_object() else {
                continue;
            };
            let task_id = safe_slug(&value_text(row.get("task_id")));
            if task_id.is_empty() {
                continue;
            }
            let task = value_text(row.get("task"));
            let task_value = if task.is_empty() {
                Value::String(task_id.clone())
            } else {
                Value::String(task)
            };
            rows.push(json!({
                "task_id": task_id,
                "task": task_value,
                "updated_at": nonempty_string(row.get("updated_at")),
                "status": nonempty_string(row.get("status")),
                "phase": nonempty_string(row.get("phase")),
                "resume_allowed": value_bool_or_none(row.get("resume_allowed")),
            }));
        }
    }
    rows
}

/// Normalize task registry rows: deduplicate, sort, and split into known/recoverable lists.
pub fn normalize_task_registry_rows(
    focus_task_id: String,
    mut rows: Vec<Value>,
) -> (Value, Vec<String>, Vec<String>) {
    rows.sort_by(|left, right| {
        registry_task_sort_key(right)
            .cmp(&registry_task_sort_key(left))
            .then_with(|| value_text(right.get("task_id")).cmp(&value_text(left.get("task_id"))))
    });

    let mut seen = HashSet::new();
    let mut tasks = Vec::new();
    let mut known_task_ids = Vec::new();
    let mut recoverable_task_ids = Vec::new();
    let mut overflow_count = 0usize;
    for row in rows {
        let task_id = safe_slug(&value_text(row.get("task_id")));
        if task_id.is_empty() || !seen.insert(task_id.clone()) {
            continue;
        }
        if value_bool_or_none(row.get("resume_allowed")) == Some(true) {
            recoverable_task_ids.push(task_id.clone());
        }
        known_task_ids.push(task_id);
        if tasks.len() >= 128 {
            overflow_count += 1;
            continue;
        }
        tasks.push(row);
    }
    tasks.sort_by(|left, right| {
        let left_focus = value_text(left.get("task_id")) == focus_task_id;
        let right_focus = value_text(right.get("task_id")) == focus_task_id;
        right_focus
            .cmp(&left_focus)
            .then_with(|| registry_task_sort_key(right).cmp(&registry_task_sort_key(left)))
            .then_with(|| value_text(left.get("task_id")).cmp(&value_text(right.get("task_id"))))
    });
    (
        json!({
            "schema_version": TASK_REGISTRY_SCHEMA_VERSION,
            "focus_task_id": if focus_task_id.is_empty() {
                Value::Null
            } else {
                Value::String(focus_task_id)
            },
            "tasks": tasks,
            "task_count": known_task_ids.len(),
            "recoverable_task_count": recoverable_task_ids.len(),
            "truncated": overflow_count > 0,
            "overflow_count": overflow_count,
        }),
        known_task_ids,
        recoverable_task_ids,
    )
}

/// Sort key for a registry row (updated_at or task_id).
fn registry_task_sort_key(row: &Value) -> String {
    first_nonempty(&[
        value_text(row.get("updated_at")),
        value_text(row.get("task_id")),
    ])
}

/// Truncate a UTF-8 string to at most `max_chars` characters.
pub fn truncate_utf8_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

/// Count evidence rows in an evidence index value.
pub fn count_evidence_rows(evidence_index: &Value) -> usize {
    evidence_index
        .get("artifacts")
        .or_else(|| evidence_index.get("evidence"))
        .and_then(Value::as_array)
        .map(|rows| rows.len())
        .unwrap_or(0)
}

/// Extract the `execution_contract` sub-object from supervisor state.
pub fn supervisor_contract(state: &Map<String, Value>) -> Map<String, Value> {
    state
        .get("execution_contract")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// Check if a lowered value matches any of the given terminal values.
pub fn is_terminal(value: &str, terminal_values: &[&str]) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    terminal_values
        .iter()
        .any(|candidate| lowered == *candidate)
}
