use serde_json::Value;
use std::collections::HashSet;

use super::HookOutput;
use super::{SESSION_KEY_CWD_FIELDS, extract_tool_input, is_path_key};

pub fn extract_session_key(
    event: &Value,
    env_var: &'static str,
    repo_fallback: &str,
    scan_tool_input: bool,
) -> String {
    framework_core::session_key::session_key_core(
        &framework_core::session_key::SessionKeyConfig {
            env_var,
            scan_tool_input,
        },
        || {
            // When scan_tool_input, first check root-level fields,
            // then tool_input, then nested objects.
            if scan_tool_input {
                if let Some(s) = extract_session_id_from_payload(event) {
                    return Some(s);
                }
                let tool_input = extract_tool_input(event);
                if let Some(s) = extract_session_id_from_tool_input(&tool_input) {
                    return Some(s);
                }
                return extract_session_id_from_nested(event);
            }
            extract_session_id_from_payload(event)
        },
        || extract_cwd_from_payload(event),
        repo_fallback,
    )
}

/// Extract explicit session id from payload (tries shared SESSION_ID_FIELDS).
fn extract_session_id_from_payload(event: &Value) -> Option<String> {
    for key in framework_core::session_key::SESSION_ID_FIELDS {
        if let Some(val) = event.get(*key).and_then(Value::as_str)
            && !val.is_empty()
        {
            return Some(val.to_string());
        }
    }
    None
}

/// Extract parent session id from `tool_input` object (Cursor scan_tool_input path).
fn extract_session_id_from_tool_input(tool_input: &Value) -> Option<String> {
    let obj = tool_input.as_object()?;
    for key in framework_core::session_key::TOOL_INPUT_SESSION_ID_FIELDS {
        if let Some(value) = obj.get(*key).and_then(Value::as_str) {
            let t = value.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    // Check nested metadata
    if let Some(meta) = obj.get("metadata").and_then(Value::as_object) {
        for key in framework_core::session_key::TOOL_INPUT_METADATA_SESSION_ID_FIELDS {
            if let Some(value) = meta.get(*key).and_then(Value::as_str) {
                let t = value.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// Extract session id from nested objects (e.g. `hookPayload`).
fn extract_session_id_from_nested(event: &Value) -> Option<String> {
    for nest in &["hookPayload", "metadata", "context"] {
        if let Some(nobj) = event.get(*nest).and_then(Value::as_object) {
            for key in framework_core::session_key::SESSION_ID_FIELDS {
                if let Some(val) = nobj.get(*key).and_then(Value::as_str)
                    && !val.is_empty()
                {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// Extract cwd from payload using standard field names.
fn extract_cwd_from_payload(event: &Value) -> Option<String> {
    for key in SESSION_KEY_CWD_FIELDS {
        if let Some(val) = event.get(*key).and_then(Value::as_str)
            && !val.is_empty()
        {
            return Some(val.to_string());
        }
    }
    None
}

pub fn add_context(event: &str, context: &str) -> Option<Value> {
    Some(serde_json::json!({ "context_append": format!("[{event}] {context}") }))
}

/// Silent success response (empty JSON object).
pub fn silent_success() -> Value {
    serde_json::json!({})
}

/// Recursively collect path-like string values from JSON.
pub fn collect_payload_paths(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if is_path_key(k) {
                    collect_path_value(v, paths);
                } else if v.is_object() || v.is_array() {
                    collect_payload_paths(v, paths);
                }
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_payload_paths(v, paths);
            }
        }
        _ => {}
    }
}

/// Collect path strings from a single value.
pub fn collect_path_value(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::String(s) if !s.is_empty() => {
            paths.insert(s.clone());
        }
        Value::Array(arr) => {
            for v in arr {
                collect_path_value(v, paths);
            }
        }
        _ => {}
    }
}

/// Extract a numeric value from JSON by trying multiple keys.
pub fn find_numeric_key(value: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        if let Some(n) = value.get(key).and_then(Value::as_i64) {
            return Some(n);
        }
        if let Some(n) = value.get(key).and_then(Value::as_u64) {
            return Some(n as i64);
        }
        if let Some(s) = value.get(key).and_then(Value::as_str)
            && let Ok(n) = s.trim().parse::<i64>()
        {
            return Some(n);
        }
    }
    None
}

/// Extract bash command from a tool payload.
pub fn bash_command(payload: &Value) -> Option<&str> {
    payload
        .get("command")
        .or_else(|| payload.get("cmd"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

/// Extract exit code from a tool payload.
pub fn payload_exit_code(payload: &Value) -> Option<i64> {
    find_numeric_key(payload, &["exit_code", "exitCode", "exit_status"])
}

/// Check if a tool name suggests a subagent/subprocess.
pub fn subagent_tool(payload: &Value) -> bool {
    let name = payload
        .get("tool_name")
        .or_else(|| payload.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or("");
    tool_name_implies_subagent(name)
}

/// Check if a tool name implies subagent execution.
pub fn tool_name_implies_subagent(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("subagent")
        || lower.contains("spawn")
        || lower.contains("dispatch")
        || lower == "task"
        || lower.contains("sub_process")
}

/// Extract a summary of tool output for evidence tracking.
pub fn extract_output_summary(payload: &Value, max_chars: usize) -> Option<String> {
    let text = payload
        .get("output")
        .or_else(|| payload.get("result"))
        .and_then(Value::as_str)?;
    if text.is_empty() {
        return None;
    }
    let trimmed = if text.len() > max_chars {
        let limit = max_chars.min(text.len());
        // Use floor_char_boundary to avoid panic on multi-byte UTF-8 boundaries.
        let boundary = text.floor_char_boundary(limit);
        &text[..boundary]
    } else {
        text
    };
    Some(trimmed.to_string())
}

/// Convert a JSON value to a HookOutput.
pub(crate) fn value_to_hook_output(val: &Value) -> Option<HookOutput> {
    if val.is_null() {
        return None;
    }
    if let Some(s) = val.get("context_append").and_then(Value::as_str) {
        return Some(HookOutput::AdditionalContext(s.to_string()));
    }
    if let Some(s) = val.get("followup_message").and_then(Value::as_str) {
        return Some(HookOutput::Advisory {
            message: s.to_string(),
        });
    }
    Some(HookOutput::Raw(val.clone()))
}

/// Convert a HookOutput to a JSON value (host-agnostic).
pub(crate) fn hook_output_to_json_value(event_name: &str, output: Option<HookOutput>) -> Value {
    match output {
        None => serde_json::json!({}),
        Some(HookOutput::None) => serde_json::json!({}),
        Some(HookOutput::AdditionalContext(ctx)) => {
            serde_json::json!({ "context_append": format!("[{event_name}] {ctx}") })
        }
        Some(HookOutput::Advisory { message }) => {
            serde_json::json!({ "followup_message": message })
        }
        Some(HookOutput::Block { reason }) => {
            serde_json::json!({ "decision": "block", "reason": reason })
        }
        Some(HookOutput::Deny { reason }) => {
            serde_json::json!({ "decision": "block", "reason": reason })
        }
        Some(HookOutput::Warn { message }) => serde_json::json!({ "warning": message }),
        Some(HookOutput::Raw(v)) => v,
    }
}

/// Check if a payload looks like a foreign host's hook stdin envelope (not the active host).
/// Detects Cursor-style envelopes by checking for `cursor_version` + `workspace_roots`.
pub fn payload_looks_like_foreign_hook_stdin(payload: &Value) -> bool {
    payload
        .get("cursor_version")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
        && payload
            .get("workspace_roots")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
}

pub(crate) fn extract_subagent_id_from_payload(payload: &Value) -> Option<String> {
    for key in &["agent_id", "subagent_id", "task_id", "id"] {
        if let Some(s) = payload
            .get(*key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return Some(s.to_string());
        }
        // Also check nested `params` object (some hosts wrap inside params)
        if let Some(s) = payload
            .get("params")
            .and_then(|p| p.get(*key))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Extract error message from a SubagentStop payload.
pub(crate) fn extract_subagent_error_from_payload(payload: &Value) -> Option<String> {
    for key in &["error", "error_message", "reason"] {
        if let Some(s) = payload
            .get(*key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Check if the payload signal text contains failure indicators.
pub(crate) fn payload_signal_contains_failure(payload: &Value) -> bool {
    let signal = payload.get("signal").and_then(Value::as_str).unwrap_or("");
    let error = payload.get("error").and_then(Value::as_str).unwrap_or("");
    let status = payload.get("status").and_then(Value::as_str).unwrap_or("");
    let text = format!("{signal} {error} {status}");
    let lower = text.to_ascii_lowercase();
    lower.contains("error")
        || lower.contains("failed")
        || lower.contains("timeout")
        || lower.contains("killed")
        || lower.contains("interrupted")
}
