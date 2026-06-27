// Quality Gate state management: path helpers and read (kept for backward compat).
// deactivate_quality_gate_for_conflict_with_goal_drive removed — Wave 4a-ii: QG is Goal's internal mode, no mutual exclusion needed.

use core_errors::FrameworkError;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use super::pointer_ops::read_active_task_id;
use super::RFV_LOOP_STATE_FILENAME;

pub fn quality_gate_state_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, FrameworkError> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts/current")
        .join(tid)
        .join(RFV_LOOP_STATE_FILENAME))
}

pub fn read_quality_gate_state(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Result<Option<Value>, FrameworkError> {
    let task_id = if let Some(t) = task_id_override {
        if t.trim().is_empty() {
            return Err(FrameworkError::validation("framework_quality_gate: task_id override is empty"));
        }
        t.trim().to_string()
    } else {
        let Some(t) = read_active_task_id(repo_root) else {
            return Ok(None);
        };
        t
    };
    crate::utils::path_guard::validate_task_id_component(&task_id)?;
    let path = quality_gate_state_path(repo_root, &task_id)?;
    if !path.is_file() { return Ok(None); }
    let raw = fs::read_to_string(&path)?;
    let value: Value = serde_json::from_str(&raw)?;
    Ok(Some(value))
}
