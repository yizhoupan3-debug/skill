//! 文字层（Markdown 生成器 - 按需渲染，不再强制双写）
//!
//! 按需从 SQLite 读取数据并渲染为 Markdown 日志文件。
//! 用于 `log:render` 命令和 Obsidian 导出。

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::models::*;

/// 渲染一条日志条目为 Markdown 文件内容。
pub fn render_entry(entry: &Entry, findings: &[Finding], tags: &[String]) -> String {
    let mut md = String::new();

    md.push_str(&format!("# {}: {}\n\n", entry.direction, entry.question));
    md.push_str(&format!("- **ID**: {}\n", entry.id));
    md.push_str(&format!("- **Status**: {}\n", entry.status));
    md.push_str(&format!("- **Importance**: {}\n", entry.importance));
    md.push_str(&format!("- **Created**: {}\n", entry.created_at));
    if !tags.is_empty() {
        md.push_str(&format!("- **Tags**: {}\n", tags.join(", ")));
    }
    md.push('\n');

    if let Some(ctx) = &entry.context {
        md.push_str("## Context\n\n```json\n");
        md.push_str(ctx);
        md.push_str("\n```\n\n");
    }

    if !findings.is_empty() {
        md.push_str("## Findings\n\n");
        for f in findings {
            let emoji = match f.kind.as_str() {
                "finding" => "🔍",
                "decision" => "⚡",
                "insight" => "💡",
                "question" => "❓",
                "plan" => "📋",
                _ => "•",
            };
            md.push_str(&format!("### {} {}\n\n", emoji, f.kind));
            md.push_str(&format!("{}\n\n", f.content));
            if let Some(conf) = f.confidence {
                md.push_str(&format!("> Confidence: {:.0}%\n\n", conf * 100.0));
            }
        }
    }

    md
}

/// 生成每日 INDEX.md 的表格行。
pub fn render_index_row(entry: &Entry, tags: &[String]) -> String {
    let tag_str = if tags.is_empty() {
        "—".to_string()
    } else {
        tags.join(", ")
    };
    format!(
        "| {} | {} | {} | {} | {} |\n",
        entry.created_at.split('T').next().unwrap_or(&entry.created_at),
        entry.direction,
        entry.question.chars().take(48).collect::<String>(),
        tag_str,
        entry.status,
    )
}

/// 生成 INDEX.md 的完整表头。
pub fn render_index_header() -> String {
    "| 日期 | 方向 | 问题 | 标签 | 状态 |\n|------|------|------|------|------|\n".to_string()
}

/// 写入可选的 Markdown 文本层到文件系统。
/// 仅用于 `log:render --write` 命令，不再作为默认写入路径。
pub fn write_entry_md(root: &Path, entry: &Entry, findings: &[Finding], tags: &[String]) -> Result<PathBuf> {
    let date = entry.created_at.split('T').next().unwrap_or("unknown");
    let month = &date[..date.len().saturating_sub(3)];
    let dir = root.join(month);
    std::fs::create_dir_all(&dir)?;

    let safe_name = entry
        .direction
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .trim_matches('-')
        .to_string();

    let path = dir.join(format!("{}_{}.md", date, safe_name));
    let content = render_entry(entry, findings, tags);
    std::fs::write(&path, content)?;
    Ok(path)
}
