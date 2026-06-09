//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Delegates to `core_policy::test_env_sync` so tests share one process-wide mutex.

#[cfg(test)]
pub use core_policy::test_env_sync::{process_env_lock_held, ProcessEnvLockGuard};

/// Serialize env / hook test overrides.
#[cfg(test)]
pub fn process_env_lock() -> ProcessEnvLockGuard {
    crate::touch_test_kernel_bootstrap();
    core_policy::test_env_sync::process_env_lock()
}
