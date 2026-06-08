use super::types::DriverCommandSpec;

pub(crate) fn is_safe_worktree_slug(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub(crate) fn resolve_worktree_cwd(
    cwd: &str,
    worktree_name: Option<&str>,
    worktree_path: Option<&str>,
) -> String {
    if let Some(path) = worktree_path {
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            if p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return cwd.to_string();
            }
            return path.to_string();
        }
        let resolved = std::path::Path::new(cwd).join(p);
        if resolved
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return cwd.to_string();
        }
        return resolved.to_string_lossy().to_string();
    }
    if let Some(name) = worktree_name {
        if !is_safe_worktree_slug(name) {
            return cwd.to_string();
        }
        let wt_path = std::path::Path::new(cwd)
            .join(".claude/worktrees")
            .join(name);
        return wt_path.to_string_lossy().to_string();
    }
    cwd.to_string()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_driver_command(
    host: &str,
    cwd: &str,
    prompt: Option<String>,
    resume_target: Option<String>,
    resume_mode: &str,
    resume_only: bool,
    worktree_name: Option<String>,
    worktree_path: Option<String>,
) -> Result<DriverCommandSpec, String> {
    let effective_cwd = resolve_worktree_cwd(cwd, worktree_name.as_deref(), worktree_path.as_deref());
    let lowered = host.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "codex" | "codex-cli" => {
            let mut args = vec!["-C".to_string(), effective_cwd.clone()];
            if resume_only {
                args.push("resume".to_string());
                if let Some(target) = resume_target {
                    if target == "last" || resume_mode == "last" {
                        args.push("--last".to_string());
                    } else {
                        args.push(target);
                    }
                } else {
                    args.push("--last".to_string());
                }
            } else if let Some(prompt) = prompt {
                args.push(prompt);
            }
            Ok(DriverCommandSpec {
                driver_id: "codex_driver".to_string(),
                binary: "codex".to_string(),
                shell_command: shell_join("codex", &args),
                args,
                supports_resume: true,
            })
        }
        "claude" | "claude-code" => {
            let mut args = vec!["--print".to_string()];
            if resume_only {
                if let Some(target) = resume_target {
                    args.push("--resume".to_string());
                    args.push(target);
                }
            } else if let Some(ref p) = prompt {
                args.push("-p".to_string());
                args.push(p.clone());
            }
            Ok(DriverCommandSpec {
                driver_id: "claude_code_driver".to_string(),
                binary: "claude".to_string(),
                shell_command: shell_join("claude", &args),
                args,
                supports_resume: true,
            })
        }
        // Short-lived shell for §6.4 real-process smoke (`ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE`).
        "smoke" | "smoke-shell" => {
            let shell = if cfg!(unix) {
                "/bin/sh".to_string()
            } else {
                "sh".to_string()
            };
            let script = if resume_only {
                "echo smoke-resume"
            } else {
                "while true; do sleep 1; done"
            };
            let args = vec!["-c".to_string(), script.to_string()];
            Ok(DriverCommandSpec {
                driver_id: "smoke_shell_driver".to_string(),
                binary: shell.clone(),
                shell_command: shell_join(&shell, &args),
                args,
                supports_resume: false,
            })
        }
        other => Err(format!("Unsupported session supervisor host: {other}")),
    }
}

pub(crate) fn driver_id_for_host(host: &str) -> &'static str {
    match host.trim().to_ascii_lowercase().as_str() {
        "codex" | "codex-cli" => "codex_driver",
        "claude" | "claude-code" => "claude_code_driver",
        "smoke" | "smoke-shell" => "smoke_shell_driver",
        _ => "unknown_driver",
    }
}

pub(crate) fn default_resume_mode(_host: &str) -> &'static str {
    "last"
}

fn shell_join(binary: &str, args: &[String]) -> String {
    let mut parts = vec![shell_escape(binary)];
    parts.extend(args.iter().map(|arg| shell_escape(arg)));
    parts.join(" ")
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=+".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}
