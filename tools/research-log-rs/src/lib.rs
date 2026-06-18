pub mod cli;
pub mod db;
pub mod export;
pub mod models;
pub mod text_layer;

use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub const ARTIFACTS_LOG_DIR: &str = "artifacts/research-log";

/// 初始化日志工作空间：创建目录树并初始化数据库。
pub fn init_log_workspace(log_root: &Path) -> Result<()> {
    let db_path = log_root.join("research-log.db");
    db::init_database(&db_path)?;
    Ok(())
}

/// 从 auto/*.jsonl 导入活动记录到 activity_log 表。
/// 同时检查 log_root/auto/ 和全局 artifacts/research-log/auto/（hook 路径）。
/// 完成后重命名 JSONL 为 .done 后缀，防重复处理。
pub fn consolidate_activity_log(log_root: &Path) -> Result<(usize, usize)> {
    let mut total = 0usize;
    let mut files = 0usize;

    // 检查两个可能的 auto 目录，避免重复处理
    let auto_dirs = [
        log_root.join("auto"),
        Path::new(ARTIFACTS_LOG_DIR).join("auto"),
    ];
    let mut seen = std::collections::HashSet::new();

    for auto_dir in &auto_dirs {
        if !auto_dir.is_dir() {
            continue;
        }

        let db_path = log_root.join("research-log.db");
        let conn = db::init_database(&db_path)?;

        let entries: Vec<_> = match std::fs::read_dir(auto_dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .filter(|e| {
                    !seen.contains(&e.path())
                        && e.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl")
                })
                .collect(),
            Err(_) => continue,
        };

        for entry in &entries {
            seen.insert(entry.path());
        }

        for entry in &entries {
            let path = entry.path();
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let activities: Vec<models::ActivityEntry> = content
                .lines()
                .filter_map(|line| {
                    let v: serde_json::Value = serde_json::from_str(line).ok()?;
                    Some(models::ActivityEntry {
                        id: 0,
                        tool_name: v.get("tool")?.as_str()?.to_string(),
                        summary: v.get("summary")?.as_str()?.to_string(),
                        source: v
                            .get("source")
                            .and_then(Value::as_str)
                            .unwrap_or("auto")
                            .to_string(),
                        metadata: v.get("metadata").map(|m| m.to_string()),
                        created_at: v
                            .get("ts")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect();

            if !activities.is_empty() {
                db::bulk_insert_activities(&conn, &activities)?;
                total += activities.len();
            }

            // 重命名防重复处理（非原子，但幂等：失败时下次重试）
            let done_path = path.with_extension("jsonl.done");
            let _ = std::fs::rename(&path, &done_path);
            files += 1;
        }
    }

    Ok((total, files))
}
