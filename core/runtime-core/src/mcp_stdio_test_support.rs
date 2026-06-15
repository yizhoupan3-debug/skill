//! Process-wide unique temp directories for MCP stdio harness tests.
//!
//! Parallel `cargo test` runs many tests in one process; per-module `AtomicU64`
//! counters that share the same path template can collide and cause flakes when
//! one test's `remove_dir_all` deletes another test's tree.

use std::path::{Path, PathBuf};

/// Allocate a unique directory under the system temp dir.
/// Delegates to `core_policy::test_env_sync::unique_temp_repo` to share the same
/// sequence counter with `host-projection`.
pub fn unique_temp_repo(prefix: &str) -> PathBuf {
    core_policy::test_env_sync::unique_temp_repo(prefix)
}

/// Copy hot routing index into a temp repo for MCP routing tests.
pub fn seed_skill_routing_runtime(repo: &Path) {
    let skills = repo.join("skills");
    let _ = std::fs::create_dir_all(&skills);
    let src =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_ROUTING_RUNTIME.json");
    if src.is_file() {
        let _ = std::fs::copy(src, skills.join("SKILL_ROUTING_RUNTIME.json"));
    }
}

/// Copy `RUNTIME_REGISTRY.json` so MCP tests resolve `lifecycle_profiles` (e.g. my-light).
pub fn seed_runtime_registry(repo: &Path) {
    let dest_dir = repo.join("configs/framework");
    let _ = std::fs::create_dir_all(&dest_dir);
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../configs/framework/RUNTIME_REGISTRY.json");
    if src.is_file() {
        let _ = std::fs::copy(&src, dest_dir.join("RUNTIME_REGISTRY.json"));
    }
}

/// Minimal continuity layout used by MCP integration tests.
pub fn seed_minimal_current_task_layout(repo: &Path) {
    seed_runtime_registry(repo);
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
