use crate::router_env_flags::router_rs_task_ledger_flock_enabled;
use crate::task_write_lock::TASK_LEDGER_LOCK_BASENAME;
use fs2::FileExt;
use std::fs::{self, OpenOptions, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::thread;
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

pub fn task_ledger_path(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("current")
        .join(task_id)
        .join(TASK_LEDGER_FILENAME)
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
                    return Err(format!("task ledger lock: flock {} failed: {err}", lock_path.display()));
                }
                if start.elapsed() > timeout {
                    return Err(format!("task ledger lock: flock {} timeout after {:?}", lock_path.display(), timeout));
                }
                thread::sleep(delay);
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }
    }
    Ok(TaskLedgerLockGuard { _file: Some(file) })
}

pub fn append_transaction(
    repo_root: &Path,
    task_id: &str,
    tx: LedgerTransaction,
) -> Result<(), String> {
    let _guard = acquire_task_ledger_lock_with_timeout(repo_root, Duration::from_millis(500))?;
    let path = task_ledger_path(repo_root, task_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create dir {}: {err}", parent.display()))?;
    }

    // Read existing transactions to verify idempotency and compute sequence number
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
                // Deduplicate by idempotency key if present
                if let (Some(ref key), Some(ref existing_key)) = (&tx.idempotency_key, &existing_tx.idempotency_key) {
                    if key == existing_key {
                        // Already written, skip writing but return success
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

    use std::io::Write;
    writeln!(file, "{}", serialized)
        .map_err(|err| format!("failed to write transaction: {err}"))?;

    Ok(())
}
