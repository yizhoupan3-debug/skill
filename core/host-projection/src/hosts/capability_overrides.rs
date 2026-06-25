//! Host-specific capability overrides organized by capability, not by host.
//!
//! Only `build_driver_args` remains — it contains per-host CLI argument syntax
//! that cannot be expressed as simple registry templates (Codex has conditional
//! resume logic). All other overrides have been moved to registry-driven code
//! generation (observation surfaces, dispatcher config, etc.).


// ── build_driver_args ──────────────────────────────────────────────────────

/// Build driver CLI args for claude host.
/// Called directly from build.rs generated code (no central dispatch).
pub fn build_claude_driver_args(
    _cwd: &str,
    prompt: Option<&str>,
    resume_target: Option<&str>,
    _resume_mode: &str,
    resume_only: bool,
) -> Option<(Vec<String>, String)> {
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
    Some((args, shell_cmd))
}

/// Build driver CLI args for codex host.
/// Called directly from build.rs generated code (no central dispatch).
pub fn build_codex_driver_args(
    cwd: &str,
    prompt: Option<&str>,
    resume_target: Option<&str>,
    resume_mode: &str,
    resume_only: bool,
) -> Option<(Vec<String>, String)> {
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
    Some((args, shell_cmd))
}
