//! Goal state management tool handler (`domain:goal`).
//! Payload construction + core_state state_manager call.

use core_errors::FrameworkError;
use serde_json::{json, Value};
use std::path::Path;

/// goal_state_manage: payload construction + core_state state_manager call.
pub fn goal_state_manage_dispatch(
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
                .ok_or_else(|| FrameworkError::validation("start requires 'goal' argument (string)"))?;
            payload["goal"] = json!(goal);

            let drive_until_done = arguments
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            payload["drive_until_done"] = json!(drive_until_done);

            // Auto-fill contract fields when drive_until_done=true and not explicitly provided
            if drive_until_done {
                if arguments.get("non_goals").is_none() {
                    payload["non_goals"] = json!(["不处理此 goal 范围外的功能"]);
                }
                if arguments.get("done_when").is_none() {
                    payload["done_when"] = json!([
                        format!("goal 已完成: {goal}"),
                        "cargo check / test 通过".to_string(),
                    ]);
                }
                if arguments.get("validation_commands").is_none() {
                    payload["validation_commands"] = json!(["cargo check --workspace", "cargo test --workspace"]);
                }
            }

            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments.get("validation_commands").and_then(Value::as_array) {
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
                    // goal_type=loop without explicit lifecycle_profile → auto-set loop-auto
                    if arguments.get("lifecycle_profile").is_none() {
                        payload["lifecycle_profile"] = json!("loop-auto");
                    }
                } else if gt == "linear" {
                    return Err(FrameworkError::validation(
                        "goal_type=linear was removed in v10 — all goals follow loop semantics. Remove goal_type or set goal_type=loop."
                    ));
                } else {
                    return Err(FrameworkError::validation(format!("Invalid goal_type: {gt}. Only goal_type=loop is supported in v10.")));
                }
            }
            if let Some(lp) = arguments.get("lifecycle_profile").and_then(Value::as_str) {
                match lp {
                    "task" | "loop-auto" => payload["lifecycle_profile"] = json!(lp),
                    _ => return Err(FrameworkError::validation(format!("Invalid lifecycle_profile: {lp}. Must be one of: task, loop-auto"))),
                }
                // Validate consistency: goal_type=loop conflicts with lifecycle_profile=task
                if let Some(gt) = arguments.get("goal_type").and_then(Value::as_str) {
                    if gt == "loop" && lp == "task" {
                        return Err(FrameworkError::validation(
                            "goal_type=loop 与 lifecycle_profile=task 冲突。循环目标需要 lifecycle_profile=loop-auto。"
                        ));
                    }
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
                .ok_or_else(|| FrameworkError::validation("checkpoint requires 'note' argument (string)"))?;
            payload["note"] = json!(note);
        }
        "block" => {
            let blocker = arguments
                .get("blocker")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| FrameworkError::validation("block requires 'blocker' argument (string)"))?;
            payload["blocker"] = json!(blocker);
        }
        "append_round" => {
            return Err(FrameworkError::validation(
                "append_round is not a valid goal_state_manage operation. \
                 Use quality_gate_manage with operation=append_round instead.",
            ));
        }
        "pause" | "resume" | "complete" | "clear" => {}
        "amend" => {
            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments.get("validation_commands").and_then(Value::as_array) {
                payload["validation_commands"] = json!(vc);
            }
            if let Some(g) = arguments.get("goal").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
                payload["goal"] = json!(g);
            }
            if let Some(kp) = arguments.get("keep_progress").and_then(Value::as_bool) {
                payload["keep_progress"] = json!(kp);
            }
        }
        _ => return Err(FrameworkError::validation(format!(
            "Unknown goal operation: {operation}. Valid operations: start, checkpoint, pause, resume, complete, clear, block, amend"
        ))),
    }

    let result = core_state::state_manager::framework_goal_drive(payload)?;
    Ok(serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// goal_type=loop + lifecycle_profile=task must be rejected before
    /// framework_goal_drive (early validation at the handler boundary).
    #[test]
    fn rejects_loop_with_task_lifecycle() {
        let repo = Path::new("/tmp"); // never hit the filesystem — validation is early
        let err = goal_state_manage_dispatch(
            &json!({
                "operation": "start",
                "goal": "loop task",
                "task_id": "t-loop",
                "goal_type": "loop",
                "lifecycle_profile": "task",
            }),
            repo,
            "test-session",
        )
        .expect_err("loop+task should be rejected");
        let msg = err.to_string();
        assert!(msg.contains("冲突"), "err must mention conflict: {msg}");
    }

    /// goal_type=loop + lifecycle_profile=loop-auto is valid
    #[test]
    fn accepts_loop_with_loop_auto_lifecycle() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let repo = std::env::temp_dir().join(format!("goal-handler-loop-auto-{suffix}"));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");

        let result = goal_state_manage_dispatch(
            &json!({
                "operation": "start",
                "goal": "auto lifecycle",
                "task_id": "t-auto",
                "goal_type": "loop",
                "lifecycle_profile": "loop-auto",
            }),
            &repo,
            "test-session",
        )
        .expect("loop+loop-auto should succeed");
        assert!(result.contains("\"ok\": true"), "result: {result}");

        let _ = std::fs::remove_dir_all(&repo);
    }
}
