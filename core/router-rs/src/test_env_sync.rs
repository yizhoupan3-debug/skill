//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Delegates to `core_policy::test_env_sync` so `main_tests`, `host_extensions::cursor`, and
//! `core-policy` unit tests share one process-wide mutex (parallel `#[test]` safe).

#[cfg(test)]
pub use core_policy::test_env_sync::ProcessEnvLockGuard;

/// Serialize env / hook test overrides (`ROUTER_RS_*`, my-light override, etc.).
#[cfg(test)]
pub fn process_env_lock() -> ProcessEnvLockGuard {
    crate::touch_test_kernel_bootstrap();
    core_policy::test_env_sync::process_env_lock()
}
