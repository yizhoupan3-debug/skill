use super::types::DriverCommandSpec;

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
pub fn build_driver_command(
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

    // Try trait-based dispatch via host provider registry.
    if let Some(provider) = crate::hosts::host_provider_for_routing_spelling(host) {
        let binary = provider.driver_binary();
        if !binary.is_empty() {
            if let Some((args, shell_command)) = provider.build_driver_args(
                &effective_cwd,
                prompt.as_deref(),
                resume_target.as_deref(),
                resume_mode,
                resume_only,
            ) {
                return Ok(DriverCommandSpec {
                    driver_id: provider.session_supervisor_driver().to_string(),
                    binary: binary.to_string(),
                    shell_command,
                    args,
                    supports_resume: provider.driver_supports_resume(),
                });
            }
        }
    }

    // Fallback: smoke-shell test host (not in provider registry).
    let lowered = host.trim().to_ascii_lowercase();
    match lowered.as_str() {
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

pub fn driver_id_for_host(host: &str) -> &'static str {
    crate::hosts::host_provider_for_routing_spelling(host)
        .map(|p| p.session_supervisor_driver())
        .unwrap_or("unknown_driver")
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
