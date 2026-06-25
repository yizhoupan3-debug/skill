//! 科研活动日志 hook — 记录工具调用到研究日志。
//!
//! 在 PostToolUse 中被调用：检测 repo_root 是否有科研标记文件
//! （research-state.yaml / .research.toml），若有则以 JSONL 追加
//! 到 `artifacts/research-log/auto/YYYY-MM-DD.jsonl`。

use std::path::Path;

use anyhow::Result;
use chrono::Local;

/// 检测 repo_root 或其祖先目录中是否有科研工作空间标记文件。
fn detect_research_workspace(repo_root: &Path) -> bool {
    repo_root.ancestors().any(|dir| {
        dir.join("research-state.yaml").is_file() || dir.join(".research.toml").is_file()
    })
}

/// 根据工具名和参数判断是否需要记录科研活动，如需要则写入日志。
///
/// 策略：
/// - 先检测 repo_root 是否有科研标记（research-state.yaml / .research.toml）
/// - 若无标记，直接返回（零成本）
/// - 若有标记，以 JSONL 格式追加到 `artifacts/research-log/auto/YYYY-MM-DD.jsonl`
pub fn maybe_log_research_activity(
    tool_name: &str,
    args: &str,
    repo_root: &Path,
) -> Result<()> {
    if !detect_research_workspace(repo_root) {
        return Ok(());
    }

    let auto_dir = repo_root.join("artifacts/research-log/auto");
    std::fs::create_dir_all(&auto_dir)?;

    let now = Local::now();
    let date = now.format("%Y-%m-%d");
    let log_path = auto_dir.join(format!("{date}.jsonl"));

    let entry = serde_json::json!({
        "ts": now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "tool": tool_name,
        "summary": args,
        "auto": true,
    });

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    use std::io::Write;
    writeln!(f, "{}", entry)?;
    f.flush()?; // 确保日志写入磁盘，崩溃时不丢失

    Ok(())
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
    fn writes_jsonl_when_in_research_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("research-state.yaml"), "project: test").unwrap();
        maybe_log_research_activity("WebFetch", "https://arxiv.org/abs/2301.07041", dir.path())
            .unwrap();
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
    fn noop_without_marker() {
        let dir = tempfile::tempdir().unwrap();
        maybe_log_research_activity("Bash", "cargo test", dir.path()).unwrap();
        let auto_dir = dir.path().join("artifacts/research-log/auto");
        assert!(!auto_dir.exists());
    }
}
