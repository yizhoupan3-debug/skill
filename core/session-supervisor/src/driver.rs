use crate::types::DriverCommandSpec;
use core_errors::FrameworkError;

pub fn is_safe_worktree_slug(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn resolve_worktree_cwd(
    cwd: &str,
    worktree_name: Option<&str>,
    worktree_path: Option<&str>,
) -> String {
    if let Some(path) = worktree_path {
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            // Check for path traversal in the ORIGINAL path BEFORE canonicalize,
            // which resolves ParentDir components away and renders the check useless.
            if p
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return cwd.to_string();
            }
            // Resolve symlinks; if canonicalize fails, use the original path
            // (which already passed the ParentDir check above).
            let resolved =
                std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            return resolved.to_string_lossy().to_string();
        }
        let raw = std::path::Path::new(cwd).join(p);
        // Check for ParentDir before canonicalize resolves it away.
        if raw
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return cwd.to_string();
        }
        let resolved =
            std::fs::canonicalize(&raw).unwrap_or_else(|_| raw);
        return resolved.to_string_lossy().to_string();
    }
    if let Some(name) = worktree_name {
        if !is_safe_worktree_slug(name) {
            return cwd.to_string();
        }
        let wt_path = std::path::Path::new(cwd)
            .join(
                std::env::var("ROUTER_RS_WORKTREE_DIR")
                    .unwrap_or_else(|_| ".router-rs/worktrees".to_string()),
            )
            .join(name);
        return wt_path.to_string_lossy().to_string();
    }
    cwd.to_string()
}

pub fn build_driver_command(
    host: &str,
    cwd: &str,
    prompt: Option<String>,
    resume_target: Option<String>,
    resume_mode: &str,
    resume_only: bool,
    worktree_name: Option<String>,
    worktree_path: Option<String>,
) -> Result<DriverCommandSpec, FrameworkError> {
    let _effective_cwd =
        resolve_worktree_cwd(cwd, worktree_name.as_deref(), worktree_path.as_deref());

    // Host-specific driver command building.
    let lowered = host.trim().to_ascii_lowercase();
    let (binary, args, driver_id, supports_resume) = match lowered.as_str() {
        "codex" => {
            let mut args: Vec<String> = Vec::new();
            if resume_only {
                args.push("exec".to_string());
                args.push("resume".to_string());
                args.push("--cd".to_string());
                args.push(cwd.to_string());
                match resume_target {
                    Some(ref t) if resume_mode == "last" || t == "last" => {
                        args.push("--last".to_string());
                    }
                    Some(ref t) => args.push(t.clone()),
                    None => args.push("--last".to_string()),
                }
            } else if let Some(p) = prompt {
                args.push("exec".to_string());
                args.push("--cd".to_string());
                args.push(cwd.to_string());
                args.push(p);
            }
            ("codex".to_string(), args, "codex_driver".to_string(), true)
        }
        "claude" => {
            let mut args = vec!["--print".to_string()];
            if resume_only {
                match resume_target {
                    Some(ref t) if resume_mode == "last" || t == "last" => {
                        args.push("--continue".to_string());
                    }
                    Some(ref t) => {
                        args.push("--resume".to_string());
                        args.push(t.clone());
                    }
                    None => args.push("--continue".to_string()),
                }
            } else if let Some(p) = prompt {
                args.push("-p".to_string());
                args.push(p);
            }
            ("claude".to_string(), args, "claude_driver".to_string(), true)
        }
        "opencode" => {
            let mut args = vec!["run".to_string()];
            if resume_only {
                match resume_target {
                    Some(ref t) if resume_mode == "last" || t == "last" => {
                        args.push("-c".to_string());
                    }
                    Some(ref t) => {
                        args.push("-s".to_string());
                        args.push(t.clone());
                    }
                    None => args.push("-c".to_string()),
                }
            } else if let Some(p) = prompt {
                args.push(p);
            }
            ("opencode".to_string(), args, "opencode_driver".to_string(), true)
        }
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
            return Ok(DriverCommandSpec {
                driver_id: "smoke_shell_driver".to_string(),
                binary: shell.clone(),
                shell_command: format!(
                    "/bin/sh -c '{}'",
                    script.replace('\'', "'\"'\"'")
                ),
                args,
                supports_resume: false,
            });
        }
        other => {
            tracing::warn!(
                "Unknown session supervisor host: \"{other}\" — using placeholder driver spec. \
                 Check that the host provider is registered via hooks."
            );
            let shell = if cfg!(unix) {
                "/bin/sh".to_string()
            } else {
                "sh".to_string()
            };
            let script = if resume_only {
                "echo resume-placeholder"
            } else {
                "echo launch-placeholder"
            };
            let args = vec!["-c".to_string(), script.to_string()];
            return Ok(DriverCommandSpec {
                driver_id: format!("{other}_driver"),
                binary: shell.clone(),
                shell_command: shell_join(&shell, &args),
                args,
                supports_resume: false,
            });
        }
    };

    let shell_command = shell_join(&binary, &args);
    Ok(DriverCommandSpec {
        driver_id,
        binary,
        shell_command,
        args,
        supports_resume,
    })
}

pub fn driver_id_for_host(host: &str) -> &'static str {
    match host.trim().to_ascii_lowercase().as_str() {
        "codex" => "codex_driver",
        "claude" => "claude_driver",
        "opencode" => "opencode_driver",
        "smoke" | "smoke-shell" => "smoke_shell_driver",
        _ => "unknown_driver",
    }
}

pub fn default_resume_mode(_host: &str) -> &'static str {
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
