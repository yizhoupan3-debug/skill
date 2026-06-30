//! Chain Engine — background polling thread and top-level orchestration.
//!
//! The engine runs a polling loop that periodically:
//! 1. Loads TASK_CHAIN.json
//! 2. Runs the DAG scheduler (`advance_dag`)
//! 3. Processes timeouts and failures
//! 4. Generates CHAIN_OUTPUT.json when the chain completes
//! 5. Writes back to disk
//!
//! NOTE: Currently does NOT use cross-process file locking. The `poll_tick`
//! RMW cycle is not lock-protected. For production use with concurrent
//! `chain_dag_tick` callers, wrap TASK_CHAIN.json access with
//! `apply_task_ledger_mutation`.

use core_errors::FrameworkError;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::compat;
use crate::scheduler::{advance_dag, is_chain_complete, load_advance_write, write_chain_file};
use crate::tracker::process_post_tick;
use crate::types::ChainDagRoot;

/// Default polling interval in seconds.
pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;

/// Spawn a background polling thread that drives a DAG chain to completion.
///
/// The thread:
/// - Loads TASK_CHAIN.json every `interval_secs` seconds
/// - Calls `advance_dag()` to transition ready tasks to running
/// - Calls `process_post_tick()` for timeouts and failures
/// - Writes back to disk
/// - Exits when the chain is complete, paused, or the stop flag is set.
///
/// Returns a `Drop` guard that stops the poller when dropped (or when
/// `stop()` is called on the handle).
pub fn spawn_dag_poller(
    repo_root: &Path,
    chain_id: &str,
    interval_secs: Option<u64>,
) -> Result<PollerHandle, FrameworkError> {
    let repo = repo_root.to_path_buf();
    let cid = chain_id.to_string();
    let interval = interval_secs.unwrap_or(DEFAULT_POLL_INTERVAL_SECS);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag = stop_flag.clone();

    let handle = thread::Builder::new()
        .name(format!("chain-poller-{chain_id}"))
        .spawn(move || {
            poll_loop(&repo, &cid, interval, &flag);
        })
        .map_err(|e| {
            FrameworkError::config(format!("failed to spawn chain poller thread: {e}"))
        })?;

    Ok(PollerHandle {
        thread: Some(handle),
        stop_flag,
    })
}

/// The main polling loop with exponential backoff on errors.
fn poll_loop(repo_root: &Path, chain_id: &str, interval_secs: u64, stop: &AtomicBool) {
    let path = crate::chain_file_path(repo_root);
    let base_interval = Duration::from_secs(interval_secs);
    let max_backoff = Duration::from_secs(60);
    let mut error_backoff = Duration::ZERO;

    loop {
        thread::sleep(base_interval + error_backoff);

        // Check stop signal
        if stop.load(Ordering::Relaxed) {
            tracing::info!(%chain_id, "chain poller stopped");
            return;
        }

        // Load, advance, process, write
        let result = poll_tick(repo_root, &path);
        match result {
            Ok(true) => {
                tracing::info!(%chain_id, "chain completed — poller exiting");
                return;
            }
            Ok(false) => {
                error_backoff = Duration::ZERO; // Reset on success
            }
            Err(e) => {
                tracing::warn!(%chain_id, error = %e, "chain poller tick failed");
                // Exponential backoff: 5s, 10s, 20s, 40s, max 60s
                error_backoff = if error_backoff == Duration::ZERO {
                    Duration::from_secs(5)
                } else {
                    (error_backoff * 2).min(max_backoff)
                };
            }
        }
    }
}

/// Execute a single poll tick. Returns `true` if the chain is complete.
fn poll_tick(repo_root: &Path, path: &Path) -> Result<bool, FrameworkError> {
    if !path.is_file() {
        return Err(FrameworkError::not_found("TASK_CHAIN.json not found".to_string()));
    }

    let mut root = compat::load_chain_file(path)?;

    // Check if paused
    if root.paused {
        return Ok(false);
    }

    // Check if already complete
    if is_chain_complete(&root) {
        return Ok(true);
    }

    // Run the scheduler
    advance_dag(&mut root);

    // Process timeouts and failures
    process_post_tick(&mut root);

    // Write back
    write_chain_file(path, &root)?;

    // Generate CHAIN_OUTPUT.json on completion
    if is_chain_complete(&root) {
        let _ = core_state::chain_output::build_and_write_chain_aggregate(repo_root);
        return Ok(true);
    }

    Ok(false)
}

/// A handle to a running poller thread. Dropping the handle stops the poller.
pub struct PollerHandle {
    thread: Option<thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

impl PollerHandle {
    /// Signal the poller to stop on its next iteration and join the thread.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Signal the poller to stop without joining (non-blocking).
    /// Use when blocking on join is not acceptable (e.g. in Drop).
    pub fn signal_stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Check if the poller thread is still running.
    pub fn is_running(&self) -> bool {
        if self.stop_flag.load(Ordering::Relaxed) {
            return false;
        }
        self.thread.as_ref().map_or(false, |h| !h.is_finished())
    }
}

impl Drop for PollerHandle {
    fn drop(&mut self) {
        // Signal stop but don't block on join (avoid indefinite blocking in Drop).
        self.signal_stop();
        // The thread continues running; it will exit on next iteration.
        // If attached JoinHandle is dropped without join, the thread is detached.
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::types::*;
    use std::fs;

    fn setup_test_chain(repo_root: &Path, mode: ChainMode) -> std::path::PathBuf {
        let chain_path = repo_root.join("artifacts/current/TASK_CHAIN.json");
        if let Some(parent) = chain_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let root = ChainDagRoot {
            chain_id: "test-chain".to_string(),
            mode,
            tasks: vec![
                DagTaskEntry {
                    task_id: "a".to_string(),
                    status: TaskStatus::Pending,
                    ..DagTaskEntry::new("a")
                },
            ],
            ..ChainDagRoot::new("test-chain", vec![])
        };
        write_chain_file(&chain_path, &root).unwrap();
        chain_path
    }

    #[test]
    fn poll_tick_advances_chain() {
        let repo = std::env::temp_dir().join("chain-poller-test-tick");
        let _ = fs::remove_dir_all(&repo);
        let chain_path = setup_test_chain(&repo, ChainMode::Dag);

        let complete = poll_tick(&repo, &chain_path).unwrap();
        assert!(!complete, "chain should not be complete after first tick");

        // Verify 'a' was transitioned to running
        let root = compat::load_chain_file(&chain_path).unwrap();
        assert_eq!(root.tasks[0].status, TaskStatus::Running);

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn poll_tick_idempotent_no_crash_on_empty() {
        let repo = std::env::temp_dir().join("chain-poller-idempotent");
        let _ = fs::remove_dir_all(&repo);
        let chain_path = repo.join("artifacts/current/TASK_CHAIN.json");
        // Chain doesn't exist — should error, not crash
        let result = poll_tick(&repo, &chain_path);
        assert!(result.is_err(), "non-existent chain should error");
        let _ = fs::remove_dir_all(&repo);
    }
}
