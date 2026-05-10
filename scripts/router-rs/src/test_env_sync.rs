//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Unit tests run in parallel threads by default, but [`std::env`] is process-global.
//! Several hooks consult `ROUTER_RS_*` variables; tests that `set_var` / `remove_var`
//! must serialize against each other across `main_tests` and `cursor_hooks` test modules.

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

/// Serialize mutations to process environment for tests (hold for the whole test body).
#[cfg(test)]
pub fn process_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("router-rs test process env lock poisoned")
}
