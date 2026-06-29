//! Test-only shims for host-projection tests.
//!
//! Delegates to shared core-policy implementations where possible,
//! keeping legacy re-exports for caller compatibility.

use std::path::PathBuf;

/// Acquire a lock on the process environment for test isolation.
/// Delegates to `framework_core::test_env_sync` (single mutex across all crates).
pub use framework_core::test_env_sync::process_env_lock;

/// Create a unique temporary repo path for testing.
/// Delegates to `framework_core::test_env_sync::unique_temp_repo` to share the same
/// sequence counter with `runtime-core`.
pub fn unique_temp_repo(prefix: &str) -> PathBuf {
    framework_core::test_env_sync::unique_temp_repo(prefix)
}
