//! Cursor host: full `HostHookDispatcher` implementation.
//!
//! Cursor uses `router-rs hook --host-id cursor --event=...` for all hook events,
//! unified with claude/codex/opencode via the shared `HostHookDispatcher` trait.
//!
//! The existing cursor-specific `dispatch_cursor_hook_event` remains unchanged
//! for non-standard events. This dispatcher implements the 7-method trait by
//! delegating to the individual `super::handle_*` functions.

use serde_json::{json, Value};

use super::super::hook_dispatch::{HookEvent, HookOutput, HostHookConfig, HostHookDispatcher};

#[derive(Debug, Default, Clone, Copy)]
pub struct CursorHookDispatcher;

impl HostHookConfig for CursorHookDispatcher {
    fn host_id(&self) -> &'static str { "cursor" }
    fn state_dir_leaf(&self) -> &'static str { ".cursor" }
    fn hook_state_unreadable_tag(&self) -> &'static str { "CURSOR_HOOK_STATE_UNREADABLE" }
    fn session_namespace_env(&self) -> &'static str { "ROUTER_RS_CURSOR_SESSION_NAMESPACE" }
    fn log_label(&self) -> &'static str { "cursor" }
    fn supports_session_start(&self) -> bool { true }
    fn supports_subagent_start(&self) -> bool { true }
    fn supports_subagent_stop(&self) -> bool { true }
}

/// Convert a `Value` returned by a cursor handler into `Option<HookOutput>`.
/// Empty JSON objects (`{}`) map to `None`; all other values wrap as `Raw`.
fn output_from_value(val: Value) -> Option<HookOutput> {
    if val == Value::Object(Default::default()) {
        None
    } else {
        Some(HookOutput::Raw(val))
    }
}

impl HostHookDispatcher for CursorHookDispatcher {
    fn handle_pre_tool_use(&self, _event: &HookEvent) -> Option<HookOutput> {
        // Cursor does not currently implement PreToolUse path protection.
        None
    }

    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput> {
        output_from_value(super::handle_before_submit(event.repo_root, event.payload))
    }

    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        output_from_value(super::handle_post_tool_use(event.repo_root, event.payload))
    }

    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        output_from_value(super::handle_stop(event.repo_root, event.payload))
    }

    fn handle_session_start(&self, event: &HookEvent) -> Option<HookOutput> {
        output_from_value(super::handle_session_start(event.repo_root, event.payload))
    }

    fn handle_subagent_start(&self, event: &HookEvent) -> Option<HookOutput> {
        output_from_value(super::handle_subagent_start(event.repo_root, event.payload))
    }

    fn handle_subagent_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        output_from_value(super::handle_subagent_stop(event.repo_root, event.payload))
    }
}

/// Test harness: dispatch through the unified trait and convert back to cursor JSON.
#[cfg(any(test, feature = "test-support"))]
pub fn dispatch_cursor_hook_event_via_trait(
    repo_root: &std::path::Path,
    event_name: &str,
    payload: &Value,
) -> Value {
    let event = HookEvent { repo_root, event_name, payload };
    match CursorHookDispatcher.dispatch(&event) {
        None | Some(HookOutput::None) => json!({}),
        Some(HookOutput::Raw(val)) => val,
        Some(HookOutput::AdditionalContext(ctx)) => json!({
            "additional_context": ctx,
        }),
        Some(HookOutput::Deny { reason }) => json!({
            "decision": "block",
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            },
        }),
        Some(HookOutput::Block { reason }) => json!({
            "followup_message": reason,
        }),
        Some(HookOutput::Advisory { message }) => json!({
            "followup_message": message,
        }),
        Some(HookOutput::Warn { message }) => json!({
            "warning": message,
        }),
    }
}
