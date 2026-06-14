//! Test-only shims for host-projection tests.
//!
//! Delegates to shared core-policy implementations where possible,
//! keeping legacy re-exports for caller compatibility.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Acquire a lock on the process environment for test isolation.
/// Delegates to `core_policy::test_env_sync` (single mutex across all crates).
pub use core_policy::test_env_sync::process_env_lock;

static TEMP_REPO_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Create a unique temporary repo path for testing.
pub fn unique_temp_repo(prefix: &str) -> PathBuf {
    let seq = TEMP_REPO_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "router-rs-mcp-stdio-{prefix}-{}-{seq}",
        std::process::id()
    ));
    path
}
