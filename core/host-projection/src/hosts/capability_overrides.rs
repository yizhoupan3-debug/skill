//! Host-specific capability overrides organized by capability, not by host.
//!
//! These functions cannot be registry-driven because they contain control flow:
//! - `build_driver_args`: each host has unique CLI argument syntax
//! - `extract_observation_surfaces`: each host emits hook output with different JSON keys
//!
//! Both functions dispatch on `host_id` and are called from the generated
//! `HostLifecycle` / `HostTelemetry` impls when the `override_*` field is
//! `true` in `RUNTIME_REGISTRY.json host_targets.metadata.<host>`.

use serde_json::Value;

// ── build_driver_args ──────────────────────────────────────────────────────

/// Build driver CLI args for hosts with custom resume/prompt syntax.
/// Returns `None` for hosts that don't override (cursor, opencode),
/// matching the trait default.
pub fn host_build_driver_args(
    host_id: &str,
    cwd: &str,
    prompt: Option<&str>,
    resume_target: Option<&str>,
    resume_mode: &str,
    resume_only: bool,
) -> Option<(Vec<String>, String)> {
    match host_id {
        "claude" => Some(build_claude_args(cwd, prompt, resume_target, resume_mode, resume_only)),
        "codex" => Some(build_codex_args(cwd, prompt, resume_target, resume_mode, resume_only)),
        _ => None,
    }
}

fn build_claude_args(
    _cwd: &str,
    prompt: Option<&str>,
    resume_target: Option<&str>,
    _resume_mode: &str,
    resume_only: bool,
) -> (Vec<String>, String) {
    let mut args = vec!["--print".to_string()];
    if resume_only {
        if let Some(target) = resume_target {
            args.push("--resume".to_string());
            args.push(target.to_string());
        }
    } else if let Some(p) = prompt {
        args.push("-p".to_string());
        args.push(p.to_string());
    }
    let shell_cmd = format!("claude {}", args.join(" "));
    (args, shell_cmd)
}

fn build_codex_args(
    cwd: &str,
    prompt: Option<&str>,
    resume_target: Option<&str>,
    resume_mode: &str,
    resume_only: bool,
) -> (Vec<String>, String) {
    let mut args = vec!["-C".to_string(), cwd.to_string()];
    if resume_only {
        args.push("resume".to_string());
        if let Some(target) = resume_target {
            if target == "last" || resume_mode == "last" {
                args.push("--last".to_string());
            } else {
                args.push(target.to_string());
            }
        } else {
            args.push("--last".to_string());
        }
    } else if let Some(p) = prompt {
        args.push(p.to_string());
    }
    let shell_cmd = format!("codex {}", args.join(" "));
    (args, shell_cmd)
}

// ── extract_observation_surfaces ───────────────────────────────────────────

/// Extract followup and additional_context surfaces from hook output JSON.
///
/// Each host's hook output has different JSON key conventions:
/// - Claude: 5-level fallback chain (`stopReason` -> `systemMessage` -> `followup_message` -> `message` -> `reason`)
/// - OpenCode: flat root-level `followup_message` + `additional_context` (no pointer)
/// - Others: use the `HostTelemetry` trait default (pointer to `/hookSpecificOutput/additionalContext` with `additional_context` fallback)
pub fn host_extract_observation_surfaces(
    host_id: &str,
    output: &Value,
) -> (Option<String>, Option<String>) {
    match host_id {
        "claude" => extract_claude_surfaces(output),
        "opencode" => extract_opencode_surfaces(output),
        _ => extract_default_surfaces(output),
    }
}

fn extract_claude_surfaces(output: &Value) -> (Option<String>, Option<String>) {
    let followup = output
        .get("stopReason")
        .or_else(|| output.get("systemMessage"))
        .or_else(|| output.get("followup_message"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| {
            output
                .get("message")
                .or_else(|| output.get("reason"))
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        });
    let additional = output
        .pointer("/hookSpecificOutput/additionalContext")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    (followup, additional)
}

fn extract_opencode_surfaces(output: &Value) -> (Option<String>, Option<String>) {
    let followup = output
        .get("followup_message")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let additional = output
        .get("additional_context")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    (followup, additional)
}

fn extract_default_surfaces(output: &Value) -> (Option<String>, Option<String>) {
    let followup = output
        .get("followup_message")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let additional = output
        .pointer("/hookSpecificOutput/additionalContext")
        .or_else(|| output.get("additional_context"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    (followup, additional)
}
