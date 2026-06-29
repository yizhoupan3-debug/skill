use crate::types::{LoopAction, LoopError};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Default timeout in seconds for subagent action execution (10 minutes).
pub const DEFAULT_ACTION_TIMEOUT_SECS: u64 = 600;
/// Interval in seconds between kill signal polls and subagent process checks.
pub const KILL_POLL_INTERVAL_SECS: u64 = 5;

/// Global semaphore guarding concurrent subagent OS process count.
fn subagent_semaphore() -> &'static Mutex<u32> {
    static SEM: std::sync::OnceLock<Mutex<u32>> = std::sync::OnceLock::new();
    SEM.get_or_init(|| Mutex::new(crate::env_flags::max_concurrent_procs()))
}

/// RAII guard: acquires a subagent permit on construction, releases on drop.
/// Blocks with backoff sleep until capacity is available.
struct SubagentPermit<'a> {
    sem: &'a Mutex<u32>,
}

impl<'a> SubagentPermit<'a> {
    fn acquire(sem: &'a Mutex<u32>) -> Self {
        let mut backoff_ms: u64 = 50;
        let mut poison_retries: u32 = 0;
        loop {
            match sem.lock() {
                Ok(mut count) => {
                    poison_retries = 0; // Healthy lock — reset poison counter.
                    if *count > 0 {
                        *count -= 1;
                        return Self { sem };
                    }
                    drop(count);
                }
                Err(poisoned) => {
                    poison_retries += 1;
                    if poison_retries >= 10 {
                        panic!(
                            "SubagentPermit semaphore permanently poisoned after {} retries",
                            poison_retries,
                        );
                    }
                    // Recover the locked value through the poison wrapper.
                    let mut count = poisoned.into_inner();
                    if *count > 0 {
                        *count -= 1;
                        return Self { sem };
                    }
                    drop(count);
                }
            }
            thread::sleep(Duration::from_millis(backoff_ms));
            backoff_ms = (backoff_ms * 2).min(2000);
        }
    }
}

impl Drop for SubagentPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut count) = self.sem.lock().map_err(|e| e.into_inner()) {
            *count += 1;
        }
    }
}

/// Result of a subagent action execution, wrapping success/failure status and stdout/stderr output.
pub struct SubagentResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Poll a subprocess until completion, kill signal, or deadline.
///
/// Eliminates the repeated try_wait → kill_signal → deadline poll loop
/// that was duplicated between `runner.rs` (discovery) and `dispatcher.rs` (action execution).
///
/// Returns `Ok(output)` on natural completion, `Err(KillSignaled)` when the
/// loop's kill signal fires, or `Err(Timeout)` when the deadline is reached.
pub(crate) fn poll_subprocess(
    mut child: std::process::Child,
    repo_root: &Path,
    loop_id: &str,
    label: &str,
    deadline: Instant,
    timeout_duration: Duration,
) -> Result<std::process::Output, LoopError> {
    loop {
        match child
            .try_wait()
            .map_err(|e| LoopError::Io(format!("{label} try_wait: {e}")))?
        {
            Some(_status) => {
                return child
                    .wait_with_output()
                    .map_err(|e| LoopError::Io(format!("{label} collect: {e}")));
            }
            None => {
                if match crate::kill_switch::take_kill_signal(repo_root, loop_id) {
                    Ok(signaled) => signaled,
                    Err(e) => {
                        tracing::warn!(%loop_id, error = %e, "kill_switch IO error — treating as no signal");
                        false
                    }
                } {
                    child
                        .kill()
                        .map_err(|e| LoopError::Io(format!("{label} kill: {e}")))?;
                    child
                        .wait()
                        .map_err(|e| LoopError::Io(format!("{label} wait: {e}")))?;
                    return Err(LoopError::KillSignaled(format!(
                        "{label} killed by loop {loop_id} signal",
                    )));
                }
                if Instant::now() > deadline {
                    child
                        .kill()
                        .map_err(|e| LoopError::Io(format!("{label} kill timeout: {e}")))?;
                    child
                        .wait()
                        .map_err(|e| LoopError::Io(format!("{label} wait timeout: {e}")))?;
                    return Err(LoopError::Timeout(timeout_duration.as_secs()));
                }
                thread::sleep(Duration::from_secs(KILL_POLL_INTERVAL_SECS));
            }
        }
    }
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

/// Escape special characters in handoff text to prevent injection through
/// action descriptions or scope paths that contain newlines or backslashes.
fn sanitize_handoff_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Build a subagent handoff message from an action definition.
/// The handoff includes the action description, scope constraints, closeout instructions, and kill-signal path.
pub fn build_handoff(action: &LoopAction, loop_id: &str, run_id: &str) -> String {
    let scope_display = if action.scope_paths.is_empty() {
        "all files".to_string()
    } else {
        sanitize_handoff_text(&action.scope_paths.join(", "))
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
        desc = sanitize_handoff_text(action.description.as_deref().unwrap_or(&action.action_type)),
        scope = scope_display,
        action_id = sanitize_handoff_text(&action.action_id),
        loop_id = loop_id,
        run_id = run_id,
    )
}

/// Apply process resource limits via setrlimit in the forked child (pre_exec).
/// Prevents runaway subprocesses from exhausting system resources.
/// Delegates to the shared implementation in `fr-utils`.
///
/// # Safety level equality
/// Resource limits are applied uniformly across all safety levels (L1, L2, L3).
/// There is no per-safety-level differentiation (e.g., stricter CPU/memory limits
/// for L3 unattended actions). All subagents — discovery, dispatch, and barrier
/// escalation — receive the same rlimit profile. If per-level limits are needed,
/// introduce a `safety_level` parameter and route to level-specific limit profiles.
#[cfg(unix)]
pub(crate) fn apply_subprocess_rlimits() -> Result<(), std::io::Error> {
    fr_utils::process_utils::apply_subprocess_rlimits()
}

#[cfg(not(unix))]
fn apply_subprocess_rlimits() -> Result<(), std::io::Error> {
    Ok(())
}

/// Resolve the subagent binary path from `ROUTER_RS_SUBAGENT_BIN` env var.
/// Returns an error if the env var is not set or is empty.
pub fn resolve_subagent_binary() -> Result<String, LoopError> {
    crate::env_flags::subagent_binary().map_err(|e| LoopError::SpawnFailed(e.to_string()))
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

    // Acquire a global concurrency permit before spawning the OS process.
    let _permit = SubagentPermit::acquire(subagent_semaphore());

    let mut cmd = Command::new(&binary);
    cmd.args(["-p", &handoff])
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: pre_exec runs in single-threaded forked child; setrlimit is async-signal-safe.
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| apply_subprocess_rlimits());
    }
    let child = cmd
        .spawn()
        .map_err(|e| LoopError::SpawnFailed(format!("{binary}: {e}")))?;

    let deadline = Instant::now() + timeout_duration;

    let output = poll_subprocess(
        child,
        repo_root,
        loop_id,
        &action.action_id,
        deadline,
        timeout_duration,
    )?;
    Ok(SubagentResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Generate a dry-run description string for an action (no subagent is launched).
pub fn run_action_dry_run(action: &LoopAction, loop_id: &str, run_id: &str) -> String {
    let handoff = build_handoff(action, loop_id, run_id);
    format!(
        "[dry-run] action={} type={} scope={:?}\n\n{}",
        action.action_id, action.action_type, action.scope_paths, handoff,
    )
}

/// Time-to-live for the cached `git diff` result (seconds).
///
/// # TTL window limitation
/// The 10-second TTL is a trade-off between responsiveness and freshness.
/// In tight poll loops (e.g., multi-action verification), this prevents
/// spawning `git diff` on every call. However, during the TTL window,
/// newly modified files are not detected. Extending this window increases
/// the risk of stale scope compliance checks. For stricter freshness,
/// reduce the TTL or bypass the cache entirely.
const GIT_DIFF_CACHE_TTL_SECS: u64 = 10;

/// Check that modified tracked files are within the allowed scope paths.
/// Returns a list of file paths that violate the scope constraint.
///
/// Results are cached for [`GIT_DIFF_CACHE_TTL_SECS`] seconds to avoid
/// spawning `git diff` on every call in a tight poll loop.
pub fn check_scope_compliance(repo_root: &Path, scope_paths: &[String]) -> Vec<String> {
    let changes = resolve_cached_git_diff(repo_root);
    if scope_paths.is_empty() || changes.is_empty() {
        return Vec::new();
    }
    changes
        .into_iter()
        .filter(|f| !scope_paths.iter().any(|s| f.starts_with(s)))
        .collect()
}

/// Cache entry for git diff output — (cached_at, repo_root, changes).
type GitDiffCacheEntry = (std::time::Instant, std::path::PathBuf, Vec<String>);

/// Cache-backed git diff resolution — spawns `git diff` at most once per TTL.
fn resolve_cached_git_diff(repo_root: &Path) -> Vec<String> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<GitDiffCacheEntry>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());

    if let Some((cached_at, cached_root, cached_changes)) = guard.as_ref()
        && cached_root.as_os_str() == repo_root.as_os_str()
        && cached_at.elapsed() < std::time::Duration::from_secs(GIT_DIFF_CACHE_TTL_SECS)
    {
        return cached_changes.clone();
    }

    // Cache miss — run git diff.
    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=ACMR"])
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let changes: Vec<String> = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    };
    *guard = Some((
        std::time::Instant::now(),
        repo_root.to_path_buf(),
        changes.clone(),
    ));
    changes
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
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
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe {
            core_state_utils::env_sync::set_env("ROUTER_RS_SUBAGENT_BIN", "/usr/bin/fake-opencode");
        }
        let result = resolve_subagent_binary();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/usr/bin/fake-opencode");
        // SAFETY: test-only; no other thread reads/writes env concurrently in this test context.
        unsafe {
            core_state_utils::env_sync::remove_env("ROUTER_RS_SUBAGENT_BIN");
        }
    }
}
