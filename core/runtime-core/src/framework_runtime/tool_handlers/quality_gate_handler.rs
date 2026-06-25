//! Quality gate management tool handler (`domain:quality-gate`).
//! Payload construction + registered quality gate hook call.

use core_policy::error::FrameworkError;
use serde_json::{json, Value};
use std::path::Path;

/// quality_gate_manage: payload construction + registered quality gate hook call.
pub fn quality_gate_manage_dispatch(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, FrameworkError> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: operation (string)"))?;
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let repo_root_str = repo_root.to_string_lossy().to_string();

    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
    });
    if let Some(tid) = task_id {
        payload["task_id"] = json!(tid);
    }

    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("start requires 'goal' argument (string)"))?;
            payload["goal"] = json!(goal);
            if let Some(mr) = arguments.get("max_rounds").and_then(Value::as_u64) {
                payload["max_rounds"] = json!(mr);
            }
            if let Some(er) = arguments.get("allow_external_research").and_then(Value::as_bool) {
                payload["allow_external_research"] = json!(er);
            }
            let session_id = arguments
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(connection_session_id);
            payload["session_id"] = json!(session_id);
        }
        "append_round" => {
            let round = arguments
                .get("round")
                .and_then(Value::as_u64)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'round' argument (integer)"))?;
            payload["round"] = json!(round);

            let review_summary = arguments
                .get("review_summary")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'review_summary' argument (string)"))?;
            payload["review_summary"] = json!(review_summary);

            let fix_summary = arguments
                .get("fix_summary")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'fix_summary' argument (string)"))?;
            payload["fix_summary"] = json!(fix_summary);

            let verify_result = arguments
                .get("verify_result")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'verify_result' argument (string)"))?;
            if !matches!(verify_result, "PASS" | "FAIL" | "SKIPPED" | "UNKNOWN") {
                return Err(FrameworkError::validation(format!("verify_result must be one of PASS/FAIL/SKIPPED/UNKNOWN, got: {verify_result}")));
            }
            payload["verify_result"] = json!(verify_result);
            payload["supervisor_decision"] = json!(arguments
                .get("supervisor_decision")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'supervisor_decision' argument (string)"))?);
            payload["reason"] = json!(arguments
                .get("reason")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'reason' argument (string)"))?);
        }
        _ => return Err(FrameworkError::validation(format!(
            "Unknown quality gate operation: {operation}. Valid operations: start, append_round"
        ))),
    }

    // Delegate to the registered quality gate hook (runtime_exit_gate)
    let result = match host_projection::hooks::quality_gate_drive_registered() {
        Some(f) => f(payload)?,
        None => return Err(FrameworkError::validation(
            "framework_quality_gate runtime-core hook not registered; \
             runtime-core::boot() must be called before quality gate operations",
        )),
    };

    Ok(serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?)
}
