//! Shared state management & evidence recording for all 4 hosts.
//!
//! Extracted from `hook_dispatch.rs` as part of the modularization.

use serde_json::Value;
use std::path::Path;

/// Auto-record verification evidence from a Bash tool call.
pub fn auto_record_verification_evidence(repo_root: &Path, payload: &Value) {
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(command) = crate::hosts::hook_dispatch::bash_command(payload) else {
        return;
    };
    let cmd_trimmed = command.trim();
    if !crate::hosts::hook_dispatch::is_verification_command(tool_name, cmd_trimmed) {
        return;
    }
    let exit_code = crate::hosts::hook_dispatch::payload_exit_code(payload);
    let output_summary = crate::hosts::hook_dispatch::extract_output_summary(payload, 500);
    let mut entry = serde_json::Map::new();
    entry.insert("kind".to_string(), serde_json::json!("auto_evidence"));
    entry.insert(
        "source".to_string(),
        serde_json::json!("post_tool_use_auto"),
    );
    entry.insert("tool_name".to_string(), serde_json::json!("Bash"));
    entry.insert(
        "command_preview".to_string(),
        serde_json::json!(cmd_trimmed),
    );
    entry.insert(
        "recorded_at".to_string(),
        serde_json::json!(crate::hooks::current_local_timestamp()),
    );
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), serde_json::json!(ec));
        entry.insert("success".to_string(), serde_json::json!(ec == 0));
    }
    if let Some(ref text) = output_summary {
        entry.insert("output".to_string(), serde_json::json!(text));
    }
    if let Err(e) = crate::hooks::append_evidence_index(repo_root, None, entry) {
        tracing::warn!(error = %e, "failed to append evidence index");
    }
}

/// Auto-record research activity from tool calls.
pub fn auto_record_research_activity(repo_root: &Path, payload: &Value) {
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let summary = match tool_name {
        "Bash" => crate::hosts::hook_dispatch::bash_command(payload)
            .unwrap_or_default()
            .to_string(),
        "WebFetch" | "web_fetch" => payload
            .get("tool_input")
            .and_then(|ti| ti.get("url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        other => other.to_string(),
    };
    if summary.is_empty() || summary == tool_name {
        return;
    }
    crate::hooks::maybe_record_research_activity(repo_root, tool_name, &summary);
}
