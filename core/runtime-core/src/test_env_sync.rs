//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Delegates to `framework_core::test_env_sync` so tests share one process-wide mutex.

#[cfg(test)]
pub use framework_core::test_env_sync::{ProcessEnvLockGuard, process_env_lock_held};

/// Serialize env / hook test overrides.
#[cfg(test)]
pub fn process_env_lock() -> ProcessEnvLockGuard {
    crate::touch_test_kernel_bootstrap();
    framework_core::test_env_sync::process_env_lock()
}
