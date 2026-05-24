use crate::path_guard::validate_task_id_component;
use crate::router_env_flags::router_rs_task_ledger_flock_enabled;
use crate::task_write_lock::TASK_LEDGER_LOCK_BASENAME;
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
    let max_delay = Duration::from_millis(50);
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
pub(crate) fn append_transaction_assuming_l1_held(
    repo_root: &Path,
    task_id: &str,
    tx: LedgerTransaction,
) -> Result<(), String> {
    let path = task_ledger_path(repo_root, task_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create dir {}: {err}", parent.display()))?;
    }

    let mut transactions = Vec::new();
    if path.is_file() {
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read task ledger: {err}"))?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(existing_tx) = serde_json::from_str::<LedgerTransaction>(line) {
                if let (Some(ref key), Some(ref existing_key)) =
                    (&tx.idempotency_key, &existing_tx.idempotency_key)
                {
                    if key == existing_key {
                        return Ok(());
                    }
                }
                transactions.push(existing_tx);
            }
        }
    }

    let next_seq = transactions.len() as u64;
    let mut final_tx = tx;
    final_tx.seq = Some(next_seq);

    let serialized = serde_json::to_string(&final_tx)
        .map_err(|err| format!("failed to serialize transaction: {err}"))?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("failed to open task ledger: {err}"))?;

    writeln!(file, "{}", serialized)
        .map_err(|err| format!("failed to write transaction: {err}"))?;

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
    use crate::test_env_sync::process_env_lock;
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
        let _g = process_env_lock();
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
        let _g = process_env_lock();
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
}
