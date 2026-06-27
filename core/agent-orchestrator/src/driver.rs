use crate::hooks;
use crate::types::DriverCommandSpec;

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
            // Resolve symlinks via canonicalize before checking for path
            // traversal. If canonicalize fails, fall back to the original
            // path for the ParentDir check.
            let check = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            if check
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return cwd.to_string();
            }
            return path.to_string();
        }
        let resolved = std::path::Path::new(cwd).join(p);
        let check =
            std::fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());
        if check
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
            .join(std::env::var("ROUTER_RS_WORKTREE_DIR").unwrap_or_else(|_| ".router-rs/worktrees".to_string()))
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
) -> Result<DriverCommandSpec, String> {
    let effective_cwd =
        resolve_worktree_cwd(cwd, worktree_name.as_deref(), worktree_path.as_deref());

    // Try trait-based dispatch via host provider registry (via hooks).
    if let Some(h) = hooks::hooks()
        && let Some(result) = (h.build_driver_command)(
            host,
            &effective_cwd,
            prompt.clone(),
            resume_target.clone(),
            resume_mode,
            resume_only,
            worktree_name.clone(),
            worktree_path.clone(),
        ) {
            return result;
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
        other => {
            tracing::warn!(
                "Unknown orchestrator host: \"{other}\" — using placeholder driver spec. \
                 Check that the host provider is registered via hooks."
            );
            // Placeholder spec so state management (launch/terminate/list) can proceed
            // in tests and dry-run mode. Actual driver dispatch happens at the
            // host projection layer via hooks.
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
            Ok(DriverCommandSpec {
                driver_id: format!("{other}_driver"),
                binary: shell.clone(),
                shell_command: shell_join(&shell, &args),
                args,
                supports_resume: false,
            })
        }
    }
}

pub fn driver_id_for_host(host: &str) -> &'static str {
    if let Some(h) = hooks::hooks()
        && let Some(id) = (h.driver_id_for_host)(host) {
            return id;
        }
    "unknown_driver"
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
