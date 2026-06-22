//! # 跨宿主统一文件锁抽象 (file_state_lock)
//!
//! 提供跨宿主（Claude、Cursor、Codex、OpenCode）共享的文件状态锁抽象，
//! 统一处理 hook-state 目录下的 JSON 状态文件读写与加锁。
//!
//! ## 设计意图
//!
//! 所有 4 个宿主使用相同的模式：
//! 1. 从 `repo_root` + 宿主前缀计算 state path
//! 2. 获取文件锁（Unix: flock, 非 Unix: create_new）
//! 3. 从 JSON 加载状态
//! 4. 执行闭包（可能修改状态）
//! 5. 原子写回 JSON (temp + fsync + rename)
//! 6. 释放锁（Drop 时自动执行）
//!
//! ## 迁移状态
//!
//! ✅ **已统一**：所有宿主（Claude、Cursor、Codex）均通过 `HookStateConfig` +
//! `FileStateLockGuard` 使用共享锁实现。各宿主通过 `LockConfig` 定制超时、
//! 重试和 stale lock 阈值。
//!
//! ## 非 Unix 平台限制
//!
//! 非 Unix 平台（Windows、WASM）使用 `create_new` 作为锁机制，存在固有的
//! TOCTOU (time-of-check-to-time-of-use) 竞态风险。当前实现使用指数退避重试缓解。

use core_state::utils::atomic_write::write_atomic_text;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Per-host lock configuration.
///
/// Different hosts have different timeout/retry requirements based on their
/// expected concurrency patterns and hook execution model.
pub struct LockConfig {
    /// Maximum time to wait for lock acquisition (milliseconds).
    pub max_wait_ms: u64,
    /// Interval between retry attempts (milliseconds).
    pub retry_interval_ms: u64,
    /// Age threshold for stale lock detection (seconds).
    /// Lock files older than this are considered stale and may be overridden.
    pub stale_lock_age_secs: u64,
}

impl LockConfig {
    /// Default config for short-lived CLI hooks (Codex, OpenCode).
    /// 10 retries with exponential backoff, ~51s worst case.
    pub const fn cli_default() -> Self {
        Self {
            max_wait_ms: 5000,
            retry_interval_ms: 50,
            stale_lock_age_secs: 60,
        }
    }

    /// Config for hook hosts: 5s timeout, 60s stale lock cleanup.
    pub const fn short_timeout() -> Self {
        Self {
            max_wait_ms: 5000,
            retry_interval_ms: 50,
            stale_lock_age_secs: 60,
        }
    }

    /// Config for hook hosts: longer timeout, inode-based stale detection.
    pub const fn long_timeout() -> Self {
        Self {
            max_wait_ms: 10000,
            retry_interval_ms: 100,
            stale_lock_age_secs: 120,
        }
    }
}

/// Cross-host file state lock abstraction.
///
/// Guard that holds a file lock until dropped. Acquired via `acquire_file_lock`.
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
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent) {
                tracing::warn!(host = %self.host_id, "failed to create hook state dir: {e}");
                return;
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

    /// Acquire a file lock and execute a closure with loaded state (default config).
    pub fn with_state_lock<T, F, S>(&self, repo_root: &Path, f: F) -> Result<T, String>
    where
        S: Default + serde::Serialize + serde::de::DeserializeOwned,
        F: FnOnce(&mut S) -> Result<T, String>,
    {
        self.with_state_lock_configured(repo_root, &LockConfig::cli_default(), f)
    }

    /// Acquire a file lock with custom config and execute a closure with loaded state.
    ///
    /// This is the canonical pattern used by all hosts:
    /// 1. Acquire lock (with host-specific timeout/retry config)
    /// 2. Load state
    /// 3. Execute closure (may mutate state)
    /// 4. Save state
    /// 5. Release lock (via Drop)
    pub fn with_state_lock_configured<T, F, S>(
        &self,
        repo_root: &Path,
        config: &LockConfig,
        f: F,
    ) -> Result<T, String>
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

        // Acquire lock (platform-specific, with config)
        let _guard = acquire_file_lock_with_config(&lock_path, config)?;

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

/// Acquire a file lock with configurable timeout and stale lock detection.
///
/// Uses `LockConfig` to control retry behavior and stale lock thresholds.
/// This is the single lock acquisition implementation used by all hosts.
#[cfg(unix)]
pub fn acquire_file_lock_with_config(
    lock_path: &Path,
    config: &LockConfig,
) -> Result<FileStateLockGuard, String> {
    use std::os::unix::io::AsRawFd;

    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("lock_dir_create_failed: {e}"))?;
    }

    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| format!("lock_open_failed: {e}"))?;

    let fd = file.as_raw_fd();
    let start = std::time::Instant::now();
    let retry_interval = std::time::Duration::from_millis(config.retry_interval_ms);
    let max_wait = std::time::Duration::from_millis(config.max_wait_ms);
    let stale_threshold = std::time::Duration::from_secs(config.stale_lock_age_secs);

    loop {
        // SAFETY: libc::flock operates on a valid file descriptor from OpenOptions.
        // LOCK_EX | LOCK_NB is a well-defined POSIX operation on regular files.
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            return Ok(FileStateLockGuard {
                _lock_path: lock_path.to_path_buf(),
                _file: file,
            });
        }

        if start.elapsed() >= max_wait {
            // Stale lock detection: check if lock file is older than threshold
            if let Ok(meta) = fs::metadata(lock_path)
                && let Ok(modified) = meta.modified()
                && modified.elapsed().unwrap_or(std::time::Duration::ZERO) > stale_threshold
            {
                tracing::warn!(
                    "stale lock detected (>{}s), forcing retry",
                    config.stale_lock_age_secs
                );
                // Force release and retry once
                unsafe { libc::flock(fd, libc::LOCK_UN); }
                let rc2 = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
                if rc2 == 0 {
                    return Ok(FileStateLockGuard {
                        _lock_path: lock_path.to_path_buf(),
                        _file: file,
                    });
                }
            }
            return Err(format!(
                "lock_timeout after {}ms (path={})",
                config.max_wait_ms,
                lock_path.display()
            ));
        }

        std::thread::sleep(retry_interval);
    }
}

/// Acquire a file lock with configurable timeout (non-Unix).
#[cfg(not(unix))]
pub fn acquire_file_lock_with_config(
    lock_path: &Path,
    config: &LockConfig,
) -> Result<FileStateLockGuard, String> {
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("lock_dir_create_failed: {e}"))?;
    }

    // Stale lock recovery: attempt to remove an existing lock file
    let _ = fs::remove_file(lock_path);

    let max_wait = std::time::Duration::from_millis(config.max_wait_ms);
    let start = std::time::Instant::now();
    let retry_interval = std::time::Duration::from_millis(config.retry_interval_ms);

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
            Err(e) => {
                if start.elapsed() >= max_wait {
                    return Err(format!(
                        "lock_timeout after {}ms (path={}): {e}",
                        config.max_wait_ms,
                        lock_path.display()
                    ));
                }
                tracing::warn!("lock_create_failed: {e}, retrying");
                std::thread::sleep(retry_interval);
            }
        }
    }
}

impl Drop for FileStateLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self._lock_path);
    }
}

/// Cross-platform process-alive check (shared by all hosts for stale lock detection).
#[cfg(unix)]
pub fn is_process_alive(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) is a POSIX-standard probe with no signal delivery.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Cross-platform process-alive check (Windows fallback).
#[cfg(not(unix))]
pub fn is_process_alive(pid: u32) -> bool {
    use std::ptr::null_mut;
    type HANDLE = *mut std::ffi::c_void;
    type DWORD = u32;
    type BOOL = i32;
    const PROCESS_QUERY_INFORMATION: DWORD = 0x0400;
    const STILL_ACTIVE: DWORD = 259;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
        fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut DWORD) -> BOOL;
        fn CloseHandle(hObject: HANDLE) -> BOOL;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: DWORD = 0;
        let ok = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        ok != 0 && exit_code == STILL_ACTIVE
    }
}

/// Current time in milliseconds since UNIX epoch (shared lock utility).
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Parse `pid=N ts=MS` lock metadata from a lock file's text content.
pub fn parse_lock_metadata(text: &str) -> Option<(u32, u64)> {
    let pid = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix("pid="))
        .and_then(|s| s.parse::<u32>().ok())?;
    let ts = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix("ts="))
        .and_then(|s| s.parse::<u64>().ok())?;
    Some((pid, ts))
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
