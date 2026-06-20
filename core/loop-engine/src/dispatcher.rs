use crate::types::{LoopAction, LoopError};
use crate::kill_switch::{is_kill_signal_active, clear_kill_signal};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Default timeout in seconds for subagent action execution (10 minutes).
pub const DEFAULT_ACTION_TIMEOUT_SECS: u64 = 600;
/// Interval in seconds between kill signal polls and subagent process checks.
pub const KILL_POLL_INTERVAL_SECS: u64 = 5;

/// Result of a subagent action execution, wrapping success/failure status and stdout/stderr output.
pub struct SubagentResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl SubagentResult {
    /// Build a `SubagentResult` from a `std::process::Output` reference.
    pub fn from_output(output: &std::process::Output) -> Self {
        SubagentResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }
}

/// Build a subagent handoff message from an action definition.
/// The handoff includes the action description, scope constraints, closeout instructions, and kill-signal path.
pub fn build_handoff(action: &LoopAction, loop_id: &str, run_id: &str) -> String {
    let scope_display = if action.scope_paths.is_empty() {
        "all files".to_string()
    } else {
        action.scope_paths.join(", ")
    };

    format!(
        "## Objective\n\
         {desc}\n\n\
         ## Scope (HARD)\n\
         - Write scope: {scope}\n\
         - Forbidden: 不得修改 scope 外的任何文件\n\n\
         ## Action\n\
         - 文件修改 + 运行验证命令\n\n\
         ## Closeout\n\
         - 写入 changed_files\n\
         - 运行验证命令并记录输出\n\
         - 写入 evidence 到 artifacts/loop/{loop_id}/evidence/{action_id}/\n\
         - 结果写入 artifacts/loop/{loop_id}/closeout/{run_id}-{action_id}.json\n\n\
         ## Safety\n\
         - 每 10000 tokens 检查 kill 信号（软防线）\n\
         - Kill 信号文件: .loop-kill/{loop_id}",
        desc = action.description.as_deref().unwrap_or(&action.action_type),
        scope = scope_display,
        action_id = action.action_id,
        loop_id = loop_id,
        run_id = run_id,
    )
}

/// Resolve the subagent binary path from `ROUTER_RS_SUBAGENT_BIN` env var or `which opencode`.
/// Returns an error if neither source yields a valid binary path.
///
/// # Platform dependency
/// The fallback uses the `which` command, which is available on Unix/macOS by default
/// and on Windows when Git for Windows or similar tooling is installed (as `where.exe`).
/// On bare Windows without these tools, this fallback will fail. Always set
/// `ROUTER_RS_SUBAGENT_BIN` on Windows for reliable binary resolution.
pub fn resolve_subagent_binary() -> Result<String, LoopError> {
    if let Ok(bin) = std::env::var("ROUTER_RS_SUBAGENT_BIN")
        && !bin.is_empty() {
            return Ok(bin);
        }
    let output = Command::new("which")
        .arg("opencode")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
        _ => {}
    }
    Err(LoopError::SpawnFailed(
        "opencode binary not found. Set ROUTER_RS_SUBAGENT_BIN or ensure opencode is in PATH.".to_string(),
    ))
}

/// Execute a single action synchronously through a subagent process, with kill-signal and timeout support.
pub fn run_action_sync(
    repo_root: &Path,
    loop_id: &str,
    run_id: &str,
    action: &LoopAction,
    timeout: Option<Duration>,
) -> Result<SubagentResult, LoopError> {
    let handoff = build_handoff(action, loop_id, run_id);
    let binary = resolve_subagent_binary()?;
    let timeout_duration = timeout.unwrap_or(Duration::from_secs(DEFAULT_ACTION_TIMEOUT_SECS));

    let mut child = Command::new(&binary)
        .args(["-p", &handoff])
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| LoopError::SpawnFailed(format!("{binary}: {e}")))?;

    let deadline = Instant::now() + timeout_duration;
    let poll_interval = Duration::from_secs(KILL_POLL_INTERVAL_SECS);

    loop {
        match child.try_wait().map_err(|e| LoopError::Io(format!("try_wait: {e}")))? {
            Some(_status) => {
                let output = child.wait_with_output()
                    .map_err(|e| LoopError::Io(format!("collect output: {e}")))?;
                return Ok(SubagentResult {
                    success: output.status.success(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }
            None => {
                if is_kill_signal_active(repo_root, loop_id) {
                    let _ = clear_kill_signal(repo_root, loop_id);
                    child.kill().map_err(|e| LoopError::Io(format!("kill: {e}")))?;
                    child.wait().map_err(|e| LoopError::Io(format!("wait after kill: {e}")))?;
                    return Err(LoopError::KillSignaled(format!(
                        "action {} killed by loop {} signal",
                        action.action_id, loop_id,
                    )));
                }
                if Instant::now() > deadline {
                    child.kill().map_err(|e| LoopError::Io(format!("kill timeout: {e}")))?;
                    child.wait().map_err(|e| LoopError::Io(format!("wait after timeout: {e}")))?;
                    return Err(LoopError::Timeout(timeout_duration.as_secs()));
                }
                thread::sleep(poll_interval);
            }
        }
    }
}

/// Generate a dry-run description string for an action (no subagent is launched).
pub fn run_action_dry_run(action: &LoopAction, loop_id: &str, run_id: &str) -> String {
    let handoff = build_handoff(action, loop_id, run_id);
    format!(
        "[dry-run] action={} type={} scope={:?}\n\n{}",
        action.action_id,
        action.action_type,
        action.scope_paths,
        handoff,
    )
}

/// Check that modified tracked files are within the allowed scope paths.
/// Returns a list of file paths that violate the scope constraint.
pub fn check_scope_compliance(
    repo_root: &Path,
    scope_paths: &[String],
) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=ACMR"])
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let changes: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if scope_paths.is_empty() {
                return Vec::new();
            }
            changes.into_iter()
                .filter(|f| !scope_paths.iter().any(|s| f.starts_with(s)))
                .collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LoopAction;

    fn make_action(id: &str) -> LoopAction {
        LoopAction {
            action_id: id.to_string(),
            action_type: "fix".to_string(),
            scope_paths: vec!["src/main.rs".to_string()],
            safety: "L2".to_string(),
            description: Some("fix deprecation".to_string()),
        }
    }

    #[test]
    fn test_build_handoff_contains_scope() {
        let action = make_action("a1");
        let handoff = build_handoff(&action, "test-loop", "run-1");
        assert!(handoff.contains("src/main.rs"));
        assert!(handoff.contains("a1"));
        assert!(handoff.contains("Scope (HARD)"));
        assert!(handoff.contains("test-loop"));
        assert!(handoff.contains("run-1"));
    }

    #[test]
    fn test_build_handoff_no_scope() {
        let mut action = make_action("a2");
        action.scope_paths = Vec::new();
        let handoff = build_handoff(&action, "test-loop", "run-1");
        assert!(handoff.contains("all files"));
    }

    #[test]
    fn test_run_action_dry_run() {
        let action = make_action("a1");
        let output = run_action_dry_run(&action, "test-loop", "run-1");
        assert!(output.contains("[dry-run]"));
        assert!(output.contains("a1"));
    }

    #[test]
    fn test_resolve_subagent_binary_env() {
        unsafe {
            std::env::set_var("ROUTER_RS_SUBAGENT_BIN", "/usr/bin/fake-opencode");
        }
        let result = resolve_subagent_binary();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/usr/bin/fake-opencode");
        unsafe {
            std::env::remove_var("ROUTER_RS_SUBAGENT_BIN");
        }
    }
}
