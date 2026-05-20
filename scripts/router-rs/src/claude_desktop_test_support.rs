//! Process-wide unique temp directories for Claude Desktop hook tests.
//!
//! Parallel `cargo test` runs many tests in one process; per-module `AtomicU64`
//! counters that share the same path template can collide and cause flakes when
//! one test's `remove_dir_all` deletes another test's tree.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_REPO_SEQ: AtomicU64 = AtomicU64::new(0);

/// Allocate a unique directory under the system temp dir.
pub fn unique_temp_repo(prefix: &str) -> PathBuf {
    let seq = TEMP_REPO_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "router-rs-claude-desktop-{prefix}-{}-{seq}",
        std::process::id()
    ));
    path
}

/// Minimal continuity layout used by MCP integration tests.
pub fn seed_minimal_current_task_layout(repo: &Path) {
    let _ = std::fs::create_dir_all(repo.join("artifacts/current"));
    let _ = std::fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id": "test-task"}"#,
    );
    let _ = std::fs::write(
        repo.join("artifacts/current/SESSION_SUMMARY.md"),
        "# Test Session\n",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_temp_repo_never_reuses_sequence_slot() {
        let a = unique_temp_repo("a");
        let b = unique_temp_repo("b");
        assert_ne!(a, b);
    }
}
