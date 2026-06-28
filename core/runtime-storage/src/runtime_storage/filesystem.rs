use super::paths::stable_memory_key;
use core_errors::FrameworkError;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Serialize `append_text` for the in-memory regression backend (no `flock`); parallel writers
/// could otherwise interleave bytes on the same logical path.
pub static MEMORY_APPEND_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
#[tracing::instrument(level = "debug", skip_all)]
pub fn memory_storage_root() -> Result<PathBuf, String> {
    let cwd =
        std::env::current_dir().map_err(|err| format!("resolve current dir failed: {err}"))?;
    let mut digest = Sha256::new();
    digest.update(cwd.display().to_string().as_bytes());
    let namespace = hex::encode(digest.finalize());
    Ok(std::env::temp_dir()
        .join("router-rs-runtime-memory-v1")
        .join(namespace))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn memory_artifact_path(path: &Path) -> Result<PathBuf, String> {
    let stable_key = stable_memory_key(path)?;
    let mut digest = Sha256::new();
    digest.update(stable_key.as_bytes());
    let key = hex::encode(digest.finalize());
    Ok(memory_storage_root()?.join(format!("{key}.payload")))
}
/// Symlink policy for filesystem `write_text` / `append_text`:
/// reject when the final path already exists as a symlink (`symlink_metadata`).
/// This avoids following a symlink on append and makes the write target explicit
/// (callers must write to a normal file path, not an alias).
#[tracing::instrument(level = "debug", skip_all)]
pub fn filesystem_reject_symlink_write_target(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_symlink() {
                return Err(format!(
                    "runtime storage path {} must not be a symlink",
                    path.display()
                ));
            }
            Ok(())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!(
            "stat runtime storage path {} failed: {err}",
            path.display()
        )),
    }
}

/// RAII guard for a cross-process advisory lock keyed by an arbitrary
/// runtime path. The guard owns a sentinel `.router-rs.<filename>.lock`
/// file alongside the target; advisory `flock(LOCK_EX)` is held for the
/// lifetime of the guard and released on drop. The sentinel file is
/// intentionally left on disk so future acquisitions reuse the same
/// inode (avoiding TOCTOU races on lock-file creation).
pub struct RuntimePathLockGuard {
    _file: fs::File,
}

/// Acquire an exclusive cross-process lock for `path` with a timeout.
/// Multiple writers (codex/cursor/test harness) racing on the same shared
/// runtime artifact will serialize through this lock. Uses `try_lock_exclusive`
/// with retry so hook dispatch never blocks indefinitely.
///
/// Timeout: 5s (50 attempts × 100ms). If the lock cannot be acquired within
/// this window, the call returns an error rather than hanging forever.
const LOCK_ACQUIRE_TIMEOUT_MS: u64 = 5_000;
const LOCK_RETRY_INTERVAL_MS: u64 = 100;
const LOCK_MAX_ATTEMPTS: u64 = LOCK_ACQUIRE_TIMEOUT_MS / LOCK_RETRY_INTERVAL_MS;

#[tracing::instrument(level = "debug", skip_all)]
pub fn acquire_runtime_path_lock(path: &Path) -> Result<RuntimePathLockGuard, String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "runtime path {} has no parent directory for lock placement",
            path.display()
        )
    })?;
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create runtime lock parent directory failed for {}: {err}",
                path.display()
            )
        })?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-storage");
    let lock_path = parent.join(format!(".router-rs.{file_name}.lock"));
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| {
            format!(
                "open runtime path lock {} failed: {err}",
                lock_path.display()
            )
        })?;
    let mut attempt = 0u64;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => break,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                attempt += 1;
                if attempt >= LOCK_MAX_ATTEMPTS {
                    return Err(format!(
                        "acquire runtime path lock {} timed out after {}ms ({} attempts)",
                        lock_path.display(),
                        LOCK_ACQUIRE_TIMEOUT_MS,
                        attempt,
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(LOCK_RETRY_INTERVAL_MS));
            }
            Err(err) => {
                return Err(format!(
                    "acquire runtime path lock {} failed: {err}",
                    lock_path.display()
                ));
            }
        }
    }
    Ok(RuntimePathLockGuard { _file: file })
}

pub fn filesystem_write_text_inner(
    path: &Path,
    payload_text: &str,
    _nanos: u128,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create runtime storage parent directory failed for {}: {err}",
                path.display()
            )
        })?;
    }
    // Cross-process lock: serialize concurrent writers (codex+cursor+tests)
    // sharing the same artifact path to prevent last-writer-wins overwrites.
    let _path_lock = acquire_runtime_path_lock(path)?;
    filesystem_reject_symlink_write_target(path)?;

    // Delegate to core-state-utils's canonical atomic write (temp + write + fsync + rename + fsync_parent_dir).
    core_state_utils::atomic_write::write_atomic_text(path, payload_text).map_err(|e| e.to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn filesystem_write_text(path: &Path, payload_text: &str) -> Result<(), FrameworkError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system time before unix epoch: {err}"))?
        .as_nanos();
    filesystem_write_text_inner(path, payload_text, nanos)
        .map_err(|e| FrameworkError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn filesystem_append_text(path: &Path, payload_text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create runtime storage parent directory failed for {}: {err}",
                path.display()
            )
        })?;
    }
    // Cross-process append lock prevents JSONL line-interleaving when codex
    // and cursor (or parallel tests) tail the same trace/event stream.
    let _path_lock = acquire_runtime_path_lock(path)?;
    filesystem_reject_symlink_write_target(path)?;
    let mut file = filesystem_open_append_text(path)?;
    file.write_all(payload_text.as_bytes()).map_err(|err| {
        format!(
            "append runtime storage payload failed for {}: {err}",
            path.display()
        )
    })?;
    file.sync_data().map_err(|err| {
        format!(
            "sync runtime storage append failed for {}: {err}",
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(unix)]
pub fn filesystem_open_append_text(path: &Path) -> Result<fs::File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|err| {
            format!(
                "open runtime storage payload for append failed for {}: {err}",
                path.display()
            )
        })
}

#[cfg(not(unix))]
pub fn filesystem_open_append_text(path: &Path) -> Result<fs::File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            format!(
                "open runtime storage payload for append failed for {}: {err}",
                path.display()
            )
        })
}
