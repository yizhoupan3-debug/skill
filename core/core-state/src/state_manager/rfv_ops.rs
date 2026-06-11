// RFV loop state management: path helpers, read, and GOAL/RFV mutual exclusion.
// Extracted from state_manager.rs during module split.

use crate::utils::atomic_write::write_atomic_json;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use super::pointer_ops::read_active_task_id;
use super::{goal_state_path_for_task, now_iso, read_goal_state};

pub fn rfv_loop_state_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts/current")
        .join(tid)
        .join("RFV_LOOP_STATE.json"))
}

pub(crate) fn deactivate_rfv_for_conflict_with_autopilot(
    repo_root: &Path,
    task_id: &str,
) -> Result<bool, String> {
    if task_id.trim().is_empty() {
        return Ok(false);
    }
    if crate::utils::path_guard::safe_task_id_component(task_id).is_none() {
        return Ok(false);
    }
    let path = rfv_loop_state_path(repo_root, task_id)?;
    if !path.is_file() {
        return Ok(false);
    }
    let mut state = read_rfv_loop_state(repo_root, Some(task_id))?
        .ok_or_else(|| format!("RFV_LOOP_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "RFV_LOOP_STATE root must be object".to_string())?;
    let active = obj
        .get("loop_status")
        .and_then(Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("active"));
    if !active {
        return Ok(false);
    }
    obj.insert("loop_status".to_string(), json!("superseded"));
    obj.insert("superseded_by".to_string(), json!("autopilot_goal"));
    obj.insert(
        "updated_at".to_string(),
        json!(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
    );
    write_atomic_json(&path, &state)?;
    Ok(true)
}

/// RFV 在同 task 上 `start`/`upsert` 时标记 GOAL 为 superseded（与 goal supersede RFV 对称）。
pub fn deactivate_goal_for_conflict_with_rfv(
    repo_root: &Path,
    task_id: &str,
) -> Result<bool, String> {
    if task_id.trim().is_empty() {
        return Ok(false);
    }
    if crate::utils::path_guard::safe_task_id_component(task_id).is_none() {
        return Ok(false);
    }
    let path = goal_state_path_for_task(repo_root, task_id)?;
    if !path.is_file() {
        return Ok(false);
    }
    let mut state = read_goal_state(repo_root, Some(task_id))?
        .ok_or_else(|| "GOAL_STATE missing for RFV conflict resolution".to_string())?;
    if let Some(obj) = state.as_object_mut() {
        obj.insert("status".to_string(), json!("superseded"));
        obj.insert("updated_at".to_string(), json!(now_iso()));
        obj.entry("metadata".to_string())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .map(|m| m.insert("superseded_by".to_string(), json!("rfv_loop")));
    }
    write_atomic_json(&path, &state)?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, task_id);
    Ok(true)
}

/// 供 Cursor hook / 工具读取当前任务的 RFV 账本（无覆盖则用 `active_task.json`）。
pub fn read_rfv_loop_state(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Result<Option<Value>, String> {
    let task_id = if let Some(t) = task_id_override {
        if t.trim().is_empty() {
            return Err("framework_rfv_loop: task_id override is empty".to_string());
        }
        t.trim().to_string()
    } else {
        let Some(t) = read_active_task_id(repo_root) else {
            return Ok(None);
        };
        t
    };
    crate::utils::path_guard::validate_task_id_component(&task_id)
        .map_err(|e| format!("framework_rfv_loop: invalid task_id for RFV_LOOP_STATE path: {e}"))?;
    let path = rfv_loop_state_path(repo_root, &task_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("read RFV_LOOP_STATE: {err}"))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|err| format!("parse RFV_LOOP_STATE: {err}"))?;
    Ok(Some(value))
}
