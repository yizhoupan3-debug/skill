//! Neutral entry for Cursor hook JSON stdin → review/subagent gate (implementation in `cursor_hooks`).

use std::io::Write;
use std::path::Path;

fn cursor_hook_event_is_critical(event: &str) -> bool {
    matches!(
        event.trim().to_ascii_lowercase().as_str(),
        "beforesubmitprompt" | "stop" | "subagentstart" | "posttooluse" | "subagentstop"
    )
}

fn emit_cursor_hook_fail_closed_stdout(event: &str) -> Result<(), String> {
    let lowered = event.trim().to_ascii_lowercase();
    let msg = "router-rs binary unavailable or invalid hook stdin (fail-closed)";
    let out = match lowered.as_str() {
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
    };
    let mut stdout = std::io::stdout();
    let serialized = serde_json::to_string(&out).map_err(|e| e.to_string())?;
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn run_review_gate(event: &str, cli_repo_root: Option<&Path>) -> Result<(), String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    crate::hook_timing::mark_hook_start();
    let result = (|| -> Result<(), String> {
        let payload = match crate::cursor_hooks::read_cursor_hook_stdin_json() {
            Ok(v) => v,
            Err(e) if cursor_hook_event_is_critical(event) => {
                emit_cursor_hook_fail_closed_stdout(event)?;
                return Err(e);
            }
            Err(_) => serde_json::json!({}),
        };
        if !payload.is_object() {
            if cursor_hook_event_is_critical(event) {
                emit_cursor_hook_fail_closed_stdout(event)?;
                return Err("stdin_json_not_object".to_string());
            }
        }
        let repo_root =
            crate::cursor_hooks::resolve_cursor_hook_repo_root(cli_repo_root, &payload)?;
        let _registry_guard = crate::runtime_registry::HookRegistryRepoGuard::new(&repo_root);
        let mut output =
            crate::cursor_hooks::dispatch_cursor_hook_event(&repo_root, event, &payload);
        crate::telemetry_emit::emit_hook_fired(
            event,
            crate::telemetry_emit::hook_action_from_output(&output),
        );
        crate::autopilot_goal::scrub_followup_fields_in_hook_output(&mut output);
        crate::cursor_hooks::apply_cursor_hook_output_policy(&mut output);
        crate::cursor_hooks::apply_cursor_hook_silent_policy(&mut output);
        let mut stdout = std::io::stdout();
        let serialized = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        stdout
            .write_all(format!("{serialized}\n").as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    crate::hook_timing::emit_hook_timing_line(event);
    result
}
