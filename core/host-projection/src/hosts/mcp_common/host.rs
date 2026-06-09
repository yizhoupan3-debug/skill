use std::fs;
use std::path::{Path, PathBuf};

use router_rs::goal_state::ARTIFACTS_CURRENT_DIR;

pub fn mcp_host_supports_hard_closeout(host_id: &str) -> bool {
    matches!(
        host_id,
        "antigravity-app" | "antigravity" | "opencode"
    )
}

pub fn mcp_host_hard_block_label(host_id: &str) -> &'static str {
    match host_id {
        "antigravity-app" | "antigravity" => "Antigravity App",
        "opencode" => "Opencode",
        _ => "MCP Host",
    }
}

/// Check if closeout hard-block is disabled for the given task-level `LifecycleMode`.
/// Empty or "my-light" modes disable the hard block (advisory only).
pub fn mcp_closeout_hard_block_disabled(_repo_root: &Path, lifecycle_mode: &str) -> bool {
    lifecycle_mode.is_empty() || lifecycle_mode == "my-light"
}

pub fn list_known_task_ids(repo_root: &Path) -> Vec<String> {
    let current = repo_root.join(ARTIFACTS_CURRENT_DIR);
    let Ok(entries) = fs::read_dir(&current) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name != "review-lanes" && !name.starts_with('.'))
        .collect();
    ids.sort();
    ids
}

pub fn task_artifact_dir(repo_root: &Path, task_id: Option<&str>) -> PathBuf {
    let base = repo_root.join(ARTIFACTS_CURRENT_DIR);
    if let Some(task_id) = task_id.filter(|value| !value.is_empty()) {
        match router_rs::path_guard::validate_task_id_component(task_id.trim()) {
            Ok(safe) => base.join(safe),
            // Poisoned or hostile task_id must not escape artifacts/current via `..`.
            Err(_) => base,
        }
    } else {
        base
    }
}
