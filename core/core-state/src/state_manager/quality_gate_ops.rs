// Quality Gate state management: path helpers, read, and GOAL/QG mutual exclusion.

use crate::utils::atomic_write::write_atomic_json;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use super::pointer_ops::read_active_task_id;
use super::{goal_state_path_for_task, read_goal_state, QUALITY_GATE_STATE_FILENAME};

pub fn quality_gate_state_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts/current")
        .join(tid)
        .join(QUALITY_GATE_STATE_FILENAME))
}

pub(crate) fn deactivate_quality_gate_for_conflict_with_goal_drive(
    repo_root: &Path,
    task_id: &str,
) -> Result<bool, String> {
    if task_id.trim().is_empty() {
        return Ok(false);
    }
    if crate::utils::path_guard::safe_task_id_component(task_id).is_none() {
        return Ok(false);
    }
    let path = quality_gate_state_path(repo_root, task_id)?;
    if !path.is_file() {
        return Ok(false);
    }
    let mut state = read_quality_gate_state(repo_root, Some(task_id))?
        .ok_or_else(|| format!("QUALITY_GATE_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "QUALITY_GATE_STATE root must be object".to_string())?;
    let active = obj
        .get("loop_status")
        .and_then(Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("active"));
    if !active {
        return Ok(false);
    }
    obj.insert("loop_status".to_string(), json!("superseded"));
    obj.insert("superseded_by".to_string(), json!("goal_drive"));
    obj.insert(
        "updated_at".to_string(),
        json!(framework_kernel::time::now_iso()),
    );
    write_atomic_json(&path, &state)?;
    Ok(true)
}

/// Quality Gate 在同 task 上 `start`/`upsert` 时标记 GOAL 为 superseded（与 goal supersede QG 对称）。
pub fn deactivate_goal_for_conflict_with_quality_gate(
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
        .ok_or_else(|| "GOAL_STATE missing for QG conflict resolution".to_string())?;
    if let Some(obj) = state.as_object_mut() {
        obj.insert("status".to_string(), json!("superseded"));
        obj.insert("updated_at".to_string(), json!(framework_kernel::time::now_iso()));
        obj.entry("metadata".to_string())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .map(|m| m.insert("superseded_by".to_string(), json!("quality_gate")));
    }
    write_atomic_json(&path, &state)?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, task_id);
    Ok(true)
}

/// 供 Cursor hook / 工具读取当前 task 的 Quality Gate 账本（无覆盖则用 `active_task.json`）。
pub fn read_quality_gate_state(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Result<Option<Value>, String> {
    let task_id = if let Some(t) = task_id_override {
        if t.trim().is_empty() {
            return Err("framework_quality_gate: task_id override is empty".to_string());
        }
        t.trim().to_string()
    } else {
        let Some(t) = read_active_task_id(repo_root) else {
            return Ok(None);
        };
        t
    };
    crate::utils::path_guard::validate_task_id_component(&task_id)
        .map_err(|e| format!("framework_quality_gate: invalid task_id for QUALITY_GATE_STATE path: {e}"))?;
    let path = quality_gate_state_path(repo_root, &task_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("read QUALITY_GATE_STATE: {err}"))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|err| format!("parse QUALITY_GATE_STATE: {err}"))?;
    Ok(Some(value))
}
