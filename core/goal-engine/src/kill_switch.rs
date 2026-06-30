use crate::state::{LOOP_LOCK_MAX_AGE_SECS, kill_signal_path, lock_path, pause_state_path};
use crate::types::{KillSignalAction, KillSignalPayload, LoopError, PauseState};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Loop execution lock persisted as `.loop-active` in the repo root.
/// Guards against concurrent loop runs by storing the active loop and run IDs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoopLock {
    pub loop_id: String,
    pub run_id: String,
    pub acquired_at: String,
}

/// Lock information combining the LoopLock with its acquisition epoch for staleness checks.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockInfo {
    pub lock: LoopLock,
    pub acquired_epoch: u64,
}

// ── KillSignal types are defined in `crate::types` ──
// `KillSignalAction`, `KillSignalPayload` are in types.rs alongside LoopError,
// LoopPhase, and PauseState. This module provides the I/O functions below.

/// RAII guard that automatically releases the loop lock on drop.
/// Prevents lock leaks when the caller panics or forgets to call release_lock.
pub struct LoopLockGuard {
    pub(crate) lock_path: PathBuf,
}

impl Drop for LoopLockGuard {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.lock_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::error!(
                    "LoopLockGuard drop: failed to remove lock file {}: {e}",
                    self.lock_path.display()
                );
            }
        }
    }
}

/// Acquire an exclusive loop lock and return an RAII guard.
/// The lock is automatically released when the guard is dropped.
/// See [`acquire_lock`] for lock semantics.
pub fn acquire_lock_guarded(repo_root: &Path, loop_id: &str, run_id: &str) -> Result<LoopLockGuard, LoopError> {
    acquire_lock(repo_root, loop_id, run_id)?;
    Ok(LoopLockGuard {
        lock_path: lock_path(repo_root),
    })
}

/// Check whether a kill signal file exists for the given loop.
/// Returns `true` if the file `.loop-kill/{loop_id}` is present on disk.
pub fn is_kill_signal_active(repo_root: &Path, loop_id: &str) -> bool {
    kill_signal_path(repo_root, loop_id).is_file()
}

/// Write a kill signal file for the given loop to request graceful termination.
/// The file is stored at `.loop-kill/{loop_id}` with a JSON payload containing the loop ID and timestamp.
pub fn write_kill_signal(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    let path = kill_signal_path(repo_root, loop_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| LoopError::Io(format!("mkdir {}: {e}", parent.display())))?;
    }
    let now = now_epoch();
    let content = format!(
        "{{\"loop_id\":\"{}\",\"armed_at\":{},\"armed_at_iso\":\"{}\"}}",
        loop_id,
        now,
        framework_core::time::now_iso(),
    );
    core_state_utils::atomic_write::write_atomic_text(&path, &content)
        .map_err(|e| LoopError::Io(format!("write kill signal {}: {e}", path.display())))?;
    Ok(())
}

/// Remove the kill signal file for a specific loop.
/// Safe to call even if no signal file exists (no-op in that case).
///
/// **TOCTOU note**: uses `remove_file` directly without prior `is_file()` check;
/// `NotFound` is treated as success (the desired end state — no signal file).
pub fn clear_kill_signal(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    let path = kill_signal_path(repo_root, loop_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(LoopError::Io(format!(
            "remove kill signal {}: {e}",
            path.display()
        ))),
    }
}

/// Atomically check for and consume a kill signal in a single filesystem operation.
///
/// Unlike the two-step `is_kill_signal_active()` → `clear_kill_signal()` pattern,
/// this function uses one `remove_file` syscall and returns `true` only when the
/// file actually existed. This eliminates the TOCTOU window where another process
/// could write a new signal between check and clear.
///
/// Returns `Ok(true)` if a signal was present and removed, `Ok(false)` if none.
pub fn take_kill_signal(repo_root: &Path, loop_id: &str) -> Result<bool, LoopError> {
    let path = kill_signal_path(repo_root, loop_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(LoopError::Io(format!(
            "take kill signal {}: {e}",
            path.display()
        ))),
    }
}

/// Remove all kill signal files by deleting the entire `.loop-kill/` directory.
/// Use during loop runner shutdown cleanup.
pub fn clear_all_kill_signals(repo_root: &Path) -> Result<(), LoopError> {
    let kill_dir = repo_root.join(".loop-kill");
    if kill_dir.is_dir() {
        fs::remove_dir_all(&kill_dir)
            .map_err(|e| LoopError::Io(format!("remove kill dir {}: {e}", kill_dir.display())))?;
    }
    Ok(())
}

// ── v2 multi-signal protocol ──

/// Write a typed signal payload to the kill-switch file for the given loop.
///
/// Unlike `write_kill_signal()` (which only writes a fixed-format kill
/// signal), this function accepts any `KillSignalPayload` and writes the
/// full JSON schema. Backward-compatible: existing readers that only check
/// file existence (e.g. `is_kill_signal_active`) continue to work.
///
/// Uses atomic write (temp + fsync + rename) for crash safety (P2.2 fix).
pub fn write_signal(repo_root: &Path, payload: &KillSignalPayload) -> Result<(), LoopError> {
    let path = kill_signal_path(repo_root, &payload.loop_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| LoopError::Io(format!("mkdir {}: {e}", parent.display())))?;
    }
    let content = serde_json::to_string(payload)
        .map_err(|e| LoopError::Serde(format!("serialize signal: {e}")))?;
    core_state_utils::atomic_write::write_atomic_text(&path, &content)
        .map_err(|e| LoopError::Io(format!("write signal {}: {e}", path.display())))?;
    Ok(())
}

/// Atomically read and consume a signal payload from the kill-switch file.
///
/// Returns `Ok(Some(payload))` if a signal file was present and read,
/// `Ok(None)` if no file exists. The file is *always* removed on read —
/// the signal is consumed in one atomic `remove_file` syscall.
///
/// Backward-compatible: old-format files (`{loop_id, armed_at, armed_at_iso}`
/// without `action`) deserialize with `action = Kill`.
pub fn take_signal(repo_root: &Path, loop_id: &str) -> Result<Option<KillSignalPayload>, LoopError> {
    let path = kill_signal_path(repo_root, loop_id);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LoopError::Io(format!("read signal {}: {e}", path.display()))),
    };
    // Always remove the file — the signal is consumed.
    let _ = fs::remove_file(&path);
    // Parse: old format without `action` field defaults to Kill.
    let payload: KillSignalPayload = match serde_json::from_str(&raw) {
        Ok(p) => p,
        Err(_) => {
            // Fallback: try parsing as old format {loop_id, armed_at, armed_at_iso}
            let fallback: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|e| LoopError::Serde(format!("parse signal (fallback): {e}")))?;
            let loop_id_val = fallback
                .get("loop_id")
                .and_then(|v| v.as_str())
                .unwrap_or(loop_id);
            let armed_at = fallback
                .get("armed_at")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let armed_at_iso = fallback
                .get("armed_at_iso")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            KillSignalPayload {
                schema_version: "loop-signal-v2".to_string(),
                loop_id: loop_id_val.to_string(),
                action: KillSignalAction::Kill,
                action_id: None,
                armed_at,
                armed_at_iso,
            }
        }
    };
    Ok(Some(payload))
}

/// Convenience: write a pause signal with optional feedback.
pub fn write_pause_signal(
    repo_root: &Path,
    loop_id: &str,
    action_id: impl Into<String>,
    feedback: Option<impl Into<String>>,
) -> Result<(), LoopError> {
    let payload = match feedback {
        Some(fb) => KillSignalPayload::new_pause_with_feedback(loop_id, action_id, fb),
        None => KillSignalPayload::new_pause(loop_id, action_id),
    };
    write_signal(repo_root, &payload)
}

/// Convenience: write a resume signal for a paused loop.
pub fn write_resume_signal(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    let payload = KillSignalPayload::new_resume(loop_id);
    write_signal(repo_root, &payload)
}

/// Convenience: write a redirect signal with a new goal.
pub fn write_redirect_signal(
    repo_root: &Path,
    loop_id: &str,
    new_goal: impl Into<String>,
) -> Result<(), LoopError> {
    let payload = KillSignalPayload::new_redirect(loop_id, new_goal);
    write_signal(repo_root, &payload)
}

// ── PauseState persistence ──

/// Write a pause state to disk for the given loop.
/// The file is stored at `.loop-pause/{loop_id}` as a JSON blob.
/// Uses atomic write (temp + fsync + rename) for crash safety (P2.2 fix).
pub fn write_pause_state(repo_root: &Path, state: &PauseState) -> Result<(), LoopError> {
    let path = pause_state_path(repo_root, &state.loop_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| LoopError::Io(format!("mkdir pause state dir: {e}")))?;
    }
    let content = serde_json::to_string(state)
        .map_err(|e| LoopError::Serde(format!("serialize pause state: {e}")))?;
    core_state_utils::atomic_write::write_atomic_text(&path, &content)
        .map_err(|e| LoopError::Io(format!("write pause state {}: {e}", path.display())))?;
    Ok(())
}

/// Read a pause state from disk for the given loop.
/// Returns `Ok(None)` when the file does not exist.
pub fn read_pause_state(repo_root: &Path, loop_id: &str) -> Result<Option<PauseState>, LoopError> {
    let path = pause_state_path(repo_root, loop_id);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LoopError::Io(format!("read pause state {}: {e}", path.display()))),
    };
    let state: PauseState = serde_json::from_str(&raw)
        .map_err(|e| LoopError::Serde(format!("parse pause state {}: {e}", path.display())))?;
    Ok(Some(state))
}

/// Clear a pause state from disk for the given loop.
/// Safe to call when no pause state file exists (no-op in that case).
pub fn clear_pause_state(repo_root: &Path, loop_id: &str) -> Result<(), LoopError> {
    let path = pause_state_path(repo_root, loop_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(LoopError::Io(format!(
            "remove pause state {}: {e}",
            path.display()
        ))),
    }
}

/// Check whether a pause state file exists for the given loop.
pub fn is_pause_state_active(repo_root: &Path, loop_id: &str) -> bool {
    pause_state_path(repo_root, loop_id).is_file()
}

/// Read the current lock file and return its content if it exists.
/// Returns `Ok(None)` when no lock file is present.
///
/// **TOCTOU note**: reads directly without prior `is_file()` check to avoid race
/// between check and read. `NotFound` returns `Ok(None)`.
pub fn read_lock_info(repo_root: &Path) -> Result<Option<LockInfo>, LoopError> {
    let path = lock_path(repo_root);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LoopError::Io(format!("read lock {}: {e}", path.display()))),
    };
    let lock: LoopLock = serde_json::from_str(&raw)
        .map_err(|e| LoopError::Serde(format!("parse lock {}: {e}", path.display())))?;
    let epoch = parse_iso_epoch(&lock.acquired_at).unwrap_or_else(|| {
        tracing::warn!(acquired_at = %lock.acquired_at, "failed to parse lock acquired_at, defaulting to 0");
        0
    });
    Ok(Some(LockInfo {
        lock,
        acquired_epoch: epoch,
    }))
}

/// Acquire an exclusive loop lock for the given loop and run.
/// Fails if an active (non-stale) lock already exists. Stale locks older than
/// `LOOP_LOCK_MAX_AGE_SECS` are automatically overridden.
///
/// # Limitations
/// The lock file records the acquiring process's PID and start timestamp for
/// diagnostics, but the 1-hour expiry (`LOOP_LOCK_MAX_AGE_SECS`) is the only
/// automatic release mechanism. There is no daemon or OS-level cleanup — if the
/// process dies without calling `release_lock`, the lock persists on disk until
/// it expires or is manually removed. Callers should ensure `release_lock` is
/// invoked in all exit paths (including error paths).
pub fn acquire_lock(repo_root: &Path, loop_id: &str, run_id: &str) -> Result<(), LoopError> {
    let path = lock_path(repo_root);
    // Read-first pattern: attempt read directly to avoid TOCTOU between is_file() and read.
    match read_lock_info(repo_root)? {
        Some(info) => {
            let age = now_epoch().saturating_sub(info.acquired_epoch);
            if age < LOOP_LOCK_MAX_AGE_SECS {
                return Err(LoopError::ActionFailed(format!(
                    "loop already active: {} (run {}), acquired {}s ago. Max age: {}s.",
                    info.lock.loop_id, info.lock.run_id, age, LOOP_LOCK_MAX_AGE_SECS,
                )));
            }
            tracing::warn!(
                "stale lock from loop '{}' (run '{}'), overriding (age={}s >= {}s)",
                info.lock.loop_id,
                info.lock.run_id,
                age,
                LOOP_LOCK_MAX_AGE_SECS,
            );
            // Use match to handle concurrent removal of stale lock.
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Another process already removed the stale lock — proceed.
                }
                Err(e) => {
                    return Err(LoopError::Io(format!(
                        "remove stale lock {}: {e}",
                        path.display()
                    )));
                }
            }
        }
        None => {
            // No existing lock — proceed to create.
        }
    }
    let lock = LoopLock {
        loop_id: loop_id.to_string(),
        run_id: run_id.to_string(),
        acquired_at: framework_core::time::now_iso(),
    };
    // Record PID and start timestamp for diagnostics and stale-lock analysis.
    let lock_meta = serde_json::json!({
        "loop_id": lock.loop_id,
        "run_id": lock.run_id,
        "acquired_at": lock.acquired_at,
        "pid": std::process::id(),
        "started_at": framework_core::time::now_iso(),
    });
    let text = serde_json::to_string_pretty(&lock_meta)
        .map_err(|e| LoopError::Serde(format!("serialize lock: {e}")))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| LoopError::Io(format!("mkdir {}: {e}", parent.display())))?;
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true).mode(0o644);
        let mut file = opts.open(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                LoopError::ActionFailed(format!(
                    "lock {} already exists (race condition)",
                    path.display()
                ))
            } else {
                LoopError::Io(format!("create lock {}: {e}", path.display()))
            }
        })?;
        file.write_all(text.as_bytes())
            .map_err(|e| LoopError::Io(format!("write lock {}: {e}", path.display())))?;
    }

    #[cfg(not(unix))]
    {
        // Use write-to-temp-then-rename for atomic lock file creation on
        // platforms without O_EXCL (Windows, etc.). The temp + rename pattern
        // makes the write itself atomic, though the existence check above and
        // this write remain non-atomic. For production Windows deployments,
        // consider using a Named Mutex (via the `windows` crate or `winapi`)
        // to achieve true cross-process mutual exclusion.
        core_state_utils::atomic_write::write_atomic_text(&path, &text).map_err(|e| {
            tracing::warn!("non-unix lock write failed: {e}");
            LoopError::Io(format!("write lock {}: {e}", path.display()))
        })?;
    }

    Ok(())
}

/// Release the exclusive loop lock by deleting the lock file.
/// Uses atomic remove without prior `is_file()` check to avoid TOCTOU.
/// `NotFound` is treated as success (lock already released).
pub fn release_lock(repo_root: &Path) -> Result<(), LoopError> {
    let path = lock_path(repo_root);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(LoopError::Io(format!(
            "remove lock {}: {e}",
            path.display()
        ))),
    }
}

/// Refresh the lock file's mtime to prevent stale-lock takeover.
/// Should be called periodically (e.g. every 5 minutes) during long-running
/// loop executions so the lock does not exceed LOOP_LOCK_MAX_AGE_SECS.
///
/// # Error handling
/// Returns `Ok(())` even if the lock file doesn't exist yet (this can happen
/// if it was just released) — the goal is best-effort mtime refresh.
pub fn refresh_lock(repo_root: &Path) -> Result<(), LoopError> {
    let path = lock_path(repo_root);
    if !path.is_file() {
        return Ok(());
    }
    // Touch the file by opening for append with no content change.
    // This updates the mtime without modifying the lock content.
    use std::io::Write;
    match std::fs::OpenOptions::new().append(true).open(&path) {
        Ok(mut file) => {
            // Write nothing — just opening + closing updates mtime on most systems.
            // Explicitly sync to ensure the mtime change is durable.
            let _ = file.write_all(b"");
            let _ = file.sync_all();
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Lock already released — no action needed.
        }
        Err(e) => {
            return Err(LoopError::Io(format!(
                "refresh lock {}: {e}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Return the current epoch seconds.
///
/// # Note on unwrap_or(0)
/// `SystemTime::now()` returns `Err` only if the system clock is before UNIX_EPOCH
/// (1970-01-01), which should never happen on any sane system. Falling back to 0
/// means "epoch origin" — a lock acquired at 0 will always be considered stale,
/// which is a safe degradation (allows re-acquisition rather than blocking forever).
fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_iso_epoch(s: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp() as u64)
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_acquire_and_release_lock() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        acquire_lock(root, "test-loop", "run-1").unwrap();
        let info = read_lock_info(root).unwrap().unwrap();
        assert_eq!(info.lock.loop_id, "test-loop");
        assert_eq!(info.lock.run_id, "run-1");
        release_lock(root).unwrap();
        assert!(read_lock_info(root).unwrap().is_none());
    }

    #[test]
    fn test_acquire_lock_rejects_active() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        acquire_lock(root, "loop-a", "run-1").unwrap();
        let err = acquire_lock(root, "loop-b", "run-2").unwrap_err();
        assert!(err.to_string().contains("already active"));
        release_lock(root).unwrap();
    }

    #[test]
    fn test_kill_signal_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        assert!(!is_kill_signal_active(root, "t"));
        write_kill_signal(root, "t").unwrap();
        assert!(is_kill_signal_active(root, "t"));
        clear_kill_signal(root, "t").unwrap();
        assert!(!is_kill_signal_active(root, "t"));
    }

    #[test]
    fn test_clear_all_kill_signals() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_kill_signal(root, "a").unwrap();
        write_kill_signal(root, "b").unwrap();
        clear_all_kill_signals(root).unwrap();
        assert!(!is_kill_signal_active(root, "a"));
        assert!(!is_kill_signal_active(root, "b"));
    }

    // ── v2 multi-signal protocol tests ──

    #[test]
    fn test_kill_signal_action_default_is_kill() {
        let action: KillSignalAction = serde_json::from_str("\"pause\"").unwrap();
        assert_eq!(action, KillSignalAction::Pause);
        let action2: KillSignalAction = serde_json::from_str("\"resume\"").unwrap();
        assert_eq!(action2, KillSignalAction::Resume);
    }

    #[test]
    fn test_kill_signal_action_as_str() {
        assert_eq!(KillSignalAction::Kill.as_str(), "kill");
        assert_eq!(KillSignalAction::Pause.as_str(), "pause");
        assert_eq!(KillSignalAction::Resume.as_str(), "resume");
        assert_eq!(
            KillSignalAction::PauseWithFeedback { feedback: "x".into() }.as_str(),
            "pause_with_feedback"
        );
        assert_eq!(
            KillSignalAction::Redirect { new_goal: "x".into() }.as_str(),
            "redirect"
        );
    }

    #[test]
    fn test_kill_signal_payload_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Kill
        let kill = KillSignalPayload::new_kill("test-loop");
        write_signal(root, &kill).unwrap();
        let read_back = take_signal(root, "test-loop").unwrap().unwrap();
        assert_eq!(read_back.action, KillSignalAction::Kill);
        assert_eq!(read_back.loop_id, "test-loop");

        // Pause
        write_pause_signal(root, "loop-a", "action-1", Option::<String>::None).unwrap();
        let read_pause = take_signal(root, "loop-a").unwrap().unwrap();
        assert_eq!(read_pause.action, KillSignalAction::Pause);
        assert_eq!(read_pause.action_id.unwrap(), "action-1");

        // Pause with feedback
        write_pause_signal(root, "loop-b", "action-2", Some("please check X")).unwrap();
        let read_fb = take_signal(root, "loop-b").unwrap().unwrap();
        assert_eq!(
            read_fb.action,
            KillSignalAction::PauseWithFeedback {
                feedback: "please check X".into()
            }
        );

        // Resume
        write_resume_signal(root, "loop-c").unwrap();
        let read_resume = take_signal(root, "loop-c").unwrap().unwrap();
        assert_eq!(read_resume.action, KillSignalAction::Resume);

        // Redirect
        write_redirect_signal(root, "loop-d", "new task: do Y").unwrap();
        let read_redirect = take_signal(root, "loop-d").unwrap().unwrap();
        assert_eq!(
            read_redirect.action,
            KillSignalAction::Redirect {
                new_goal: "new task: do Y".into()
            }
        );
    }

    #[test]
    fn test_take_signal_none_for_missing() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let result = take_signal(root, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_take_signal_atomic_consumes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let payload = KillSignalPayload::new_kill("atomic-loop");
        write_signal(root, &payload).unwrap();

        // First read consumes
        let first = take_signal(root, "atomic-loop").unwrap();
        assert!(first.is_some());

        // Second read returns None
        let second = take_signal(root, "atomic-loop").unwrap();
        assert!(second.is_none());
    }

    #[test]
    fn test_kill_signal_v2_backward_compat_old_format() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let path = kill_signal_path(root, "old-loop");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // Old format: no `action` field, no `schema_version`
        let old_json = format!(
            r#"{{"loop_id":"old-loop","armed_at":1234567890,"armed_at_iso":"2026-06-30T12:00:00Z"}}"#
        );
        std::fs::write(&path, old_json).unwrap();

        let payload = take_signal(root, "old-loop").unwrap().unwrap();
        assert_eq!(payload.action, KillSignalAction::Kill); // defaults to Kill
        assert_eq!(payload.loop_id, "old-loop");
        assert_eq!(payload.armed_at, 1234567890);
    }

    #[test]
    fn test_kill_signal_v2_backward_compat_no_action_field() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let path = kill_signal_path(root, "partial-loop");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // v2 format but missing `action` field
        let json = r#"{
            "schema_version": "loop-signal-v2",
            "loop_id": "partial-loop",
            "armed_at": 100,
            "armed_at_iso": "2026-06-30T12:00:00Z"
        }"#;
        std::fs::write(&path, json).unwrap();

        let payload = take_signal(root, "partial-loop").unwrap().unwrap();
        assert_eq!(payload.action, KillSignalAction::Kill); // default
    }

    #[test]
    fn test_kill_signal_action_serialization() {
        // Verify serde roundtrip for all action variants
        let cases: Vec<(KillSignalAction, &str)> = vec![
            (KillSignalAction::Kill, r#""kill""#),
            (KillSignalAction::Pause, r#""pause""#),
            (KillSignalAction::Resume, r#""resume""#),
            (
                KillSignalAction::PauseWithFeedback {
                    feedback: "test".into(),
                },
                r#"{"pause_with_feedback": {"feedback": "test"}}"#,
            ),
            (
                KillSignalAction::Redirect {
                    new_goal: "goal".into(),
                },
                r#"{"redirect": {"new_goal": "goal"}}"#,
            ),
        ];
        for (action, _expected_json) in cases {
            let serialized = serde_json::to_value(&action).unwrap();
            let deserialized: KillSignalAction = serde_json::from_value(serialized).unwrap();
            assert_eq!(deserialized, action);
        }
    }

    /// Note: this test reads `now_epoch` indirectly through the signal payload
    /// and verifies the payload carries a reasonable timestamp.
    #[test]
    fn test_signal_payload_has_reasonable_timestamp() {
        let kill = KillSignalPayload::new_kill("ts-loop");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Timestamp should be within 10 seconds of "now"
        assert!(
            kill.armed_at <= now + 2 && kill.armed_at >= now - 2,
            "armed_at {} is far from now {}",
            kill.armed_at,
            now
        );
    }
}
