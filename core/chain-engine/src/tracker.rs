//! DAG Tracker — timeout groups, retry scheduling, and failure strategy application.
//!
//! The tracker is called after `advance_dag` to handle post-execution state:
//! - Mark tasks in expired timeout groups as failed
//! - Schedule retries for failed tasks with retry policies
//! - Apply failure strategies (pause_dag / abort_dag / continue)

use core_errors::FrameworkError;
use std::collections::{HashMap, HashSet};

use crate::types::{
    ChainDagRoot, ChainMode, FailureStrategy, TaskStatus,
};

/// Return value from `process_timeouts` — IDs of tasks that were failed by timeout.
pub struct TimeoutResult {
    pub tasks_failed: Vec<String>,
    pub groups_expired: Vec<String>,
}

/// Return value from `process_failures` — actions taken.
pub struct FailureActionResult {
    /// Tasks that were marked as retry_scheduled.
    pub retry_scheduled: Vec<String>,
    /// Tasks that were skipped due to abort/pause.
    pub tasks_skipped: Vec<String>,
    /// Whether the DAG was paused.
    pub dag_paused: bool,
    /// Whether the DAG was aborted.
    pub dag_aborted: bool,
}

/// Process timeout groups — mark expired groups' running tasks as failed.
///
/// A timeout group starts its clock when the first task in that group enters
/// Running state. All tasks in the group are expected to complete within
/// `max_seconds`. If any task in the group exceeds the threshold, all
/// remaining running tasks in the group are marked as failed with reason
/// "group_timeout".
pub fn process_timeouts(root: &mut ChainDagRoot) -> TimeoutResult {
    if root.mode != ChainMode::Dag {
        return TimeoutResult {
            tasks_failed: Vec::new(),
            groups_expired: Vec::new(),
        };
    }

    let now = framework_core::time::now_iso();
    // Track group-level info: for each timeout group, find the earliest started_at
    // across ALL tasks in the group (including completed/failed ones).
    struct GroupInfo {
        max_seconds: u64,
        earliest_started: Option<String>,
        running_tasks: Vec<String>,
    }

    let mut groups: std::collections::HashMap<&str, GroupInfo> = std::collections::HashMap::new();

    for task in &root.tasks {
        let Some(ref tg) = task.timeout_group else { continue };

        let info = groups.entry(tg.group_id.as_str()).or_insert(GroupInfo {
            max_seconds: tg.max_seconds,
            earliest_started: None,
            running_tasks: Vec::new(),
        });

        // Validate max_seconds consistency (CE-13 fix)
        if info.max_seconds != tg.max_seconds {
            tracing::warn!(
                "timeout group '{}' has inconsistent max_seconds: {} vs {}",
                tg.group_id, info.max_seconds, tg.max_seconds
            );
        }

        // Track earliest started_at across ALL tasks in this group
        if let Some(ref started) = task.started_at {
            let is_earlier = info.earliest_started.as_ref().map_or(true, |earliest| {
                started.as_str() < earliest.as_str()
            });
            if is_earlier {
                info.earliest_started = Some(started.clone());
            }
        }

        // Track running tasks for the group
        if task.status == TaskStatus::Running {
            info.running_tasks.push(task.task_id.clone());
        }
    }

    let mut tasks_failed = Vec::new();
    let mut groups_expired = Vec::new();

    // Collect expired groups
    let expired: Vec<(String, Vec<String>, u64)> = groups
        .iter()
        .filter_map(|(group_id, info)| {
            let earliest = info.earliest_started.as_ref()?;
            if is_timed_out(earliest, &now, info.max_seconds) {
                Some((
                    group_id.to_string(),
                    info.running_tasks.clone(),
                    info.max_seconds,
                ))
            } else {
                None
            }
        })
        .collect();

    for (group_id, task_ids, max_secs) in &expired {
        groups_expired.push(group_id.clone());
        for task_id in task_ids {
            if let Some(task) = root.task_by_id_mut(task_id) {
                if task.status == TaskStatus::Running {
                    task.status = TaskStatus::Failed;
                    task.error = Some(format!(
                        "group_timeout: group '{group_id}' exceeded {}s limit",
                        max_secs
                    ));
                    tasks_failed.push(task_id.clone());
                }
            }
        }
    }

    TimeoutResult {
        tasks_failed,
        groups_expired,
    }
}

/// Check if an ISO-8601 timestamp is older than `max_secs` from now.
fn is_timed_out(started_at: &str, now: &str, max_secs: u64) -> bool {
    // Simple heuristic: compare second-level ISO timestamps
    // YYYY-MM-DDTHH:MM:SS format — extract the time portion and do a rough check
    let start_secs = parse_iso_seconds(started_at);
    let now_secs = parse_iso_seconds(now);
    match (start_secs, now_secs) {
        (Some(s), Some(n)) => n.saturating_sub(s) >= max_secs,
        _ => false,
    }
}

/// Parse an ISO-8601 timestamp and return seconds since epoch (or approximate).
fn parse_iso_seconds(iso: &str) -> Option<u64> {
    // Handle common ISO formats: "2026-06-30T12:34:56Z" or "2026-06-30T12:34:56.123Z"
    let clean = iso.trim_end_matches('Z').trim_end_matches('z');
    // Parse just the date/time part
    let dt = chrono::NaiveDateTime::parse_from_str(clean, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(clean, "%Y-%m-%dT%H:%M:%S%.f"))
        .ok()?;
    // Use a fixed epoch offset — relative comparison is what matters
    let epoch = chrono::NaiveDateTime::parse_from_str("2026-01-01T00:00:00", "%Y-%m-%dT%H:%M:%S").ok()?;
    Some((dt - epoch).num_seconds().max(0) as u64)
}

/// Process failed tasks: apply retry policies and failure strategies.
///
/// This should be called after `process_timeouts` and after the caller
/// has updated task statuses from external completion/failure signals.
pub fn process_failures(root: &mut ChainDagRoot) -> FailureActionResult {
    if root.mode != ChainMode::Dag {
        return FailureActionResult {
            retry_scheduled: Vec::new(),
            tasks_skipped: Vec::new(),
            dag_paused: false,
            dag_aborted: false,
        };
    }

    let mut action = FailureActionResult {
        retry_scheduled: Vec::new(),
        tasks_skipped: Vec::new(),
        dag_paused: false,
        dag_aborted: false,
    };

    // Step 1: Handle retry policies for failed tasks
    for task in &mut root.tasks {
        if task.status != TaskStatus::Failed {
            continue;
        }

        // Determine retry policy (task-level or global default)
        let policy = task.retry.as_ref().or(root.global_config.default_retry.as_ref());
        let Some(policy) = policy.cloned() else {
            continue; // No retry policy — task stays failed
        };

        if task.attempt < policy.max_attempts {
            // Schedule retry
            let backoff_ms = policy.backoff_ms(task.attempt.saturating_add(1));
            let backoff_duration = chrono::Duration::milliseconds(backoff_ms as i64);

            // Compute backoff_until timestamp
            let now_chrono = chrono::Utc::now();
            let backoff_until = now_chrono + backoff_duration;
            task.backoff_until = Some(backoff_until.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string());
            task.status = TaskStatus::RetryScheduled;
            action.retry_scheduled.push(task.task_id.clone());
        }
    }

    // Step 2: Abort remaining tasks if any failed
    let any_failed = root.tasks.iter().any(|t| t.status == TaskStatus::Failed);
    if !any_failed {
        return action;
    }

    match root.global_config.on_any_failure {
        FailureStrategy::AbortDag => {
            action.dag_aborted = true;
            let skipped_ids: Vec<String> = {
                let mut ids = Vec::new();
                // Skip all non-terminal tasks
                for task in &mut root.tasks {
                    if !task.status.is_terminal()
                        && task.status != TaskStatus::Failed
                        && task.status != TaskStatus::RetryScheduled
                    {
                        task.status = TaskStatus::Skipped;
                        task.error = Some("aborted by failure strategy".to_string());
                        ids.push(task.task_id.clone());
                        action.tasks_skipped.push(task.task_id.clone());
                    }
                }
                // Also skip retry-scheduled tasks
                for task in &mut root.tasks {
                    if task.status == TaskStatus::RetryScheduled {
                        task.status = TaskStatus::Skipped;
                        task.error = Some("aborted by failure strategy".to_string());
                        ids.push(task.task_id.clone());
                        action.tasks_skipped.push(task.task_id.clone());
                    }
                }
                ids
            };
            // CE-11: Populate caused_skips on the failed tasks
            for task in &mut root.tasks {
                if task.status == TaskStatus::Failed {
                    task.caused_skips = skipped_ids.clone();
                }
            }
        }
        FailureStrategy::PauseDag => {
            action.dag_paused = true;
            root.paused = true;
            let blocked_ids: Vec<String> = {
                let mut ids = Vec::new();
                for task in &mut root.tasks {
                    if task.status == TaskStatus::Pending || task.status == TaskStatus::RetryScheduled {
                        task.status = TaskStatus::Blocked;
                        task.error = Some("paused by failure strategy".to_string());
                        ids.push(task.task_id.clone());
                        action.tasks_skipped.push(task.task_id.clone());
                    }
                }
                ids
            };
            // CE-11: Populate caused_skips on the failed tasks
            for task in &mut root.tasks {
                if task.status == TaskStatus::Failed {
                    task.caused_skips = blocked_ids.clone();
                }
            }
        }
        FailureStrategy::Continue => {
            // Do nothing — let other tasks continue
        }
    }

    action
}

/// Full post-execution processing: timeouts → failures → strategy.
/// Convenience wrapper for the poller.
pub fn process_post_tick(root: &mut ChainDagRoot) -> (TimeoutResult, FailureActionResult) {
    let timeout_result = process_timeouts(root);
    let failure_result = process_failures(root);
    (timeout_result, failure_result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::types::*;

    fn make_root(tasks: Vec<DagTaskEntry>) -> ChainDagRoot {
        let mut root = ChainDagRoot::new("test", tasks);
        root.mode = ChainMode::Dag;
        root
    }

    #[test]
    fn retry_scheduled_on_failure() {
        let mut root = make_root(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                retry: Some(RetryPolicy {
                    max_attempts: 3,
                    backoff_base_ms: 100,
                    backoff_multiplier: 2.0,
                    max_backoff_ms: None,
                }),
                status: TaskStatus::Failed,
                attempt: 1,
                error: Some("something broke".to_string()),
                ..DagTaskEntry::new("a")
            },
        ]);

        let result = process_failures(&mut root);
        assert_eq!(result.retry_scheduled.len(), 1);
        assert_eq!(root.tasks[0].status, TaskStatus::RetryScheduled);
        assert!(root.tasks[0].backoff_until.is_some());
    }

    #[test]
    fn retry_exhausted_stays_failed() {
        let mut root = make_root(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                retry: Some(RetryPolicy {
                    max_attempts: 2,
                    backoff_base_ms: 100,
                    backoff_multiplier: 1.0,
                    max_backoff_ms: None,
                }),
                status: TaskStatus::Failed,
                attempt: 2,
                ..DagTaskEntry::new("a")
            },
        ]);

        let result = process_failures(&mut root);
        assert!(result.retry_scheduled.is_empty());
        assert_eq!(root.tasks[0].status, TaskStatus::Failed);
    }

    #[test]
    fn abort_dag_skips_remaining() {
        let mut root = make_root(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                status: TaskStatus::Failed,
                ..DagTaskEntry::new("a")
            },
            DagTaskEntry {
                task_id: "b".to_string(),
                status: TaskStatus::Pending,
                ..DagTaskEntry::new("b")
            },
            DagTaskEntry::with_title("c", "C"),
        ]);
        root.global_config.on_any_failure = FailureStrategy::AbortDag;

        let result = process_failures(&mut root);
        assert!(result.dag_aborted);
        assert_eq!(result.tasks_skipped.len(), 2);
        assert_eq!(root.tasks[0].status, TaskStatus::Failed);
        assert_eq!(root.tasks[1].status, TaskStatus::Skipped);
        assert_eq!(root.tasks[2].status, TaskStatus::Skipped);
    }

    #[test]
    fn pause_dag_blocks_pending() {
        let mut root = make_root(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                status: TaskStatus::Failed,
                ..DagTaskEntry::new("a")
            },
            DagTaskEntry {
                task_id: "b".to_string(),
                status: TaskStatus::Pending,
                ..DagTaskEntry::new("b")
            },
            DagTaskEntry {
                task_id: "c".to_string(),
                status: TaskStatus::Running,
                ..DagTaskEntry::new("c")
            },
        ]);
        root.global_config.on_any_failure = FailureStrategy::PauseDag;

        let result = process_failures(&mut root);
        assert!(result.dag_paused);
        assert!(root.paused);
        assert_eq!(root.tasks[1].status, TaskStatus::Blocked);
        assert_eq!(root.tasks[2].status, TaskStatus::Running); // Running untouched
    }

    #[test]
    fn continue_keeps_going() {
        let mut root = make_root(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                status: TaskStatus::Failed,
                ..DagTaskEntry::new("a")
            },
            DagTaskEntry {
                task_id: "b".to_string(),
                status: TaskStatus::Running,
                ..DagTaskEntry::new("b")
            },
        ]);
        root.global_config.on_any_failure = FailureStrategy::Continue;

        let result = process_failures(&mut root);
        assert!(!result.dag_paused);
        assert!(!result.dag_aborted);
        assert!(result.tasks_skipped.is_empty());
        assert_eq!(root.tasks[1].status, TaskStatus::Running);
    }

    #[test]
    fn timeout_group_expires() {
        let mut root = make_root(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                timeout_group: Some(TimeoutGroupSpec {
                    group_id: "fast-group".to_string(),
                    max_seconds: 1,
                }),
                status: TaskStatus::Running,
                started_at: Some("2026-01-01T00:00:00Z".to_string()), // Long ago
                ..DagTaskEntry::new("a")
            },
        ]);

        let result = process_timeouts(&mut root);
        assert_eq!(result.groups_expired.len(), 1);
        assert_eq!(result.tasks_failed.len(), 1);
        assert_eq!(root.tasks[0].status, TaskStatus::Failed);
        assert!(root.tasks[0]
            .error
            .as_ref()
            .unwrap()
            .contains("group_timeout"));
    }

    #[test]
    fn timeout_group_not_expired() {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.fZ").to_string();
        let mut root = make_root(vec![
            DagTaskEntry {
                task_id: "a".to_string(),
                timeout_group: Some(TimeoutGroupSpec {
                    group_id: "fast-group".to_string(),
                    max_seconds: 3600,
                }),
                status: TaskStatus::Running,
                started_at: Some(now),
                ..DagTaskEntry::new("a")
            },
        ]);

        let result = process_timeouts(&mut root);
        assert!(result.groups_expired.is_empty());
        assert_eq!(root.tasks[0].status, TaskStatus::Running);
    }
}
