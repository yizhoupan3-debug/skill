//! JSONL maintenance helpers: corrupt-tail truncation and line-count compaction.
//!
//! Both TASK_LEDGER.jsonl and STEP_LEDGER.jsonl can accumulate corrupt tail lines
//! (e.g. truncated writes from a crash).  `truncate_corrupt_tail` removes everything
//! after the last successfully-parsed JSONL line.  `compact_jsonl_if_needed` keeps
//! the file bounded when it grows past a configurable threshold.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde_json::Value;

/// Scan a JSONL file from the **end** and, if trailing corrupt lines are found,
/// truncate the file to the last valid JSONL line boundary.
///
/// Returns `Ok(true)` if the file was actually truncated, `Ok(false)` if it was
/// already clean (or empty / missing).  Any I/O error is propagated.
pub fn truncate_corrupt_tail(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }

    // Fast path: read the whole file and record byte offsets of each line end.
    let content = fs::read_to_string(path)
        .map_err(|err| format!("truncate_corrupt_tail: read {}: {err}", path.display()))?;

    if content.is_empty() {
        return Ok(false);
    }

    // Build a list of (line_start_byte, line_end_byte_exclusive) for every line.
    let mut line_ranges: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    for (idx, ch) in content.char_indices() {
        if ch == '\n' {
            let end_exclusive = idx + 1; // include the \n
            line_ranges.push((start, end_exclusive));
            start = end_exclusive;
        }
    }
    // Handle last line without trailing newline.
    if start < content.len() {
        line_ranges.push((start, content.len()));
    }

    // Walk from the tail to find the last valid JSONL line.
    let mut last_valid_end: Option<usize> = None;
    for &(ls, le) in line_ranges.iter().rev() {
        let trimmed = content[ls..le].trim();
        if trimmed.is_empty() {
            continue;
        }
        if serde_json::from_str::<Value>(trimmed).is_ok() {
            last_valid_end = Some(le);
            break;
        }
    }

    match last_valid_end {
        None => {
            // Entire file is corrupt/empty — truncate to zero.
            // Use set_len to avoid rewriting.
            fs::OpenOptions::new()
                .write(true)
                .open(path)
                .and_then(|f| f.set_len(0))
                .map_err(|err| {
                    format!(
                        "truncate_corrupt_tail: truncate-all {}: {err}",
                        path.display()
                    )
                })?;
            Ok(true)
        }
        Some(end) if end < content.len() => {
            // There are trailing corrupt bytes after the last valid line.
            fs::OpenOptions::new()
                .write(true)
                .open(path)
                .and_then(|f| {
                    f.set_len(end as u64)?;
                    f.sync_all()
                })
                .map_err(|err| {
                    format!(
                        "truncate_corrupt_tail: set_len {}: {err}",
                        path.display()
                    )
                })?;
            Ok(true)
        }
        _ => Ok(false), // File ends cleanly.
    }
}

/// Count non-empty lines in a JSONL file.  Returns 0 for missing / empty files.
fn count_jsonl_lines(path: &Path) -> usize {
    let Ok(file) = fs::File::open(path) else {
        return 0;
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// If the JSONL file at `path` has more than `max_lines` non-empty lines, compact
/// it by writing valid lines to a temp file and atomically renaming.
///
/// Returns `Ok(true)` if compaction occurred, `Ok(false)` if the file was under the
/// threshold or missing.
pub fn compact_jsonl_if_needed(path: &Path, max_lines: usize) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }

    let line_count = count_jsonl_lines(path);
    if line_count <= max_lines {
        return Ok(false);
    }

    // Read all lines, keep only valid non-empty ones (preserve order).
    let content = fs::read_to_string(path)
        .map_err(|err| format!("compact_jsonl: read {}: {err}", path.display()))?;

    let mut valid_lines: Vec<&str> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if serde_json::from_str::<Value>(trimmed).is_ok() {
            valid_lines.push(line);
        }
        // Silently skip corrupt lines during compaction.
    }

    if valid_lines.is_empty() {
        // Nothing valid — truncate to zero.
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|f| f.set_len(0))
            .map_err(|err| {
                format!("compact_jsonl: truncate-all {}: {err}", path.display())
            })?;
        return Ok(true);
    }

    // Atomic write: tmp file in the same directory, then rename.
    let compacted = {
        let mut buf = String::new();
        for line in &valid_lines {
            buf.push_str(line);
            if !line.ends_with('\n') {
                buf.push('\n');
            }
        }
        buf
    };

    let tmp_path = derive_compact_tmp_path(path);
    let parent = path
        .parent()
        .ok_or_else(|| format!("compact_jsonl: no parent for {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("compact_jsonl: mkdir {}: {err}", parent.display()))?;

    // Write tmp, fsync, rename, fsync parent (reuse atomic_write patterns).
    {
        let mut tmp_file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|err| format!("compact_jsonl: open tmp {}: {err}", tmp_path.display()))?;
        tmp_file
            .write_all(compacted.as_bytes())
            .map_err(|err| format!("compact_jsonl: write tmp {}: {err}", tmp_path.display()))?;
        tmp_file
            .sync_all()
            .map_err(|err| format!("compact_jsonl: fsync tmp {}: {err}", tmp_path.display()))?;
    }
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "compact_jsonl: rename {} -> {}: {err}",
            tmp_path.display(),
            path.display()
        )
    })?;

    // Best-effort fsync parent dir.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(parent_dir) = path.parent() {
            if let Ok(dir) = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_RDONLY)
                .open(parent_dir)
            {
                let _ = dir.sync_all();
            }
        }
    }

    Ok(true)
}

/// Derive a deterministic-ish temp path next to `path` for the compaction atomic write.
fn derive_compact_tmp_path(path: &Path) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NONCE: AtomicU64 = AtomicU64::new(0);

    let pid = std::process::id();
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed);

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jsonl");
    let new_ext = format!("{ext}.compact.tmp-{pid}-{micros}-{nonce}");
    path.with_extension(new_ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("router-rs-jsonl-maint-{label}-{nanos}"))
    }

    // ---- truncate_corrupt_tail tests ----

    #[test]
    fn truncate_corrupt_tail_noop_on_clean_file() {
        let tmp = unique_tmp("truncate-clean");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        let valid = json!({"a": 1}).to_string();
        fs::write(&path, format!("{valid}\n")).unwrap();
        let truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(!truncated, "clean file should not be truncated");
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), valid);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_corrupt_tail_removes_trailing_corrupt_lines() {
        let tmp = unique_tmp("truncate-corrupt");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        let valid1 = json!({"a": 1}).to_string();
        let valid2 = json!({"b": 2}).to_string();
        let corrupt = "not-json-at-all";
        fs::write(&path, format!("{valid1}\n{valid2}\n{corrupt}\n")).unwrap();

        let truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(truncated, "corrupt tail must be truncated");

        let content = fs::read_to_string(&path).unwrap();
        assert!(
            content.contains(&valid1),
            "first valid line must survive"
        );
        assert!(
            content.contains(&valid2),
            "second valid line must survive"
        );
        assert!(
            !content.contains(corrupt),
            "corrupt line must be removed"
        );
        // Verify the file parses as valid JSONL (no corrupt residue).
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                serde_json::from_str::<serde_json::Value>(trimmed)
                    .expect("every remaining line must be valid JSON");
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_corrupt_tail_removes_mid_corrupt_lines_at_tail() {
        let tmp = unique_tmp("truncate-mid");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        let valid1 = json!({"a": 1}).to_string();
        let corrupt1 = "broken1";
        let corrupt2 = "broken2";
        // Pattern: valid, corrupt, corrupt — last valid is line 1.
        fs::write(
            &path,
            format!("{valid1}\n{corrupt1}\n{corrupt2}\n"),
        )
        .unwrap();

        let truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(truncated);
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), valid1);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_corrupt_tail_all_corrupt_truncates_to_zero() {
        let tmp = unique_tmp("truncate-all-corrupt");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        fs::write(&path, "not-json\nalso-bad\n").unwrap();

        let truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(truncated, "all-corrupt file must be truncated to zero");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.is_empty(), "file should be empty after truncating all corrupt");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_corrupt_tail_missing_file_is_noop() {
        let tmp = unique_tmp("truncate-missing");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("nonexistent.jsonl");
        let truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(!truncated, "missing file is a no-op");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_corrupt_tail_no_trailing_newline() {
        let tmp = unique_tmp("truncate-no-nl");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        let valid = json!({"a": 1}).to_string();
        // Write valid line without trailing newline, then corrupt with no newline.
        fs::write(&path, format!("{valid}\ntruncated-corrupt")).unwrap();

        let truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(truncated);
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), valid);
        let _ = fs::remove_dir_all(&tmp);
    }

    // ---- compact_jsonl_if_needed tests ----

    #[test]
    fn compact_jsonl_noop_when_under_threshold() {
        let tmp = unique_tmp("compact-under");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        // Write 3 lines, threshold = 100.
        let mut content = String::new();
        for i in 0..3 {
            content.push_str(&format!("{}\n", json!({"i": i})));
        }
        fs::write(&path, &content).unwrap();

        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(!compacted, "should not compact when under threshold");
        let readback = fs::read_to_string(&path).unwrap();
        assert_eq!(readback, content);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_jsonl_compacts_when_over_threshold() {
        let tmp = unique_tmp("compact-over");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        // Write 105 valid lines.
        let mut content = String::new();
        for i in 0..105 {
            content.push_str(&format!("{}\n", json!({"i": i})));
        }
        fs::write(&path, &content).unwrap();

        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(compacted, "should compact when over threshold");
        let readback = fs::read_to_string(&path).unwrap();
        // All 105 valid lines should survive (compaction preserves valid lines).
        let line_count = readback.lines().filter(|l| !l.trim().is_empty()).count();
        assert_eq!(line_count, 105);
        // Verify every line is valid JSON.
        for line in readback.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                serde_json::from_str::<serde_json::Value>(trimmed)
                    .expect("compacted line must be valid JSON");
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_jsonl_strips_corrupt_during_compaction() {
        let tmp = unique_tmp("compact-corrupt");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        // 99 valid + 2 corrupt = 101 non-empty lines (over threshold).
        let mut content = String::new();
        for i in 0..99 {
            content.push_str(&format!("{}\n", json!({"i": i})));
        }
        content.push_str("corrupt-line-1\ncorrupt-line-2\n");
        fs::write(&path, &content).unwrap();

        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(compacted);
        let readback = fs::read_to_string(&path).unwrap();
        let line_count = readback.lines().filter(|l| !l.trim().is_empty()).count();
        assert_eq!(line_count, 99, "corrupt lines should be stripped during compaction");
        // All remaining lines are valid JSON.
        for line in readback.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                serde_json::from_str::<serde_json::Value>(trimmed).expect("valid");
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_jsonl_missing_file_is_noop() {
        let tmp = unique_tmp("compact-missing");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("nonexistent.jsonl");
        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(!compacted);
        let _ = fs::remove_dir_all(&tmp);
    }

    /// Integration: truncate + compact in sequence.
    #[test]
    fn truncate_then_compact_integration() {
        let tmp = unique_tmp("truncate-compact-integration");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");

        // 102 valid lines + trailing corrupt.
        let mut content = String::new();
        for i in 0..102 {
            content.push_str(&format!("{}\n", json!({"i": i})));
        }
        content.push_str("broken-tail\n");
        fs::write(&path, &content).unwrap();

        // Step 1: truncate corrupt tail.
        let was_truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(was_truncated);

        // Step 2: compact (102 lines > 100 threshold).
        let was_compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(was_compacted);

        let readback = fs::read_to_string(&path).unwrap();
        let line_count = readback.lines().filter(|l| !l.trim().is_empty()).count();
        assert_eq!(line_count, 102, "all valid lines preserved");
        for line in readback.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                serde_json::from_str::<serde_json::Value>(trimmed).expect("valid");
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }
}
