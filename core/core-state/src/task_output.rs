//! Structured task output — `TASK_OUTPUT.json` schema `task-output-v1`.
//!
//! Each task produces a structured output record that captures what was done,
//! what evidence was generated, and what context should flow to downstream
//! consumers (next task in a chain, goal engine loop, swarm supervisor).
//!
//! This file defines the types only — the I/O layer is in [`crate::state_manager`]
//! and MCP tools in `host-projection`.
//!
//! Schema const: TASK_OUTPUT_SCHEMA_VERSION = "task-output-v1"

use core_errors::FrameworkError;
use serde::{Deserialize, Serialize};

/// Schema version for TASK_OUTPUT.json files.
pub const TASK_OUTPUT_SCHEMA_VERSION: &str = "task-output-v1";

/// Status of a single task's execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskOutputStatus {
    /// Task is still in progress.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed during execution.
    Failed,
}

impl Default for TaskOutputStatus {
    fn default() -> Self {
        Self::Running
    }
}

/// A consumed-input record linking the output of a prior task into this task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConsumedInput {
    /// The task_id whose output was consumed.
    pub source_task_id: String,
    /// Path to the source task's TASK_OUTPUT.json (for direct re-read).
    pub source_output_path: String,
    /// Which fields within OutputData were consumed.
    #[serde(default)]
    pub consumed_fields: Vec<String>,
    /// ISO-8601 timestamp of when the consumption was recorded.
    #[serde(default)]
    pub consumed_at: Option<String>,
}

/// Reference to a parent aggregation (chain or loop).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AggregatesRef {
    /// Chain ID if this task is part of a chain.
    #[serde(default)]
    pub parent_chain_id: Option<String>,
    /// Loop ID if this task is part of a loop.
    #[serde(default)]
    pub parent_loop_id: Option<String>,
    /// Index within the parent (0-based).
    pub chain_index: Option<u64>,
}

/// Execution metadata for a task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct OutputMetadata {
    /// ISO-8601 start timestamp.
    #[serde(default)]
    pub started_at: Option<String>,
    /// ISO-8601 end timestamp.
    #[serde(default)]
    pub ended_at: Option<String>,
    /// Duration in milliseconds.
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// Rolled-up evidence counts for the outputs section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSummary {
    /// Total commands run during this task.
    #[serde(default)]
    pub total_commands: u64,
    /// Commands that exited with code 0 or have success=true.
    #[serde(default)]
    pub successful_commands: u64,
    /// Number of files changed (from changed_files list).
    #[serde(default)]
    pub total_changed_files: u64,
}

/// Structured outputs produced by this task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OutputData {
    /// Files changed during this task execution.
    #[serde(default)]
    pub changed_files: Vec<String>,
    /// Commands run (summarized as command strings).
    #[serde(default)]
    pub commands_run: Vec<String>,
    /// Overall verification status.
    #[serde(default)]
    pub verification_status: String,
    /// One-line summary of what was done.
    #[serde(default)]
    pub summary: String,
    /// Rolled-up evidence counts.
    #[serde(default)]
    pub evidence_summary: EvidenceSummary,
}

impl OutputData {
    /// Build an OutputData from a single closeout record (pre-populated).
    /// This is a convenience for the closeout_record_write → TASK_OUTPUT sync.
    pub fn from_closeout_record(
        record: &crate::closeout_validation::CloseoutRecord,
    ) -> Self {
        Self {
            changed_files: record.changed_files.clone(),
            commands_run: record
                .commands_run
                .iter()
                .map(|c| c.command.clone())
                .collect(),
            verification_status: record.verification_status.clone(),
            summary: record.summary.clone(),
            evidence_summary: EvidenceSummary {
                total_commands: record.commands_run.len() as u64,
                successful_commands: record
                    .commands_run
                    .iter()
                    .filter(|c| c.exit_code == 0)
                    .count() as u64,
                total_changed_files: record.changed_files.len() as u64,
            },
        }
    }
}

impl Default for OutputData {
    fn default() -> Self {
        Self {
            changed_files: Vec::new(),
            commands_run: Vec::new(),
            verification_status: String::new(),
            summary: String::new(),
            evidence_summary: EvidenceSummary::default(),
        }
    }
}

impl Default for EvidenceSummary {
    fn default() -> Self {
        Self {
            total_commands: 0,
            successful_commands: 0,
            total_changed_files: 0,
        }
    }
}

/// Structured task output — `TASK_OUTPUT.json` (schema `task-output-v1`).
///
/// The key design principle: **one file per task** that carries everything
/// a downstream consumer needs, without requiring access to the task's internal
/// artifact files (GOAL_STATE, EVIDENCE_INDEX, closeout record).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TaskOutput {
    /// Schema version — always "task-output-v1".
    pub schema_version: String,
    /// Unique task identifier.
    pub task_id: String,
    /// Optional human-readable title.
    #[serde(default)]
    pub title: Option<String>,
    /// Execution status.
    #[serde(default)]
    pub status: TaskOutputStatus,
    /// Producer label — "task-engine" | "goal-engine" | "swarm" | "custom".
    #[serde(default)]
    pub producer: String,
    /// Embedded closeout record (optional — present when task is completed/failed).
    #[serde(default)]
    pub closeout: Option<crate::closeout_validation::CloseoutRecord>,
    /// Structured output data (always present after completion).
    #[serde(default)]
    pub outputs: OutputData,
    /// Inputs consumed from prior tasks in a chain/loop.
    #[serde(default)]
    pub consumed_inputs: Vec<ConsumedInput>,
    /// Links to parent aggregations.
    #[serde(default)]
    pub aggregates: Option<AggregatesRef>,
    /// Execution metadata.
    #[serde(default)]
    pub metadata: OutputMetadata,
}

impl TaskOutput {
    /// Create a new, empty (running) task output with the given task_id.
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            schema_version: TASK_OUTPUT_SCHEMA_VERSION.to_string(),
            task_id: task_id.into(),
            title: None,
            status: TaskOutputStatus::Running,
            producer: String::new(),
            closeout: None,
            outputs: OutputData::default(),
            consumed_inputs: Vec::new(),
            aggregates: None,
            metadata: OutputMetadata::default(),
        }
    }

    /// Create a new task output with the given task_id and title.
    pub fn with_title(task_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            ..Self::new(task_id)
        }
    }

    /// Mark this task as completed with the given outputs.
    pub fn mark_completed(&mut self, outputs: OutputData) {
        self.status = TaskOutputStatus::Completed;
        self.outputs = outputs;
    }

    /// Mark this task as failed.
    pub fn mark_failed(&mut self) {
        self.status = TaskOutputStatus::Failed;
    }
}

// ── Serde helpers ──────────────────────────────────────────────────────────
// TaskOutputStatus is serialized as a camelCase string via rename_all = "snake_case".

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn task_output_roundtrip() {
        let output = TaskOutput::with_title("task-1", "Scan codebase");
        let json = serde_json::to_string_pretty(&output).expect("serialize");
        let deserialized: TaskOutput =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(output, deserialized);
    }

    #[test]
    fn task_output_completed_roundtrip() {
        let mut output = TaskOutput::new("task-2");
        output.producer = "goal-engine".to_string();
        output.mark_completed(OutputData {
            changed_files: vec!["src/main.rs".to_string()],
            commands_run: vec!["cargo test".to_string()],
            verification_status: "passed".to_string(),
            summary: "fixed deprecation warnings".to_string(),
            evidence_summary: EvidenceSummary {
                total_commands: 1,
                successful_commands: 1,
                total_changed_files: 1,
            },
        });
        output.metadata.started_at =
            Some("2026-06-30T09:00:00Z".to_string());
        output.metadata.ended_at =
            Some("2026-06-30T10:00:00Z".to_string());
        output.metadata.duration_ms = Some(3_600_000);

        let json = serde_json::to_string_pretty(&output).expect("serialize");
        let deserialized: TaskOutput =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(output, deserialized);
        assert_eq!(deserialized.status, TaskOutputStatus::Completed);
        assert_eq!(
            deserialized.outputs.evidence_summary.successful_commands,
            1
        );
    }

    #[test]
    fn task_output_deny_unknown_fields() {
        let json = r#"{
            "schema_version": "task-output-v1",
            "task_id": "t1",
            "unknown_field": "boom"
        }"#;
        let result: Result<TaskOutput, _> = serde_json::from_str(json);
        assert!(result.is_err(), "should reject unknown fields");
    }

    #[test]
    fn task_output_default_fields() {
        let json = r#"{
            "schema_version": "task-output-v1",
            "task_id": "t1"
        }"#;
        let output: TaskOutput =
            serde_json::from_str(json).expect("deserialize with defaults");
        assert_eq!(output.task_id, "t1");
        assert_eq!(output.status, TaskOutputStatus::Running);
        assert!(output.outputs.changed_files.is_empty());
        assert!(output.consumed_inputs.is_empty());
    }

    #[test]
    fn consumed_input_roundtrip() {
        let input = ConsumedInput {
            source_task_id: "prior-task".to_string(),
            source_output_path: "artifacts/current/prior-task/TASK_OUTPUT.json"
                .to_string(),
            consumed_fields: vec!["changed_files".to_string()],
            consumed_at: Some("2026-06-30T10:00:00Z".to_string()),
        };
        let json = serde_json::to_string_pretty(&input).expect("serialize");
        let deserialized: ConsumedInput =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(input, deserialized);
    }

    #[test]
    fn output_data_from_closeout_record() {
        let record = crate::closeout_validation::CloseoutRecord {
            schema_version: "closeout-record-v1".to_string(),
            task_id: "t1".to_string(),
            started_at: None,
            ended_at: None,
            changed_files: vec!["a.rs".to_string(), "b.rs".to_string()],
            commands_run: vec![
                crate::closeout_validation::CloseoutCommand {
                    command: "cargo test".to_string(),
                    exit_code: 0,
                    duration_ms: None,
                    stdout_summary: None,
                    stderr_summary: None,
                },
                crate::closeout_validation::CloseoutCommand {
                    command: "cargo clippy".to_string(),
                    exit_code: 1,
                    duration_ms: None,
                    stdout_summary: None,
                    stderr_summary: None,
                },
            ],
            artifacts_checked: Vec::new(),
            verification_status: "passed".to_string(),
            blockers: Vec::new(),
            risks: Vec::new(),
            summary: "fixed stuff".to_string(),
            notes: None,
        };

        let od = OutputData::from_closeout_record(&record);
        assert_eq!(od.changed_files.len(), 2);
        assert_eq!(od.commands_run.len(), 2);
        assert_eq!(od.evidence_summary.total_commands, 2);
        assert_eq!(od.evidence_summary.successful_commands, 1);
        assert_eq!(od.evidence_summary.total_changed_files, 2);
        assert_eq!(od.verification_status, "passed");
    }
}

// ── I/O functions ──────────────────────────────────────────────────────────

/// build the path `<repo_root>/artifacts/current/<task_id>/TASK_OUTPUT.json`.
pub fn task_output_path_for_task(
    repo_root: &std::path::Path,
    task_id: &str,
) -> Result<std::path::PathBuf, core_errors::FrameworkError> {
    let tid = crate::utils::path_guard::validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts/current")
        .join(tid)
        .join("TASK_OUTPUT.json"))
}

/// Write a `TaskOutput` to disk atomically at the standard path.
/// Uses `write_atomic_json` (fsync + rename) for crash safety.
pub fn write_task_output(
    repo_root: &std::path::Path,
    output: &TaskOutput,
) -> Result<(), FrameworkError> {
    let path = task_output_path_for_task(repo_root, &output.task_id)?;
    let value = serde_json::to_value(output)?;
    crate::utils::atomic_write::write_atomic_json(&path, &value)
        .map_err(|e| FrameworkError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))
}

/// Read a `TaskOutput` from disk, returning `None` if the file doesn't exist.
pub fn read_task_output(
    repo_root: &std::path::Path,
    task_id: &str,
) -> Result<Option<TaskOutput>, FrameworkError> {
    let path = task_output_path_for_task(repo_root, task_id)?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    let output: TaskOutput = serde_json::from_str(&raw)?;
    Ok(Some(output))
}

/// Initialize an empty (running) `TASK_OUTPUT.json` for a newly created task.
/// This is called from `tool_task_create` to ensure every task has a structured
/// output file from the start.
pub fn init_task_output(
    repo_root: &std::path::Path,
    task_id: &str,
) -> Result<(), FrameworkError> {
    let output = TaskOutput::new(task_id);
    write_task_output(repo_root, &output)
}
