//! Minimal RFV loop surface for tests and goal/RFV mutex.

use crate::state_manager::{
    deactivate_goal_for_conflict_with_rfv, read_active_task_id, read_rfv_loop_state,
    rfv_loop_state_path,
};
use crate::utils::atomic_write::write_atomic_json;
use crate::utils::path_guard;
use crate::utils::task_write_lock;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

const RFV_LOOP_SCHEMA_VERSION: &str = "router-rs-rfv-loop-v1";

pub fn framework_rfv_loop(payload: Value) -> Result<Value, String> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    if operation == "status" {
        framework_rfv_loop_impl(payload)
    } else {
        let repo_root = payload
            .get("repo_root")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| "framework_rfv_loop requires repo_root".to_string())?;
        task_write_lock::apply_task_ledger_mutation(&repo_root, || framework_rfv_loop_impl(payload))
    }
}

fn framework_rfv_loop_impl(payload: Value) -> Result<Value, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "framework_rfv_loop requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_rfv_loop: repo_root is not a directory: {}",
            repo_root.display()
        ));
    }
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    let task_id_override = payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match operation.as_str() {
        "status" => {
            let state = read_rfv_loop_state(&repo_root, task_id_override)?;
            let tid = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .unwrap_or_default();
            let path = if tid.is_empty() {
                PathBuf::new()
            } else {
                rfv_loop_state_path(&repo_root, &tid).unwrap_or_else(|_| PathBuf::new())
            };
            Ok(json!({
                "ok": true,
                "operation": "status",
                "task_id": tid,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": state,
            }))
        }
        "start" | "upsert" => {
            let task_id = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .ok_or_else(|| {
                    "framework_rfv_loop start requires task_id in payload or active_task.json"
                        .to_string()
                })?;
            path_guard::validate_task_id_component(&task_id)?;
            let goal = payload
                .get("goal")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "framework_rfv_loop start requires non-empty goal".to_string())?;
            let max_rounds = payload
                .get("max_rounds")
                .and_then(Value::as_u64)
                .unwrap_or(3);
            let mut obj = Map::new();
            obj.insert("schema_version".to_string(), json!(RFV_LOOP_SCHEMA_VERSION));
            obj.insert("goal".to_string(), json!(goal));
            obj.insert("loop_status".to_string(), json!("active"));
            obj.insert("max_rounds".to_string(), json!(max_rounds));
            obj.insert(
                "updated_at".to_string(),
                json!(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            );
            let path = rfv_loop_state_path(&repo_root, &task_id)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir RFV task dir: {e}"))?;
            }
            write_atomic_json(&path, &Value::Object(obj.clone()))?;
            let goal_state_cleared = deactivate_goal_for_conflict_with_rfv(&repo_root, &task_id)?;
            Ok(json!({
                "ok": true,
                "operation": operation,
                "task_id": task_id,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": Value::Object(obj),
                "goal_state_cleared": goal_state_cleared,
            }))
        }
        other => Err(format!("framework_rfv_loop: unsupported operation `{other}`")),
    }
}
