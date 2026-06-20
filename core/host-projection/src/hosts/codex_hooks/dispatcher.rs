//! Codex host: full `HostHookDispatcher` implementation.
//!
//! Codex uses `router-rs hook --host-id codex --event=...` for all hook events,
//! unified with cursor/claude/codex via the shared `HostHookDispatcher` trait.
//!
//! This dispatcher replaces the hand-written event_name match dispatch in
//! `run_codex_lifecycle_context_hook_inner` with the shared trait dispatch.

use super::super::hook_dispatch::{HookEvent, HookOutput, HostHookConfig, HostHookDispatcher};
use super::handlers;
use super::pretool;
use super::state;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Codex lifecycle constants (mirrors CODEX_STRINGS in mod.rs)
// ---------------------------------------------------------------------------

const CODEX_REVIEW_GATE_TAG: &str = "CODEX_REVIEW_GATE";
const CODEX_REQUIRE_STABLE_SESSION_KEY_ENV: &str =
    "ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY";

/// Stable session key error message — mirrors `codex_lifecycle_input_error` in handlers.rs.
fn cod_ex_session_key_error() -> String {
    format!(
        "Codex lifecycle hook blocked: stable session key required ({} defaults on). \
         Add session_id / conversation_id / thread_id (snake_case or camelCase) to \
         hook JSON, or set session env fallbacks. Review gate ({}) cannot run without \
         per-session hook-state.",
        CODEX_REQUIRE_STABLE_SESSION_KEY_ENV,
        CODEX_REVIEW_GATE_TAG,
    )
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy)]
pub struct CodexHookDispatcher;

impl HostHookConfig for CodexHookDispatcher {
    fn host_id(&self) -> &'static str {
        "codex"
    }

    fn state_dir_leaf(&self) -> &'static str {
        ".codex"
    }

    fn hook_state_unreadable_tag(&self) -> &'static str {
        "CODEX_HOOK_STATE_UNREADABLE"
    }

    fn session_namespace_env(&self) -> &'static str {
        "ROUTER_RS_CODEX_SESSION_NAMESPACE"
    }

    fn log_label(&self) -> &'static str {
        "codex"
    }

    fn supports_session_start(&self) -> bool {
        true
    }

    fn supports_subagent_start(&self) -> bool {
        true
    }

    fn supports_subagent_stop(&self) -> bool {
        true
    }
}

impl HostHookDispatcher for CodexHookDispatcher {
    // ── PreToolUse: path protection ──

    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        match pretool::run_codex_pre_tool_use(event.repo_root, event.payload) {
            Ok(Some(val)) => Some(HookOutput::Raw(val)),
            Ok(None) => None,
            Err(err) => Some(HookOutput::Deny { reason: err }),
        }
    }

    // ── UserPromptSubmit: review gate init + context injection ──

    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput> {
        // Session key check (same as run_codex_lifecycle_context_hook_inner)
        if let Some(block) = check_codex_stable_session_key(event.payload) {
            return Some(block);
        }
        handlers::handle_codex_userpromptsubmit(event.repo_root, event.payload)
            .map(HookOutput::Raw)
    }

    // ── PostToolUse: evidence + subagent tracking ──

    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        // Session key check (same as run_codex_lifecycle_context_hook_inner)
        if let Some(block) = check_codex_stable_session_key(event.payload) {
            return Some(block);
        }
        handlers::handle_codex_posttooluse(event.repo_root, event.payload)
            .map(HookOutput::Raw)
    }

    // ── Stop: closeout gate + review gate check ──

    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        // Session key check (same as run_codex_lifecycle_context_hook_inner)
        if let Some(block) = check_codex_stable_session_key(event.payload) {
            return Some(block);
        }
        handlers::handle_codex_stop(event.repo_root, event.payload)
            .map(HookOutput::Raw)
    }

    // ── SessionStart: context injection ──

    fn handle_session_start(&self, event: &HookEvent) -> Option<HookOutput> {
        handlers::handle_codex_session_start(event.repo_root, event.payload)
            .map(HookOutput::Raw)
    }

    // ── SubagentStart: review lane tracking ──

    fn handle_subagent_start(&self, event: &HookEvent) -> Option<HookOutput> {
        handlers::handle_codex_subagent_start(event.repo_root, event.payload)
            .map(HookOutput::Raw)
    }

    // ── SubagentStop: informational (always no-op) ──

    fn handle_subagent_stop(&self, _event: &HookEvent) -> Option<HookOutput> {
        None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a stable session key is required and available.
/// Mirrors the check from `run_codex_lifecycle_context_hook_inner` in handlers.rs.
fn check_codex_stable_session_key(payload: &Value) -> Option<HookOutput> {
    if state::require_stable_session_key_enabled()
        && state::stable_session_raw(payload).is_none()
    {
        return Some(HookOutput::Block {
            reason: cod_ex_session_key_error(),
        });
    }
    None
}

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

#[cfg(any(test, feature = "test-support"))]
pub fn dispatch_codex_hook_event(
    repo_root: &std::path::Path,
    event_name: &str,
    payload: &Value,
) -> Value {
    use super::super::hook_dispatch::HostHookDispatcher;
    let event = HookEvent {
        repo_root,
        event_name,
        payload,
    };
    match CodexHookDispatcher.dispatch(&event) {
        None | Some(HookOutput::None) => serde_json::json!({}),
        Some(HookOutput::Raw(val)) => val,
        Some(HookOutput::AdditionalContext(ctx)) => serde_json::json!({
            "hookSpecificOutput": { "additionalContext": ctx }
        }),
        Some(HookOutput::Deny { reason }) => serde_json::json!({
            "decision": "block",
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            },
        }),
        Some(HookOutput::Block { reason }) => serde_json::json!({
            "decision": "block",
            "followup_message": reason,
        }),
        Some(HookOutput::Advisory { message }) => serde_json::json!({
            "followup_message": message,
        }),
        Some(HookOutput::Warn { message }) => serde_json::json!({
            "warning": message,
        }),
    }
}
