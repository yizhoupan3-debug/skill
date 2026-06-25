//! Cross-process **advisory flock** for **continuity ledger** mutations under a repo’s
//! `artifacts/current/**`: `GOAL_STATE.json`, `RFV_LOOP_STATE.json`, `STEP_LEDGER.jsonl` append,
//! session artifact batch writes, and `EVIDENCE_INDEX.json` read-modify-write.
//!
//! Serialization for multiple hook processes is **`flock(2)`** on
//! `artifacts/current/.router-rs.task-ledger.lock`. A legacy process-local `Mutex` is not used
//! here — separate router-rs hook subprocesses cannot share Rust `std::sync::Mutex`.
//!
//! **`ROUTER_RS_TASK_LEDGER_FLOCK`** (default ON; `0`|`false`|`off`|`no` disables): skip flock when
//! the filesystem rejects locks — parallel ledger writes remain best-effort; see harness docs.
//!
//! [`apply_task_ledger_mutation`] must stay consistent with per-path wrappers: **repo flock first**,
//! then narrower locks such as `runtime_storage::acquire_runtime_path_lock` where applicable.
//! The in-memory `runtime_storage` regression backend uses a process-local mutex for `append_text`
//! only; it does **not** participate in the repo-wide task-ledger flock.

/// Canonical env flag for task-ledger flock. Parses `ROUTER_RS_TASK_LEDGER_FLOCK`
/// (default ON; `0`/`false`/`off`/`no` disables).
/// NOTE: A matching function lives in core-policy/env_flags.rs for crates that
/// depend on core-policy but cannot depend on core-state-utils (circular dep).
/// The two copies are intentionally kept in sync.
pub fn router_rs_task_ledger_flock_enabled() -> bool {
    match std::env::var("ROUTER_RS_TASK_LEDGER_FLOCK") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t == "0" || t == "false" || t == "off" || t == "no")
        }
        Err(_) => true,
    }
}
use fs2::FileExt;
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub const TASK_LEDGER_LOCK_BASENAME: &str = ".router-rs.task-ledger.lock";

/// Holds `artifacts/current/.router-rs.task-ledger.lock` open + exclusively locked until dropped.
pub struct TaskLedgerRepoLockGuard {
    _file: Option<std::fs::File>,
}

fn ledger_lock_abs_path(repo_root: &Path) -> std::path::PathBuf {
    repo_root
        .join("artifacts")
        .join("current")
        .join(TASK_LEDGER_LOCK_BASENAME)
}

/// Acquire an exclusive cross-process lock for all task-ledger writers sharing this `repo_root`.
pub fn acquire_task_ledger_repo_lock(
    repo_root: &Path,
    timeout: Duration,
) -> Result<TaskLedgerRepoLockGuard, String> {
    if !router_rs_task_ledger_flock_enabled() {
        return Ok(TaskLedgerRepoLockGuard { _file: None });
    }
    let current = repo_root.join("artifacts").join("current");
    fs::create_dir_all(&current).map_err(|err| {
        format!(
            "task ledger lock: create_dir_all {} failed: {err}",
            current.display()
        )
    })?;
    let lock_path = ledger_lock_abs_path(repo_root);
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| {
            format!(
                "task ledger lock: open {} failed: {err}",
                lock_path.display()
            )
        })?;
    let mut delay = Duration::from_millis(10);
    let max_delay = Duration::from_millis(200);
    let start = Instant::now();

    loop {
        match file.try_lock_exclusive() {
            Ok(_) => break,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::WouldBlock {
                    return Err(format!(
                        "task ledger lock: flock {} failed with fatal error: {err}",
                        lock_path.display()
                    ));
                }
                if start.elapsed() > timeout {
                    return Err(format!(
                        "task ledger lock: flock {} timeout after {:?} (concurrent write contention): {err}",
                        lock_path.display(),
                        timeout
                    ));
                }
                thread::sleep(delay);
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }
    }
    Ok(TaskLedgerRepoLockGuard { _file: Some(file) })
}

/// Run `f` while holding the repo task-ledger flock (when enabled via env).
pub fn apply_task_ledger_mutation<T>(
    repo_root: &Path,
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _guard = acquire_task_ledger_repo_lock(repo_root, Duration::from_secs(30))?;
    f()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TASK_LEDGER_FLOCK_ENV_TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn unique_tmp() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "router_rs_task_ledger_lock_{}_{}",
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn apply_runs_closure_under_lock() {
        let _env_guard = TASK_LEDGER_FLOCK_ENV_TEST_MUTEX
            .lock()
            .expect("task ledger flock env mutex poisoned");
        let prev = std::env::var_os("ROUTER_RS_TASK_LEDGER_FLOCK");
        // SAFETY: test-only; TASK_LEDGER_FLOCK_ENV_TEST_MUTEX prevents concurrent env access from other tests.
        unsafe { crate::env_sync::remove_env("ROUTER_RS_TASK_LEDGER_FLOCK") };
        let tmp = unique_tmp();
        fs::create_dir_all(tmp.join("artifacts/current")).expect("mkdir");
        let v = apply_task_ledger_mutation(&tmp, || Ok(7_u8)).expect("apply");
        assert_eq!(v, 7);
        assert!(
            ledger_lock_abs_path(&tmp).is_file(),
            "expected lock sentinel file"
        );
        match prev {
            Some(p) => {
                // SAFETY: test-only; TASK_LEDGER_FLOCK_ENV_TEST_MUTEX prevents concurrent env access from other tests.
                unsafe { crate::env_sync::set_env("ROUTER_RS_TASK_LEDGER_FLOCK", &p) }
            }
            None => {
                // SAFETY: test-only; TASK_LEDGER_FLOCK_ENV_TEST_MUTEX prevents concurrent env access from other tests.
                unsafe { crate::env_sync::remove_env("ROUTER_RS_TASK_LEDGER_FLOCK") }
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn flock_disabled_skips_flock_but_runs_closure() {
        let _env_guard = TASK_LEDGER_FLOCK_ENV_TEST_MUTEX
            .lock()
            .expect("task ledger flock env mutex poisoned");
        let prev = std::env::var_os("ROUTER_RS_TASK_LEDGER_FLOCK");
        // SAFETY: test-only; TASK_LEDGER_FLOCK_ENV_TEST_MUTEX prevents concurrent env access from other tests.
        unsafe { crate::env_sync::set_env("ROUTER_RS_TASK_LEDGER_FLOCK", "0") };
        let tmp = unique_tmp();
        fs::create_dir_all(tmp.join("artifacts/current")).expect("mkdir");
        let v = apply_task_ledger_mutation(&tmp, || Ok(9_u8)).expect("apply");
        assert_eq!(v, 9);
        match prev {
            Some(p) => {
                // SAFETY: test-only; TASK_LEDGER_FLOCK_ENV_TEST_MUTEX prevents concurrent env access from other tests.
                unsafe { crate::env_sync::set_env("ROUTER_RS_TASK_LEDGER_FLOCK", &p) }
            }
            None => {
                // SAFETY: test-only; TASK_LEDGER_FLOCK_ENV_TEST_MUTEX prevents concurrent env access from other tests.
                unsafe { crate::env_sync::remove_env("ROUTER_RS_TASK_LEDGER_FLOCK") }
            }
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    // NOTE: The include_str! assertions that validated core-state's goal_ops.rs and
    // step_ledger.rs invariants have been removed during the core-state-utils extraction.
    // Those invariants are now validated by core-state's own test suite.
}
