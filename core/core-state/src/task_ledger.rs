use core_errors::FrameworkError;
use crate::utils::path_guard::validate_task_id_component;
use crate::utils::task_write_lock::acquire_task_ledger_repo_lock;
use crate::utils::task_write_lock::TaskLedgerRepoLockGuard;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const TASK_LEDGER_FILENAME: &str = "TASK_LEDGER.jsonl";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LedgerTransaction {
    pub ts: String,
    pub tx_type: String,
    pub payload: Value,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub seq: Option<u64>,
    #[serde(default)]
    pub schema_version: Option<i64>,
}

pub fn task_ledger_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, FrameworkError> {
    let tid = validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts")
        .join("current")
        .join(tid)
        .join(TASK_LEDGER_FILENAME))
}

/// Append one ledger row while the caller already holds L1 (`apply_task_ledger_mutation`).
///
/// Optimised to avoid O(n) full deserialisation on every append:
/// - idempotency key check scans lines from the tail (recent duplicates are
///   near the end; rare worst-case still O(n) but constant-factor much smaller
///   because we skip full `serde_json::from_str` until a substring match).
/// - `seq` is derived from line count, not from a parsed `Vec`.
/// - Compaction reuses the in-memory post-append content, avoiding a file re-read.
pub fn append_transaction_assuming_l1_held(
    repo_root: &Path,
    task_id: &str,
    tx: LedgerTransaction,
) -> Result<(), String> {
    let path = task_ledger_path(repo_root, task_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create dir {}: {err}", parent.display()))?;
    }

    let mut compacted_inline = false;

    // Read-first pattern: avoids TOCTOU between is_file() and open().
    // When the file exists, content is already in hand; when NotFound, create fresh.
    match fs::read_to_string(&path) {
        Ok(mut content) => {
            // Repair any trailing corrupt lines before using content for idempotency
            // and before counting lines for seq.
            let truncated = crate::utils::jsonl_maintenance::truncate_corrupt_tail(&path)
                .unwrap_or(false);

            // When truncation occurred, re-read to get accurate content for both
            // idempotency check and line counting. This avoids a seq gap where
            // seq would be computed from pre-truncation line count.
            if truncated {
                content = fs::read_to_string(&path)
                    .map_err(|err| format!("re-read task ledger after truncate: {err}"))?;
            }

            // --- idempotency: reverse-scan lines, skip full parse when possible ---
            if let Some(ref new_key) = tx.idempotency_key {
                for line in content.lines().rev() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    // Cheap substring pre-filter: only pay for full deserialisation
                    // when the raw JSON line actually contains the key string.
                    if !line.contains(new_key.as_str()) {
                        continue;
                    }
                    if let Ok(existing_tx) = serde_json::from_str::<LedgerTransaction>(line)
                        && existing_tx.idempotency_key.as_deref() == Some(new_key.as_str()) {
                            return Ok(());
                        }
                }
            }

            // --- seq: count non-empty lines (no deserialisation needed) ---
            let line_count = content.lines().filter(|l| !l.trim().is_empty()).count() as u64;

            let mut final_tx = tx;
            final_tx.seq = Some(line_count);

            let serialized = serde_json::to_string(&final_tx)
                .map_err(|err| format!("failed to serialize transaction: {err}"))?;

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|err| format!("failed to open task ledger: {err}"))?;

            writeln!(file, "{}", serialized)
                .map_err(|err| format!("failed to write transaction: {err}"))?;
            file.sync_all()
                .map_err(|e| format!("fsync task_ledger failed: {e}"))?;
            drop(file);

            // ── compact using in-memory content (avoids re-read) ─────────────
            // Append the serialized new line to the pre-append content so the
            // compaction function sees the full post-append file state.
            content.push_str(&serialized);
            content.push('\n');

            match crate::utils::jsonl_maintenance::compact_jsonl_with_content(
                &path, &content, 300,
            ) {
                Ok(true) => {
                    // Compaction renumbers seq to 0,1,2,…N-1.
                    // TASK_STATE.json aggregate was removed in Wave 2b so there
                    // is nothing to sync here any longer.
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "compact_jsonl_with_content failed for TASK_LEDGER",
                    );
                }
            }
            compacted_inline = true;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File does not exist yet — first entry, seq = 0.
            let mut final_tx = tx;
            final_tx.seq = Some(0);

            let serialized = serde_json::to_string(&final_tx)
                .map_err(|err| format!("failed to serialize transaction: {err}"))?;

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|err| format!("failed to open task ledger: {err}"))?;

            writeln!(file, "{}", serialized)
                .map_err(|err| format!("failed to write transaction: {err}"))?;
            file.sync_all()
                .map_err(|e| format!("fsync task_ledger failed: {e}"))?;
        }
        Err(e) => return Err(format!("failed to read task ledger: {e}")),
    }

    // Fallback compaction for the NotFound branch (first entry, 1 line — won't trigger).
    if !compacted_inline
        && let Err(e) = crate::utils::jsonl_maintenance::compact_jsonl_if_needed(&path, 300) {
            tracing::warn!(error = %e, "compact_jsonl_if_needed failed for TASK_LEDGER");
        }

    Ok(())
}

pub fn append_transaction(
    repo_root: &Path,
    task_id: &str,
    tx: LedgerTransaction,
) -> Result<(), String> {
    let _guard: TaskLedgerRepoLockGuard = acquire_task_ledger_repo_lock(repo_root, Duration::from_millis(500))?;
    append_transaction_assuming_l1_held(repo_root, task_id, tx)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("router_rs_task_ledger_{label}_{nanos}"))
    }

    #[test]
    fn append_transaction_rejects_unsafe_task_id() {
        let prev = std::env::var_os("ROUTER_RS_TASK_LEDGER_FLOCK");
        // SAFETY: test-only; ENV_LOCK prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_TASK_LEDGER_FLOCK") };
        let tmp = unique_tmp("unsafe-id");
        fs::create_dir_all(tmp.join("artifacts/current")).expect("mkdir");
        let tx = LedgerTransaction {
            ts: "2026-01-01T00:00:00Z".to_string(),
            tx_type: "goal_state".to_string(),
            payload: serde_json::json!({"status": "running"}),
            idempotency_key: None,
            seq: None,
            schema_version: Some(1),
        };
        for bad in ["", "../x", "a/b", ".."] {
            let err = append_transaction(&tmp, bad, tx.clone()).expect_err("reject");
            assert!(
                err.contains("task_id must be a single safe path component"),
                "bad id {bad:?}: {err}"
            );
        }
        match prev {
            // SAFETY: test-only; ENV_LOCK prevents concurrent env access from other tests.
            Some(p) => unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_TASK_LEDGER_FLOCK", &p) },
            // SAFETY: test-only; ENV_LOCK prevents concurrent env access from other tests.
            None => unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_TASK_LEDGER_FLOCK") },
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn append_transaction_assuming_l1_held_writes_seq() {
        let tmp = unique_tmp("assume-held");
        fs::create_dir_all(tmp.join("artifacts/current/t1")).expect("mkdir");
        let tx = LedgerTransaction {
            ts: "2026-01-01T00:00:00Z".to_string(),
            tx_type: "goal_state".to_string(),
            payload: serde_json::json!({"status": "running"}),
            idempotency_key: None,
            seq: None,
            schema_version: Some(1),
        };
        append_transaction_assuming_l1_held(&tmp, "t1", tx).expect("append");
        let path = task_ledger_path(&tmp, "t1").expect("path");
        let raw = fs::read_to_string(&path).expect("read");
        let line: LedgerTransaction = serde_json::from_str(raw.trim()).expect("parse");
        assert_eq!(line.seq, Some(0));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn append_dedup_by_idempotency_key_and_seq_increments() {
        let tmp = unique_tmp("dedup");
        fs::create_dir_all(tmp.join("artifacts/current/t2")).expect("mkdir");

        // Append three distinct transactions.
        for i in 0..3u64 {
            let tx = LedgerTransaction {
                ts: format!("2026-01-01T00:00:0{i}Z"),
                tx_type: "step".to_string(),
                payload: serde_json::json!({"i": i}),
                idempotency_key: Some(format!("key-{i}")),
                seq: None,
                schema_version: Some(1),
            };
            append_transaction_assuming_l1_held(&tmp, "t2", tx).expect("append");
        }

        // Re-submit key-1 — must be silently deduped.
        let dup = LedgerTransaction {
            ts: "2026-01-01T00:00:99Z".to_string(),
            tx_type: "step".to_string(),
            payload: serde_json::json!({"dup": true}),
            idempotency_key: Some("key-1".to_string()),
            seq: None,
            schema_version: Some(1),
        };
        append_transaction_assuming_l1_held(&tmp, "t2", dup).expect("dedup should be no-op");

        // File must still have exactly 3 lines.
        let path = task_ledger_path(&tmp, "t2").expect("path");
        let raw = fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 3, "duplicate must not add a new line");

        // seq values must be 0, 1, 2.
        for (idx, line) in lines.iter().enumerate() {
            let tx: LedgerTransaction = serde_json::from_str(line).expect("parse");
            assert_eq!(tx.seq, Some(idx as u64));
        }

        let _ = fs::remove_dir_all(&tmp);
    }
}
