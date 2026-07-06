//! Chain-level aggregation output — `CHAIN_OUTPUT.json` schema `chain-output-v1`.
//!
//! When all tasks in a chain complete, an aggregate record is produced that
//! rolls up individual task outputs into a single summary for downstream
//! consumption (e.g. closeout gate, review, or supervisor).

use core_errors::FrameworkError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Schema version for CHAIN_OUTPUT.json files.
pub const CHAIN_OUTPUT_SCHEMA_VERSION: &str = "chain-output-v1";

/// Status of a single task within the chain aggregate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChainTaskEntry {
    /// Task identifier.
    pub task_id: String,
    /// Task execution status at chain completion time.
    #[serde(default)]
    pub status: String,
    /// One-line summary from the task's closeout.
    #[serde(default)]
    pub summary: String,
    /// Verification status from the task's closeout.
    #[serde(default)]
    pub verification_status: String,
}

/// Rolled-up quality-gate result for the entire chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChainQualityGate {
    /// Whether the chain's quality gate passed.
    #[serde(default)]
    pub passed: bool,
    /// Overall gate rating.
    #[serde(default)]
    pub overall_gate: String,
    /// Blockers that prevented gate pass.
    #[serde(default)]
    pub blockers: Vec<String>,
}

/// Aggregated evidence across all tasks in a chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ChainAggregatedEvidence {
    /// Total commands run across all tasks.
    #[serde(default)]
    pub total_commands_run: u64,
    /// Total files changed across all tasks.
    #[serde(default)]
    pub total_changed_files: u64,
    /// Overall verification outcome ("passed" / "failed" / "partial").
    #[serde(default)]
    pub overall_verification: String,
}

/// Chain-level aggregated output — `CHAIN_OUTPUT.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChainOutput {
    /// Schema version — always "chain-output-v1".
    pub schema_version: String,
    /// Chain identifier (usually the first task_id or an explicit chain_id).
    pub chain_id: String,
    /// Overall chain execution status.
    #[serde(default)]
    pub overall_status: String,
    /// Total number of tasks in the chain.
    pub task_count: u64,
    /// Number of tasks with status "completed".
    #[serde(default)]
    pub completed_count: u64,
    /// Number of tasks with status "failed".
    #[serde(default)]
    pub failed_count: u64,
    /// Per-task entries.
    #[serde(default)]
    pub tasks: Vec<ChainTaskEntry>,
    /// Aggregated evidence rollup.
    #[serde(default)]
    pub aggregated_evidence: ChainAggregatedEvidence,
    /// Quality gate result (if gates were enabled).
    #[serde(default)]
    pub quality_gate_result: Option<ChainQualityGate>,
}

impl ChainOutput {
    /// Create a new chain output for the given chain_id with the given task count.
    pub fn new(chain_id: impl Into<String>, task_count: u64) -> Self {
        Self {
            schema_version: CHAIN_OUTPUT_SCHEMA_VERSION.to_string(),
            chain_id: chain_id.into(),
            overall_status: String::new(),
            task_count,
            completed_count: 0,
            failed_count: 0,
            tasks: Vec::new(),
            aggregated_evidence: ChainAggregatedEvidence {
                total_commands_run: 0,
                total_changed_files: 0,
                overall_verification: String::new(),
            },
            quality_gate_result: None,
        }
    }
}

// ── I/O functions ──────────────────────────────────────────────────────────

/// Path to CHAIN_OUTPUT.json in the artifacts/current directory.
pub fn chain_output_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    repo_root
        .join("artifacts/current")
        .join("CHAIN_OUTPUT.json")
}

/// Write a ChainOutput to disk atomically.
pub fn write_chain_output(
    repo_root: &std::path::Path,
    output: &ChainOutput,
) -> Result<(), FrameworkError> {
    let path = chain_output_path(repo_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = serde_json::to_value(output)?;
    crate::utils::atomic_write::write_atomic_json(&path, &value)?;
    Ok(())
}

/// Read CHAIN_OUTPUT.json, returning None if it doesn't exist.
pub fn read_chain_output(
    repo_root: &std::path::Path,
) -> Result<Option<ChainOutput>, FrameworkError> {
    let path = chain_output_path(repo_root);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    let output: ChainOutput = serde_json::from_str(&raw)?;
    Ok(Some(output))
}

/// Build a `ChainOutput` aggregate from the current TASK_CHAIN.json and its
/// individual TASK_OUTPUT.json files.  Returns the aggregate without writing it.
pub fn build_chain_aggregate(
    repo_root: &std::path::Path,
) -> Result<ChainOutput, FrameworkError> {
    let chain_path = repo_root.join("artifacts/current/TASK_CHAIN.json");
    if !chain_path.is_file() {
        return Err(FrameworkError::not_found(
            "TASK_CHAIN.json not found".to_string(),
        ));
    }
    let raw = std::fs::read_to_string(&chain_path)?;
    let chain_val: Value = serde_json::from_str(&raw)?;

    let tasks_arr = chain_val
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| FrameworkError::config("TASK_CHAIN.json: missing 'tasks' array"))?;

    let chain_id = chain_val
        .get("chain_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown-chain")
        .to_string();

    let mut output = ChainOutput::new(&chain_id, tasks_arr.len() as u64);

    let mut total_commands: u64 = 0;
    let mut total_changed: u64 = 0;
    let mut any_failed = false;
    let mut any_partial = false;
    let mut any_pass = false;

    for task_val in tasks_arr {
        let tid = task_val
            .get("task_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let status = task_val
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let mut summary = String::new();
        let mut verification_status = String::new();

        if !tid.is_empty() {
            if let Ok(Some(task_out)) = crate::task_output::read_task_output(repo_root, tid) {
                summary = task_out.outputs.summary.clone();
                verification_status = task_out.outputs.verification_status.clone();
                total_commands += task_out.outputs.evidence_summary.total_commands;
                total_changed += task_out.outputs.evidence_summary.total_changed_files;
            }
        }

        if status == "completed" || status == "done" {
            output.completed_count += 1;
        }
        if status == "failed" {
            output.failed_count += 1;
            any_failed = true;
        }

        match verification_status.as_str() {
            "passed" => any_pass = true,
            "failed" => any_failed = true,
            "partial" => any_partial = true,
            _ => {}
        }

        output.tasks.push(ChainTaskEntry {
            task_id: tid.to_string(),
            status: status.clone(),
            summary,
            verification_status,
        });
    }

    output.aggregated_evidence = ChainAggregatedEvidence {
        total_commands_run: total_commands,
        total_changed_files: total_changed,
        overall_verification: if any_failed {
            "failed".to_string()
        } else if any_partial {
            "partial".to_string()
        } else if any_pass {
            "passed".to_string()
        } else {
            "unknown".to_string()
        },
    };

    output.overall_status = if any_failed {
        "fail".to_string()
    } else if any_partial {
        "partial".to_string()
    } else {
        "pass".to_string()
    };

    Ok(output)
}

/// Build and write the chain aggregate in one call.
pub fn build_and_write_chain_aggregate(
    repo_root: &std::path::Path,
) -> Result<ChainOutput, FrameworkError> {
    let output = build_chain_aggregate(repo_root)?;
    write_chain_output(repo_root, &output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn chain_output_roundtrip() {
        let output = ChainOutput {
            schema_version: CHAIN_OUTPUT_SCHEMA_VERSION.to_string(),
            chain_id: "fix-bugs".to_string(),
            overall_status: "pass".to_string(),
            task_count: 3,
            completed_count: 3,
            failed_count: 0,
            tasks: vec![
                ChainTaskEntry {
                    task_id: "scan".to_string(),
                    status: "completed".to_string(),
                    summary: "scanned codebase".to_string(),
                    verification_status: "passed".to_string(),
                },
                ChainTaskEntry {
                    task_id: "fix".to_string(),
                    status: "completed".to_string(),
                    summary: "fixed issues".to_string(),
                    verification_status: "passed".to_string(),
                },
            ],
            aggregated_evidence: ChainAggregatedEvidence {
                total_commands_run: 8,
                total_changed_files: 4,
                overall_verification: "passed".to_string(),
            },
            quality_gate_result: Some(ChainQualityGate {
                passed: true,
                overall_gate: "pass".to_string(),
                blockers: Vec::new(),
            }),
        };

        let json = serde_json::to_string_pretty(&output).expect("serialize");
        let deserialized: ChainOutput =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(output, deserialized);
    }

    #[test]
    fn chain_output_deny_unknown_fields() {
        let json = r#"{
            "schema_version": "chain-output-v1",
            "chain_id": "c1",
            "task_count": 1,
            "unknown": "nope"
        }"#;
        let result: Result<ChainOutput, _> = serde_json::from_str(json);
        assert!(result.is_err(), "should reject unknown fields");
    }

    #[test]
    fn chain_output_new() {
        let co = ChainOutput::new("my-chain", 5);
        assert_eq!(co.chain_id, "my-chain");
        assert_eq!(co.task_count, 5);
        assert!(co.tasks.is_empty());
    }
}
