//! Unified UserPromptSubmit/PostToolUse event handlers (ADR §2.1).
//!
//! All hosts share the same event handling pipeline. Host-specific differences
//! are injected via the HostProvider trait.

use serde_json::{Value, json};
use std::path::Path;

/// Unified UserPromptSubmit handler.
pub fn handle_user_prompt_submit(repo_root: &Path, payload: &Value, host_id: &str) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();

    let prompt = crate::hosts::hook_dispatch::extract_prompt_text(payload);
    if prompt.trim().is_empty() {
        return None;
    }

    // Touch state update (shared) — delegated to host-specific handler
    // via HostProvider trait. No-op when not registered.

    None
}

/// Unified PostToolUse handler.
pub fn handle_post_tool_use(repo_root: &Path, payload: &Value, host_id: &str) -> Option<Value> {
    crate::hooks::ensure_kernel_bootstrap();

    // Evidence tracking (shared)
    if let Some(tool_name) = payload.get("tool_name").and_then(Value::as_str) {
        let _ = crate::hooks::maybe_record_research_activity(repo_root, tool_name, "");
    }

    None
}
