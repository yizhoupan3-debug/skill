//! Unified PreToolUse path protection (ADR §2.1).
//!
//! Guards against unauthorized file system operations during tool execution.
//! All hosts share the same guard logic.
//!
//! Currently a no-op placeholder. The actual guard evaluation is registered
//! via L0 function pointer by L4 at bootstrap. Per ADR P10, the fallback
//! when no guard is registered is a no-op (not panic or hard block).

use serde_json::Value;
use std::path::Path;

/// Evaluate PreToolUse guard for a tool call.
///
/// Returns `Some(output)` to block the tool call, or `None` to allow it.
pub fn evaluate_pre_tool_guard(
    _repo_root: &Path,
    _tool_name: &str,
    _tool_input: &Value,
    _host_id: &str,
) -> Option<Value> {
    // No-op when guard not registered (ADR P10: no-op fallback)
    None
}
