//! Host-specific capability overrides organized by capability, not by host.
//!
//! Only `build_driver_args` remains — it contains per-host CLI argument syntax
//! that cannot be expressed as simple registry templates (Codex has conditional
//! resume logic). All other overrides have been moved to registry-driven code
//! generation (observation surfaces, dispatcher config, etc.).

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
