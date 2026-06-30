//! Task output MCP tools: task_output_write, task_output_read, task_output_init,
//! task_output_pull.
//!
//! These tools manage the structured TASK_OUTPUT.json file for each task,
//! supporting the task output pipeline across chains, goal-engine loops, and
//! swarm teams.

use super::*;
use core_errors::FrameworkError;
use core_state::chain_output::{self as chain_output_mod, ChainOutput};
use core_state::task_output::{
    self as task_output_mod, TaskOutput, OutputData, ConsumedInput,
};
use serde_json::{Value, json};
use std::path::Path;

const PRODUCER_TASK_ENGINE: &str = "task-engine";

/// Write a structured TASK_OUTPUT.json for a given task.
///
/// Accepts a closeout record inline and derives the `outputs` from it.
pub(crate) fn tool_task_output_write(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "task_output_write: missing required argument 'task_id'".to_string(),
            )
        })?
        .trim();
    if task_id.is_empty() {
        return Err(FrameworkError::validation(
            "task_output_write: task_id must not be empty".to_string(),
        ));
    }

    let status_str = arguments
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let verification_status = arguments
        .get("verification_status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let changed_files: Vec<String> = arguments
        .get("changed_files")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let commands_run: Vec<String> = arguments
        .get("commands_run")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut output = TaskOutput::new(task_id);
    output.producer = PRODUCER_TASK_ENGINE.to_string();
    output.status = if status_str == "failed" {
        core_state::task_output::TaskOutputStatus::Failed
    } else {
        core_state::task_output::TaskOutputStatus::Completed
    };

    output.outputs = OutputData {
        changed_files: changed_files.clone(),
        commands_run: commands_run.clone(),
        verification_status: verification_status.clone(),
        summary: summary.clone(),
        evidence_summary: core_state::task_output::EvidenceSummary {
            total_commands: commands_run.len() as u64,
            successful_commands: 0, // caller can override via closeout
            total_changed_files: changed_files.len() as u64,
        },
    };

    // If a `closeout` sub-object is provided, embed it.
    if let Some(closeout_val) = arguments.get("closeout") {
        let closeout_v = closeout_val.clone();
        if let Ok(record) =
            serde_json::from_value::<core_state::closeout_validation::CloseoutRecord>(
                closeout_v,
            )
        {
            output.closeout = Some(record);
            // Re-derive evidence summary from closeout command records if available.
            if let Some(ref rec) = output.closeout {
                output.outputs = OutputData::from_closeout_record(rec);
            }
        }
    }

    task_output_mod::write_task_output(repo_root, &output)?;

    Ok(serde_json::to_string(&json!({
        "ok": true,
        "task_id": task_id,
        "status": status_str,
        "path": task_output_mod::task_output_path_for_task(repo_root, task_id)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    }))?)
}

/// Read the current TASK_OUTPUT.json for a given task.
pub(crate) fn tool_task_output_read(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "task_output_read: missing required argument 'task_id'".to_string(),
            )
        })?
        .trim();
    if task_id.is_empty() {
        return Err(FrameworkError::validation(
            "task_output_read: task_id must not be empty".to_string(),
        ));
    }

    let output = task_output_mod::read_task_output(repo_root, task_id)?;
    match output {
        Some(o) => serde_json::to_string(&o)
            .map_err(|e| FrameworkError::from(e.to_string())),
        None => Ok(serde_json::to_string(&json!({
            "ok": false,
            "task_id": task_id,
            "error": "TASK_OUTPUT.json not found"
        }))?),
    }
}

/// Initialize an empty TASK_OUTPUT.json for a newly created task.
/// Called automatically by task_create, but also available as a standalone tool.
pub(crate) fn tool_task_output_init(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "task_output_init: missing required argument 'task_id'".to_string(),
            )
        })?
        .trim();
    if task_id.is_empty() {
        return Err(FrameworkError::validation(
            "task_output_init: task_id must not be empty".to_string(),
        ));
    }

    let title = arguments
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or(task_id);
    let producer = arguments
        .get("producer")
        .and_then(Value::as_str)
        .unwrap_or(PRODUCER_TASK_ENGINE);

    let mut output = TaskOutput::with_title(task_id, title);
    output.producer = producer.to_string();

    task_output_mod::write_task_output(repo_root, &output)?;

    Ok(serde_json::to_string(&json!({
        "ok": true,
        "task_id": task_id,
        "status": "initialized",
        "path": task_output_mod::task_output_path_for_task(repo_root, task_id)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    }))?)
}

/// Pull output from a previous task and add it as consumed_inputs to the current task.
/// This is the explicit "task→task output pipeline" tool.
pub(crate) fn tool_task_output_pull(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let current_task_id = arguments
        .get("current_task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "task_output_pull: missing required argument 'current_task_id'".to_string(),
            )
        })?
        .trim();
    if current_task_id.is_empty() {
        return Err(FrameworkError::validation(
            "task_output_pull: current_task_id must not be empty".to_string(),
        ));
    }

    let source_task_id = arguments
        .get("source_task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "task_output_pull: missing required argument 'source_task_id'".to_string(),
            )
        })?
        .trim();
    if source_task_id.is_empty() {
        return Err(FrameworkError::validation(
            "task_output_pull: source_task_id must not be empty".to_string(),
        ));
    }

    let fields: Vec<String> = arguments
        .get("fields")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Read the source task's output
    let source_output = task_output_mod::read_task_output(repo_root, source_task_id)?
        .ok_or_else(|| {
            FrameworkError::not_found(format!(
                "task_output_pull: source task '{source_task_id}' has no TASK_OUTPUT.json"
            ))
        })?;

    // Build the consumed_input entry
    let consumed = ConsumedInput {
        source_task_id: source_task_id.to_string(),
        source_output_path: task_output_mod::task_output_path_for_task(repo_root, source_task_id)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        consumed_fields: if fields.is_empty() {
            // Default: consume all available output fields
            vec![
                "changed_files".to_string(),
                "commands_run".to_string(),
                "verification_status".to_string(),
                "summary".to_string(),
            ]
        } else {
            fields
        },
        consumed_at: Some(framework_core::time::now_iso()),
    };

    // Read current task output, add consumption, write back
    let mut current_output = task_output_mod::read_task_output(repo_root, current_task_id)?
        .unwrap_or_else(|| TaskOutput::new(current_task_id));
    current_output.consumed_inputs.push(consumed);
    task_output_mod::write_task_output(repo_root, &current_output)?;

    // Build the response with the consumed data from the source
    Ok(serde_json::to_string(&json!({
        "ok": true,
        "current_task_id": current_task_id,
        "source_task_id": source_task_id,
        "consumed_data": {
            "changed_files": source_output.outputs.changed_files,
            "commands_run": source_output.outputs.commands_run,
            "verification_status": source_output.outputs.verification_status,
            "summary": source_output.outputs.summary,
        },
    }))?)
}

/// Build and write the CHAIN_OUTPUT.json aggregate from TASK_CHAIN.json
/// and individual TASK_OUTPUT.json files.
pub(crate) fn tool_chain_aggregate(
    _arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let output = chain_output_mod::build_and_write_chain_aggregate(repo_root)?;
    Ok(serde_json::to_string(&serde_json::json!({
        "ok": true,
        "chain_id": &output.chain_id,
        "overall_status": &output.overall_status,
        "task_count": output.task_count,
        "completed_count": output.completed_count,
        "failed_count": output.failed_count,
        "overall_verification": &output.aggregated_evidence.overall_verification,
    }))?)
}

/// Validate a TASK_OUTPUT.json for field completeness and schema consistency.
pub(crate) fn tool_task_output_validate(
    arguments: &Value,
    repo_root: &Path,
) -> std::result::Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "task_output_validate: missing required argument 'task_id'".to_string(),
            )
        })?
        .trim();
    if task_id.is_empty() {
        return Err(FrameworkError::validation(
            "task_output_validate: task_id must not be empty".to_string(),
        ));
    }

    let mut issues: Vec<String> = Vec::new();

    match task_output_mod::read_task_output(repo_root, task_id) {
        Ok(Some(output)) => {
            if output.schema_version != core_state::task_output::TASK_OUTPUT_SCHEMA_VERSION {
                issues.push(format!(
                    "schema_version mismatch: expected {}, got {}",
                    core_state::task_output::TASK_OUTPUT_SCHEMA_VERSION,
                    output.schema_version
                ));
            }
            if output.task_id.is_empty() {
                issues.push("task_id is empty".to_string());
            }
            if output.task_id != task_id {
                issues.push(format!(
                    "task_id mismatch: file has '{}', expected '{task_id}'",
                    output.task_id
                ));
            }
            match output.status {
                core_state::task_output::TaskOutputStatus::Completed
                | core_state::task_output::TaskOutputStatus::Failed => {
                    // Completed/failed tasks should have non-empty outputs
                    if output.outputs.summary.is_empty() {
                        issues.push("completed/failed task has empty summary".to_string());
                    }
                    if output.outputs.verification_status.is_empty() {
                        issues.push(
                            "completed/failed task has empty verification_status".to_string(),
                        );
                    }
                    if output.closeout.is_none() {
                        issues.push(
                            "completed/failed task has no embedded closeout record".to_string(),
                        );
                    }
                }
                core_state::task_output::TaskOutputStatus::Running => {
                    // Running tasks may have minimal outputs
                }
            }
        }
        Ok(None) => {
            issues.push(format!("TASK_OUTPUT.json not found for task '{task_id}'"));
        }
        Err(e) => {
            issues.push(format!("read error: {e}"));
        }
    }

    let is_valid = issues.is_empty();
    Ok(serde_json::to_string(&serde_json::json!({
        "ok": is_valid,
        "task_id": task_id,
        "valid": is_valid,
        "issues": issues,
    }))?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;
    use std::fs;

    fn setup_repo(test_name: &str) -> std::path::PathBuf {
        let repo = std::env::temp_dir().join(format!("router-rs-output-tools-{test_name}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/test-task")).expect("mkdir");
        fs::create_dir_all(repo.join("artifacts/current/source-task")).expect("mkdir");
        repo
    }

    #[test]
    fn write_and_read_roundtrip() {
        let repo = setup_repo("write_read");
        let args = json!({
            "task_id": "test-task",
            "status": "completed",
            "summary": "fixed bugs",
            "verification_status": "passed",
            "changed_files": ["src/main.rs"],
            "commands_run": ["cargo test"],
        });
        let resp = tool_task_output_write(&args, &repo).expect("write");
        let v: Value = serde_json::from_str(&resp).expect("parse");
        assert_eq!(v["ok"], json!(true));

        // Read it back
        let read_resp = tool_task_output_read(&json!({"task_id": "test-task"}), &repo)
            .expect("read");
        let read_v: Value = serde_json::from_str(&read_resp).expect("parse read");
        assert_eq!(read_v["task_id"], "test-task");
        assert_eq!(read_v["outputs"]["verification_status"], "passed");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn init_creates_empty_output() {
        let repo = setup_repo("init");
        let args = json!({
            "task_id": "test-task",
            "title": "My Task",
            "producer": "task-engine",
        });
        let resp = tool_task_output_init(&args, &repo).expect("init");
        let v: Value = serde_json::from_str(&resp).expect("parse");
        assert_eq!(v["ok"], json!(true));

        let read_v = tool_task_output_read(&json!({"task_id": "test-task"}), &repo)
            .expect("read");
        let output: Value = serde_json::from_str(&read_v).expect("parse");
        assert_eq!(output["status"], "running");
        assert_eq!(output["title"], "My Task");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn pull_consumes_from_source() {
        let repo = setup_repo("pull");
        repo.join("artifacts/current/source-task").exists();
        let _ = fs::create_dir_all(repo.join("artifacts/current/source-task"));

        // Write source output
        tool_task_output_write(
            &json!({
                "task_id": "source-task",
                "status": "completed",
                "summary": "scanned",
                "verification_status": "passed",
                "changed_files": ["src/lib.rs"],
                "commands_run": ["cargo check"],
            }),
            &repo,
        )
        .expect("write source");

        // Init current task
        tool_task_output_init(
            &json!({"task_id": "test-task", "title": "My Task"}),
            &repo,
        )
        .expect("init current");

        // Pull from source
        let pull_resp = tool_task_output_pull(
            &json!({
                "current_task_id": "test-task",
                "source_task_id": "source-task",
                "fields": ["changed_files", "summary"],
            }),
            &repo,
        )
        .expect("pull");
        let pull_v: Value = serde_json::from_str(&pull_resp).expect("parse");
        assert_eq!(pull_v["ok"], json!(true));
        assert_eq!(pull_v["source_task_id"], "source-task");

        // Verify consumed_inputs on current
        let read_v = tool_task_output_read(&json!({"task_id": "test-task"}), &repo)
            .expect("read");
        let output: Value = serde_json::from_str(&read_v).expect("parse");
        let inputs = output["consumed_inputs"].as_array().expect("consumed inputs");
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0]["source_task_id"], "source-task");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn write_closeout_record_sync() {
        let repo = setup_repo("closeout_sync");
        let args = json!({
            "task_id": "test-task",
            "status": "completed",
            "closeout": {
                "schema_version": "closeout-record-v1",
                "task_id": "test-task",
                "summary": "fixed deprecation warnings",
                "verification_status": "passed",
                "changed_files": ["src/lib.rs"],
                "commands_run": [
                    {"command": "cargo test", "exit_code": 0},
                    {"command": "cargo clippy", "exit_code": 1}
                ],
                "blockers": [],
                "risks": []
            }
        });
        let resp = tool_task_output_write(&args, &repo).expect("write");
        let v: Value = serde_json::from_str(&resp).expect("parse");
        assert_eq!(v["ok"], json!(true));

        // Verify outputs are derived from closeout
        let read_v = tool_task_output_read(&json!({"task_id": "test-task"}), &repo)
            .expect("read");
        let output: Value = serde_json::from_str(&read_v).expect("parse");
        assert_eq!(output["closeout"]["verification_status"], "passed");
        assert_eq!(
            output["outputs"]["evidence_summary"]["total_commands"],
            2
        );
        assert_eq!(
            output["outputs"]["evidence_summary"]["successful_commands"],
            1
        );

        let _ = fs::remove_dir_all(&repo);
    }
}
