use core_state::utils::atomic_write::write_atomic_text;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Cross-host file state lock abstraction.
///
/// All 4 hosts use the same pattern:
/// 1. Compute state path from repo_root + host prefix
/// 2. Acquire file lock (flock on Unix, create_new on non-Unix)
/// 3. Load state from JSON
/// 4. Execute closure with mutable state
/// 5. Save state back to JSON
/// 6. Release lock
///
/// Guard that holds a file lock until dropped.
pub struct FileStateLockGuard {
    _lock_path: PathBuf,
    #[cfg(unix)]
    _file: fs::File,
    #[cfg(not(unix))]
    _file: fs::File,
}

/// Configuration for a host's hook state directory layout.
pub struct HookStateConfig {
    pub host_id: &'static str,
    pub state_dir_leaf: &'static str,
    pub state_filename: &'static str,
    pub unreadable_tag: &'static str,
}

impl HookStateConfig {
    /// Compute the hook-state directory path for this host.
    pub fn state_dir(&self, repo_root: &Path) -> PathBuf {
        repo_root.join(self.state_dir_leaf).join("hook-state")
    }

    /// Compute the full state file path.
    pub fn state_path(&self, repo_root: &Path) -> PathBuf {
        self.state_dir(repo_root).join(self.state_filename)
    }

    /// Load state from disk, returning default if missing or corrupt.
    pub fn load_state<T: Default + serde::de::DeserializeOwned>(&self, repo_root: &Path) -> T {
        let path = self.state_path(repo_root);
        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(state) = serde_json::from_str::<T>(&content) {
                debug!(host = %self.host_id, "hook state loaded");
                return state;
            }
        debug!(host = %self.host_id, "hook state default (missing or corrupt)");
        T::default()
    }

    /// Save state to disk atomically (temp + fsync + rename), creating parent directories as needed.
    pub fn save_state<T: serde::Serialize>(&self, repo_root: &Path, state: &T) {
        let path = self.state_path(repo_root);
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::warn!(host = %self.host_id, "failed to create hook state dir: {e}");
                return;
            }
        }
        match serde_json::to_string_pretty(state) {
            Ok(json) => {
                if let Err(e) = write_atomic_text(&path, &json) {
                    tracing::warn!(host = %self.host_id, "failed to save hook state: {e}");
                } else {
                    debug!(host = %self.host_id, "hook state saved");
                }
            }
            Err(e) => {
                tracing::warn!(host = %self.host_id, "failed to serialize hook state: {e}");
            }
        }
    }

    /// Remove the state file (cleanup on session end).
    pub fn remove_state(&self, repo_root: &Path) {
        let _ = fs::remove_file(self.state_path(repo_root));
    }

    /// Acquire a file lock and execute a closure with loaded state.
    ///
    /// This is the canonical pattern used by all hosts:
    /// 1. Acquire lock
    /// 2. Load state
    /// 3. Execute closure (may mutate state)
    /// 4. Save state
    /// 5. Release lock (via Drop)
    pub fn with_state_lock<T, F, S>(&self, repo_root: &Path, f: F) -> Result<T, String>
    where
        S: Default + serde::Serialize + serde::de::DeserializeOwned,
        F: FnOnce(&mut S) -> Result<T, String>,
    {
        let state_path = self.state_path(repo_root);
        let lock_path = state_path.with_extension("json.lock");

        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Acquire lock (platform-specific)
        let _guard = acquire_file_lock(&lock_path)?;

        // Load state
        let mut state = if state_path.exists() {
            let content = fs::read_to_string(&state_path).map_err(|e| e.to_string())?;
            serde_json::from_str::<S>(&content).unwrap_or_default()
        } else {
            S::default()
        };

        // Execute closure
        let result = f(&mut state)?;

        // Save state atomically (temp + fsync + rename)
        let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
        write_atomic_text(&state_path, &json).map_err(|e| e.to_string())?;

        Ok(result)
    }
}

/// Acquire a file lock (Unix: flock, non-Unix: create_new).
#[cfg(unix)]
fn acquire_file_lock(lock_path: &Path) -> Result<FileStateLockGuard, String> {
    use std::os::unix::io::AsRawFd;
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| format!("lock_open_failed: {e}"))?;

    // Try flock with retries
    let mut retries = 10;
    loop {
        // SAFETY: libc::flock operates on a valid file descriptor from OpenOptions.
        // LOCK_EX | LOCK_NB is a well-defined POSIX operation on regular files.
        // The fd is valid for the duration of this scope (file is not closed until drop).
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            break;
        }
        retries -= 1;
        if retries == 0 {
            return Err(format!("lock_acquisition_failed: {} retries exhausted", 10));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(FileStateLockGuard {
        _lock_path: lock_path.to_path_buf(),
        _file: file,
    })
}

#[cfg(not(unix))]
fn acquire_file_lock(lock_path: &Path) -> Result<FileStateLockGuard, String> {
    // Non-Unix: use exclusive create as a simple lock.
    //
    // WARNING: This approach has an inherent TOCTOU (time-of-check-to-time-of-use) race:
    // between `create_new` succeeding and writing to the file, another process may also
    // succeed. On Unix the locker can rely on flock(2) kernel lease semantics; non-Unix
    // platforms (Windows, WASM) do not have equivalent primitives. This implementation
    // uses a retry loop with exponential backoff to mitigate, but correctness in truly
    // concurrent scenarios is not guaranteed.
    //
    // Stale lock recovery: attempt to remove an existing lock file (left by a crashed
    // process) before the first retry. If the remove succeeds (file was stale), fall
    // through to the retry loop which will create the new lock.
    let _ = fs::remove_file(lock_path);
    let max_retries: u32 = 10; // 10 attempts, ~51s worst case
    let mut attempt: u32 = 0;
    loop {
        let result = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(lock_path);

        match result {
            Ok(file) => {
                return Ok(FileStateLockGuard {
                    _lock_path: lock_path.to_path_buf(),
                    _file: file,
                });
            }
            Err(e) if attempt < max_retries => {
                tracing::warn!(
                    "lock_create_failed (attempt {}/{}): {e}",
                    attempt + 1,
                    max_retries
                );
                std::thread::sleep(std::time::Duration::from_millis(
                    50u64 * 2u64.pow(attempt),
                ));
                attempt += 1;
            }
            Err(e) => {
                let err_msg = format!(
                    "lock_acquisition_failed: {} retries exhausted, last error: {e}",
                    max_retries
                );
                tracing::warn!("{err_msg}");
                return Err(err_msg);
            }
        }
    }
}

impl Drop for FileStateLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self._lock_path);
    }
}

/// Read JSON from stdin with size limit (shared by all hosts).
pub fn read_stdin_json() -> Result<serde_json::Value, String> {
    const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024; // 4 MiB
    let mut stdin = io::stdin();
    let mut buf = String::new();
    stdin
        .by_ref()
        .take(MAX_STDIN_BYTES)
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;

    // Check for overflow
    let mut probe = [0_u8; 1];
    let overflow = stdin.read(&mut probe).map_err(|e| e.to_string())?;
    if overflow > 0 {
        return Err("stdin_too_large".to_string());
    }

    if buf.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }

    let value: serde_json::Value =
        serde_json::from_str(&buf).map_err(|_| "stdin_json_invalid".to_string())?;
    if value.is_object() {
        Ok(value)
    } else {
        Err("stdin_json_not_object".to_string())
    }
}

/// Resolve repo root from CLI arg or payload `cwd` field.
pub fn resolve_repo_root(
    cli_root: Option<&Path>,
    payload: &serde_json::Value,
) -> Result<PathBuf, String> {
    cli_root
        .map(|p| p.to_path_buf())
        .or_else(|| {
            payload
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
        })
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            "repo_root required (pass --repo-root or include cwd in payload)".to_string()
        })
}
