//! I/O 实用函数：带锁追加写、写路径安全验证。
//!
//! These were originally defined in `cli/common.inc` and are extracted here
//! to break the `cli ↔ framework_runtime` circular dependency.

use core_errors::FrameworkError;
use fs2::FileExt;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn append_io_lock() -> &'static Mutex<()> {
    static APPEND_IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    APPEND_IO_LOCK.get_or_init(|| Mutex::new(()))
}

/// Re-export canonicalize from runtime-storage (single source of truth).
fn canonicalize_existing_ancestors(path: &Path) -> Result<PathBuf, FrameworkError> {
    rt_storage::runtime_storage::paths::canonicalize_existing_ancestors(path)
        .map_err(|e| FrameworkError::Validation { message: e })
}

/// Unified security guard for all write entry points.
///
/// Performs the following checks (in order):
///   1. Delegates to `core_state_utils::path_guard::reject_unsafe_path` for
///      path traversal (`..`) and symlink checks.
///   2. If `allowed_root` is provided, resolve symlinks on existing ancestor
///      directories of both the root and the target path, then verify the
///      target remains under the root (prevents ancestor-directory symlink escape).
///   3. Create parent directories if needed (after all validation passes).
pub fn validate_write_path(path: &Path, allowed_root: Option<&Path>) -> Result<(), FrameworkError> {
    // 1. Path traversal + symlink check (shared with core_state_utils::path_guard).
    core_state_utils::path_guard::reject_unsafe_path(path)?;

    // 2. Optional root containment: resolve symlinks on existing ancestors of
    //    both the allowed root and the target path, then verify containment.
    //    both the allowed root and the target path, then verify containment.
    //    This prevents a symlink in an ancestor directory from redirecting the
    //    write outside the allowed boundary.
    if let Some(root) = allowed_root {
        let canonical_root = canonicalize_existing_ancestors(root)?;
        let canonical_path = canonicalize_existing_ancestors(path)?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(FrameworkError::Validation {
                message: format!(
                    "write path {} must stay under allowed root {} after symlink resolution",
                    canonical_path.display(),
                    canonical_root.display()
                ),
            });
        }
    }
    // 4. Create parent directories (after all validation has passed).
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Open a file for append with O_NOFOLLOW on Unix to prevent symlink-following
/// at the file-descriptor level (defense-in-depth beyond the metadata check in
/// `validate_write_path`).
#[cfg(unix)]
fn open_append_nofollow(path: &Path) -> Result<fs::File, std::io::Error> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_append_nofollow(path: &Path) -> Result<fs::File, std::io::Error> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

pub fn append_text_with_process_lock(path: &Path, payload: &str, context: &str) -> Result<(), FrameworkError> {
    validate_write_path(path, None)?;
    let _guard = append_io_lock()
        .lock()
        .map_err(|_| FrameworkError::Lock { message: format!("{context} append lock poisoned") })?;
    let mut file = open_append_nofollow(path)?;
    file.lock_exclusive()?;
    file.write_all(payload.as_bytes())?;
    file.sync_data()?;
    Ok(())
}
