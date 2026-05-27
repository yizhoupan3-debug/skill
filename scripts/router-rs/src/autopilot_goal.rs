//! Autopilot 宏目标：Rust 真源 `GOAL_STATE.json` + stdio 控制面（`framework_goal_drive`）。
//! 不替代 LLM 执行；hook 不再注入 `GOAL_CONTINUE`（2026-05 连续性拔除）。

use crate::atomic_write::write_atomic_json;
use crate::framework_runtime::resolve_repo_root_arg;
use crate::route::invalidate_records_cache;
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const GOAL_STATE_FILENAME: &str = "GOAL_STATE.json";
pub const GOAL_STATE_SCHEMA_VERSION: &str = "router-rs-autopilot-goal-v1";
pub const EVIDENCE_INDEX_FILENAME: &str = "EVIDENCE_INDEX.json";
const REQUIRES_COMPLETION_EVIDENCE_KEY: &str = "requires_completion_evidence";
/// Legacy paragraph prefixes stripped when refreshing hook output (scrub stale injected text).
pub const LEGACY_AUTOPILOT_DRIVE_PARAGRAPH_PREFIX: &str = "AUTOPILOT_DRIVE";

/// Invalidate route records cache after GOAL_STATE mutations (best-effort).
fn invalidate_route_records_cache_on_write() {
    if let Err(e) = invalidate_records_cache() {
        eprintln!("WARN: route records cache invalidation failed: {}", e);
    }
}

/// 从 `artifacts/current/active_task.json` 读取 `task_id`。
/// Active task pointer, else focus — used by `framework_goal_drive` write paths when override absent.
pub fn read_primary_task_id(repo_root: &Path) -> Option<String> {
    let (active, focus) = read_task_pointer_pair(repo_root);
    active.or(focus)
}

pub fn read_active_task_id(repo_root: &Path) -> Option<String> {
    let path = repo_root.join("artifacts/current/active_task.json");
    let raw = fs::read_to_string(&path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    let t = data
        .get("task_id")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    crate::path_guard::safe_task_id_component(&t)?;
    Some(t)
}

/// 从 `artifacts/current/focus_task.json` 读取 `task_id`（与 framework 指针一致，作 active 指针缺失时的回退）。
pub fn read_focus_task_id(repo_root: &Path) -> Option<String> {
    let path = repo_root.join("artifacts/current/focus_task.json");
    let raw = fs::read_to_string(&path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    let t = data
        .get("task_id")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    crate::path_guard::safe_task_id_component(&t)?;
    Some(t)
}

/// Ensure the task directory exists, creating it if necessary.
/// Returns the path on success, or an error message if creation fails.
pub fn ensure_task_directory(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    let tid = crate::path_guard::validate_task_id_component(task_id)?;
    let task_dir = repo_root.join("artifacts/current").join(tid);

    if task_dir.is_dir() {
        return Ok(task_dir);
    }

    fs::create_dir_all(&task_dir)
        .map_err(|e| format!("failed to create task directory '{}': {}", tid, e))?;

    eprintln!("[router-rs] Created task directory: {}", task_dir.display());

    Ok(task_dir)
}

fn parse_task_id_from_pointer_json(raw: &str) -> Option<String> {
    let data: Value = serde_json::from_str(raw).ok()?;
    let t = data
        .get("task_id")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    crate::path_guard::safe_task_id_component(&t)?;
    Some(t)
}

/// RFV 在同 task 上 `start`/`upsert` 时移除 GOAL（与 RFV 互斥）。
pub(crate) fn deactivate_goal_for_conflict_with_rfv(
    repo_root: &Path,
    task_id: &str,
) -> Result<bool, String> {
    if task_id.trim().is_empty() {
        return Ok(false);
    }
    if crate::path_guard::safe_task_id_component(task_id).is_none() {
        return Ok(false);
    }
    let path = goal_state_path_for_task(repo_root, task_id)?;
    if !path.is_file() {
        return Ok(false);
    }
    fs::remove_file(&path).map_err(|e| format!("remove GOAL_STATE for RFV mutex: {e}"))?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, task_id);
    Ok(true)
}

/// `artifacts/current/<rel>/GOAL_STATE.json` 当 `rel` 为多段路径（仅测试诊断扫描用）；
/// 每段须通过 [`crate::path_guard::safe_task_id_component`]，否则 `None`。
fn goal_state_path_for_nested_under_current(repo_root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim().trim_matches('/');
    if rel.is_empty() {
        return None;
    }
    let mut dir = repo_root.join("artifacts/current");
    for seg in rel.split(['/', '\\']) {
        let seg = seg.trim();
        if seg.is_empty() || crate::path_guard::safe_task_id_component(seg).is_none() {
            return None;
        }
        dir = dir.join(seg);
    }
    Some(dir.join(GOAL_STATE_FILENAME))
}
