//! Core types for the DAG Task Chain Engine — chain-dag-v1 schema.
//!
//! These types extend the existing linear TASK_CHAIN.json format with DAG
//! capabilities: conditional branching, parallel groups, retry policies,
//! timeout groups, and failure strategies.

use serde::{Deserialize, Serialize};

// ── Schema version ──

pub const CHAIN_DAG_SCHEMA_VERSION: &str = "chain-dag-v1";

// ── Enums ──

/// Chain execution mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChainMode {
    /// Original linear mode — tasks execute one by one in array order.
    Linear,
    /// DAG mode — tasks execute according to depends_on, parallel_group, conditions.
    Dag,
}

impl Default for ChainMode {
    fn default() -> Self {
        Self::Linear
    }
}

/// Task execution status within a DAG chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting for dependencies to complete.
    Pending,
    /// Running (dispatched to a subagent or user).
    Running,
    /// Completed successfully.
    Completed,
    /// Failed (terminal, unless retry is configured).
    Failed,
    /// Blocked by failure strategy or manual pause.
    Blocked,
    /// Skipped by condition evaluation (condition was false).
    Skipped,
    /// Retry scheduled (backoff timer running).
    RetryScheduled,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Skipped | TaskStatus::Blocked
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Skipped => "skipped",
            TaskStatus::RetryScheduled => "retry_scheduled",
        }
    }
}

/// Failure strategy when any task in the chain fails.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FailureStrategy {
    /// Pause the DAG — mark pending tasks as blocked, await manual intervention.
    PauseDag,
    /// Abort the DAG — skip all remaining tasks.
    AbortDag,
    /// Continue execution despite failure.
    Continue,
}

impl Default for FailureStrategy {
    fn default() -> Self {
        Self::PauseDag
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionType {
    OutputField,
    Status,
    Expression,
}

impl Default for ConditionType {
    fn default() -> Self {
        Self::OutputField
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Eq,
    Ne,
    Gte,
    Lte,
    Gt,
    Lt,
    In,
    NotIn,
}

impl Default for ConditionOperator {
    fn default() -> Self {
        Self::Eq
    }
}

// ── Structs ──

/// Condition gate that determines whether a task should execute.
/// Evaluated against the source task's TASK_OUTPUT.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DagCondition {
    /// Source task_id whose output is evaluated.
    pub source: String,
    /// Condition type.
    #[serde(rename = "type")]
    #[serde(default)]
    pub condition_type: ConditionType,
    /// Field path within the source task's output (e.g. "outputs.verification_status").
    #[serde(default)]
    pub field: Option<String>,
    /// Comparison operator.
    #[serde(default)]
    pub operator: ConditionOperator,
    /// Value to compare against.
    pub value: serde_json::Value,
}

/// Timeout group specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimeoutGroupSpec {
    /// Group identifier — tasks sharing this ID share a single timeout clock.
    pub group_id: String,
    /// Maximum wall-clock seconds for the group (from first task start).
    pub max_seconds: u64,
}

/// Retry policy for a single task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
    /// Maximum number of attempts.
    pub max_attempts: u32,
    /// Base backoff delay in milliseconds.
    #[serde(default = "default_backoff_base_ms")]
    pub backoff_base_ms: u64,
    /// Backoff multiplier (applied each retry).
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Maximum backoff in milliseconds (optional cap).
    #[serde(default)]
    pub max_backoff_ms: Option<u64>,
}

const fn default_backoff_base_ms() -> u64 {
    1000
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

impl RetryPolicy {
    /// Compute the backoff delay for the next retry attempt.
    /// attempt is 1-based (first retry = 1).
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        let base = self.backoff_base_ms as f64;
        let multiplier = self.backoff_multiplier;
        let raw = (base * multiplier.powi((attempt - 1) as i32)) as u64;
        match self.max_backoff_ms {
            Some(max) => raw.min(max),
            None => raw,
        }
    }
}

/// Per-task DAG entry in the chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct DagTaskEntry {
    pub task_id: String,
    #[serde(default)]
    pub title: Option<String>,

    // ── DAG dependency fields (all #[serde(default)]) ──

    /// Upstream task IDs that must complete before this task.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Round number for hierarchical grouping (default 1).
    /// Steps in round N must complete before round N+1 begins.
    #[serde(default)]
    pub round: Option<u32>,
    /// Optional condition gate — if present, must evaluate to true to execute.
    #[serde(default)]
    pub condition: Option<DagCondition>,
    /// Parallel group ID — tasks in the same group may execute concurrently.
    #[serde(default)]
    pub parallel_group: Option<String>,
    /// Timeout group specification.
    #[serde(default)]
    pub timeout_group: Option<TimeoutGroupSpec>,
    /// Retry policy for this task.
    #[serde(default)]
    pub retry: Option<RetryPolicy>,

    // ── Runtime state (managed by the engine) ──

    #[serde(default)]
    pub status: TaskStatus,
    /// Current attempt number (0 = first attempt, 1+ = retries).
    #[serde(default)]
    pub attempt: u32,
    /// ISO-8601 timestamp of when the task started running.
    #[serde(default)]
    pub started_at: Option<String>,
    /// ISO-8601 timestamp of when the task completed.
    #[serde(default)]
    pub completed_at: Option<String>,
    /// Error message from the last failed attempt.
    #[serde(default)]
    pub error: Option<String>,
    /// Backoff expiry timestamp (ISO-8601) for retry_scheduled tasks.
    #[serde(default)]
    pub backoff_until: Option<String>,
    /// Which tasks skipped due to this task's failure (populated by abort/pause).
    #[serde(default)]
    pub caused_skips: Vec<String>,
}

impl DagTaskEntry {
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            title: None,
            depends_on: Vec::new(),
            round: None,
            condition: None,
            parallel_group: None,
            timeout_group: None,
            retry: None,
            status: TaskStatus::Pending,
            attempt: 0,
            started_at: None,
            completed_at: None,
            error: None,
            backoff_until: None,
            caused_skips: Vec::new(),
        }
    }

    pub fn with_title(task_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            ..Self::new(task_id)
        }
    }
}

/// Global configuration for the DAG chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GlobalDagConfig {
    /// Maximum number of tasks that may be running concurrently.
    #[serde(default = "default_max_concurrent_tasks")]
    pub max_concurrent_tasks: u32,
    /// Default retry policy (applied when task has none specified).
    #[serde(default)]
    pub default_retry: Option<RetryPolicy>,
    /// Strategy when any task fails.
    #[serde(default)]
    pub on_any_failure: FailureStrategy,
}

const fn default_max_concurrent_tasks() -> u32 {
    4
}

impl Default for GlobalDagConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 4,
            default_retry: None,
            on_any_failure: FailureStrategy::PauseDag,
        }
    }
}

/// Root schema for the DAG chain file (chain-dag-v1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChainDagRoot {
    pub schema_version: String,
    pub chain_id: String,
    #[serde(default)]
    pub mode: ChainMode,
    pub tasks: Vec<DagTaskEntry>,
    #[serde(default)]
    pub global_config: GlobalDagConfig,
    /// When true, the background poller is paused.
    #[serde(default)]
    pub paused: bool,
}

impl ChainDagRoot {
    /// Create a new DAG chain with the given chain_id and tasks.
    pub fn new(chain_id: impl Into<String>, tasks: Vec<DagTaskEntry>) -> Self {
        Self {
            schema_version: CHAIN_DAG_SCHEMA_VERSION.to_string(),
            chain_id: chain_id.into(),
            mode: ChainMode::Dag,
            tasks,
            global_config: GlobalDagConfig::default(),
            paused: false,
        }
    }

    /// Return the task entry by ID, if present.
    pub fn task_by_id(&self, task_id: &str) -> Option<&DagTaskEntry> {
        self.tasks.iter().find(|t| t.task_id == task_id)
    }

    /// Return the task entry by ID mutably, if present.
    pub fn task_by_id_mut(&mut self, task_id: &str) -> Option<&mut DagTaskEntry> {
        self.tasks.iter_mut().find(|t| t.task_id == task_id)
    }

    /// Collect all task IDs.
    pub fn task_ids(&self) -> Vec<&str> {
        self.tasks.iter().map(|t| t.task_id.as_str()).collect()
    }

    /// Count tasks in each status.
    pub fn status_counts(&self) -> std::collections::HashMap<&str, usize> {
        let mut counts = std::collections::HashMap::new();
        for task in &self.tasks {
            *counts.entry(task.status.as_str()).or_insert(0) += 1;
        }
        counts
    }
}

// ── Serde helpers for TaskStatus ──

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn chain_dag_root_roundtrip() {
        let root = ChainDagRoot::new(
            "fix-bugs",
            vec![
                DagTaskEntry::with_title("scan", "Scan codebase"),
                DagTaskEntry {
                    task_id: "fix".to_string(),
                    title: None,
                    depends_on: vec!["scan".to_string()],
                    round: None,
                    condition: None,
                    parallel_group: None,
                    timeout_group: None,
                    retry: Some(RetryPolicy {
                        max_attempts: 3,
                        backoff_base_ms: 1000,
                        backoff_multiplier: 2.0,
                        max_backoff_ms: Some(30000),
                    }),
                    status: TaskStatus::Pending,
                    attempt: 0,
                    started_at: None,
                    completed_at: None,
                    error: None,
                    backoff_until: None,
                    caused_skips: Vec::new(),
                },
            ],
        );

        let json = serde_json::to_string_pretty(&root).expect("serialize");
        let deserialized: ChainDagRoot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.chain_id, "fix-bugs");
        assert_eq!(deserialized.mode, ChainMode::Dag);
        assert_eq!(deserialized.tasks.len(), 2);
        assert_eq!(deserialized.tasks[0].task_id, "scan");
        assert_eq!(deserialized.tasks[1].depends_on, vec!["scan"]);
        assert!(deserialized.tasks[1].retry.is_some());
    }

    #[test]
    fn chain_dag_root_default_mode() {
        let json = r#"{
            "schema_version": "chain-dag-v1",
            "chain_id": "test",
            "tasks": [{"task_id": "a"}]
        }"#;
        let root: ChainDagRoot = serde_json::from_str(json).expect("deserialize");
        assert_eq!(root.mode, ChainMode::Linear); // default when not specified
    }

    #[test]
    fn retry_backoff_ms() {
        let policy = RetryPolicy {
            max_attempts: 3,
            backoff_base_ms: 1000,
            backoff_multiplier: 2.0,
            max_backoff_ms: Some(10000),
        };
        assert_eq!(policy.backoff_ms(1), 1000);
        assert_eq!(policy.backoff_ms(2), 2000);
        assert_eq!(policy.backoff_ms(3), 4000);
    }

    #[test]
    fn retry_backoff_ms_with_cap() {
        let policy = RetryPolicy {
            max_attempts: 5,
            backoff_base_ms: 2000,
            backoff_multiplier: 3.0,
            max_backoff_ms: Some(30000),
        };
        assert_eq!(policy.backoff_ms(1), 2000);
        assert_eq!(policy.backoff_ms(2), 6000);
        assert_eq!(policy.backoff_ms(3), 18000);
        assert_eq!(policy.backoff_ms(4), 30000); // capped
    }

    #[test]
    fn task_status_is_terminal() {
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Skipped.is_terminal());
        assert!(TaskStatus::Blocked.is_terminal());
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
        assert!(!TaskStatus::RetryScheduled.is_terminal());
    }

    #[test]
    fn denys_unknown_fields() {
        let json = r#"{
            "schema_version": "chain-dag-v1",
            "chain_id": "test",
            "tasks": [{"task_id": "a", "unknown_field": true}]
        }"#;
        let result: Result<ChainDagRoot, _> = serde_json::from_str(json);
        assert!(result.is_err(), "should reject unknown fields");
    }

    #[test]
    fn status_counts() {
        let mut root = ChainDagRoot::new("counts", vec![
            DagTaskEntry::new("a"),
            DagTaskEntry::new("b"),
        ]);
        root.tasks[1].status = TaskStatus::Completed;
        let counts = root.status_counts();
        assert_eq!(counts.get("pending"), Some(&1));
        assert_eq!(counts.get("completed"), Some(&1));
    }

    #[test]
    fn dag_condition_roundtrip() {
        let cond = DagCondition {
            source: "scan".to_string(),
            condition_type: ConditionType::OutputField,
            field: Some("outputs.verification_status".to_string()),
            operator: ConditionOperator::Eq,
            value: serde_json::json!("passed"),
        };
        let json = serde_json::to_string_pretty(&cond).expect("serialize");
        let deserialized: DagCondition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.source, "scan");
        assert_eq!(deserialized.field.as_deref(), Some("outputs.verification_status"));
    }

    #[test]
    fn task_entry_with_caused_skips() {
        let entry = DagTaskEntry {
            task_id: "root".to_string(),
            caused_skips: vec!["child-a".to_string(), "child-b".to_string()],
            ..DagTaskEntry::new("root")
        };
        assert_eq!(entry.caused_skips.len(), 2);
    }
}
