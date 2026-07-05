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

    // Diagnostic: log repo_root and operation for MCP persistence debugging
    tracing::info!(
        operation = operation,
        repo_root = %repo_root.display(),
        artifacts_dir_exists = repo_root.join("artifacts/current").is_dir(),
        "goal_state_manage_dispatch entry"
    );

    // Auto-resolve task_id from TASK_POINTERS.json
    // For start: auto-generate from goal text when TASK_POINTERS has no active task_id.
    let is_start = operation.trim().to_ascii_lowercase() == "start";
    let task_id = match arguments.get("task_id").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        Some(tid) => tid.to_string(),
        None => {
            if is_start {
                let goal = arguments.get("goal").and_then(Value::as_str).unwrap_or("unnamed");
                slugify_goal_text(goal)
            } else {
                // Fallback: try TASK_POINTERS, then discover from filesystem
                core_state::state_manager::read_primary_task_id(repo_root)
                    .or_else(|| discover_most_recent_goal_task_id(repo_root))
                    .ok_or_else(|| FrameworkError::validation(
                        "No active task_id in TASK_POINTERS.json and no task_id provided. \
                         No goal states found on disk either. \
                         Provide task_id explicitly or create a goal first."
                    ))?
            }
        }
    };
    core_state_utils::path_guard::validate_task_id_component(&task_id)?;

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

    // Diagnostic: verify file persistence after drive
    if operation == "start" {
        if let Some(task_id_val) = result.get("task_id").and_then(Value::as_str) {
            let goal_path = repo_root.join("artifacts/current").join(task_id_val).join("GOAL_STATE.json");
            tracing::info!(
                task_id = task_id_val,
                goal_path = %goal_path.display(),
                file_exists = goal_path.is_file(),
                "goal_state_manage_dispatch post-start: file persistence check"
            );
        }
    }

    Ok(serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?)
}

/// Generate a kebab-case task_id from a Chinese/English goal text.
/// E.g., "修复权限检查bug" → "fix-permission-check-bug"
/// Falls back to "goal-{timestamp}" if no recognizable words.
fn slugify_goal_text(goal: &str) -> String {
    // 1. Try to extract meaningful Chinese chars (not punctuation)
    let has_chinese = goal.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}');
    if has_chinese {
        let slug: String = goal
            .chars()
            .filter(|c| {
                c.is_ascii_alphanumeric() || (*c >= '\u{4e00}' && *c <= '\u{9fff}')
            })
            .take(40)
            .collect();
        if !slug.is_empty() {
            return slug;
        }
    }

    // 2. English path: take first 3 meaningful words, kebab-case
    let en_words: Vec<&str> = goal
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty() && w.len() > 1)
        .collect();
    if !en_words.is_empty() {
        let limit = en_words.len().min(3);
        return en_words[..limit].join("-").to_ascii_lowercase();
    }

    // 3. Fallback: timestamp-based
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("goal-{ts}")
}

/// Fallback: scan `artifacts/current/` for any GOAL_STATE.json and return
/// the directory name (used as task_id) of the most recently modified one.
fn discover_most_recent_goal_task_id(repo_root: &Path) -> Option<String> {
    let current_dir = repo_root.join("artifacts/current");
    let dir = std::fs::read_dir(&current_dir).ok()?;
    let mut candidates: Vec<(String, std::time::SystemTime)> = dir
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if !path.is_dir() {
                return None;
            }
            let tid = path.file_name()?.to_str()?;
            if core_state_utils::path_guard::safe_task_id_component(tid).is_none() {
                return None;
            }
            let goal_path = path.join("GOAL_STATE.json");
            if !goal_path.is_file() {
                return None;
            }
            let mtime = std::fs::metadata(&goal_path).ok()?.modified().ok()?;
            Some((tid.to_string(), mtime))
        })
        .collect();
    candidates.sort_by(|a, b| b.1.cmp(&a.1)); // most recent first
    candidates.into_iter().next().map(|(tid, _)| tid)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// Basic goal start should succeed without goal_type
    #[test]
    fn accepts_goal_start() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let repo = std::env::temp_dir().join(format!("goal-handler-basic-{suffix}"));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(repo.join("artifacts/current")).expect("mkdir");

        let result = goal_state_manage_dispatch(
            &json!({
                "operation": "start",
                "goal": "loop task",
                "task_id": "t-loop",
            }),
            &repo,
            "test-session",
        )
        .expect("goal start should be accepted");
        assert!(result.contains("\"ok\": true"), "result: {result}");

        let _ = std::fs::remove_dir_all(&repo);
    }
}
