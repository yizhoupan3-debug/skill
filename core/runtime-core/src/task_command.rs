//! Named **task ledger** mutations (phase 2.5): single dispatch surface over goal drive, RFV loop,
//! session artifact batch write, and hook evidence append.
//!
//! Writers serialize through **repo-root `flock`** on `artifacts/current/.router-rs.task-ledger.lock`
//! (see [`crate::task_write_lock`]), not a cross-process `std::sync::Mutex`.
//!
//! See `core/core-state/src/task_state.rs` for the unified resolve model.

use core_errors::FrameworkError;
use core_state::transition_validation::{TaskTransition, validate_transition};
use quality_gate;
use serde_json::Value;

pub const TASK_LEDGER_COMMAND_ENVELOPE_SCHEMA: &str = "router-rs-task-ledger-command-envelope-v1";

#[derive(Debug, Clone)]
pub enum TaskLedgerCommand {
    GoalDrive(Value),
    QualityGate(Value),
    SessionArtifacts(Value),
    HookEvidenceAppend(Value),
}

/// Parse `{ schema_version?, kind, payload }` → [`TaskLedgerCommand`].
pub fn parse_task_ledger_command_envelope(
    envelope: &Value,
) -> Result<TaskLedgerCommand, FrameworkError> {
    let schema = envelope
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if !schema.is_empty() && schema != TASK_LEDGER_COMMAND_ENVELOPE_SCHEMA {
        return Err(FrameworkError::validation(format!(
            "task_ledger_command: expected schema_version {:?} or omit; got {:?}",
            TASK_LEDGER_COMMAND_ENVELOPE_SCHEMA, schema
        )));
    }
    let kind = envelope
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| FrameworkError::validation("task_ledger_command: missing kind"))?;
    let payload = envelope
        .get("payload")
        .cloned()
        .ok_or_else(|| FrameworkError::validation("task_ledger_command: missing payload"))?;

    match kind.to_ascii_lowercase().as_str() {
        "goal_drive" => Ok(TaskLedgerCommand::GoalDrive(payload)),
        "rfv_loop" | "quality_gate" => Ok(TaskLedgerCommand::QualityGate(payload)),
        "session_artifacts" => Ok(TaskLedgerCommand::SessionArtifacts(payload)),
        "hook_evidence_append" => Ok(TaskLedgerCommand::HookEvidenceAppend(payload)),
        _ => Err(FrameworkError::validation(format!(
            "task_ledger_command: unknown kind {kind:?}"
        ))),
    }
}

/// Dispatch without taking an extra outer lock (`apply_task_ledger_mutation` is invoked inside handlers where needed).
pub fn dispatch_task_ledger_command(cmd: TaskLedgerCommand) -> Result<Value, FrameworkError> {
    match cmd {
        TaskLedgerCommand::GoalDrive(p) => {
            runtime_infra::kernel_utils::framework_goal_drive(p)
        }
        TaskLedgerCommand::QualityGate(p) => {
            let repo_root =
                std::path::Path::new(p.get("repo_root").and_then(|v| v.as_str()).unwrap_or("."));
            let task_id = p.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
            let goal = p.get("goal").and_then(|v| v.as_str()).unwrap_or("");
            let round = p.get("round").and_then(|v| v.as_u64()).unwrap_or(1);
            let scene = p
                .get("scene")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(quality_gate::scene::GENERAL);
            let sub_scene = p
                .get("sub_scene")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());

            // Stage 1: transition validation as blocking gate (anti-fraud evidence check)
            if !task_id.is_empty() {
                let transition_v =
                    validate_transition(repo_root, task_id, TaskTransition::Complete);
                if !transition_v.passed {
                    return Ok(serde_json::to_value(&quality_gate::types::GateVerdict {
                        passed: false,
                        checkers_ran: 0,
                        blockers: vec![quality_gate::types::Finding {
                            id: "transition_validation_blocked".to_string(),
                            severity: quality_gate::types::Severity::P0,
                            description: transition_v.reason.clone(),
                            location: None,
                            suggestion: Some(
                                "record evidence artifacts (exit_code=0 or success=true) before completing".to_string(),
                            ),
                        }],
                        advisories: vec![],
                        reason: Some(format!("transition validation blocked: {}", transition_v.reason)),
                    })?);
                }
            }

            let output_data = p.get("output_data").cloned();

            // Stage 2: QG Route evaluation
            let verdict = crate::qg_entry::trigger(
                repo_root,
                task_id,
                scene,
                goal,
                sub_scene,
                round,
                None,
                output_data,
            );
            Ok(serde_json::to_value(&verdict)?)
        }
        TaskLedgerCommand::SessionArtifacts(p) => {
            framework_extra::session_artifacts::write_framework_session_artifacts(p)
        }
        TaskLedgerCommand::HookEvidenceAppend(p) => {
            framework_extra::evidence::framework_hook_evidence_append(p)
        }
    }
}

pub fn dispatch_task_ledger_command_envelope(envelope: Value) -> Result<Value, FrameworkError> {
    let cmd = parse_task_ledger_command_envelope(&envelope)?;
    dispatch_task_ledger_command(cmd)
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_repo(label: &str) -> std::path::PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("router-rs-task-cmd-{label}-{n}"))
    }

    #[test]
    fn parse_rejects_wrong_schema() {
        let e = json!({
            "schema_version": "wrong",
            "kind": "goal_drive",
            "payload": {}
        });
        assert!(parse_task_ledger_command_envelope(&e).is_err());
    }

    #[test]
    fn dispatch_goal_drive_status_roundtrip() {
        let repo = tmp_repo("ag");
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/ag")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"ag"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        let out = dispatch_task_ledger_command_envelope(json!({
            "schema_version": TASK_LEDGER_COMMAND_ENVELOPE_SCHEMA,
            "kind": "goal_drive",
            "payload": {
                "repo_root": rr,
                "operation": "status",
                "task_id": "ag"
            }
        }))
        .expect("dispatch");
        assert_eq!(out.get("ok"), Some(&json!(true)));
        assert_eq!(out.get("task_id"), Some(&json!("ag")));
        let _ = fs::remove_dir_all(&repo);
    }
}
