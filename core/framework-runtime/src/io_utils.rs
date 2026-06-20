//! I/O 实用函数：带锁追加写、写路径安全验证。
//!
//! These were originally defined in `cli/common.inc` and are extracted here
//! to break the `cli ↔ framework_runtime` circular dependency.

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
fn canonicalize_existing_ancestors(path: &Path) -> Result<PathBuf, String> {
    rt_storage::runtime_storage::paths::canonicalize_existing_ancestors(path)
}

/// Unified security guard for all write entry points.
///
/// Performs the following checks (in order):
///   1. Reject `..` path traversal components.
///   2. Reject symlinks at the target path itself.
///   3. If `allowed_root` is provided, resolve symlinks on existing ancestor
///      directories of both the root and the target path, then verify the
///      target remains under the root (prevents ancestor-directory symlink escape).
///   4. Create parent directories if needed (after all validation passes).
pub fn validate_write_path(path: &Path, allowed_root: Option<&Path>) -> Result<(), String> {
    // 1. Reject path traversal via `..` components.
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "write path {} must not contain '..' traversal segments",
            path.display()
        ));
    }
    // 2. Reject symlink at the target path itself.
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_symlink() => {
            return Err(format!(
                "write path {} must not be a symlink",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "stat write path {} failed: {err}",
                path.display()
            ));
        }
    }
    // 3. Optional root containment: resolve symlinks on existing ancestors of
    //    both the allowed root and the target path, then verify containment.
    //    This prevents a symlink in an ancestor directory from redirecting the
    //    write outside the allowed boundary.
    if let Some(root) = allowed_root {
        let canonical_root = canonicalize_existing_ancestors(root)?;
        let canonical_path = canonicalize_existing_ancestors(path)?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(format!(
                "write path {} must stay under allowed root {} after symlink resolution",
                canonical_path.display(),
                canonical_root.display()
            ));
        }
    }
    // 4. Create parent directories (after all validation has passed).
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create parent directory for {} failed: {err}",
                path.display()
            )
        })?;
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

pub fn append_text_with_process_lock(path: &Path, payload: &str, context: &str) -> Result<(), String> {
    validate_write_path(path, None)?;
    let _guard = append_io_lock()
        .lock()
        .map_err(|_| format!("{context} append lock poisoned"))?;
    let mut file = open_append_nofollow(path)
        .map_err(|err| format!("open {context} append failed for {}: {err}", path.display()))?;
    file.lock_exclusive()
        .map_err(|err| format!("lock {context} append failed for {}: {err}", path.display()))?;
    file.write_all(payload.as_bytes()).map_err(|err| {
        format!(
            "write {context} append failed for {}: {err}",
            path.display()
        )
    })?;
    file.sync_data().map_err(|err| {
        format!(
            "sync {context} append failed for {}: {err}",
            path.display()
        )
    })
}
