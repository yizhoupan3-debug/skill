//! Task CRUD MCP tools: task_create, task_list, task_complete, task_focus.
//!
//! User-facing layer over the Task Engine: define todo → view todo → complete todo.

use super::*;
use core_errors::FrameworkError;
use core_state::task_ledger::{LedgerTransaction, append_transaction_assuming_l1_held};
use core_state::transition_validation::{TaskTransition, validate_transition};
use core_state_utils::task_write_lock::apply_task_ledger_mutation;
use serde_json::{Value, json};
use std::path::Path;

/// Create a new task: ensure directory, set focus, append `task_created` ledger entry.
///
/// Idempotent: if `task_id` directory already has a `TASK_LEDGER.jsonl`, returns early.
pub(crate) fn tool_task_create(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, String> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or("task_create: missing required argument 'task_id'")?
        .trim();
    if task_id.is_empty() {
        return Err("task_create: task_id must not be empty".to_string());
    }

    let title = arguments
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or(task_id);

    let task_id_owned = task_id.to_string();
    let title_owned = title.to_string();
    let repo_root_owned = repo_root.to_path_buf();

    let created = apply_task_ledger_mutation(repo_root, || {
        let task_dir =
            core_state::state_manager::ensure_task_directory(&repo_root_owned, &task_id_owned)?;

        // Idempotent: if task directory already has a TASK_LEDGER.jsonl, skip creation.
        let ledger_path = task_dir.join("TASK_LEDGER.jsonl");
        if ledger_path.is_file() {
            return Ok(false);
        }

        core_state::state_manager::set_task_focus(&repo_root_owned, &task_id_owned, &title_owned)?;

        append_transaction_assuming_l1_held(
            &repo_root_owned,
            &task_id_owned,
            LedgerTransaction {
                ts: framework_kernel::time::now_iso(),
                tx_type: "task_created".to_string(),
                payload: json!({
                    "task_id": &task_id_owned,
                    "title": &title_owned,
                }),
                idempotency_key: Some(format!("task_create:{task_id_owned}")),
                seq: None,
                schema_version: None,
            },
        )?;

        Ok(true)
    })?;

    Ok(json!({
        "ok": true,
        "task_id": &task_id_owned,
        "created": created,
    })
    .to_string())
}

/// List all known tasks with status, goal summary, and active/focus flags.
pub(crate) fn tool_task_list(repo_root: &Path) -> std::result::Result<String, String> {
    let task_ids = list_known_task_ids(repo_root);
    let (active_task_id, focus_task_id) =
        core_state::state_manager::read_task_pointer_pair(repo_root);

    let mut tasks: Vec<Value> = Vec::new();
    for task_id in &task_ids {
        let is_active = active_task_id.as_deref() == Some(task_id.as_str());
        let is_focus = focus_task_id.as_deref() == Some(task_id.as_str());

        let task_dir = repo_root.join("artifacts/current").join(task_id);

        // Read from GOAL_STATE.json (TASK_STATE.json removed in Wave 2b).
        let goal_path = task_dir.join("GOAL_STATE.json");

        let (status, goal_summary, has_evidence, goal_type, iteration_count) =
            if goal_path.is_file() {
                let raw = fs::read_to_string(&goal_path).unwrap_or_default();
                let v: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
                let status = v
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let goal_summary = v
                    .get("goal")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let evidence_path = task_dir.join("EVIDENCE_INDEX.json");
                let has_evidence = evidence_path.is_file()
                    && fs::read_to_string(&evidence_path)
                        .ok()
                        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                        .and_then(|v| {
                            v.get("artifacts")
                                .and_then(Value::as_array)
                                .map(|a| !a.is_empty())
                        })
                        .unwrap_or(false);
                let goal_type = v
                    .get("goal_type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let iteration_count = v
                    .get("iteration_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                (
                    status,
                    goal_summary,
                    has_evidence,
                    goal_type,
                    iteration_count,
                )
            } else {
                (
                    "created".to_string(),
                    String::new(),
                    false,
                    String::new(),
                    0,
                )
            };

        tasks.push(json!({
            "task_id": task_id,
            "status": status,
            "goal_summary": goal_summary,
            "has_evidence": has_evidence,
            "goal_type": goal_type,
            "iteration_count": iteration_count,
            "is_active": is_active,
            "is_focus": is_focus,
        }));
    }

    Ok(json!({
        "tasks": tasks,
        "count": tasks.len(),
        "active_task_id": active_task_id,
        "focus_task_id": focus_task_id,
    })
    .to_string())
}

/// Complete a task. If GOAL_STATE exists, delegates to `framework_goal_drive(complete)`.
/// Otherwise, neutralizes pointers and appends `task_completed` ledger entry.
pub(crate) fn tool_task_complete(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, String> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| core_state::state_manager::read_active_task_id(repo_root))
        .ok_or("task_complete: no task_id provided and no active task")?;

    let task_id_owned = task_id.clone();
    let repo_root_owned = repo_root.to_path_buf();

    // Check if GOAL_STATE exists → delegate to framework_goal_drive
    let task_dir = repo_root.join("artifacts/current").join(&task_id);
    let goal_path = task_dir.join("GOAL_STATE.json");
    let has_goal = goal_path.is_file()
        && fs::read_to_string(&goal_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .is_some_and(|v| v.get("archived").and_then(Value::as_bool) != Some(true));

    if has_goal {
        let payload = json!({
            "repo_root": repo_root.display().to_string(),
            "operation": "complete",
            "task_id": &task_id_owned,
        });
        let result = core_state::state_manager::framework_goal_drive(payload)?;
        return Ok(result.to_string());
    }

    // No GOAL_STATE → evidence check + pointer neutralization + ledger
    let transition_v = validate_transition(repo_root, &task_id, TaskTransition::Complete);
    if !transition_v.passed {
        return Err(format!(
            "task_complete blocked by evidence gate: {}",
            transition_v.reason
        ));
    }
    apply_task_ledger_mutation(repo_root, || {
        core_state::state_manager::neutralize_task_pointers_for_task(
            &repo_root_owned,
            &task_id_owned,
        )?;

        append_transaction_assuming_l1_held(
            &repo_root_owned,
            &task_id_owned,
            LedgerTransaction {
                ts: framework_kernel::time::now_iso(),
                tx_type: "task_completed".to_string(),
                payload: json!({
                    "task_id": &task_id_owned,
                }),
                idempotency_key: Some(format!("task_complete:{task_id_owned}")),
                seq: None,
                schema_version: None,
            },
        )?;

        Ok(())
    })?;

    // TASK_STATE.json aggregate was removed in Wave 2b.

    Ok(json!({
        "ok": true,
        "task_id": &task_id_owned,
        "completed": true,
    })
    .to_string())
}

/// Set focus to an existing task. Validates directory exists, then atomically writes both
/// active + focus pointers under the task write lock.
///
/// Runs `set_task_focus` under [`apply_task_ledger_mutation`] to prevent RMW races
/// with concurrent `tool_task_focus` or `neutralize_task_pointers_for_task`
/// (called from `tool_task_complete` / `framework_goal_drive complete`)
/// on the shared `TASK_POINTERS.json`.
pub(crate) fn tool_task_focus(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, String> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or("task_focus: missing required argument 'task_id'")?
        .trim();
    if task_id.is_empty() {
        return Err("task_focus: task_id must not be empty".to_string());
    }

    // Validate task_id is a safe path component before using it in filesystem ops
    let task_id = core_state_utils::path_guard::validate_task_id_component(task_id)
        .map_err(|_| format!("task_focus: invalid task_id '{task_id}'"))?;

    // Validate directory exists (cheap read — outside the lock)
    let task_dir = repo_root.join("artifacts/current").join(task_id);
    if !task_dir.is_dir() {
        return Err(format!(
            "task_focus: task directory '{task_id}' does not exist. Use task_create first."
        ));
    }

    // Acquire task write lock: prevents RMW race on TASK_POINTERS.json with
    // concurrent set_task_focus / neutralize_task_pointers_for_task.
    apply_task_ledger_mutation(repo_root, || {
        let goal_path = task_dir.join("GOAL_STATE.json");
        let label = if goal_path.is_file() {
            fs::read_to_string(&goal_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .and_then(|v| v.get("goal").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_else(|| task_id.to_string())
        } else {
            task_id.to_string()
        };

        core_state::state_manager::set_task_focus(repo_root, task_id, &label)?;
        Ok(())
    })?;

    Ok(json!({
        "ok": true,
        "task_id": task_id,
        "focused": true,
    })
    .to_string())
}

/// Advance the task chain to the next task.
/// Reads TASK_CHAIN.json, marks current as completed, switches focus to next.
pub(crate) fn tool_task_chain_advance(
    _arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, String> {
    let chain_path = repo_root.join("artifacts/current/TASK_CHAIN.json");
    if !chain_path.is_file() {
        return Err(
            "TASK_CHAIN.json not found — create one first to use task_chain_advance".to_string(),
        );
    }

    // Goals always follow loop semantics (GoalType::Linear removed in v10).
    // Chain advance is skipped — loop iteration is managed by explicit tool call.
    let (active, _) = core_state::state_manager::read_task_pointer_pair(repo_root);
    if let Some(ref tid) = active {
        return Ok(json!({
            "ok": true,
            "status": "loop_goal_skipped",
            "message": "current task follows loop semantics — task chain advance skipped",
            "task_id": tid,
        })
        .to_string());
    }

    // Wrap the read-modify-write cycle in the task lock to prevent concurrent
    // chain_advance calls from corrupting TASK_CHAIN.json.
    let repo_root_owned = repo_root.to_path_buf();
    let result = apply_task_ledger_mutation(repo_root, || {
        let raw =
            std::fs::read_to_string(&repo_root_owned.join("artifacts/current/TASK_CHAIN.json"))
                .map_err(FrameworkError::Io)?;
        let mut chain: Value = serde_json::from_str(&raw).map_err(FrameworkError::Json)?;
        let current_index = chain["current_index"].as_u64().unwrap_or(0) as usize;
        let tasks_len = chain["tasks"].as_array().map(|a| a.len()).unwrap_or(0);

        if current_index >= tasks_len {
            return Err(FrameworkError::validation(
                "all tasks in chain are completed — no task to advance to",
            ));
        }

        // Extract next task info before any writes (avoids borrow conflicts)
        let next = current_index + 1;
        let (next_id, next_title, all_complete) = if next >= tasks_len {
            (None, None, true)
        } else {
            let tasks = chain["tasks"].as_array().ok_or_else(|| {
                FrameworkError::config("TASK_CHAIN.json: 'tasks' is not an array")
            })?;
            let next_obj = &tasks[next];
            let nid = next_obj["task_id"]
                .as_str()
                .ok_or_else(|| {
                    FrameworkError::config("TASK_CHAIN.json: next task missing 'task_id'")
                })?
                .to_string();
            let ntitle = next_obj["title"].as_str().unwrap_or(&nid).to_string();
            (Some(nid), Some(ntitle), false)
        };

        // Mark current as completed
        if let Some(tasks) = chain["tasks"].as_array_mut() {
            if current_index < tasks.len() {
                if let Some(obj) = tasks[current_index].as_object_mut() {
                    obj.insert("status".to_string(), json!("completed"));
                }
            }
        }

        if all_complete {
            chain["current_index"] = json!(tasks_len);
            let path = repo_root_owned.join("artifacts/current/TASK_CHAIN.json");
            core_state_utils::atomic_write::write_atomic_json(&path, &chain)?;
            return Ok(json!({
                "ok": true,
                "status": "chain_complete",
                "message": "all tasks in chain completed",
            })
            .to_string());
        }

        #[allow(clippy::unwrap_used)]
        let next_id = next_id.unwrap();
        #[allow(clippy::unwrap_used)]
        let next_title = next_title.unwrap();

        // Mark next as running
        if let Some(tasks) = chain["tasks"].as_array_mut() {
            if next < tasks.len() {
                if let Some(obj) = tasks[next].as_object_mut() {
                    obj.insert("status".to_string(), json!("running"));
                }
            }
        }
        chain["current_index"] = json!(next);
        let path = repo_root_owned.join("artifacts/current/TASK_CHAIN.json");
        core_state_utils::atomic_write::write_atomic_json(&path, &chain)?;

        // Atomically switch focus to the next task
        core_state::state_manager::set_task_focus(&repo_root_owned, &next_id, &next_title)?;

        Ok(json!({
            "ok": true,
            "next_task_id": next_id,
            "next_title": next_title,
        })
        .to_string())
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(prefix: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("task-tools-{prefix}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("artifacts/current")).expect("mkdir");
        path
    }

    #[test]
    fn task_create_and_list_roundtrip() {
        let repo = unique_test_dir("create-list");
        let _rr = repo.display().to_string();

        // Create
        let result = tool_task_create(&json!({"task_id": "my-task", "title": "My Task"}), &repo)
            .expect("create");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["task_id"], json!("my-task"));
        assert_eq!(v["created"], json!(true));

        // Idempotent: create again
        let result2 = tool_task_create(&json!({"task_id": "my-task"}), &repo).expect("create2");
        let v2: Value = serde_json::from_str(&result2).expect("parse2");
        assert_eq!(v2["created"], json!(false));

        // List
        let list_result = tool_task_list(&repo).expect("list");
        let lv: Value = serde_json::from_str(&list_result).expect("parse list");
        assert_eq!(lv["count"], json!(1));
        assert_eq!(lv["tasks"][0]["task_id"], json!("my-task"));
        assert_eq!(lv["tasks"][0]["status"], json!("created"));
        assert_eq!(lv["tasks"][0]["is_focus"], json!(true));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_focus_switches_active() {
        let repo = unique_test_dir("focus");
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/task-a")).expect("mkdir a");
        fs::create_dir_all(repo.join("artifacts/current/task-b")).expect("mkdir b");

        // Focus task-a
        let result = tool_task_focus(&json!({"task_id": "task-a"}), &repo).expect("focus a");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["ok"], json!(true));

        // Focus task-b
        let result = tool_task_focus(&json!({"task_id": "task-b"}), &repo).expect("focus b");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["ok"], json!(true));

        // List: task-b is focus and active
        let list = tool_task_list(&repo).expect("list");
        let lv: Value = serde_json::from_str(&list).expect("parse list");
        for t in lv["tasks"].as_array().unwrap() {
            if t["task_id"] == "task-b" {
                assert_eq!(t["is_focus"], json!(true));
                assert_eq!(t["is_active"], json!(true));
            } else {
                assert_eq!(t["is_focus"], json!(false));
            }
        }

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_focus_rejects_missing_dir() {
        let repo = unique_test_dir("focus-missing");
        let err = tool_task_focus(&json!({"task_id": "nope"}), &repo).unwrap_err();
        assert!(err.contains("does not exist"), "err={err}");
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_complete_without_goal_neutralizes_pointers() {
        let repo = unique_test_dir("complete-no-goal");
        // Create task (sets both pointers)
        tool_task_create(&json!({"task_id": "done-task"}), &repo).expect("create");

        // Create evidence artifacts for the validate_transition gate
        let evidence_dir = repo.join("artifacts/current/done-task");
        fs::write(
            evidence_dir.join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[{"exit_code":0,"command":"test"}]}"#,
        )
        .expect("write evidence");

        // Complete
        let result = tool_task_complete(&json!({"task_id": "done-task"}), &repo).expect("complete");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["completed"], json!(true));

        // Verify pointers neutralized
        let (active, focus) = core_state::state_manager::read_task_pointer_pair(&repo);
        assert!(active.is_none() || active.as_deref() != Some("done-task"));
        assert!(focus.is_none() || focus.as_deref() != Some("done-task"));

        // Verify raw file has no active/focus keys and task removed from tasks array
        let raw = fs::read_to_string(repo.join("artifacts/current/TASK_POINTERS.json"))
            .unwrap_or_default();
        let v: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
        assert!(v.get("active_task_id").is_none());
        assert!(v.get("focus_task_id").is_none());
        if let Some(tasks) = v.get("tasks").and_then(Value::as_array) {
            assert!(
                tasks
                    .iter()
                    .all(|t| t.get("task_id").and_then(Value::as_str) != Some("done-task")),
                "done-task should be removed from tasks array"
            );
        }

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_complete_defaults_to_active_task() {
        let repo = unique_test_dir("complete-active");
        tool_task_create(&json!({"task_id": "auto-complete"}), &repo).expect("create");

        // Create evidence artifacts for the validate_transition gate
        let evidence_dir = repo.join("artifacts/current/auto-complete");
        fs::write(
            evidence_dir.join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[{"exit_code":0,"command":"test"}]}"#,
        )
        .expect("write evidence");

        // Complete without task_id — should use active task
        let result = tool_task_complete(&json!({}), &repo).expect("complete active");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["task_id"], json!("auto-complete"));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_list_empty_when_no_tasks() {
        let repo = unique_test_dir("list-empty");
        let result = tool_task_list(&repo).expect("list");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["count"], json!(0));
        assert_eq!(v["tasks"], json!([]));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_list_shows_goal_type_and_iteration_for_loop_goal() {
        let repo = unique_test_dir("list-loop-gt");
        let rr = repo.display().to_string();

        // Start a loop goal
        core_state::state_manager::framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "loop-list",
            "goal": "loop list test",
            "goal_type": "loop",
            "drive_until_done": false,
        }))
        .expect("start loop goal");

        // Complete one iteration
        // Create evidence artifacts for the validate_transition gate
        let evidence_dir = repo.join("artifacts/current/loop-list");
        fs::create_dir_all(&evidence_dir).expect("mkdir evidence");
        fs::write(
            evidence_dir.join("EVIDENCE_INDEX.json"),
            r#"{"artifacts":[{"exit_code":0,"command":"test"}]}"#,
        )
        .expect("write evidence");

        core_state::state_manager::framework_goal_drive(json!({
            "repo_root": repo.display().to_string(),
            "operation": "complete",
            "task_id": "loop-list",
        }))
        .expect("complete iter");

        // task_list should expose goal_type and iteration_count
        let result = tool_task_list(&repo).expect("list");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["count"], json!(1));
        let task = &v["tasks"][0];
        assert_eq!(task["task_id"], json!("loop-list"));
        assert_eq!(task["goal_type"], json!("loop"));
        assert_eq!(task["iteration_count"], json!(1));
        assert_eq!(task["status"], json!("running"));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_chain_advance_skips_loop_goal() {
        let repo = unique_test_dir("chain-loop-skip");
        let rr = repo.display().to_string();

        // Start a loop goal
        core_state::state_manager::framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "loop-task",
            "goal": "loop chain test",
            "goal_type": "loop",
            "drive_until_done": false,
        }))
        .expect("start loop goal");

        // Write a minimal TASK_CHAIN.json
        fs::write(
            repo.join("artifacts/current/TASK_CHAIN.json"),
            r#"{"tasks":[{"task_id":"loop-task","title":"Loop Task","status":"running"}],"current_index":0}"#,
        )
        .expect("write chain");

        // Chain advance must return loop_goal_skipped
        let result = tool_task_chain_advance(&json!({}), &repo).expect("chain advance");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["status"], json!("loop_goal_skipped"));
        assert_eq!(v["task_id"], json!("loop-task"));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_chain_advance_skips_loop_when_focus_different() {
        // When active task is a loop goal and focus is different,
        // chain advance should still detect the active loop goal and skip.
        let repo = unique_test_dir("chain-loop-focus");
        let rr = repo.display().to_string();

        // Create two tasks
        core_state::state_manager::framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "linear-task",
            "goal": "linear task",
            "drive_until_done": false,
        }))
        .expect("start linear");

        // Create a loop task and explicitly focus it
        core_state::state_manager::framework_goal_drive(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "loop-task",
            "goal": "loop chain test",
            "goal_type": "loop",
            "drive_until_done": false,
            "set_focus": true,
        }))
        .expect("start loop goal");

        // Write TASK_CHAIN.json
        fs::write(
            repo.join("artifacts/current/TASK_CHAIN.json"),
            r#"{"tasks":[{"task_id":"loop-task","title":"Loop Task","status":"running"}],"current_index":0}"#,
        )
        .expect("write chain");

        let result = tool_task_chain_advance(&json!({}), &repo).expect("chain advance");
        let v: Value = serde_json::from_str(&result).expect("parse");
        assert_eq!(v["status"], json!("loop_goal_skipped"));

        let _ = fs::remove_dir_all(&repo);
    }
}
