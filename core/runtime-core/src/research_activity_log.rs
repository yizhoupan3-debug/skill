//! 科研活动内联日志：委托 research-harness 独立实现。
//!
//! 保留 runtime-core 的函数签名以维持 host-projection 的函数指针注册兼容。

use std::path::Path;

/// 科研活动日志记录器。在 PostToolUse 中被调用。
///
/// 委托 research-harness 的 `maybe_log_research_activity` 实现。
pub fn record_research_activity(repo_root: &Path, tool_name: &str, summary: &str) {
    if let Err(e) = research_harness::hooks::activity_log::maybe_log_research_activity(
        tool_name, summary, repo_root,
    ) {
        eprintln!("[research-activity-log] failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_research_workspace_finds_state_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("research-state.yaml"), "project: test").unwrap();
        // Call should succeed and write JSONL
        record_research_activity(dir.path(), "WebFetch", "https://arxiv.org/abs/2301.07041");
        let auto_dir = dir.path().join("artifacts/research-log/auto");
        assert!(auto_dir.is_dir());
    }

    #[test]
    fn record_research_activity_noop_without_marker() {
        let dir = tempfile::tempdir().unwrap();
        record_research_activity(dir.path(), "Bash", "cargo test");
        let auto_dir = dir.path().join("artifacts/research-log/auto");
        assert!(!auto_dir.exists());
    }
}
