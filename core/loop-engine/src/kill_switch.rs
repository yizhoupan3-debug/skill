use crate::types::LoopError;
use crate::state::{kill_signal_path, lock_path, LOOP_LOCK_MAX_AGE_SECS};
use std::fs;
use std::path::Path;
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
        framework_kernel::time::now_iso(),
    );
    fs::write(&path, content)
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
        Err(e) => Err(LoopError::Io(format!("remove kill signal {}: {e}", path.display()))),
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
    let epoch = parse_iso_epoch(&lock.acquired_at).unwrap_or(0);
    Ok(Some(LockInfo { lock, acquired_epoch: epoch }))
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
                info.lock.loop_id, info.lock.run_id, age, LOOP_LOCK_MAX_AGE_SECS,
            );
            // Use match to handle concurrent removal of stale lock.
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Another process already removed the stale lock — proceed.
                }
                Err(e) => return Err(LoopError::Io(format!("remove stale lock {}: {e}", path.display()))),
            }
        }
        None => {
            // No existing lock — proceed to create.
        }
    }
    let lock = LoopLock {
        loop_id: loop_id.to_string(),
        run_id: run_id.to_string(),
        acquired_at: framework_kernel::time::now_iso(),
    };
    // Record PID and start timestamp for diagnostics and stale-lock analysis.
    let lock_meta = serde_json::json!({
        "loop_id": lock.loop_id,
        "run_id": lock.run_id,
        "acquired_at": lock.acquired_at,
        "pid": std::process::id(),
        "started_at": framework_kernel::time::now_iso(),
    });
    let text = serde_json::to_string_pretty(&lock_meta)
        .map_err(|e| LoopError::Serde(format!("serialize lock: {e}")))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| LoopError::Io(format!("mkdir {}: {e}", parent.display())))?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut opts = fs::OpenOptions::new();
        opts.write(true)
            .create_new(true)
            .mode(0o644);
        let mut file = opts.open(&path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    LoopError::ActionFailed(format!(
                        "lock {} already exists (race condition)", path.display()
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
        // NOTE: On non-Unix platforms (Windows, etc.) we cannot use O_EXCL for atomic
        // file creation. The `fs::write` call below is susceptible to a TOCTOU race:
        // another process could create the lock file between the existence check above
        // and this write. For production Windows deployments, consider using a Named Mutex
        // (via the `windows` crate or `winapi`) to achieve true cross-process mutual
        // exclusion. The current approach is acceptable for single-user CI/development
        // environments where concurrent loop runners on Windows are unlikely.
        fs::write(&path, text)
            .map_err(|e| LoopError::Io(format!("write lock {}: {e}", path.display())))?;
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
        Err(e) => Err(LoopError::Io(format!("remove lock {}: {e}", path.display()))),
    }
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
}
