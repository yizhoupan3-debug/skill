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
    kill_signal_path(repo_root, loop_id).is_file()
}

/// Write a kill signal file for the given loop to request graceful termination.
/// The file is stored at `.loop-kill/{loop_id}` with a JSON payload containing the loop ID and timestamp.
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
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    );
    fs::write(&path, content)
        .map_err(|e| LoopError::Io(format!("write kill signal {}: {e}", path.display())))?;
    Ok(())
}

/// Remove the kill signal file for a specific loop.
/// Safe to call even if no signal file exists (no-op in that case).
    let path = kill_signal_path(repo_root, loop_id);
    if path.is_file() {
        fs::remove_file(&path)
            .map_err(|e| LoopError::Io(format!("remove kill signal {}: {e}", path.display())))?;
    }
    Ok(())
}

/// Remove all kill signal files by deleting the entire `.loop-kill/` directory.
/// Use during loop runner shutdown cleanup.
    let kill_dir = repo_root.join(".loop-kill");
    if kill_dir.is_dir() {
        fs::remove_dir_all(&kill_dir)
            .map_err(|e| LoopError::Io(format!("remove kill dir {}: {e}", kill_dir.display())))?;
    }
    Ok(())
}

/// Read the current lock file and return its content if it exists.
/// Returns `Ok(None)` when no lock file is present.
    let path = lock_path(repo_root);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| LoopError::Io(format!("read lock {}: {e}", path.display())))?;
    let lock: LoopLock = serde_json::from_str(&raw)
        .map_err(|e| LoopError::Serde(format!("parse lock {}: {e}", path.display())))?;
    let epoch = parse_iso_epoch(&lock.acquired_at).unwrap_or(0);
    Ok(Some(LockInfo { lock, acquired_epoch: epoch }))
}

/// Acquire an exclusive loop lock for the given loop and run.
/// Fails if an active (non-stale) lock already exists. Stale locks older than
/// `LOOP_LOCK_MAX_AGE_SECS` are automatically overridden.
    let path = lock_path(repo_root);
    if path.is_file() {
        let info = read_lock_info(repo_root)?;
        if let Some(info) = info {
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
            fs::remove_file(&path)
                .map_err(|e| LoopError::Io(format!("remove stale lock {}: {e}", path.display())))?;
        }
    }
    let lock = LoopLock {
        loop_id: loop_id.to_string(),
        run_id: run_id.to_string(),
        acquired_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let text = serde_json::to_string_pretty(&lock)
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
        fs::write(&path, text)
            .map_err(|e| LoopError::Io(format!("write lock {}: {e}", path.display())))?;
    }

    Ok(())
}

/// Release the exclusive loop lock by deleting the lock file.
/// Safe to call even when no lock file exists (no-op in that case).
    let path = lock_path(repo_root);
    if path.is_file() {
        fs::remove_file(&path)
            .map_err(|e| LoopError::Io(format!("remove lock {}: {e}", path.display())))?;
    }
    Ok(())
}

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
