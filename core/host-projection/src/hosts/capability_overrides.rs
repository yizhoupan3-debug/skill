//! Host-specific capability overrides organized by capability, not by host.
//!
//! Only `build_driver_args` remains — it contains per-host CLI argument syntax
//! that cannot be expressed as simple registry templates (Codex has conditional
//! resume logic). All other overrides have been moved to registry-driven code
//! generation (observation surfaces, dispatcher config, etc.).

// ── build_driver_args ──────────────────────────────────────────────────────

/// Build driver CLI args for any host.
/// Called from build.rs generated code via `self.host_id()` dispatch.
/// The binary name is read from the runtime registry (driver_binary field).
pub fn build_driver_args(
    host_id: &str,
    cwd: &str,
    prompt: Option<&str>,
    resume_target: Option<&str>,
    resume_mode: &str,
    resume_only: bool,
) -> Option<(Vec<String>, String)> {
    let binary_name = framework_core::runtime_registry::host_driver_binary(host_id);
    match host_id {
        // Codex: exec [--cd <cwd>] <prompt> / exec resume [--cd <cwd>] [--last|<session-id>]
        "codex" => {
            let mut args: Vec<String> = Vec::new();
            if resume_only {
                args.push("exec".to_string());
                args.push("resume".to_string());
                args.push("--cd".to_string());
                args.push(cwd.to_string());
                match resume_target {
                    Some(t) if resume_mode == "last" || t == "last" => {
                        args.push("--last".to_string());
                    }
                    Some(t) => args.push(t.to_string()),
                    None => args.push("--last".to_string()),
                }
            } else if let Some(p) = prompt {
                args.push("exec".to_string());
                args.push("--cd".to_string());
                args.push(cwd.to_string());
                args.push(p.to_string());
            }
            let shell_cmd = format!("{binary_name} {}", args.join(" "));
            Some((args, shell_cmd))
        }
        // Claude: --print [-p <prompt>] / --print [--continue | --resume <target>]
        "claude" => {
            let mut args = vec!["--print".to_string()];
            if resume_only {
                match resume_target {
                    Some(t) if resume_mode == "last" || t == "last" => {
                        args.push("--continue".to_string());
                    }
                    Some(t) => {
                        args.push("--resume".to_string());
                        args.push(t.to_string());
                    }
                    None => args.push("--continue".to_string()),
                }
            } else if let Some(p) = prompt {
                args.push("-p".to_string());
                args.push(p.to_string());
            }
            let shell_cmd = format!("{binary_name} {}", args.join(" "));
            Some((args, shell_cmd))
        }
        // OpenCode: run <prompt> / run [-c | -s <session-id>]
        "opencode" => {
            let mut args = vec!["run".to_string()];
            if resume_only {
                match resume_target {
                    Some(t) if resume_mode == "last" || t == "last" => {
                        args.push("-c".to_string());
                    }
                    Some(t) => {
                        args.push("-s".to_string());
                        args.push(t.to_string());
                    }
                    None => args.push("-c".to_string()),
                }
            } else if let Some(p) = prompt {
                args.push(p.to_string());
            }
            let shell_cmd = format!("{binary_name} {}", args.join(" "));
            Some((args, shell_cmd))
        }
        _ => {
            tracing::warn!(
                "build_driver_args: unknown host_id={host_id}, no driver args built"
            );
            None
        }
    }
}
