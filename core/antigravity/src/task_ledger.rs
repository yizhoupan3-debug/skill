use crate::utils::path_guard::validate_task_id_component;
use crate::utils::task_write_lock::router_rs_task_ledger_flock_enabled;
use crate::utils::task_write_lock::TASK_LEDGER_LOCK_BASENAME;
use fs2::FileExt;
use std::fs::{self, OpenOptions, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

pub struct TaskLedgerLockGuard {
    _file: Option<File>,
}

pub fn task_ledger_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    let tid = validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts")
        .join("current")
        .join(tid)
        .join(TASK_LEDGER_FILENAME))
}

pub fn acquire_task_ledger_lock_with_timeout(
    repo_root: &Path,
    timeout: Duration,
) -> Result<TaskLedgerLockGuard, String> {
    if !router_rs_task_ledger_flock_enabled() {
        return Ok(TaskLedgerLockGuard { _file: None });
    }
    let current = repo_root.join("artifacts").join("current");
    let lock_path = current.join(TASK_LEDGER_LOCK_BASENAME);
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| format!("task ledger lock: open {} failed: {err}", lock_path.display()))?;

    let mut delay = Duration::from_millis(10);
    let max_delay = Duration::from_millis(200);
    let start = Instant::now();

    loop {
        match file.try_lock_exclusive() {
            Ok(_) => break,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    return Err(format!(
                        "task ledger lock: flock {} failed: {err}",
                        lock_path.display()
                    ));
                }
                if start.elapsed() > timeout {
                    return Err(format!(
                        "task ledger lock: flock {} timeout after {:?}",
                        lock_path.display(),
                        timeout
                    ));
                }
                thread::sleep(delay);
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }
    }
    Ok(TaskLedgerLockGuard { _file: Some(file) })
}

/// Append one ledger row while the caller already holds L1 (`apply_task_ledger_mutation`).
///
/// Optimised to avoid O(n) full deserialisation on every append:
/// - idempotency key check scans lines from the tail (recent duplicates are
///   near the end; rare worst-case still O(n) but constant-factor much smaller
///   because we skip full `serde_json::from_str` until a substring match).
/// - `seq` is derived from line count, not from a parsed `Vec`.
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

    if path.is_file() {
        // Repair any trailing corrupt lines before reading.
        if let Err(e) = crate::utils::jsonl_maintenance::truncate_corrupt_tail(&path) {
            eprintln!("[router-rs] truncate_corrupt_tail failed for TASK_LEDGER: {e}");
        }

        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read task ledger: {err}"))?;

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
                if let Ok(existing_tx) = serde_json::from_str::<LedgerTransaction>(line) {
                    if existing_tx.idempotency_key.as_deref() == Some(new_key.as_str()) {
                        return Ok(());
                    }
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
    } else {
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

    // Auto-compact when the file grows past 100 lines.
    if let Err(e) = crate::utils::jsonl_maintenance::compact_jsonl_if_needed(&path, 100) {
        eprintln!("[router-rs] compact_jsonl_if_needed failed for TASK_LEDGER: {e}");
    }

    Ok(())
}

pub fn append_transaction(
    repo_root: &Path,
    task_id: &str,
    tx: LedgerTransaction,
) -> Result<(), String> {
    let _guard = acquire_task_ledger_lock_with_timeout(repo_root, Duration::from_millis(500))?;
    append_transaction_assuming_l1_held(repo_root, task_id, tx)
}

#[cfg(test)]
mod tests {
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
        std::env::remove_var("ROUTER_RS_TASK_LEDGER_FLOCK");
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
            Some(p) => std::env::set_var("ROUTER_RS_TASK_LEDGER_FLOCK", p),
            None => std::env::remove_var("ROUTER_RS_TASK_LEDGER_FLOCK"),
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
