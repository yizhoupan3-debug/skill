//! JSONL maintenance helpers: corrupt-tail truncation and line-count compaction.
//!
//! Both TASK_LEDGER.jsonl and STEP_LEDGER.jsonl can accumulate corrupt tail lines
//! (e.g. truncated writes from a crash).  `truncate_corrupt_tail` removes everything
//! after the last successfully-parsed JSONL line.  `compact_jsonl_if_needed` /
//! `compact_jsonl_with_content` keep the file bounded when it grows past a configurable
//! threshold by discarding redundant state-snapshot rows and renumbering seq densely.

use core_errors::FrameworkError;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};
use std::collections::HashMap;

/// tx_type values in TASK_LEDGER that carry a **full state snapshot** — only the
/// *last* occurrence of each type matters for replay.  Older rows of the same type
/// can be discarded during compaction.
const STATE_SNAPSHOT_TX_TYPES: &[&str] = &[
    "goal_state",
    "rfv_loop_state",   // legacy — old QG tx_type (retained for ledger replay compat)
    "quality_gate_state", // legacy — old QG tx_type (retained for ledger replay compat)
    "evidence",
];

// ═══════════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════════

/// If the JSONL file at `path` has more than `max_lines` non-empty lines, compact
/// it by discarding redundant state-snapshot entries (keeping only the last entry per
/// `tx_type`) and rewriting via atomic rename.  All kept lines get **dense seq**
/// (0, 1, 2, … *N*−1) so post-compaction seq monotonicity is preserved.
///
/// **Note:** `seq` is NOT stable across compactions — it is a dense ordering counter,
/// not a durable transaction ID.  External consumers that use `seq` for deduplication
/// or external-key correlation must account for renumbering after compaction.
/// For idempotency, use `idempotency_key` instead of `seq`.
///
/// This is a thin wrapper that reads the file and delegates to
/// [`compact_jsonl_with_content`].
pub fn compact_jsonl_if_needed(path: &Path, max_lines: usize) -> Result<bool, FrameworkError> {
    if !path.is_file() {
        return Ok(false);
    }
    let content = fs::read_to_string(path).map_err(|err| {
        FrameworkError::validation(format!("compact_jsonl: read {}: {err}", path.display()))
    })?;
    compact_jsonl_with_content(path, &content, max_lines)
}

/// Same semantics as [`compact_jsonl_if_needed`] but accepts the file content as an
/// already-read `&str` — avoids a redundant `read_to_string` when the caller (e.g.
/// `task_ledger.rs`) already has the post-append file content in memory.
pub fn compact_jsonl_with_content(
    path: &Path,
    content: &str,
    max_lines: usize,
) -> Result<bool, FrameworkError> {
    // Clean up stale temp files from prior crashes before touching the file.
    cleanup_stale_compact_tmp_files(path);

    if content.is_empty() {
        return Ok(false);
    }

    let result = compact_jsonl_core(content, max_lines)?;

    let compacted = match result {
        Some(c) => c,
        None => return Ok(false),
    };

    if compacted.is_empty() {
        // All rows were snapshot rows and got fully deduplicated.
        let parent = path.parent().ok_or_else(|| {
            FrameworkError::validation(format!("compact_jsonl: no parent for {}", path.display()))
        })?;
        fs::create_dir_all(parent).map_err(|err| {
            FrameworkError::validation(format!("compact_jsonl: mkdir {}: {err}", parent.display()))
        })?;
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|f| f.set_len(0))
            .map_err(|err| {
                FrameworkError::validation(format!(
                    "compact_jsonl: truncate-all {}: {err}",
                    path.display()
                ))
            })?;
        return Ok(true);
    }

    super::atomic_write::write_atomic_text(path, &compacted)?;
    Ok(true)
}

/// Find the byte offset immediately after the last valid JSONL line in `content`.
///
/// Returns `None` when no valid JSON line is found (entire file is corrupt).
/// The offset is the position **after** the trailing `\n` of the last valid line
/// (or `content.len()` if the last line has no trailing newline).  Callers can
/// compare this with `content.len()` to detect trailing corrupt bytes.
fn find_last_valid_line_end(content: &str) -> Option<usize> {
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
    for &(ls, le) in line_ranges.iter().rev() {
        let trimmed = content[ls..le].trim();
        if trimmed.is_empty() {
            continue;
        }
        if serde_json::from_str::<Value>(trimmed).is_ok() {
            return Some(le);
        }
    }
    None
}

/// Scan a JSONL file from the **end** and, if trailing corrupt lines are found,
/// truncate the file to the last valid JSONL line boundary.
///
/// Returns `Ok(true)` if the file was actually truncated, `Ok(false)` if it was
/// already clean (or empty / missing).  Any I/O error is propagated.
pub fn truncate_corrupt_tail(path: &Path) -> Result<bool, FrameworkError> {
    if !path.is_file() {
        return Ok(false);
    }

    let content = fs::read_to_string(path).map_err(|err| {
        FrameworkError::validation(format!(
            "truncate_corrupt_tail: read {}: {err}",
            path.display()
        ))
    })?;

    if content.is_empty() {
        return Ok(false);
    }

    let end = find_last_valid_line_end(&content);

    match end {
        None => {
            // Entire file is corrupt/empty — truncate to zero.
            fs::OpenOptions::new()
                .write(true)
                .open(path)
                .and_then(|f| f.set_len(0))
                .map_err(|err| {
                    FrameworkError::validation(format!(
                        "truncate_corrupt_tail: truncate-all {}: {err}",
                        path.display()
                    ))
                })?;
            Ok(true)
        }
        Some(e) if e < content.len() => {
            // There are trailing corrupt bytes after the last valid line.
            fs::OpenOptions::new()
                .write(true)
                .open(path)
                .and_then(|f| {
                    f.set_len(e as u64)?;
                    f.sync_all()
                })
                .map_err(|err| {
                    FrameworkError::validation(format!(
                        "truncate_corrupt_tail: set_len {}: {err}",
                        path.display()
                    ))
                })?;
            Ok(true)
        }
        _ => Ok(false), // File ends cleanly.
    }
}

/// Combined truncate + compact for a single-read pass.
///
/// Reads the file once, truncates corrupt tail in-memory, applies line-count
/// compaction on the clean content, and writes only once — halving the I/O
/// compared to calling [`truncate_corrupt_tail`] followed by
/// [`compact_jsonl_if_needed`] separately.
pub fn truncate_and_compact(path: &Path, max_lines: usize) -> Result<bool, FrameworkError> {
    if !path.is_file() {
        return Ok(false);
    }

    let content = fs::read_to_string(path).map_err(|err| {
        FrameworkError::validation(format!(
            "truncate_and_compact: read {}: {err}",
            path.display()
        ))
    })?;

    if content.is_empty() {
        return Ok(false);
    }

    let trunc_end = find_last_valid_line_end(&content);
    let was_truncated = trunc_end.is_none_or(|end| end < content.len());

    let clean = match trunc_end {
        Some(end) if end < content.len() => &content[..end],
        Some(_) => &content, // already clean
        None => "",
    };

    let compacted = compact_jsonl_core(clean, max_lines)?;

    let output = match compacted {
        Some(ref c) => c.as_str(),
        None if was_truncated => clean,
        None => return Ok(false),
    };

    if output.is_empty() {
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|f| f.set_len(0))
            .map_err(|err| {
                FrameworkError::validation(format!(
                    "truncate_and_compact: truncate-all {}: {err}",
                    path.display()
                ))
            })?;
    } else {
        super::atomic_write::write_atomic_text(path, output)?;
    }

    Ok(true)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Core compaction logic — single parse pass + dense seq renumbering
// ═══════════════════════════════════════════════════════════════════════════════

/// Core compaction: parse `content`, deduplicate state-snapshot rows, renumber seq.
///
/// Returns `Ok(Some(compacted_text))` if compaction occurred,
/// `Ok(None)` if the content is under threshold or has nothing to deduplicate.
fn compact_jsonl_core(content: &str, max_lines: usize) -> Result<Option<String>, FrameworkError> {
    // ── single parse pass: keep owned `Map` + snapshot label per line ──
    // We use owned `Map<String, Value>` so we can mutate `seq` later without
    // worrying about borrows.  This is ~50–300 small allocations per compaction.
    let mut entries: Vec<(Map<String, Value>, Option<String>)> = Vec::new();
    // Track non-object JSON lines (arrays, primitives) so they survive compaction.
    let mut non_object_lines: Vec<String> = Vec::new();
    let mut total_valid = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let val: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!(
                    "compact_jsonl_core: skipping corrupt JSONL line; truncate_corrupt_tail may have already removed trailing corruption"
                );
                continue;
            }
        };
        total_valid += 1;
        match val {
            Value::Object(map) => {
                let snap_type = extract_snapshot_type_from_map(&map);
                entries.push((map, snap_type));
            }
            _ => {
                // Preserve non-object JSON values (arrays, primitives, etc.)
                // as-is — they are not candidates for snapshot dedup.
                non_object_lines.push(trimmed.to_string());
            }
        };
    }

    // Early exit: under threshold
    if total_valid <= max_lines {
        return Ok(None);
    }

    // Find the *last* occurrence index of each state-snapshot tx_type.
    let mut last_snapshot: HashMap<String, usize> = HashMap::new();
    for (i, (_, snap_type)) in entries.iter().enumerate() {
        if let Some(tt) = snap_type {
            last_snapshot.insert(tt.clone(), i);
        }
    }

    if last_snapshot.is_empty() {
        // No state-snapshot rows to deduplicate — compaction won't help.
        return Ok(None);
    }

    // ── build compacted output with dense seq ────────────────────────────
    // Pre-allocate the output buffer (upper bound ≈ original content length).
    // Each kept line is re-serialised with a corrected seq value equal to its
    // new position in the file.  This eliminates the seq fragmentation bug
    // where post-compaction seq values no longer reflect file position.
    let mut compacted = String::with_capacity(content.len());
    let mut kept_count = 0usize;
    let entry_count = entries.len(); // captured before the consuming into_iter below

    for (i, (mut obj, snap_type)) in entries.into_iter().enumerate() {
        if let Some(tt) = snap_type
            && last_snapshot.get(tt.as_str()) != Some(&i)
        {
            continue; // Redundant — a newer snapshot of this type exists.
        }

        // Renumber seq to its dense position.  Every kept line gets 0,1,2,...
        // so the global max is always `kept_count - 1`, and future appends
        // (which derive seq from line count) will always be > this max.
        obj.insert("seq".to_string(), Value::from(kept_count as u64));

        let line = serde_json::to_string(&Value::Object(obj)).map_err(|e| {
            FrameworkError::validation(format!("compact_jsonl: serialize seq {kept_count}: {e}"))
        })?;
        compacted.push_str(&line);
        compacted.push('\n');
        kept_count += 1;
    }

    // Every valid object that wasn't skipped was tracked — "did we reduce?"
    // We compare against entries.len() (not the full line count) because only
    // JSON objects participate in snapshot dedup; non-object lines (if any)
    // are preserved as-is by the compacted output and must not inflate the
    // "nothing changed" check.
    if kept_count >= entry_count {
        return Ok(None);
    }

    // Preserve non-object JSON lines (arrays, primitives) at the end.
    // These do not participate in snapshot dedup or seq renumbering.
    for line in &non_object_lines {
        compacted.push_str(line);
        compacted.push('\n');
    }

    Ok(Some(compacted))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// If `val` represents a state-snapshot row, return its tx_type label.
fn extract_snapshot_type_from_map(obj: &Map<String, Value>) -> Option<String> {
    let tt = obj.get("tx_type")?.as_str()?;
    if STATE_SNAPSHOT_TX_TYPES.contains(&tt) {
        Some(tt.to_string())
    } else {
        None
    }
}

/// Remove stale `.compact.tmp-{pid}-*` temp files left by a prior crash of this
/// process in the same directory as `path`.  Only files matching the current PID
/// are cleaned — files from other processes (still running) are left alone.
fn cleanup_stale_compact_tmp_files(path: &Path) {
    let pid = std::process::id();
    let pattern = format!(".compact.tmp-{pid}-");
    if let Some(parent) = path.parent()
        && let Ok(entries) = fs::read_dir(parent)
    {
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

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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

    fn write_content(path: &Path, lines: &[Value]) {
        let mut buf = String::new();
        for line in lines {
            buf.push_str(&serde_json::to_string(line).unwrap());
            buf.push('\n');
        }
        fs::write(path, &buf).unwrap();
    }

    fn read_lines(path: &Path) -> Vec<Value> {
        let raw = fs::read_to_string(path).unwrap();
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
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
        fs::write(&path, format!("{valid}\ntruncated-corrupt")).unwrap();

        let truncated = truncate_corrupt_tail(&path).unwrap();
        assert!(truncated);
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), valid);
        let _ = fs::remove_dir_all(&tmp);
    }

    // ---- compact tests ----

    #[test]
    fn compact_jsonl_noop_when_under_threshold() {
        let tmp = unique_tmp("compact-under");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        let mut content = String::new();
        for i in 0..3 {
            content.push_str(&format!("{}\n", json!({"i": i, "seq": i})));
        }
        fs::write(&path, &content).unwrap();

        let compacted = compact_jsonl_if_needed(&path, 100).unwrap();
        assert!(!compacted, "should not compact when under threshold");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_jsonl_noop_when_no_state_snapshot_rows() {
        let tmp = unique_tmp("compact-no-snapshot");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        let mut content = String::new();
        for i in 0..105 {
            content.push_str(&format!("{}\n", json!({"i": i, "seq": i})));
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
    fn compact_jsonl_renumbers_seq_densely() {
        // This test verifies that after compaction, seq values are 0,1,2,...
        // regardless of original seq values — fixing the seq fragmentation bug.
        let tmp = unique_tmp("compact-seq");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");

        // Write lines with arbitrary seq values (55 steps + 48 goal_state = 103 lines)
        let mut lines = Vec::new();
        for i in 0..55 {
            lines.push(json!({"tx_type": "step", "seq": i * 10, "i": i}));
        }
        for i in 0..48 {
            lines.push(json!({"tx_type": "goal_state", "seq": 1000 + i, "i": i}));
        }
        write_content(&path, &lines);

        assert!(
            compact_jsonl_if_needed(&path, 100).unwrap(),
            "should compact"
        );

        let kept = read_lines(&path);
        // 55 steps + 1 goal_state = 56 lines
        assert_eq!(kept.len(), 56, "55 steps + 1 goal_state");

        // Every kept line must have dense seq 0..55
        for (idx, entry) in kept.iter().enumerate() {
            let seq = entry.get("seq").and_then(Value::as_u64).unwrap();
            assert_eq!(
                seq, idx as u64,
                "seq must be dense at position {idx}, got {seq}"
            );
        }

        // Confirm the surviving goal_state is the LAST one (i=47)
        let goal = kept.iter().find(|v| v["tx_type"] == "goal_state").unwrap();
        assert_eq!(goal["i"], 47, "the last goal_state must survive");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_jsonl_with_content_noop_on_short_content() {
        let content = json!({"a": 1}).to_string();
        let result = compact_jsonl_core(&content, 100).unwrap();
        assert!(result.is_none(), "under threshold");
    }

    #[test]
    fn compact_jsonl_with_content_noop_no_snapshots() {
        let mut buf = String::new();
        for i in 0..105 {
            buf.push_str(&json!({"i": i}).to_string());
            buf.push('\n');
        }
        let result = compact_jsonl_core(&buf, 100).unwrap();
        assert!(result.is_none(), "no snapshot types");
    }

    #[test]
    fn compact_jsonl_with_content_deduplicates_and_renumbers() {
        // Build 95 steps + 5 goal_state = 100 lines (under 100 threshold = no-op).
        // Add 5 more = 105 to trigger compaction.
        let mut buf = String::new();
        for i in 0..100 {
            if i < 95 {
                buf.push_str(
                    &json!({"tx_type": "step", "seq": i, "payload": {"i": i}}).to_string(),
                );
            } else {
                let gi = i - 95; // 0..4
                buf.push_str(
                    &json!({"tx_type": "goal_state", "seq": i, "version": gi}).to_string(),
                );
            }
            buf.push('\n');
        }
        // Add 5 more to hit 105
        for i in 0..5 {
            buf.push_str(
                &json!({"tx_type": "goal_state", "seq": 100 + i, "version": 5 + i}).to_string(),
            );
            buf.push('\n');
        }

        let result = compact_jsonl_core(&buf, 100).unwrap();
        let compacted = result.expect("should compact");
        let lines: Vec<Value> = compacted
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        // 95 steps + 1 goal_state (last one, version=9) = 96 lines
        assert_eq!(lines.len(), 96);

        // seq must be dense 0..95
        for (idx, entry) in lines.iter().enumerate() {
            assert_eq!(
                entry["seq"].as_u64().unwrap(),
                idx as u64,
                "dense seq at {idx}"
            );
        }

        let goal = lines.iter().find(|v| v["tx_type"] == "goal_state").unwrap();
        assert_eq!(
            goal["version"], 9,
            "last goal_state (version 9) must survive"
        );
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

    // ---- truncate_and_compact tests ----

    #[test]
    fn truncate_and_compact_truncates_and_deduplicates_in_one_pass() {
        let tmp = unique_tmp("trunc-and-compact");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");

        // 50 steps + 3 goal_state + corrupt tail = 54 valid + corrupt, threshold=50
        let mut buf = String::new();
        for i in 0..50 {
            buf.push_str(&json!({"tx_type": "step", "seq": i, "i": i}).to_string());
            buf.push('\n');
        }
        for i in 0..3 {
            buf.push_str(&json!({"tx_type": "goal_state", "seq": 100 + i, "i": i}).to_string());
            buf.push('\n');
        }
        buf.push_str("corrupt-line-that-should-be-removed\n");
        fs::write(&path, &buf).unwrap();

        assert!(truncate_and_compact(&path, 50).unwrap(), "should act");

        let kept = read_lines(&path);
        // 50 steps + 1 goal_state = 51 lines (corrupt removed, goal_state dedup'd)
        assert_eq!(kept.len(), 51);

        // Dense seq check
        for (idx, entry) in kept.iter().enumerate() {
            assert_eq!(entry["seq"].as_u64().unwrap(), idx as u64);
        }

        let goal = kept.iter().find(|v| v["tx_type"] == "goal_state").unwrap();
        assert_eq!(goal["i"], 2, "last goal_state must survive");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_and_compact_noop_under_threshold() {
        let tmp = unique_tmp("tac-noop");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        fs::write(&path, "{\"a\":1}\n").unwrap();
        assert!(
            !truncate_and_compact(&path, 300).unwrap(),
            "under threshold"
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_and_compact_missing_file_is_noop() {
        let tmp = unique_tmp("tac-missing");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("nonexistent.jsonl");
        assert!(!truncate_and_compact(&path, 300).unwrap());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn truncate_and_compact_all_corrupt_truncates_to_zero() {
        let tmp = unique_tmp("tac-all-corrupt");
        fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.jsonl");
        fs::write(&path, "garbage\nmore-garbage\n").unwrap();
        assert!(
            truncate_and_compact(&path, 300).unwrap(),
            "should truncate all"
        );
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.is_empty(), "all corrupt → empty file");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compact_preserves_non_object_json_lines() {
        // Non-object JSON lines (arrays, primitives) must not be silently
        // dropped during compaction.  Build 101 lines: 98 objects (5 snapshot
        // goal_state + 93 steps) + 3 non-object lines.
        let mut buf = String::new();
        for i in 0..93 {
            buf.push_str(&json!({"tx_type": "step", "seq": i, "i": i}).to_string());
            buf.push('\n');
        }
        for i in 0..5 {
            buf.push_str(
                &json!({"tx_type": "goal_state", "seq": 93 + i, "version": i}).to_string(),
            );
            buf.push('\n');
        }
        // Non-object lines: array and primitive
        buf.push_str("[1,2,3]\n");
        buf.push_str("\"note\"\n");
        buf.push_str("42\n");

        let result = compact_jsonl_core(&buf, 100).unwrap();
        let compacted =
            result.expect("should compact (93+5=98 objects + 3 non-objects = 101 lines)");

        let lines: Vec<Value> = compacted
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        // 93 steps + 1 goal_state + 3 non-object lines = 97
        assert_eq!(lines.len(), 97, "must preserve all non-object lines");

        // The 3 non-object lines should be at the end
        assert_eq!(lines[94], json!([1, 2, 3]));
        assert_eq!(lines[95], json!("note"));
        assert_eq!(lines[96], json!(42));
    }
}
