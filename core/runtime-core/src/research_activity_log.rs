//! 科研活动内联日志（§19.5）：当检测到科研工作空间时，自动记录关键工具调用
//! 到 `artifacts/research-log/auto/YYYY-MM-DD.jsonl`。
//!
//! 这是自动验证证据采集（§4.4）的科研领域特化版本，使用相同的
//! 函数指针注册模式避免 host-projection ↔ runtime-core 循环依赖。

use std::path::Path;

/// 检测 repo_root 或其祖先目录中是否有科研工作空间标记文件。
fn detect_research_workspace(repo_root: &Path) -> bool {
    repo_root.ancestors().any(|dir| {
        dir.join("research-state.yaml").is_file() || dir.join(".research.toml").is_file()
    })
}

/// 科研活动日志记录器。在 PostToolUse 中被调用。
///
/// 策略：
/// - 先检测 repo_root 是否有科研标记（research-state.yaml / .research.toml）
/// - 若无标记，直接返回（零成本）
/// - 若有标记，以 JSONL 格式追加到 `artifacts/research-log/auto/YYYY-MM-DD.jsonl`
pub fn record_research_activity(repo_root: &Path, tool_name: &str, summary: &str) {
    if !detect_research_workspace(repo_root) {
        return;
    }

    let auto_dir = repo_root.join("artifacts/research-log/auto");
    if let Err(e) = std::fs::create_dir_all(&auto_dir) {
        eprintln!("[research-activity-log] mkdir failed: {e}");
        return;
    }

    let date = chrono::Local::now().format("%Y-%m-%d");
    let log_path = auto_dir.join(format!("{date}.jsonl"));

    let entry = serde_json::json!({
        "ts": chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "tool": tool_name,
        "summary": summary,
        "auto": true,
    });

    if let Err(e) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{}", entry)
        })
    {
        eprintln!("[research-activity-log] write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_research_workspace_finds_state_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("research-state.yaml"), "project: test").unwrap();
        assert!(detect_research_workspace(dir.path()));
    }

    #[test]
    fn detect_research_workspace_finds_toml_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".research.toml"), "").unwrap();
        assert!(detect_research_workspace(dir.path()));
    }

    #[test]
    fn detect_research_workspace_scans_ancestors() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("x").join("y");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.path().join("research-state.yaml"), "project: test").unwrap();
        assert!(detect_research_workspace(&sub));
    }

    #[test]
    fn detect_research_workspace_no_marker() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!detect_research_workspace(dir.path()));
    }

    #[test]
    fn record_research_activity_writes_jsonl_when_in_research_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("research-state.yaml"), "project: test").unwrap();
        record_research_activity(dir.path(), "WebFetch", "https://arxiv.org/abs/2301.07041");
        let auto_dir = dir.path().join("artifacts/research-log/auto");
        assert!(auto_dir.is_dir());
        let entries: Vec<_> = std::fs::read_dir(&auto_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1);
        let content = std::fs::read_to_string(entries[0].path()).unwrap();
        assert!(content.contains("WebFetch"));
        assert!(content.contains("arxiv.org"));
    }

    #[test]
    fn record_research_activity_noop_without_marker() {
        let dir = tempfile::tempdir().unwrap();
        // No marker file — should not create auto-log
        record_research_activity(dir.path(), "Bash", "cargo test");
        let auto_dir = dir.path().join("artifacts/research-log/auto");
        assert!(!auto_dir.exists());
    }
}
