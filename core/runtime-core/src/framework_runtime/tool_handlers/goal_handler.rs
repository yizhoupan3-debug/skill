//! Goal state management tool handler (`domain:goal`).
//! Payload construction + core_state state_manager call.

use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::path::Path;

/// goal_state_manage: payload construction + core_state state_manager call.
pub(crate) fn goal_state_manage_dispatch(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, FrameworkError> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: operation"))?;

    // Auto-resolve task_id from TASK_POINTERS.json
    let task_id = match arguments.get("task_id").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        Some(tid) => tid.to_string(),
        None => core_state::state_manager::read_primary_task_id(repo_root)
            .ok_or_else(|| FrameworkError::validation("No active task_id in TASK_POINTERS.json (start a task first or provide task_id explicitly)"))?,
    };

    let repo_root_str = repo_root.to_string_lossy().to_string();
    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
        "task_id": task_id,
    });

    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    FrameworkError::validation("start requires 'goal' argument (string)")
                })?;
            payload["goal"] = json!(goal);

            let drive_until_done = arguments
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            payload["drive_until_done"] = json!(drive_until_done);

            // Auto-fill contract fields when drive_until_done=true and not explicitly provided
            if drive_until_done {
                if arguments.get("non_goals").is_none() {
                    payload["non_goals"] = json!(["features outside this goal's scope"]);
                }
                if arguments.get("done_when").is_none() {
                    payload["done_when"] = json!([
                        format!("goal completed: {goal}"),
                        "cargo check / test passing".to_string(),
                    ]);
                }
                if arguments.get("validation_commands").is_none() {
                    payload["validation_commands"] =
                        json!(["cargo check --workspace", "cargo test --workspace"]);
                }
            }

            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments
                .get("validation_commands")
                .and_then(Value::as_array)
            {
                payload["validation_commands"] = json!(vc);
            }

            let session_id = arguments
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(connection_session_id);
            payload["session_id"] = json!(session_id);

            if let Some(gt) = arguments.get("goal_type").and_then(Value::as_str) {
                if gt == "loop" {
                    payload["goal_type"] = json!("loop");
                } else if gt == "linear" {
                    return Err(FrameworkError::validation(
                        "goal_type=linear was removed in v10 — all goals follow loop semantics. Remove goal_type or set goal_type=loop.",
                    ));
                } else {
                    return Err(FrameworkError::validation(format!(
                        "Invalid goal_type: {gt}. Only goal_type=loop is supported in v10."
                    )));
                }
            }
            if let Some(ch) = arguments.get("current_horizon").and_then(Value::as_str) {
                payload["current_horizon"] = json!(ch);
            }
            if let Some(cg) = arguments.get("completion_gates") {
                payload["completion_gates"] = cg.clone();
            }
            if let Some(md) = arguments.get("metadata") {
                payload["metadata"] = md.clone();
            }
            if let Some(sf) = arguments.get("set_focus").and_then(Value::as_bool) {
                payload["set_focus"] = json!(sf);
            }
        }
        "checkpoint" => {
            let note = arguments
                .get("note")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    FrameworkError::validation("checkpoint requires 'note' argument (string)")
                })?;
            payload["note"] = json!(note);
        }
        "block" => {
            let blocker = arguments
                .get("blocker")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    FrameworkError::validation("block requires 'blocker' argument (string)")
                })?;
            payload["blocker"] = json!(blocker);
        }
        "resume" => {
            // Forward drive_until_done and contract fields to goal_ops
            if let Some(v) = arguments.get("drive_until_done").and_then(|v| v.as_bool()) {
                payload["drive_until_done"] = json!(v);
            }
            if let Some(v) = arguments.get("non_goals") {
                payload["non_goals"] = v.clone();
            }
            if let Some(v) = arguments.get("done_when") {
                payload["done_when"] = v.clone();
            }
            if let Some(v) = arguments.get("validation_commands") {
                payload["validation_commands"] = v.clone();
            }
        }
        "pause" | "complete" | "clear" => {}
        "amend" => {
            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments
                .get("validation_commands")
                .and_then(Value::as_array)
            {
                payload["validation_commands"] = json!(vc);
            }
            if let Some(g) = arguments
                .get("goal")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
            {
                payload["goal"] = json!(g);
            }
            if let Some(kp) = arguments.get("keep_progress").and_then(Value::as_bool) {
                payload["keep_progress"] = json!(kp);
            }
            if let Some(dud) = arguments.get("drive_until_done").and_then(|v| v.as_bool()) {
                payload["drive_until_done"] = json!(dud);
            }
            if let Some(cg) = arguments.get("completion_gates") {
                payload["completion_gates"] = cg.clone();
            }
            if let Some(md) = arguments.get("metadata") {
                payload["metadata"] = md.clone();
            }
        }
        _ => {
            return Err(FrameworkError::validation(format!(
                "Unknown goal operation: {operation}. Valid operations: start, checkpoint, pause, resume, complete, clear, block, amend"
            )));
        }
    }

    let result = core_state::state_manager::framework_goal_drive(payload)?;
    Ok(serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// goal_type=loop is the only valid type in v10
    #[test]
    fn accepts_loop_goal_type() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let repo = std::env::temp_dir().join(format!("goal-handler-loop-{suffix}"));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");

        let result = goal_state_manage_dispatch(
            &json!({
                "operation": "start",
                "goal": "loop task",
                "task_id": "t-loop",
                "goal_type": "loop",
            }),
            &repo,
            "test-session",
        )
        .expect("loop goal_type should be accepted");
        assert!(result.contains("\"ok\": true"), "result: {result}");

        let _ = std::fs::remove_dir_all(&repo);
    }
}
