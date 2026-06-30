//! Durable atomic file writes shared across the crate (temp + fsync + rename).

use core_errors::FrameworkError;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;

const MAX_TMP_RETRIES: u32 = 16;

/// Fsync the parent directory of `path` (Unix only, no-op on other platforms).
/// Ensures directory metadata (including the rename entry) is durable on disk.
#[cfg(unix)]
pub fn fsync_parent_dir(path: &Path) -> Result<(), FrameworkError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    // File::open on a directory is equivalent to open(O_RDONLY).
    let dir = fs::File::open(parent).map_err(|err| {
        FrameworkError::validation(format!(
            "open parent dir for fsync failed {}: {err}",
            parent.display()
        ))
    })?;
    if !dir.metadata().map(|m| m.is_dir()).unwrap_or(false) {
        return Ok(());
    }
    dir.sync_all().map_err(|err| {
        FrameworkError::validation(format!(
            "fsync parent dir failed for {}: {err}",
            parent.display()
        ))
    })?;
    Ok(())
}

#[cfg(not(unix))]
pub fn fsync_parent_dir(_path: &Path) -> Result<(), FrameworkError> {
    Ok(())
}

/// Generate a unique tmp filename suffix (pid + epoch-micros + nonce).
fn tmp_path_for(path: &Path, fallback_ext: &str) -> PathBuf {
    static NONCE: AtomicU64 = AtomicU64::new(0);

    let pid = std::process::id();
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or(fallback_ext);
    path.with_extension(format!("{}.tmp-{}-{}-{}", ext, pid, micros, nonce))
}

/// One-shot cleanup of stale `.tmp-{pid}-*` files left by a prior crash of this PID.
fn cleanup_stale_tmp_files(dir: &Path) {
    let pid = std::process::id();
    let pattern = format!(".tmp-{pid}-");
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = match entry.file_name().to_str() {
                Some(n) => n.to_string(),
                None => continue,
            };
            if name.contains(&pattern) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Write `content` to `tmp_path` with `create_new(true)` (O_EXCL), fsync,
/// then rename to `final_path` and fsync parent dir.
///
/// Returns `Err(FrameworkError::AlreadyExists(...))` if `tmp_path` already exists
/// so callers can retry with a different path.
pub fn write_atomic_bytes_to_temp(
    final_path: &Path,
    content: &[u8],
    tmp_path: &Path,
) -> Result<(), FrameworkError> {
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            FrameworkError::validation(format!("create parent directory failed: {err}"))
        })?;
    }
    let mut opts = OpenOptions::new();
    opts.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(tmp_path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            return FrameworkError::already_exists(format!(
                "tmp file {} already exists (collision)",
                tmp_path.display()
            ));
        }
        FrameworkError::validation(format!(
            "open temp file failed for {}: {err}",
            tmp_path.display()
        ))
    })?;
    file.write_all(content).map_err(|err| {
        let _ = fs::remove_file(tmp_path);
        FrameworkError::validation(format!(
            "write temp file failed for {}: {err}",
            tmp_path.display()
        ))
    })?;
    file.sync_all().map_err(|err| {
        let _ = fs::remove_file(tmp_path);
        FrameworkError::validation(format!(
            "fsync temp file failed for {}: {err}",
            tmp_path.display()
        ))
    })?;
    drop(file);
    fs::rename(tmp_path, final_path).map_err(|err| {
        let _ = fs::remove_file(tmp_path);
        FrameworkError::validation(format!(
            "rename temp file failed {} -> {}: {err}",
            tmp_path.display(),
            final_path.display()
        ))
    })?;
    // Rename has already completed — fsync_parent_dir failure only means
    // directory metadata may not be crash-durable; the file content is on disk.
    if let Err(err) = fsync_parent_dir(final_path) {
        tracing::warn!(
            "atomic write succeeded but fsync parent dir failed for {}: {err}",
            final_path.display(),
        );
    }
    Ok(())
}

/// Write `content` atomically, auto-generating a unique tmp path.
/// Retries on tmp-file collision (up to `MAX_TMP_RETRIES` attempts).
pub fn write_atomic_text(path: &Path, content: &str) -> Result<(), FrameworkError> {
    write_with_retry(path, content.as_bytes(), "txt")
}

/// Write text content atomically via [`write_atomic_bytes_to_temp`].
/// Caller supplies a pre-generated `tmp_path` (for concurrent-writer scenarios).
pub fn write_atomic_text_to_temp(
    final_path: &Path,
    content: &str,
    tmp_path: &Path,
) -> Result<(), FrameworkError> {
    write_atomic_bytes_to_temp(final_path, content.as_bytes(), tmp_path)
}

/// Serialize and write `value` as JSON with automatic tmp-path generation.
/// Retries on tmp-file collision (up to `MAX_TMP_RETRIES` attempts).
pub fn write_atomic_json(path: &Path, value: &Value) -> Result<(), FrameworkError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| FrameworkError::validation(format!("serialize JSON failed: {err}")))?;
    write_with_retry(path, text.as_bytes(), "json")
}

/// Binary atomic write with automatic tmp-path generation.
/// Retries on tmp-file collision (up to `MAX_TMP_RETRIES` attempts).
pub fn write_atomic_bytes(path: &Path, content: &[u8]) -> Result<(), FrameworkError> {
    write_with_retry(path, content, "bin")
}

fn write_with_retry(path: &Path, content: &[u8], fallback_ext: &str) -> Result<(), FrameworkError> {
    static ONCE: Once = Once::new();
    for attempt in 0..MAX_TMP_RETRIES {
        if attempt == 0 {
            ONCE.call_once(|| {
                if let Some(parent) = path.parent() {
                    cleanup_stale_tmp_files(parent);
                }
            });
        }
        let tmp_path = tmp_path_for(path, fallback_ext);
        match write_atomic_bytes_to_temp(path, content, &tmp_path) {
            Ok(()) => return Ok(()),
            Err(e) if e.is_already_exists() && attempt + 1 < MAX_TMP_RETRIES => continue,
            Err(e) => return Err(e),
        }
    }
    Err(FrameworkError::validation(format!(
        "{} exhausted {} retries for {}",
        fallback_ext,
        MAX_TMP_RETRIES,
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_json_path(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("core-state-atomic-{label}-{suffix}.json"))
    }

    #[test]
    fn write_atomic_text_creates_final_without_tmp_sidecar() {
        // Use an isolated directory so leftover .tmp files from other processes don't interfere.
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("core-state-atomic-isolated-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("text.json");
        write_atomic_text(&path, "hello").expect("write");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path() != path && e.path().extension().and_then(|s| s.to_str()) == Some("tmp")
            })
            .collect();
        assert!(
            leftovers.is_empty(),
            "tmp sidecar should be removed: {leftovers:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_json_round_trips_value() {
        let path = temp_json_path("json");
        let _ = std::fs::remove_file(&path);
        let value = json!({"task_id": "t1", "n": 2});
        write_atomic_json(&path, &value).expect("write json");
        let raw = std::fs::read_to_string(&path).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, value);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn write_atomic_text_to_temp_creates_nested_parent_dirs() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("core-state-atomic-nested-{suffix}"));
        let final_path = base.join("nested/out.txt");
        let tmp_path = base.join("nested/out.txt.part");
        let _ = std::fs::remove_dir_all(&base);
        write_atomic_text_to_temp(&final_path, "nested payload", &tmp_path).expect("write");
        assert_eq!(
            std::fs::read_to_string(&final_path).unwrap(),
            "nested payload"
        );
        assert!(!tmp_path.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
