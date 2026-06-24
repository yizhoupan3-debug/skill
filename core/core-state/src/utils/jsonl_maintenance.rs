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
use std::collections::HashMap;

/// tx_type values in TASK_LEDGER that carry a **full state snapshot** — only the
/// *last* occurrence of each type matters for replay.  Older rows of the same type
/// can be discarded during compaction.
const STATE_SNAPSHOT_TX_TYPES: &[&str] = &["goal_state", "rfv_loop_state", "evidence"];

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
                    format!("truncate_corrupt_tail: set_len {}: {err}", path.display())
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
/// it by discarding redundant state-snapshot entries (keeping only the last entry per
/// `tx_type`) and rewriting via atomic rename.
///
/// State-snapshot types (`goal_state`, `rfv_loop_state`, `evidence`) each represent a
/// **complete replacement**: hydration replay only cares about the most recent copy of
/// each.  Older rows of the same type are safe to remove.
///
/// Non-snapshot rows (steps, events) are always preserved.
///
/// Returns `Ok(true)` if compaction occurred, `Ok(false)` if the file was under the
/// threshold or missing.
pub fn compact_jsonl_if_needed(path: &Path, max_lines: usize) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }

    // Fast early exit: if the raw line count is at or below threshold, skip I/O.
    let line_count = count_jsonl_lines(path);
    if line_count <= max_lines {
        return Ok(false);
    }

    // Read and compact: remove redundant state-snapshot rows.
    let content = fs::read_to_string(path)
        .map_err(|err| format!("compact_jsonl: read {}: {err}", path.display()))?;

    // Pass 1: find the *last* occurrence index of each state-snapshot tx_type.
    let mut last_snapshot: HashMap<&str, usize> = HashMap::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            if let Some(tt) = extract_state_snapshot_type(&val) {
                last_snapshot.insert(tt, i);
            }
        }
    }

    if last_snapshot.is_empty() {
        // No state-snapshot rows to deduplicate — compaction won't help.
        // Return early to avoid a pointless write.
        return Ok(false);
    }

    // Pass 2: build the compacted content.
    // - State-snapshot rows: keep only the *last* occurrence per type.
    // - Non-snapshot rows: keep all (valid lines).
    // - Corrupt / empty lines: discarded.
    let mut compacted_lines: Vec<&str> = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !serde_json::from_str::<Value>(trimmed).is_ok() {
            continue; // Corrupt line — silently skip.
        }

        // If this is a state-snapshot type and it's NOT the last occurrence, drop it.
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            if let Some(tt) = extract_state_snapshot_type(&val) {
                if last_snapshot.get(tt) != Some(&i) {
                    continue; // Redundant — a newer snapshot of this type exists.
                }
            }
        }

        compacted_lines.push(line);
    }

    // If compaction didn't actually remove any rows, skip the write.
    let valid_original: Vec<&str> = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && serde_json::from_str::<Value>(t).is_ok()
        })
        .collect();
    if compacted_lines.len() >= valid_original.len() {
        return Ok(false);
    }

    if compacted_lines.is_empty() {
        // Truncate to zero.
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|f| f.set_len(0))
            .map_err(|err| {
                format!(
                    "compact_jsonl: truncate-all {}: {err}",
                    path.display()
                )
            })?;
        return Ok(true);
    }

    // Atomic write: tmp file in the same directory, then rename.
    let compacted = {
        let mut buf = String::new();
        for line in &compacted_lines {
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

    {
        let mut tmp_file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|err| {
                format!("compact_jsonl: open tmp {}: {err}", tmp_path.display())
            })?;
        tmp_file
            .write_all(compacted.as_bytes())
            .map_err(|err| {
                format!("compact_jsonl: write tmp {}: {err}", tmp_path.display())
            })?;
        tmp_file.sync_all().map_err(|err| {
            format!("compact_jsonl: fsync tmp {}: {err}", tmp_path.display())
        })?;
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
        if let Some(parent_dir) = path.parent()
            && let Ok(dir) = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_RDONLY)
                .open(parent_dir)
            && let Err(e) = dir.sync_all()
        {
            tracing::warn!(error = %e, "failed to sync parent directory after compaction");
        }
    }

    Ok(true)
}

/// If `val` represents a state-snapshot row, return its tx_type label.
/// Returns `None` for non-snapshot rows (steps, events, etc.).
fn extract_state_snapshot_type<'a>(val: &'a Value) -> Option<&'a str> {
    let tt = val.get("tx_type")?.as_str()?;
    if STATE_SNAPSHOT_TX_TYPES.contains(&tt) {
        Some(tt)
    } else {
        None
    }
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

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jsonl");
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
        assert!(content.contains(&valid1), "first valid line must survive");
        assert!(content.contains(&valid2), "second valid line must survive");
        assert!(!content.contains(corrupt), "corrupt line must be removed");
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
        fs::write(&path, format!("{valid1}\n{corrupt1}\n{corrupt2}\n")).unwrap();

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
        assert!(
            content.is_empty(),
            "file should be empty after truncating all corrupt"
        );
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
    fn compact_jsonl_noop_when_no_state_snapshot_rows() {
        // Lines without a state-snapshot tx_type (e.g. steps, plain JSON)
        // cannot be reduced by compaction — return false.
        let tmp = unique_tmp("compact-no-snapshot");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        let mut content = String::new();
        for i in 0..105 {
            content.push_str(&format!("{}\n", json!({"i": i})));
        }
        fs::write(&path, &content).unwrap();

        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(!compacted, "no state-snapshot rows to deduplicate");
        let readback = fs::read_to_string(&path).unwrap();
        let line_count = readback.lines().filter(|l| !l.trim().is_empty()).count();
        assert_eq!(line_count, 105);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_jsonl_deduplicates_state_snapshot_rows() {
        let tmp = unique_tmp("compact-dedup");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");

        // Write 95 non-snapshot lines + 10 goal_state lines = 105 total (over threshold of 100).
        let mut content = String::new();
        for i in 0..95 {
            content.push_str(&format!("{}\n", json!({"seq": i, "tx_type": "step", "payload": {"i": i}})));
        }
        // 10 goal_state snapshots — only the LAST one should survive.
        for i in 0..10 {
            content.push_str(&format!("{}\n", json!({"seq": 100 + i, "tx_type": "goal_state", "payload": {"version": i}})));
        }
        fs::write(&path, &content).unwrap();

        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(compacted, "should compact state-snapshot rows");

        let readback = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = readback.lines().filter(|l| !l.trim().is_empty()).collect();
        // 95 steps + 1 goal_state = 96 lines
        assert_eq!(lines.len(), 96, "only 1 goal_state should survive");
        // Verify only the LAST goal_state remains
        let goal_lines: Vec<&str> = lines.iter()
            .filter(|l| l.contains("goal_state"))
            .copied()
            .collect();
        assert_eq!(goal_lines.len(), 1, "only one goal_state row");
        assert!(goal_lines[0].contains(r#""version":9"#), "the last goal_state (version 9) must survive");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_jsonl_deduplicates_multiple_snapshot_types() {
        let tmp = unique_tmp("compact-multi-snap");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");

        let mut content = String::new();
        // 3 goal_state + 3 rfv_loop_state + 2 evidence + 93 steps = 101 total (over 100)
        for i in 0..3 {
            content.push_str(&format!("{}\n", json!({"seq": i, "tx_type": "goal_state", "i": i})));
        }
        for i in 0..3 {
            content.push_str(&format!("{}\n", json!({"seq": 10 + i, "tx_type": "rfv_loop_state", "i": i})));
        }
        for i in 0..2 {
            content.push_str(&format!("{}\n", json!({"seq": 20 + i, "tx_type": "evidence", "i": i})));
        }
        for i in 0..93 {
            content.push_str(&format!("{}\n", json!({"seq": 100 + i, "tx_type": "step", "i": i})));
        }
        fs::write(&path, &content).unwrap();

        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(compacted);

        let readback = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = readback.lines().filter(|l| !l.trim().is_empty()).collect();
        // 93 steps + 1 goal_state + 1 rfv_loop_state + 1 evidence = 96
        assert_eq!(lines.len(), 96);

        // Count by type
        let goal = lines.iter().filter(|l| l.contains("goal_state")).count();
        let rfv = lines.iter().filter(|l| l.contains("rfv_loop_state")).count();
        let ev = lines.iter().filter(|l| l.contains("evidence")).count();
        assert_eq!(goal, 1, "only the last goal_state");
        assert_eq!(rfv, 1, "only the last rfv_loop_state");
        assert_eq!(ev, 1, "only the last evidence");
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
}
