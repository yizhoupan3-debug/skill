//! Chain Engine DAG MCP tools: chain_dag_init, chain_dag_tick, chain_dag_status,
//! chain_dag_retry, chain_dag_skip.
//!
//! These tools allow creating and managing DAG task chains with conditional
//! branching, fan-out/fan-in, retry policies, and timeout groups.

use chain_engine::scheduler::{advance_dag, load_condition_task_outputs, validate_dag, with_chain_lock, write_chain_file};
use chain_engine::tracker::process_post_tick;
use chain_engine::types::{
    ChainDagRoot, ChainMode, DagTaskEntry, TaskStatus,
};
use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::path::Path;

/// Create a new DAG chain from structured task descriptions.
/// Supports conditional branching, parallel groups, retry, timeout.
///
/// Parameters:
///   - chain_id: unique chain identifier
///   - tasks: array of task descriptors with depends_on, condition, etc.
///   - global_config: optional global configuration
pub(crate) fn tool_chain_dag_init(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let chain_id = arguments
        .get("chain_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "chain_dag_init: missing required argument 'chain_id'".to_string(),
            )
        })?
        .trim();
    if chain_id.is_empty() {
        return Err(FrameworkError::validation(
            "chain_dag_init: chain_id must not be empty".to_string(),
        ));
    }

    let tasks_val = arguments.get("tasks").and_then(Value::as_array).ok_or_else(
        || {
            FrameworkError::validation(
                "chain_dag_init: missing or invalid 'tasks' array".to_string(),
            )
        },
    )?;

    let mode_str = arguments
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("dag");
    let mode = match mode_str {
        "linear" => ChainMode::Linear,
        "dag" => ChainMode::Dag,
        other => {
            return Err(FrameworkError::validation(format!(
                "chain_dag_init: unknown mode '{other}', expected 'linear' or 'dag'"
            )));
        }
    };

    // Parse tasks from JSON
    let mut tasks: Vec<DagTaskEntry> = Vec::new();
    for (i, t) in tasks_val.iter().enumerate() {
        let task_id = t
            .get("task_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FrameworkError::validation(format!(
                    "chain_dag_init: task at index {i} missing 'task_id'"
                ))
            })?
            .to_string();

        let mut entry = DagTaskEntry::new(&task_id);

        // Title
        if let Some(title) = t.get("title").and_then(Value::as_str) {
            entry.title = Some(title.to_string());
        }

        // Depends on
        if let Some(deps) = t.get("depends_on").and_then(Value::as_array) {
            entry.depends_on = deps
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        // Condition
        if let Some(cond) = t.get("condition") {
            entry.condition = serde_json::from_value(cond.clone()).ok();
        }

        // Parallel group
        if let Some(pg) = t.get("parallel_group").and_then(Value::as_str) {
            entry.parallel_group = Some(pg.to_string());
        }

        // Timeout group
        if let Some(tg) = t.get("timeout_group") {
            entry.timeout_group = serde_json::from_value(tg.clone()).ok();
        }

        // Retry
        if let Some(rp) = t.get("retry") {
            entry.retry = serde_json::from_value(rp.clone()).ok();
        }

        // Status (if provided)
        if let Some(st) = t.get("status").and_then(Value::as_str) {
            entry.status = match st {
                "completed" => TaskStatus::Completed,
                "failed" => TaskStatus::Failed,
                "running" => TaskStatus::Running,
                _ => TaskStatus::Pending,
            };
        }

        tasks.push(entry);
    }

    let task_count = tasks.len();

    // Parse optional global_config
    let mut root = ChainDagRoot::new(chain_id, tasks);
    root.mode = mode;

    if let Some(gc) = arguments.get("global_config") {
        if let Ok(config) = serde_json::from_value(gc.clone()) {
            root.global_config = config;
        }
    }

    // Validate DAG structure
    validate_dag(&root)?;

    // Write to disk
    let path = chain_engine::chain_file_path(repo_root);
    write_chain_file(&path, &root)?;

    Ok(serde_json::to_string(&json!({
        "ok": true,
        "chain_id": chain_id,
        "mode": mode_str,
        "task_count": task_count,
        "path": path.to_string_lossy().to_string(),
    }))?)
}

/// Advance the DAG chain by one tick.
/// Applies the scheduler, then processes timeouts, then processes failures.
/// Returns the list of newly-ready tasks and any timeout/failure actions.
pub(crate) fn tool_chain_dag_tick(
    _arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let path = chain_engine::chain_file_path(repo_root);
    if !path.is_file() {
        return Err(FrameworkError::not_found(
            "TASK_CHAIN.json not found — create one with chain_dag_init first".to_string(),
        ));
    }

    let mut root = chain_engine::load_chain_from_path(&path)?;

    if root.mode != ChainMode::Dag {
        return Err(FrameworkError::validation(
            "chain_dag_tick: current chain is in 'linear' mode — use task_chain_advance instead"
                .to_string(),
        ));
    }

    // Run the scheduler + write under the chain lock (P2.1: prevent concurrent RMW cycles)
    let (ready, timeout_result, failure_result) =
        with_chain_lock(
            || -> std::result::Result<
                (Vec<String>, chain_engine::tracker::TimeoutResult, chain_engine::tracker::FailureActionResult),
                FrameworkError,
            > {
                let task_outputs = load_condition_task_outputs(repo_root, &root)?;
                let ready = advance_dag(&mut root, &task_outputs);
                let (timeout_result, failure_result) = process_post_tick(&mut root);
                write_chain_file(&path, &root)?;
                Ok((ready, timeout_result, failure_result))
            },
        )?;

    // Build response (outside the chain lock — no blocking during JSON serialization)

    // Build response
    Ok(serde_json::to_string(&json!({
        "ok": true,
        "newly_ready_tasks": ready,
        "timeout_failures": timeout_result.tasks_failed,
        "expired_groups": timeout_result.groups_expired,
        "retry_scheduled": failure_result.retry_scheduled,
        "tasks_skipped": failure_result.tasks_skipped,
        "dag_paused": failure_result.dag_paused,
        "dag_aborted": failure_result.dag_aborted,
        "status_counts": {
            "pending": root.tasks.iter().filter(|t| t.status == TaskStatus::Pending).count(),
            "running": root.tasks.iter().filter(|t| t.status == TaskStatus::Running).count(),
            "completed": root.tasks.iter().filter(|t| t.status == TaskStatus::Completed).count(),
            "failed": root.tasks.iter().filter(|t| t.status == TaskStatus::Failed).count(),
            "skipped": root.tasks.iter().filter(|t| t.status == TaskStatus::Skipped).count(),
            "blocked": root.tasks.iter().filter(|t| t.status == TaskStatus::Blocked).count(),
        }
    }))?)
}

/// Read the current DAG chain's full status.
pub(crate) fn tool_chain_dag_status(
    _arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let path = chain_engine::chain_file_path(repo_root);
    if !path.is_file() {
        return Err(FrameworkError::not_found(
            "TASK_CHAIN.json not found".to_string(),
        ));
    }

    let root = chain_engine::load_chain_from_path(&path)?;
    let counts = root.status_counts();

    let tasks_summary: Vec<Value> = root
        .tasks
        .iter()
        .map(|t| {
            json!({
                "task_id": t.task_id,
                "title": t.title,
                "status": t.status.as_str(),
                "attempt": t.attempt,
                "depends_on": t.depends_on,
                "parallel_group": t.parallel_group,
                "error": t.error,
            })
        })
        .collect();

    Ok(serde_json::to_string(&json!({
        "ok": true,
        "chain_id": root.chain_id,
        "mode": root.mode,
        "paused": root.paused,
        "task_count": root.tasks.len(),
        "status_counts": {
            "pending": counts.get("pending").copied().unwrap_or(0),
            "running": counts.get("running").copied().unwrap_or(0),
            "completed": counts.get("completed").copied().unwrap_or(0),
            "failed": counts.get("failed").copied().unwrap_or(0),
            "skipped": counts.get("skipped").copied().unwrap_or(0),
            "blocked": counts.get("blocked").copied().unwrap_or(0),
        },
        "tasks": tasks_summary,
        "global_config": {
            "max_concurrent_tasks": root.global_config.max_concurrent_tasks,
            "on_any_failure": format!("{:?}", root.global_config.on_any_failure).to_ascii_lowercase(),
        },
        "is_complete": root.tasks.iter().all(|t| t.status.is_terminal()),
    }))?)
}

/// Manually retry a specific failed task.
pub(crate) fn tool_chain_dag_retry(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "chain_dag_retry: missing required argument 'task_id'".to_string(),
            )
        })?
        .trim();
    if task_id.is_empty() {
        return Err(FrameworkError::validation(
            "chain_dag_retry: task_id must not be empty".to_string(),
        ));
    }

    let path = chain_engine::chain_file_path(repo_root);
    let mut root = chain_engine::load_chain_from_path(&path)?;

    let task = root.task_by_id_mut(task_id).ok_or_else(|| {
        FrameworkError::not_found(format!("task '{task_id}' not found in chain"))
    })?;

    if task.status != TaskStatus::Failed && task.status != TaskStatus::RetryScheduled {
        return Err(FrameworkError::validation(format!(
            "task '{task_id}' is not failed (status: {})",
            task.status.as_str()
        )));
    }

    // Reset for retry — move to pending
    task.status = TaskStatus::Pending;
    task.error = None;
    task.backoff_until = None;

    write_chain_file(&path, &root)?;

    Ok(serde_json::to_string(&json!({
        "ok": true,
        "task_id": task_id,
        "new_status": "pending",
    }))?)
}

/// Skip a specific task (mark as skipped regardless of current state).
pub(crate) fn tool_chain_dag_skip(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "chain_dag_skip: missing required argument 'task_id'".to_string(),
            )
        })?
        .trim();
    if task_id.is_empty() {
        return Err(FrameworkError::validation(
            "chain_dag_skip: task_id must not be empty".to_string(),
        ));
    }

    let path = chain_engine::chain_file_path(repo_root);
    let mut root = chain_engine::load_chain_from_path(&path)?;

    let task = root.task_by_id_mut(task_id).ok_or_else(|| {
        FrameworkError::not_found(format!("task '{task_id}' not found in chain"))
    })?;

    if task.status.is_terminal() {
        return Err(FrameworkError::validation(format!(
            "task '{task_id}' is already in terminal state: {}",
            task.status.as_str()
        )));
    }

    task.status = TaskStatus::Skipped;
    task.error = Some("manually skipped via chain_dag_skip".to_string());

    write_chain_file(&path, &root)?;

    Ok(serde_json::to_string(&json!({
        "ok": true,
        "task_id": task_id,
        "new_status": "skipped",
    }))?)
}

/// Resume a paused DAG chain (set `paused = false`) so the scheduler
/// and poller continue processing it.
pub(crate) fn tool_chain_dag_resume(
    _arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let path = chain_engine::chain_file_path(repo_root);
    if !path.is_file() {
        return Err(FrameworkError::not_found(
            "TASK_CHAIN.json not found".to_string(),
        ));
    }

    let mut root = chain_engine::load_chain_from_path(&path)?;

    if root.mode != chain_engine::types::ChainMode::Dag {
        return Err(FrameworkError::validation(
            "chain_dag_resume: current chain is in 'linear' mode — not applicable".to_string(),
        ));
    }

    if !root.paused {
        return Ok(serde_json::to_string(&json!({
            "ok": true,
            "chain_id": root.chain_id,
            "was_paused": false,
            "message": "chain was not paused",
        }))?);
    }

    // Unpause and unblock tasks that were blocked by pause
    root.paused = false;
    for task in &mut root.tasks {
        if task.status == chain_engine::types::TaskStatus::Blocked
            && task.error.as_deref() == Some("paused by failure strategy")
        {
            task.status = chain_engine::types::TaskStatus::Pending;
            task.error = None;
        }
    }

    write_chain_file(&path, &root)?;

    Ok(serde_json::to_string(&json!({
        "ok": true,
        "chain_id": root.chain_id,
        "was_paused": true,
        "tasks_unblocked": root.tasks.iter().filter(|t| t.status == chain_engine::types::TaskStatus::Pending).count(),
    }))?)
}
