//! Cursor hook CLI entry — stdin, repo root, HostHook dispatch, and post-process policies.

use super::{
    normalize_cursor_dispatch_event, read_cursor_hook_stdin_json,
    resolve_cursor_hook_repo_root,
};
use router_rs::hosts::cursor_hook_host::CursorHookHost;
use router_rs::hosts::host_hook::HostHook;
use serde_json::Value;
use std::path::Path;

/// CLI outcome: always carries stdout JSON; critical stdin failures set `fail_closed_reason`.
pub struct CursorHookCliOutcome {
    pub output: Value,
    pub fail_closed_reason: Option<String>,
}

pub fn cursor_hook_event_is_critical(event: &str) -> bool {
    matches!(
        normalize_cursor_dispatch_event(event).as_str(),
        "beforesubmitprompt"
            | "stop"
            | "subagentstart"
            | "posttooluse"
            | "subagentstop"
    )
}

pub fn cursor_hook_fail_closed_output(event: &str) -> Value {
    let lowered = normalize_cursor_dispatch_event(event);
    let msg = "router-rs binary unavailable or invalid hook stdin (fail-closed)";
    match lowered.as_str() {
        "beforesubmitprompt" => serde_json::json!({
            "continue": false,
            "followup_message": msg,
            "user_message": msg,
        }),
        "subagentstart" => serde_json::json!({
            "permission": "deny",
            "followup_message": msg,
            "user_message": msg,
        }),
        "stop" | "posttooluse" | "subagentstop" => serde_json::json!({
            "continue": false,
            "followup_message": msg,
            "user_message": msg,
        }),
        _ => serde_json::json!({
            "permission": "deny",
            "followup_message": msg,
            "user_message": msg,
        }),
    }
}

pub fn execute_cursor_hook(
    event: &str,
    cli_repo_root: Option<&Path>,
) -> Result<CursorHookCliOutcome, String> {
    let payload = match read_cursor_hook_stdin_json() {
        Ok(v) => v,
        Err(e) if cursor_hook_event_is_critical(event) => {
            return Ok(CursorHookCliOutcome {
                output: cursor_hook_fail_closed_output(event),
                fail_closed_reason: Some(e),
            });
        }
        Err(_) => serde_json::json!({}),
    };
    if !payload.is_object()
        && cursor_hook_event_is_critical(event) {
            return Ok(CursorHookCliOutcome {
                output: cursor_hook_fail_closed_output(event),
                fail_closed_reason: Some("stdin_json_not_object".to_string()),
            });
        }
    let repo_root = resolve_cursor_hook_repo_root(cli_repo_root, &payload)?;
    let host = CursorHookHost;
    let _registry_guard = router_rs::runtime_registry::HookRegistryRepoGuard::new(&repo_root);
    let mut output = host.dispatch(&repo_root, event, &payload);
    host.finalize_cli_output(&mut output);
    Ok(CursorHookCliOutcome {
        output,
        fail_closed_reason: None,
    })
}

pub fn run_cursor_hook_cli_with_timing(
    event: &str,
    cli_repo_root: Option<&Path>,
) -> Result<CursorHookCliOutcome, String> {
    router_rs::hook_timing::mark_hook_start();
    let result = execute_cursor_hook(event, cli_repo_root);
    router_rs::hook_timing::emit_hook_timing_line(event);
    result
}
