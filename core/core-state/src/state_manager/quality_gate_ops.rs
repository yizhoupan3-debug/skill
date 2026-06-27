// Quality Gate state management: path helpers and read (kept for backward compat).
// deactivate_quality_gate_for_conflict_with_goal_drive removed — Wave 4a-ii: QG is Goal's internal mode, no mutual exclusion needed.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use super::pointer_ops::read_active_task_id;
use super::RFV_LOOP_STATE_FILENAME;

pub fn quality_gate_state_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts/current")
        .join(tid)
        .join(RFV_LOOP_STATE_FILENAME))
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
