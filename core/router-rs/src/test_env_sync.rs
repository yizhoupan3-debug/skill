//! Cross-module test synchronization for process-global environment reads/writes.
//!
//! Delegates to `framework_core::test_env_sync` so `main_tests`, `host_extensions::cursor`, and
//! `core-policy` unit tests share one process-wide mutex (parallel `#[test]` safe).

#[cfg(test)]
pub use framework_core::test_env_sync::ProcessEnvLockGuard;

/// Serialize env / hook test overrides (`ROUTER_RS_*`, interactive override, etc.).
#[cfg(test)]
pub fn process_env_lock() -> ProcessEnvLockGuard {
    crate::touch_test_kernel_bootstrap();
    framework_core::test_env_sync::process_env_lock()
}
