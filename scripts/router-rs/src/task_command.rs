//! Named **task ledger** mutations (phase 2.5): single dispatch surface over autopilot goal, RFV loop,
//! session artifact batch write, and hook evidence append — all already serialized by `task_write_lock`.
//!
//! See `docs/task_state_unified_resolve.md`.

use serde_json::Value;

pub const TASK_LEDGER_COMMAND_ENVELOPE_SCHEMA: &str = "router-rs-task-ledger-command-envelope-v1";

#[derive(Debug, Clone)]
pub enum TaskLedgerCommand {
    AutopilotGoal(Value),
    RfvLoop(Value),
    SessionArtifacts(Value),
    HookEvidenceAppend(Value),
}

/// Parse `{ schema_version?, kind, payload }` → [`TaskLedgerCommand`].
pub fn parse_task_ledger_command_envelope(envelope: &Value) -> Result<TaskLedgerCommand, String> {
    let schema = envelope
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if !schema.is_empty() && schema != TASK_LEDGER_COMMAND_ENVELOPE_SCHEMA {
        return Err(format!(
            "task_ledger_command: expected schema_version {:?} or omit; got {:?}",
            TASK_LEDGER_COMMAND_ENVELOPE_SCHEMA, schema
        ));
    }
    let kind = envelope
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "task_ledger_command: missing kind".to_string())?;
    let payload = envelope
        .get("payload")
        .cloned()
        .ok_or_else(|| "task_ledger_command: missing payload".to_string())?;

    match kind.to_ascii_lowercase().as_str() {
        "autopilot_goal" => Ok(TaskLedgerCommand::AutopilotGoal(payload)),
        "rfv_loop" => Ok(TaskLedgerCommand::RfvLoop(payload)),
        "session_artifacts" => Ok(TaskLedgerCommand::SessionArtifacts(payload)),
        "hook_evidence_append" => Ok(TaskLedgerCommand::HookEvidenceAppend(payload)),
        _ => Err(format!("task_ledger_command: unknown kind {kind:?}")),
    }
}

/// Dispatch without taking an extra lock (each handler already uses `task_write_lock` where needed).
pub fn dispatch_task_ledger_command(cmd: TaskLedgerCommand) -> Result<Value, String> {
    match cmd {
        TaskLedgerCommand::AutopilotGoal(p) => crate::autopilot_goal::framework_autopilot_goal(p),
        TaskLedgerCommand::RfvLoop(p) => crate::rfv_loop::framework_rfv_loop(p),
        TaskLedgerCommand::SessionArtifacts(p) => {
            crate::framework_runtime::write_framework_session_artifacts(p)
        }
        TaskLedgerCommand::HookEvidenceAppend(p) => {
            crate::framework_runtime::framework_hook_evidence_append(p)
        }
    }
}

pub fn dispatch_task_ledger_command_envelope(envelope: Value) -> Result<Value, String> {
    let cmd = parse_task_ledger_command_envelope(&envelope)?;
    dispatch_task_ledger_command(cmd)
}

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
            "kind": "autopilot_goal",
            "payload": {}
        });
        assert!(parse_task_ledger_command_envelope(&e).is_err());
    }

    #[test]
    fn dispatch_autopilot_goal_status_roundtrip() {
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
            "kind": "autopilot_goal",
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
