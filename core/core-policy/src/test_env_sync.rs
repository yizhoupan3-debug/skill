//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Used by `core-policy` unit tests and `router-rs` hook integration tests (via `test-sync` feature).

use std::cell::Cell;
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
        crate::hook_common::set_test_my_light_override(None);
        ProcessEnvLockGuard(Some(lock_env_mutex()))
    } else {
        ProcessEnvLockGuard(None)
    }
}
