//! Durable atomic file writes shared across the crate (temp + fsync + rename).

use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
fn fsync_parent_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_RDONLY)
        .open(parent)
        .map_err(|err| {
            format!(
                "open parent dir for fsync failed {}: {err}",
                parent.display()
            )
        })?;
    dir.sync_all()
        .map_err(|err| format!("fsync parent dir failed for {}: {err}", parent.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Write `content` to `tmp_path`, fsync, then rename to `final_path` and fsync parent dir.
pub(crate) fn write_atomic_text_to_temp(
    final_path: &Path,
    content: &str,
    tmp_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(tmp_path)
        .map_err(|err| format!("open temp file failed for {}: {err}", tmp_path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|err| format!("write temp file failed for {}: {err}", tmp_path.display()))?;
    file.sync_all()
        .map_err(|err| format!("fsync temp file failed for {}: {err}", tmp_path.display()))?;
    drop(file);
    fs::rename(tmp_path, final_path).map_err(|err| {
        let _ = fs::remove_file(tmp_path);
        format!(
            "rename temp file failed {} -> {}: {err}",
            tmp_path.display(),
            final_path.display()
        )
    })?;
    fsync_parent_dir(final_path)?;
    Ok(())
}

/// Convenience wrapper around [`write_atomic_text_to_temp`] that derives a `<ext>.tmp` sidecar
/// for single-writer call sites (e.g. framework runtime session artifacts). If multiple processes
/// may race to write the same `path` concurrently, **do not** use this helper — derive a unique
/// `tmp_path` (pid + nanos + nonce) and call [`write_atomic_text_to_temp`] directly. The codex
/// hook installer takes that route in [`crate::codex_hooks::write_atomic_text`].
pub(crate) fn write_atomic_text(path: &Path, content: &str) -> Result<(), String> {
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt")
    ));
    write_atomic_text_to_temp(path, content, &tmp_path)
}

pub(crate) fn write_atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("serialize JSON failed: {err}"))?;
    let tmp_path = path.with_extension("json.tmp");
    write_atomic_text_to_temp(path, &text, &tmp_path)
}
