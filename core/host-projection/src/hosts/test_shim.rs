//! Test-only shims for host-projection tests.
//!
//! These replace `crate::test_env_sync::process_env_lock()` and
//! `crate::harness_operator_nudges::harness_nudges_env_test_lock()`
//! from runtime-core that were used in test code.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

static PROCESS_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static TEMP_REPO_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Acquire a lock on the process environment for test isolation.
/// Replaces `crate::test_env_sync::process_env_lock()` from runtime-core.
pub fn process_env_lock() -> MutexGuard<'static, ()> {
    PROCESS_ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Acquire a lock on harness nudges env for test isolation.
/// Replaces `crate::harness_operator_nudges::harness_nudges_env_test_lock()`.
pub fn harness_nudges_env_test_lock() -> MutexGuard<'static, ()> {
    // Reuse the same global lock for simplicity in tests
    process_env_lock()
}

/// Create a unique temporary repo path for testing.
/// Replaces `crate::mcp_stdio_test_support::unique_temp_repo()`.
pub fn unique_temp_repo(prefix: &str) -> PathBuf {
    let seq = TEMP_REPO_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "router-rs-mcp-stdio-{prefix}-{}-{seq}",
        std::process::id()
    ));
    path
}
