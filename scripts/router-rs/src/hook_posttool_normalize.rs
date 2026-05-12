//! Cursor `postToolUse` → `framework_runtime` shell-evidence shape normalization.
//!
//! Crate-level boundary so append stays in [`crate::framework_runtime`] while Cursor-only
//! JSON merging stays out of that module. Field extraction helpers live in [`crate::cursor_hooks`].

use serde_json::{json, Value};

/// Normalize heterogeneous Cursor PostTool payloads into the shape [`crate::framework_runtime::try_append_post_tool_shell_evidence`]
/// understands (preserves original fields like `tool_output` / `exit_code` where present).
pub(crate) fn synthetic_post_tool_evidence_shape(event: &Value) -> Value {
    let mut out = match event.as_object() {
        Some(o) => o.clone(),
        None => serde_json::Map::new(),
    };
    out.insert(
        "tool_name".to_string(),
        json!(crate::cursor_hooks::tool_name_of(event)),
    );
    let merged_input = crate::cursor_hooks::tool_input_of(event);
    if merged_input
        .as_object()
        .map(|m| !m.is_empty())
        .unwrap_or(false)
    {
        out.insert("tool_input".to_string(), merged_input);
    }
    if let Some(s) = crate::cursor_hooks::extract_first_session_string(event) {
        out.insert("session_id".to_string(), json!(s));
    }
    Value::Object(out)
}
