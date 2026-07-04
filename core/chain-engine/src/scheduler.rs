//! DAG Scheduler — topological ready-task discovery and single-tick advancement.
//!
//! The scheduler is **idempotent**: each call to `advance_dag()` fully recomputes
//! the DAG state with no dependency on prior scheduler state. This means the
//! engine can safely crash and restart without persistent scheduler state.

use core_errors::FrameworkError;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::types::{
    ChainDagRoot, ChainMode, ConditionOperator, ConditionType, DagCondition, DagTaskEntry, TaskStatus,
};

/// Find tasks whose dependency, condition, and capacity constraints are satisfied,
/// and transition their status to "running".
///
/// `task_outputs` provides completed task outputs for evaluating `OutputField` and
/// `Expression` conditions. Pass an empty map when outputs are unavailable — `Status`
/// conditions are always evaluated from the in-memory DAG state.
///
/// Returns the IDs of tasks that were transitioned to running.
pub fn advance_dag(
    root: &mut ChainDagRoot,
    task_outputs: &HashMap<String, core_state::task_output::TaskOutput>,
) -> Vec<String> {
    if root.mode != ChainMode::Dag || root.paused {
        return Vec::new();
    }

    // Extract values from global_config before any mutable borrow of root
    let max_concurrent_tasks = root.global_config.max_concurrent_tasks;
    if max_concurrent_tasks == 0 {
        return Vec::new();
    }

    // Phase 0: mark retry_scheduled tasks whose backoff has expired as pending.
    handle_expired_backoffs(root);

    // Pre-build dependency status map (PERF-P2-002: O(n²) → O(n))
    let status_map: HashMap<&str, &TaskStatus> = root
        .tasks
        .iter()
        .map(|t| (t.task_id.as_str(), &t.status))
        .collect();

    // Phase 1: collect all eligible tasks (regardless of capacity)
    // to enable fair round-robin selection (CE-17).
    let mut eligible_by_group: std::collections::HashMap<Option<&str>, Vec<&str>> =
        std::collections::HashMap::new();
    let mut any_eligible = false;

    for task in &root.tasks {
        if task.status != TaskStatus::Pending && task.status != TaskStatus::RetryScheduled {
            continue;
        }
        if !dependencies_met_cached(task, &status_map) {
            continue;
        }
        // P1.1: Evaluate condition gate — skip tasks whose condition is not met.
        // Status conditions use the in-memory DAG state map; OutputField/Expression
        // require task outputs (loaded by the caller).
        if !condition_met(task, &status_map, task_outputs) {
            continue;
        }
        any_eligible = true;
        let group_key = task.parallel_group.as_deref();
        eligible_by_group.entry(group_key).or_default().push(task.task_id.as_str());
    }

    if !any_eligible {
        return Vec::new();
    }

    // Phase 2: round-robin select from groups until capacity is reached (CE-17 fairness).
    let group_keys: Vec<Option<&str>> = eligible_by_group.keys().copied().collect();
    let (global_running, group_running) = count_running(&root.tasks);
    let global_capacity = max_concurrent_tasks as usize;
    let group_cap = (global_capacity + 1) / 2; // ceil division (CE-09 fix)

    let mut ready: Vec<String> = Vec::new();
    loop {
        let mut any_added = false;
        for gk in &group_keys {
            if global_running + ready.len() >= global_capacity {
                break;
            }
            let tasks = eligible_by_group.get_mut(gk).unwrap();
            if tasks.is_empty() {
                continue;
            }
            // Check per-group capacity (only for named groups, CE-09 fix)
            let running_in_group = gk.map_or(0, |g| group_running.get(g).copied().unwrap_or(0));
            // Only apply group caps to tasks with an explicit parallel_group
            if gk.is_some() && running_in_group + ready.iter().filter(|t| {
                root.task_by_id(t)
                    .and_then(|e| e.parallel_group.as_deref())
                    == *gk
            }).count() >= group_cap {
                continue;
            }
            // Take the first task from this group
            if let Some(tid) = tasks.first().copied() {
                ready.push(tid.to_string());
                tasks.remove(0);
                any_added = true;
            }
        }
        if !any_added || global_running + ready.len() >= global_capacity {
            break;
        }
    }

    // Apply transitions
    for task_id in &ready {
        if let Some(task) = root.task_by_id_mut(task_id) {
            task.status = TaskStatus::Running;
            task.attempt = task.attempt.saturating_add(1);
            task.started_at = Some(framework_core::time::now_iso());
        }
    }

    ready
}

/// Cached version of dependency check using a pre-built status map (PERF-P2-002).
fn dependencies_met_cached(
    task: &DagTaskEntry,
    status_map: &HashMap<&str, &TaskStatus>,
) -> bool {
    if task.depends_on.is_empty() {
        return true;
    }
    for dep_id in &task.depends_on {
        match status_map.get(dep_id.as_str()) {
            Some(TaskStatus::Completed) | Some(TaskStatus::Skipped) => {}
            _ => return false,
        }
    }
    true
}

/// Evaluate whether a pending task's condition gate is satisfied.
///
/// - No condition → always eligible (returns true).
/// - `Status` condition → compared against source task's DAG status.
/// - `OutputField` / `Expression` → delegated to [`evaluate_condition`] (needs task outputs).
fn condition_met(
    task: &DagTaskEntry,
    status_map: &HashMap<&str, &TaskStatus>,
    task_outputs: &HashMap<String, core_state::task_output::TaskOutput>,
) -> bool {
    let Some(ref condition) = task.condition else {
        return true; // No condition = always eligible.
    };
    match condition.condition_type {
        ConditionType::Status => {
            // Evaluate against the source task's DAG execution status.
            let raw_status = match status_map.get(condition.source.as_str()) {
                Some(TaskStatus::Completed) => "completed",
                Some(TaskStatus::Failed) => "failed",
                Some(TaskStatus::Skipped) => "skipped",
                Some(TaskStatus::Pending) => "pending",
                Some(TaskStatus::Running) => "running",
                Some(TaskStatus::Blocked) => "blocked",
                Some(TaskStatus::RetryScheduled) => "retry_scheduled",
                None => return false, // Source not found → fail closed.
            };
            compare_values(
                &Value::String(raw_status.to_string()),
                &condition.value,
                &condition.operator,
            )
        }
        ConditionType::OutputField | ConditionType::Expression => {
            // OutputField/Expression need the source task's TASK_OUTPUT.json data.
            evaluate_condition(task, task_outputs)
        }
    }
}

/// Load TASK_OUTPUT for all completed/skipped tasks referenced by DAG conditions.
///
/// This is an optimization — only loads outputs that the scheduler actually needs
/// for condition evaluation. Callers of `advance_dag` that have `repo_root` should
/// call this before invoking the scheduler.
pub fn load_condition_task_outputs(
    repo_root: &Path,
    root: &ChainDagRoot,
) -> Result<HashMap<String, core_state::task_output::TaskOutput>, FrameworkError> {
    // Collect unique condition source task IDs.
    let sources: HashSet<&str> = root
        .tasks
        .iter()
        .filter_map(|t| t.condition.as_ref().map(|c| c.source.as_str()))
        .collect();

    let mut outputs = HashMap::new();
    for src in sources {
        if outputs.contains_key(src) {
            continue;
        }
        // Only load if the source task has terminal DAG status (output is final).
        if let Some(src_task) = root.task_by_id(src) {
            if !src_task.status.is_terminal() {
                continue;
            }
        }
        if let Ok(Some(output)) = core_state::task_output::read_task_output(repo_root, src) {
            outputs.insert(src.to_string(), output);
        }
    }
    Ok(outputs)
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
    let now = chrono::Utc::now();
    for task in &mut root.tasks {
        if task.status == TaskStatus::RetryScheduled {
            if let Some(ref backoff_until) = task.backoff_until {
                if let Ok(deadline) = chrono::DateTime::parse_from_rfc3339(backoff_until) {
                    if now >= deadline {
                        task.status = TaskStatus::Pending;
                        task.backoff_until = None;
                    }
                }
            }
        }
    }
}

/// Build a summary of all tasks that are ready to execute (for diagnostic output).
/// Unlike `advance_dag`, this is a pure read — it doesn't modify any status.
/// Evaluates both dependency and condition gates (Status conditions use DAG state;
/// OutputField/Expression conditions use empty outputs map → treated as unsatisfied).
pub fn compute_ready_tasks(root: &ChainDagRoot) -> Vec<&DagTaskEntry> {
    let status_map: HashMap<&str, &TaskStatus> = root
        .tasks
        .iter()
        .map(|t| (t.task_id.as_str(), &t.status))
        .collect();
    let empty_outputs = HashMap::new();
    root.tasks
        .iter()
        .filter(|t| {
            (t.status == TaskStatus::Pending || t.status == TaskStatus::RetryScheduled)
                && dependencies_met(t, &root.tasks)
                && condition_met(t, &status_map, &empty_outputs)
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

    // Check all dependency references resolve
    for task in &root.tasks {
        for dep in &task.depends_on {
            if !ids.contains(dep.as_str()) {
                return Err(FrameworkError::validation(format!(
                    "task '{}' depends on unknown task '{}'",
                    task.task_id, dep
                )));
            }
        }
        // CE-12: Validate condition.source references
        if let Some(ref cond) = task.condition {
            if !ids.contains(cond.source.as_str()) {
                return Err(FrameworkError::validation(format!(
                    "task '{}' condition references unknown task '{}'",
                    task.task_id, cond.source
                )));
            }
        }
    }

    // Cycle detection via iterative DFS (CE-06: avoid stack overflow on deep chains)
    for task in &root.tasks {
        let _visited: HashSet<&str> = HashSet::new();
        if has_cycle_iterative(task.task_id.as_str(), &root.tasks) {
            return Err(FrameworkError::validation(format!(
                "cycle detected in chain starting from '{}'",
                task.task_id
            )));
        }
    }

    Ok(())
}

/// Iterative cycle detection using explicit stack (CE-06: avoid stack overflow).
fn has_cycle_iterative<'a>(
    start_node: &'a str,
    tasks: &'a [DagTaskEntry],
) -> bool {
    // Build adjacency list
    let task_map: HashMap<&str, &[String]> = tasks
        .iter()
        .map(|t| (t.task_id.as_str(), t.depends_on.as_slice()))
        .collect();

    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();

    struct StackFrame<'a> {
        node: &'a str,
        deps: &'a [String],
        index: usize,
    }

    let mut stack: Vec<StackFrame> = Vec::new();

    visited.insert(start_node);
    in_stack.insert(start_node);
    if let Some(deps) = task_map.get(start_node) {
        stack.push(StackFrame {
            node: start_node,
            deps,
            index: 0,
        });
    }

    while let Some(frame) = stack.last_mut() {
        if frame.index >= frame.deps.len() {
            let frame = stack.pop().unwrap();
            in_stack.remove(frame.node);
            continue;
        }

        let dep = frame.deps[frame.index].as_str();
        frame.index += 1;

        if in_stack.contains(dep) {
            return true; // Cycle detected
        }

        if !visited.contains(dep) {
            visited.insert(dep);
            in_stack.insert(dep);
            if let Some(deps) = task_map.get(dep) {
                stack.push(StackFrame {
                    node: dep,
                    deps,
                    index: 0,
                });
            }
        }
    }

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
                false // non-numeric types cannot be ordered
            }
        }
        ConditionOperator::Gte => {
            if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                a >= b
            } else {
                false
            }
        }
        ConditionOperator::Lt => {
            if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                a < b
            } else {
                false
            }
        }
        ConditionOperator::Lte => {
            if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
                a <= b
            } else {
                false
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
    let mut root = crate::load_chain_from_path(&path)?;
    let task_outputs = load_condition_task_outputs(repo_root, &root)?;
    let ready = advance_dag(&mut root, &task_outputs);
    // Only write if there were actual transitions (dirty check).
    if !ready.is_empty() {
        write_chain_file(&path, &root)?;
    }
    Ok(ready)
}

/// Process-level mutex guarding the TASK_CHAIN.json RMW cycle.
///
/// Prevents concurrent read-modify-write between the background poller thread
/// and manual `chain_dag_tick` MCP calls. Without this guard, simultaneous
/// RMW cycles in the same process could corrupt the chain file or lose
/// intermediate updates.
///
/// Cross-process locking (flock / lockfile) is NOT yet implemented — a note
/// in `engine.rs` describes this as a future enhancement. For single-process
/// deployments (the current design), this mutex is sufficient.
pub fn with_chain_lock<R>(f: impl FnOnce() -> R) -> R {
    static CHAIN_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = CHAIN_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    f()
}

/// Write a ChainDagRoot back to disk atomically.
/// NOTE: does NOT acquire the chain lock — callers should wrap their full RMW
/// cycle with [`with_chain_lock`] if concurrent access is expected.
pub fn write_chain_file(
    path: &Path,
    root: &ChainDagRoot,
) -> Result<(), FrameworkError> {
    let value = serde_json::to_value(root)?;
    core_state_utils::atomic_write::write_atomic_json(path, &value)?;
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
        let ready = advance_dag(&mut chain, &HashMap::new());
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "a");
        assert_eq!(chain.tasks[0].status, TaskStatus::Running);
        assert_eq!(chain.tasks[1].status, TaskStatus::Pending);

        // Complete 'a' manually
        chain.tasks[0].status = TaskStatus::Completed;
        // Second tick: 'b' should now be ready
        let ready = advance_dag(&mut chain, &HashMap::new());
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
        assert_eq!(advance_dag(&mut chain, &HashMap::new()).len(), 1);
        assert_eq!(chain.tasks[0].status, TaskStatus::Running);
        chain.tasks[0].status = TaskStatus::Completed;

        // Tick 2: left + right (both ready)
        let ready = advance_dag(&mut chain, &HashMap::new());
        assert_eq!(ready.len(), 2);
        assert!(ready.contains(&"left".to_string()));
        assert!(ready.contains(&"right".to_string()));
        chain.tasks[1].status = TaskStatus::Completed;
        chain.tasks[2].status = TaskStatus::Completed;

        // Tick 3: merge
        let ready = advance_dag(&mut chain, &HashMap::new());
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

        let ready = advance_dag(&mut chain, &HashMap::new());
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
        let ready = advance_dag(&mut chain, &HashMap::new());
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "a");
        assert_eq!(chain.tasks[2].status, TaskStatus::Pending); // 'c' blocked

        // Complete 'a'
        chain.tasks[0].status = TaskStatus::Completed;

        // Tick 2: 'b' becomes ready (dependency 'a' is done)
        let ready = advance_dag(&mut chain, &HashMap::new());
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "b");
        assert_eq!(chain.tasks[2].status, TaskStatus::Pending); // 'c' still blocked

        // Complete 'b'
        chain.tasks[1].status = TaskStatus::Completed;

        // Tick 3: 'c' becomes ready (both dependencies resolved)
        let ready = advance_dag(&mut chain, &HashMap::new());
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
