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
pub fn write_atomic_text_to_temp(
    final_path: &Path,
    content: &str,
    tmp_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    let mut opts = OpenOptions::new();
    opts.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts
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
pub fn write_atomic_text(path: &Path, content: &str) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NONCE: AtomicU64 = AtomicU64::new(0);

    let pid = std::process::id();
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);

    let tmp_path = path.with_extension(format!(
        "{}.tmp-{}-{}-{}",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt"),
        pid,
        micros,
        nonce
    ));
    write_atomic_text_to_temp(path, content, &tmp_path)
}

#[cfg(test)]
mod tests {
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

pub fn write_atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NONCE: AtomicU64 = AtomicU64::new(0);

    let pid = std::process::id();
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);

    let text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("serialize JSON failed: {err}"))?;
    let tmp_path = path.with_extension(format!("json.tmp-{}-{}-{}", pid, micros, nonce));
    write_atomic_text_to_temp(path, &text, &tmp_path)
}
