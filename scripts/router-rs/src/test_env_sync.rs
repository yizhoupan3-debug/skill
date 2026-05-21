//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Unit tests run in parallel threads by default, but [`std::env`] is process-global.
//! Several hooks consult `ROUTER_RS_*` variables; tests that `set_var` / `remove_var`
//! must serialize against each other across `main_tests` and `cursor_hooks` test modules.

#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
thread_local! {
    static ENV_LOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Reentrant guard around the process-global env mutex (safe for `run_gate` nested in tests).
#[cfg(test)]
pub struct ProcessEnvLockGuard(Option<std::sync::MutexGuard<'static, ()>>);

#[cfg(test)]
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

/// Serialize mutations to process environment for tests (hold for the whole test body).
#[cfg(test)]
pub fn process_env_lock() -> ProcessEnvLockGuard {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let was = ENV_LOCK_DEPTH.with(|c| {
        let w = c.get();
        c.set(w + 1);
        w
    });
    if was == 0 {
        ProcessEnvLockGuard(Some(
            LOCK.get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        ))
    } else {
        ProcessEnvLockGuard(None)
    }
}
