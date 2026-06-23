//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Used by `core-policy` unit tests and `router-rs` hook integration tests (via `test-sync` feature).

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

thread_local! {
    static ENV_LOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn env_mutex() -> &'static Mutex<()> {
    static MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    MUTEX.get_or_init(|| Mutex::new(()))
}

fn lock_env_mutex() -> std::sync::MutexGuard<'static, ()> {
    env_mutex()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Reentrant guard around the process-global env mutex.
pub struct ProcessEnvLockGuard(Option<std::sync::MutexGuard<'static, ()>>);

impl Drop for ProcessEnvLockGuard {
    fn drop(&mut self) {
        let after = ENV_LOCK_DEPTH.with(|c| {
            let v = c.get();
            debug_assert!(v > 0, "env lock depth underflow");
            c.set(v - 1);
            v - 1
        });
        if after == 0 {
            drop(self.0.take());
        }
    }
}

pub fn process_env_lock_held() -> bool {
    ENV_LOCK_DEPTH.with(|c| c.get() > 0)
}

/// Serialize mutations to process environment for tests (hold for the whole test body).
pub fn process_env_lock() -> ProcessEnvLockGuard {
    let depth = ENV_LOCK_DEPTH.with(|c| {
        let v = c.get() + 1;
        c.set(v);
        v
    });
    if depth == 1 {
        crate::hook_common::set_test_interactive_override(None);
        ProcessEnvLockGuard(Some(lock_env_mutex()))
    } else {
        ProcessEnvLockGuard(None)
    }
}

/// Safe wrapper for `std::env::set_var` — acquires the env mutex first.
///
/// # Safety context
/// Rust 2024 marks `env::set_var` as `unsafe` because concurrent reads/writes
/// to the process environment are undefined behavior. This wrapper serializes
/// access via the shared test mutex, making it sound for test code.
///
/// # Panics
/// Panics if `key` contains `=` or NUL, or if `value` contains NUL
/// (same contract as `std::env::set_var`).
///
/// # Usage
/// ```ignore
/// use core_policy::test_env_sync::with_env_var;
/// // Mutex guard held for the scope of the closure; restored on drop.
/// with_env_var("MY_FLAG", "1", || {
///     assert!(my_function());
/// });
/// ```
pub fn with_env_var(key: &str, value: &str, f: impl FnOnce()) {
    let _guard = process_env_lock();
    let prev = std::env::var(key).ok();
    // SAFETY: mutex held; no other thread can mutate env concurrently.
    unsafe { std::env::set_var(key, value) };
    f();
    match prev {
        Some(v) => {
            // SAFETY: mutex held.
            unsafe { std::env::set_var(key, v) };
        }
        None => {
            // SAFETY: mutex held.
            unsafe { std::env::remove_var(key) };
        }
    }
}

/// Allocate a unique directory under the system temp dir.
/// Shared so that `runtime-core` and `host-projection` don't duplicate this logic.
pub fn unique_temp_repo(prefix: &str) -> PathBuf {
    static TEMP_REPO_SEQ: AtomicUsize = AtomicUsize::new(0);
    let seq = TEMP_REPO_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "router-rs-mcp-stdio-{prefix}-{}-{seq}",
        std::process::id()
    ));
    path
}

/// Serialize nudge-based tests that depend on env vars (used by harness tests).
///
/// Placed here so that `runtime-core` and `host-projection` can share the same lock
/// without duplicating the `OnceLock<Mutex<()>>` definition.
pub fn harness_nudges_env_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static NUDGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    NUDGE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Safe wrapper for `std::env::remove_var` — acquires the env mutex first.
///
/// Restores the previous value (or absence) after `f` returns.
pub fn with_env_var_removed(key: &str, f: impl FnOnce()) {
    let _guard = process_env_lock();
    let prev = std::env::var(key).ok();
    // SAFETY: mutex held.
    unsafe { std::env::remove_var(key) };
    f();
    match prev {
        Some(v) => {
            // SAFETY: mutex held.
            unsafe { std::env::set_var(key, v) };
        }
        None => {
            // SAFETY: mutex held.
            unsafe { std::env::remove_var(key) };
        }
    }
}
