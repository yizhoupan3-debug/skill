//! DAG Scheduler — topological ready-task discovery and single-tick advancement.
//!
//! The scheduler is **idempotent**: each call to `advance_dag()` fully recomputes
//! the DAG state with no dependency on prior scheduler state. This means the
//! engine can safely crash and restart without persistent scheduler state.

use core_errors::FrameworkError;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::compat;
use crate::types::{
    ChainDagRoot, ChainMode, ConditionOperator, ConditionType, DagCondition, DagTaskEntry,
    FailureStrategy, GlobalDagConfig, RetryPolicy, TaskStatus,
};

/// Find tasks whose dependency and capacity constraints are satisfied,
/// and transition their status to "running".
///
/// Returns the IDs of tasks that were transitioned to running.
pub fn advance_dag(root: &mut ChainDagRoot) -> Vec<String> {
    if root.mode != ChainMode::Dag || root.paused {
        return Vec::new();
    }

    // Phase 0: mark retry_scheduled tasks whose backoff has expired as pending.
    handle_expired_backoffs(root);

    let mut ready: Vec<String> = Vec::new();
    let gc = &root.global_config;

    // Count currently running tasks per parallel group and globally.
    let (global_running, group_running) = count_running(&root.tasks);
    let global_capacity = gc.max_concurrent_tasks as usize;

    // Phase 2: find ready tasks and transition them to running.
    for task in &root.tasks {
        if task.status != TaskStatus::Pending && task.status != TaskStatus::RetryScheduled {
            continue;
        }

        // Check global concurrency capacity.
        if global_running + ready.len() >= global_capacity {
            break;
        }

        // Check dependency completion.
        if !dependencies_met(task, &root.tasks) {
            continue;
        }

        // Check parallel group capacity.
        if let Some(ref pg) = task.parallel_group {
            let group_count = group_running.get(pg.as_str()).copied().unwrap_or(0)
                + ready.iter().filter(|t| {
                    root.task_by_id(t)
                        .and_then(|e| e.parallel_group.as_deref())
                        == Some(pg.as_str())
                }).count();
            // No explicit per-group limit; global capacity bounds it.
            if group_count > 0 && group_count >= global_capacity / 2.max(1) {
                continue;
            }
        }

        ready.push(task.task_id.clone());
    }

    // Apply transitions in a separate loop to avoid borrow conflicts.
    for task_id in &ready {
        if let Some(task) = root.task_by_id_mut(task_id) {
            task.status = TaskStatus::Running;
            task.attempt = task.attempt.saturating_add(1);
            task.started_at = Some(framework_core::time::now_iso());
        }
    }

    ready
}

/// Count currently running tasks globally and per parallel group.
fn count_running(tasks: &[DagTaskEntry]) -> (usize, HashMap<&str, usize>) {
    let mut global = 0usize;
    let mut by_group: HashMap<&str, usize> = HashMap::new();
    for task in tasks {
        if task.status == TaskStatus::Running {
            global += 1;
            if let Some(ref pg) = task.parallel_group {
                *by_group.entry(pg.as_str()).or_insert(0) += 1;
            }
        }
    }
    (global, by_group)
}

/// Check whether all dependencies of a task are completed.
fn dependencies_met(task: &DagTaskEntry, all_tasks: &[DagTaskEntry]) -> bool {
    if task.depends_on.is_empty() {
        return true; // Root task — no dependencies.
    }
    let task_id_to_status: HashMap<&str, &TaskStatus> = all_tasks
        .iter()
        .map(|t| (t.task_id.as_str(), &t.status))
        .collect();
    for dep_id in &task.depends_on {
        match task_id_to_status.get(dep_id.as_str()) {
            Some(TaskStatus::Completed) => {}   // OK
            Some(TaskStatus::Skipped) => {}       // Also OK — skipped is still "done"
            Some(_) => return false,               // Not yet done.
            None => return false,                  // Unknown dependency.
        }
    }
    true
}

/// Handle retry_scheduled tasks whose backoff has expired — move them back to pending.
fn handle_expired_backoffs(root: &mut ChainDagRoot) {
    let now = framework_core::time::now_iso();
    for task in &mut root.tasks {
        if task.status == TaskStatus::RetryScheduled {
            if let Some(ref backoff_until) = task.backoff_until {
                if backoff_until.as_str() <= now.as_str() {
                    task.status = TaskStatus::Pending;
                    task.backoff_until = None;
                }
            }
        }
    }
}

/// Build a summary of all tasks that are ready to execute (for diagnostic output).
/// Unlike `advance_dag`, this is a pure read — it doesn't modify any status.
pub fn compute_ready_tasks(root: &ChainDagRoot) -> Vec<&DagTaskEntry> {
    root.tasks
        .iter()
        .filter(|t| {
            (t.status == TaskStatus::Pending || t.status == TaskStatus::RetryScheduled)
                && dependencies_met(t, &root.tasks)
        })
        .collect()
}

/// Determine whether a chain is fully complete (all tasks in terminal states).
pub fn is_chain_complete(root: &ChainDagRoot) -> bool {
    root.tasks.iter().all(|t| t.status.is_terminal())
}

/// Validate the DAG structure:
/// - No cycles in the dependency graph
/// - All dependency references resolve to real task IDs
/// - No duplicate task_ids
pub fn validate_dag(root: &ChainDagRoot) -> Result<(), FrameworkError> {
    let ids: HashSet<&str> = root.tasks.iter().map(|t| t.task_id.as_str()).collect();

    // Check for duplicate task_ids
    if ids.len() != root.tasks.len() {
        let mut seen = HashSet::new();
        for t in &root.tasks {
            if !seen.insert(t.task_id.as_str()) {
                return Err(FrameworkError::validation(format!(
                    "duplicate task_id in chain: '{}'",
                    t.task_id
                )));
            }
        }
    }

    // Check all dependency references resolve and detect cycles via DFS
    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();

    for task in &root.tasks {
        for dep in &task.depends_on {
            if !ids.contains(dep.as_str()) {
                return Err(FrameworkError::validation(format!(
                    "task '{}' depends on unknown task '{}'",
                    task.task_id, dep
                )));
            }
        }
        // Cycle detection via DFS
        if !visited.contains(task.task_id.as_str()) {
            let mut stack = Vec::new();
            if detect_cycle(task.task_id.as_str(), &root.tasks, &mut visited, &mut in_stack, &mut stack) {
                return Err(FrameworkError::validation(format!(
                    "cycle detected in chain: {}",
                    stack.join(" -> ")
                )));
            }
        }
    }

    Ok(())
}

fn detect_cycle<'a>(
    node: &'a str,
    tasks: &'a [DagTaskEntry],
    visited: &mut HashSet<&'a str>,
    in_stack: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
) -> bool {
    if in_stack.contains(node) {
        path.push(node);
        return true;
    }
    if visited.contains(node) {
        return false;
    }
    visited.insert(node);
    in_stack.insert(node);
    path.push(node);

    if let Some(task) = tasks.iter().find(|t| t.task_id == node) {
        for dep in &task.depends_on {
            if detect_cycle(dep.as_str(), tasks, visited, in_stack, path) {
                return true;
            }
        }
    }

    path.pop();
    in_stack.remove(node);
    false
}

/// Evaluate a condition against a source task's TASK_OUTPUT.
/// Returns true if the condition is met (or if there is no condition).
pub fn evaluate_condition(
    task: &DagTaskEntry,
    all_task_outputs: &HashMap<String, core_state::task_output::TaskOutput>,
) -> bool {
    let Some(ref condition) = task.condition else {
        return true; // No condition = always execute.
    };

    // Find the source task's output.
    let source_output = match all_task_outputs.get(&condition.source) {
        Some(o) => o,
        None => return false, // Source task has no output — condition fails.
    };

    let field_value = resolve_field(&source_output, &condition);
    let Some(ref actual) = field_value else {
        return false; // Field not found — condition fails.
    };

    compare_values(actual, &condition.value, &condition.operator)
}

/// Resolve a condition's field path against a TaskOutput.
fn resolve_field(
    output: &core_state::task_output::TaskOutput,
    condition: &DagCondition,
) -> Option<Value> {
    match condition.condition_type {
        ConditionType::Status => {
            // Compare against the task's status string (use Debug for display)
            Some(Value::String(format!("{:?}", output.status).to_ascii_lowercase()))
        }
        ConditionType::OutputField => {
            let field = condition.field.as_deref().unwrap_or("outputs.verification_status");
            let parts: Vec<&str> = field.split('.').collect();
            let val = serde_json::to_value(output).ok()?;
            resolve_json_path(&val, &parts).cloned()
        }
        ConditionType::Expression => {
            // For expressions, just pass through the raw value (evaluated externally)
            None
        }
    }
}

/// Walk a JSON path like "outputs.verification_status" on a serde_json::Value.
fn resolve_json_path<'a>(val: &'a Value, parts: &[&str]) -> Option<&'a Value> {
    let mut current = val;
    for part in parts {
        match current {
            Value::Object(map) => {
                current = map.get(*part)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Compare two JSON values using the given operator.
fn compare_values(actual: &Value, expected: &Value, op: &ConditionOperator) -> bool {
    match op {
        ConditionOperator::Eq => {
            if let (Some(a), Some(b)) = (actual.as_str(), expected.as_str()) {
                a == b
            } else if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                (a - b).abs() < f64::EPSILON
            } else if let (Some(a), Some(b)) = (actual.as_bool(), expected.as_bool()) {
                a == b
            } else {
                actual == expected
            }
        }
        ConditionOperator::Ne => !compare_values(actual, expected, &ConditionOperator::Eq),
        ConditionOperator::Gt => {
            if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                a > b
            } else {
                actual.as_str() > expected.as_str()
            }
        }
        ConditionOperator::Gte => {
            if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                a >= b
            } else {
                actual.as_str() >= expected.as_str()
            }
        }
        ConditionOperator::Lt => {
            if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                a < b
            } else {
                actual.as_str() < expected.as_str()
            }
        }
        ConditionOperator::Lte => {
            if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                a <= b
            } else {
                actual.as_str() <= expected.as_str()
            }
        }
        ConditionOperator::In => {
            if let Some(arr) = expected.as_array() {
                arr.iter().any(|v| compare_values(actual, v, &ConditionOperator::Eq))
            } else {
                false
            }
        }
        ConditionOperator::NotIn => {
            if let Some(arr) = expected.as_array() {
                !arr.iter().any(|v| compare_values(actual, v, &ConditionOperator::Eq))
            } else {
                true
            }
        }
    }
}

/// Load and validate the chain file, apply `advance_dag`, and write back.
/// Returns the list of task IDs that were transitioned to running.
pub fn load_advance_write(repo_root: &Path) -> Result<Vec<String>, FrameworkError> {
    let path = crate::chain_file_path(repo_root);
    let mut root = compat::load_chain_file(&path)?;
    let ready = advance_dag(&mut root);
    if !ready.is_empty() || root.mode == ChainMode::Dag {
        write_chain_file(&path, &root)?;
    }
    Ok(ready)
}

/// Write a ChainDagRoot back to disk.
pub fn write_chain_file(
    path: &Path,
    root: &ChainDagRoot,
) -> Result<(), FrameworkError> {
    let json = serde_json::to_string_pretty(root)
        .map_err(|e| FrameworkError::Json(e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| FrameworkError::Io(e))?;
    }
    std::fs::write(path, &json)
        .map_err(|e| FrameworkError::Io(e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::types::*;

    fn make_chain(tasks: Vec<DagTaskEntry>) -> ChainDagRoot {
        let mut root = ChainDagRoot::new("test-chain", tasks);
        root.mode = ChainMode::Dag;
        root
    }

    #[test]
    fn simple_linear_dag() {
        let mut chain = make_chain(vec![
            DagTaskEntry::new("a"),
            DagTaskEntry {
                task_id: "b".to_string(),
                depends_on: vec!["a".to_string()],
                ..DagTaskEntry::new("b")
            },
        ]);
        // First tick: only 'a' is ready
        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "a");
        assert_eq!(chain.tasks[0].status, TaskStatus::Running);
        assert_eq!(chain.tasks[1].status, TaskStatus::Pending);

        // Complete 'a' manually
        chain.tasks[0].status = TaskStatus::Completed;
        // Second tick: 'b' should now be ready
        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "b");
    }

    #[test]
    fn diamond_topology() {
        let mut chain = make_chain(vec![
            DagTaskEntry::new("root"),
            DagTaskEntry {
                task_id: "left".to_string(),
                depends_on: vec!["root".to_string()],
                ..DagTaskEntry::new("left")
            },
            DagTaskEntry {
                task_id: "right".to_string(),
                depends_on: vec!["root".to_string()],
                ..DagTaskEntry::new("right")
            },
            DagTaskEntry {
                task_id: "merge".to_string(),
                depends_on: vec!["left".to_string(), "right".to_string()],
                ..DagTaskEntry::new("merge")
            },
        ]);

        // Tick 1: root only
        assert_eq!(advance_dag(&mut chain).len(), 1);
        assert_eq!(chain.tasks[0].status, TaskStatus::Running);
        chain.tasks[0].status = TaskStatus::Completed;

        // Tick 2: left + right (both ready)
        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 2);
        assert!(ready.contains(&"left".to_string()));
        assert!(ready.contains(&"right".to_string()));
        chain.tasks[1].status = TaskStatus::Completed;
        chain.tasks[2].status = TaskStatus::Completed;

        // Tick 3: merge
        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "merge");
    }

    #[test]
    fn global_concurrency_cap() {
        let mut chain = make_chain(vec![
            DagTaskEntry::new("a"),
            DagTaskEntry::new("b"),
            DagTaskEntry::new("c"),
        ]);
        chain.global_config.max_concurrent_tasks = 2;

        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 2);
        assert_eq!(
            chain.tasks.iter().filter(|t| t.status == TaskStatus::Running).count(),
            2
        );
    }

    #[test]
    fn fan_in_waits_for_all() {
        let mut chain = make_chain(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                ..DagTaskEntry::new("a")
            },
            DagTaskEntry {
                task_id: "b".to_string(),
                depends_on: vec!["a".to_string()],
                ..DagTaskEntry::new("b")
            },
            DagTaskEntry {
                task_id: "c".to_string(),
                depends_on: vec!["a".to_string(), "b".to_string()],
                ..DagTaskEntry::new("c")
            },
        ]);

        // Tick 1: only 'a' is ready (b depends on a)
        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "a");
        assert_eq!(chain.tasks[2].status, TaskStatus::Pending); // 'c' blocked

        // Complete 'a'
        chain.tasks[0].status = TaskStatus::Completed;

        // Tick 2: 'b' becomes ready (dependency 'a' is done)
        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "b");
        assert_eq!(chain.tasks[2].status, TaskStatus::Pending); // 'c' still blocked

        // Complete 'b'
        chain.tasks[1].status = TaskStatus::Completed;

        // Tick 3: 'c' becomes ready (both dependencies resolved)
        let ready = advance_dag(&mut chain);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "c");
    }

    #[test]
    fn validate_detects_cycle() {
        let chain = make_chain(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                depends_on: vec!["b".to_string()],
                ..DagTaskEntry::new("a")
            },
            DagTaskEntry {
                task_id: "b".to_string(),
                depends_on: vec!["a".to_string()],
                ..DagTaskEntry::new("b")
            },
        ]);
        let result = validate_dag(&chain);
        assert!(result.is_err(), "should detect cycle");
    }

    #[test]
    fn validate_detects_missing_dep() {
        let chain = make_chain(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                depends_on: vec!["nonexistent".to_string()],
                ..DagTaskEntry::new("a")
            },
        ]);
        let result = validate_dag(&chain);
        assert!(result.is_err(), "should detect missing dep");
    }

    #[test]
    fn evaluate_condition_passed() {
        let json = serde_json::json!({
            "schema_version": "task-output-v1",
            "task_id": "source",
            "status": "completed",
            "outputs": {
                "verification_status": "passed",
                "summary": "all good"
            }
        });
        let output: core_state::task_output::TaskOutput =
            serde_json::from_value(json).unwrap();
        let mut outputs = HashMap::new();
        outputs.insert("source".to_string(), output);

        let task = DagTaskEntry {
            task_id: "child".to_string(),
            condition: Some(DagCondition {
                source: "source".to_string(),
                condition_type: ConditionType::OutputField,
                field: Some("outputs.verification_status".to_string()),
                operator: ConditionOperator::Eq,
                value: serde_json::json!("passed"),
            }),
            ..DagTaskEntry::new("child")
        };

        assert!(evaluate_condition(&task, &outputs));
    }

    #[test]
    fn evaluate_condition_fails_on_mismatch() {
        let json = serde_json::json!({
            "schema_version": "task-output-v1",
            "task_id": "source",
            "status": "completed",
            "outputs": {
                "verification_status": "failed"
            }
        });
        let output: core_state::task_output::TaskOutput =
            serde_json::from_value(json).unwrap();
        let mut outputs = HashMap::new();
        outputs.insert("source".to_string(), output);

        let task = DagTaskEntry {
            task_id: "child".to_string(),
            condition: Some(DagCondition {
                source: "source".to_string(),
                condition_type: ConditionType::OutputField,
                field: Some("outputs.verification_status".to_string()),
                operator: ConditionOperator::Eq,
                value: serde_json::json!("passed"),
            }),
            ..DagTaskEntry::new("child")
        };

        assert!(!evaluate_condition(&task, &outputs));
    }

    #[test]
    fn compute_ready_finds_available() {
        let chain = make_chain(vec![
            DagTaskEntry::new("root"),
            DagTaskEntry {
                task_id: "child".to_string(),
                depends_on: vec!["root".to_string()],
                ..DagTaskEntry::new("child")
            },
        ]);
        let ready = compute_ready_tasks(&chain);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task_id, "root");
    }

    #[test]
    fn detect_chain_complete() {
        let mut chain = make_chain(vec![
            DagTaskEntry::new("a"),
        ]);
        assert!(!is_chain_complete(&chain));
        chain.tasks[0].status = TaskStatus::Completed;
        assert!(is_chain_complete(&chain));
    }
}
