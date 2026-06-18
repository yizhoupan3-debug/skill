use crate::types::{
    LoopAction, LoopActionRecord, LoopCloseoutAggregate, AggregateActionEntry,
    LoopError,
};
use crate::state::closeout_path;
use std::path::Path;
use std::fs;

#[derive(Debug, Clone)]
/// Response from a closeout verification, indicating whether closeout is allowed
/// and listing any violations found.
pub struct CloseoutVerificationResponse {
    pub closeout_allowed: bool,
    pub violations: Vec<String>,
}

/// Verify a closeout JSON record for required fields (task_id, summary, verification_status,
/// changed_files) and check for command failures or high-severity blockers.
pub fn verify_closeout_value(record: &serde_json::Value) -> CloseoutVerificationResponse {
    let mut violations = Vec::new();

    let task_id = record.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
    if task_id.trim().is_empty() {
        violations.push("task_id_missing".to_string());
    }

    let summary = record.get("summary").and_then(|v| v.as_str()).unwrap_or("");
    if summary.trim().is_empty() {
        violations.push("summary_missing".to_string());
    }

    let verification_status = record.get("verification_status")
        .and_then(|v| v.as_str()).unwrap_or("");
    if verification_status == "not_run" {
        violations.push("verification_status_not_run".to_string());
    }

    let changed_files = record.get("changed_files")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if changed_files == 0 {
        let has_risks = record.get("risks")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        let has_blockers = record.get("blockers")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if !has_risks && !has_blockers {
            violations.push("claimed_done_without_evidence".to_string());
        }
    }

    if let Some(commands) = record.get("commands_run").and_then(|v| v.as_array()) {
        for cmd in commands {
            let exit_code = cmd.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
            if exit_code != 0 {
                let command = cmd.get("command").and_then(|v| v.as_str()).unwrap_or("?");
                violations.push(format!("command_failed: {} (exit={})", command, exit_code));
            }
        }
    }

    if let Some(blockers) = record.get("blockers").and_then(|v| v.as_array()) {
        for b in blockers {
            let text = b.as_str().unwrap_or("");
            let lower = text.to_ascii_lowercase();
            if lower.contains("high") || lower.contains("critical") || lower.contains("block") {
                violations.push(format!("high_severity_blocker: {}", text));
            }
        }
    }

    CloseoutVerificationResponse {
        closeout_allowed: violations.is_empty(),
        violations,
    }
}

/// Verify that an evidence index exists and contains at least one artifact for the given task.
pub fn verify_evidence_index(repo_root: &Path, task_id: &str) -> bool {
    let path = repo_root
        .join("artifacts")
        .join("current")
        .join(task_id)
        .join("EVIDENCE_INDEX.json");
    if !path.is_file() {
        return false;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return false,
    };
    val.get("artifacts")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

/// Verify a closeout record against both structural rules (via `verify_closeout_value`)
/// and evidence index presence.
pub fn verify_closeout_with_evidence(
    record: &serde_json::Value,
    repo_root: &Path,
    task_id: &str,
) -> CloseoutVerificationResponse {
    let mut response = verify_closeout_value(record);
    if !verify_evidence_index(repo_root, task_id) {
        response.violations.push("evidence_index_missing_or_empty".to_string());
        response.closeout_allowed = false;
    }
    response
}

/// Read a persisted `LoopActionRecord` from the closeout directory.
/// Returns `Ok(None)` when the file does not exist.
pub fn read_action_record(
    repo_root: &Path,
    loop_id: &str,
    run_id: &str,
    action_id: &str,
) -> Result<Option<LoopActionRecord>, LoopError> {
    let path = closeout_path(repo_root, loop_id, run_id, action_id);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| LoopError::Io(format!("read closeout {}: {e}", path.display())))?;
    let record: LoopActionRecord = serde_json::from_str(&raw)
        .map_err(|e| LoopError::Serde(format!("parse closeout {}: {e}", path.display())))?;
    Ok(Some(record))
}

/// Build a `LoopCloseoutAggregate` from a list of actions and their individual results (committed, skipped, failed, interrupted).
pub fn build_aggregate(
    run_id: &str,
    loop_id: &str,
    actions: &[LoopAction],
    results: Vec<(String, AggregateActionResult)>,
) -> LoopCloseoutAggregate {
    let mut aggregate = LoopCloseoutAggregate {
        schema_version: "loop-closeout-aggregate-v1".to_string(),
        run_id: run_id.to_string(),
        loop_id: loop_id.to_string(),
        overall_status: "pass".to_string(),
        actions: Vec::new(),
        escalated: false,
        partial: false,
    };

    let mut any_fail = false;
    let mut any_partial = false;

    for action in actions {
        let result = results.iter().find(|(id, _)| id == &action.action_id);
        let entry = match result {
            Some((_, AggregateActionResult::Skipped)) => AggregateActionEntry {
                action_id: action.action_id.clone(),
                safety_level: action.safety.clone(),
                execution: "skipped".to_string(),
                closeout_path: None,
                verification: None,
                commit_sha: None,
                merged: None,
            },
            Some((_, AggregateActionResult::Committed { closeout_path, commit_sha })) => {
                AggregateActionEntry {
                    action_id: action.action_id.clone(),
                    safety_level: action.safety.clone(),
                    execution: "committed".to_string(),
                    closeout_path: closeout_path.clone(),
                    verification: Some("pass".to_string()),
                    commit_sha: commit_sha.clone(),
                    merged: Some(false),
                }
            }
            Some((_, AggregateActionResult::Failed { reason })) => {
                any_fail = true;
                AggregateActionEntry {
                    action_id: action.action_id.clone(),
                    safety_level: action.safety.clone(),
                    execution: "failed".to_string(),
                    closeout_path: None,
                    verification: Some(reason.clone()),
                    commit_sha: None,
                    merged: None,
                }
            }
            Some((_, AggregateActionResult::Interrupted)) => {
                any_partial = true;
                AggregateActionEntry {
                    action_id: action.action_id.clone(),
                    safety_level: action.safety.clone(),
                    execution: "interrupted".to_string(),
                    closeout_path: None,
                    verification: Some("interrupted".to_string()),
                    commit_sha: None,
                    merged: None,
                }
            }
            None => {
                any_partial = true;
                AggregateActionEntry {
                    action_id: action.action_id.clone(),
                    safety_level: action.safety.clone(),
                    execution: "unknown".to_string(),
                    closeout_path: None,
                    verification: None,
                    commit_sha: None,
                    merged: None,
                }
            }
        };
        aggregate.actions.push(entry);
    }

    aggregate.overall_status = if aggregate.escalated {
        "escalated".to_string()
    } else if any_fail {
        "fail".to_string()
    } else if any_partial {
        "partial".to_string()
    } else {
        "pass".to_string()
    };
    aggregate.partial = any_partial;

    aggregate
}

#[derive(Debug, Clone)]
/// Outcome of a single action execution for aggregation purposes.
pub enum AggregateActionResult {
    Skipped,
    Committed {
        closeout_path: Option<String>,
        commit_sha: Option<String>,
    },
    Failed {
        reason: String,
    },
    Interrupted,
}

/// Check whether the aggregate overall status is "pass".
pub fn aggregate_passes(aggregate: &LoopCloseoutAggregate) -> bool {
    aggregate.overall_status == "pass"
}

/// Check whether the aggregate overall status is "fail".
pub fn aggregate_has_failures(aggregate: &LoopCloseoutAggregate) -> bool {
    aggregate.overall_status == "fail"
}

/// Check whether the aggregate has any partial actions (interrupted or unmatched).
pub fn aggregate_has_partial(aggregate: &LoopCloseoutAggregate) -> bool {
    aggregate.partial
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_verify_closeout_passes() {
        let record = json!({
            "task_id": "a1",
            "summary": "fixed deprecation",
            "verification_status": "passed",
            "changed_files": ["src/main.rs"],
            "commands_run": [{"command": "cargo test", "exit_code": 0}],
            "blockers": [],
            "risks": []
        });
        let resp = verify_closeout_value(&record);
        assert!(resp.closeout_allowed);
        assert!(resp.violations.is_empty());
    }

    #[test]
    fn test_verify_closeout_fails_missing_task_id() {
        let record = json!({
            "summary": "fix",
            "verification_status": "passed",
            "changed_files": ["a.rs"],
            "commands_run": []
        });
        let resp = verify_closeout_value(&record);
        assert!(!resp.closeout_allowed);
        assert!(resp.violations.iter().any(|v| v.contains("task_id_missing")));
    }

    #[test]
    fn test_verify_closeout_fails_not_run() {
        let record = json!({
            "task_id": "a1",
            "summary": "fix",
            "verification_status": "not_run",
            "changed_files": ["a.rs"],
            "commands_run": []
        });
        let resp = verify_closeout_value(&record);
        assert!(!resp.closeout_allowed);
        assert!(resp.violations.iter().any(|v| v.contains("not_run")));
    }

    #[test]
    fn test_verify_closeout_fails_command_failure() {
        let record = json!({
            "task_id": "a1",
            "summary": "fix",
            "verification_status": "passed",
            "changed_files": ["a.rs"],
            "commands_run": [{"command": "cargo test", "exit_code": 1}],
            "blockers": [],
            "risks": []
        });
        let resp = verify_closeout_value(&record);
        assert!(!resp.closeout_allowed);
        assert!(resp.violations.iter().any(|v| v.contains("command_failed")));
    }

    #[test]
    fn test_build_aggregate_pass() {
        let actions = vec![LoopAction {
            action_id: "a1".into(),
            action_type: "fix".into(),
            scope_paths: vec!["src/a.rs".into()],
            safety: "L2".into(),
            description: None,
        }];
        let results = vec![(
            "a1".to_string(),
            AggregateActionResult::Committed {
                closeout_path: Some("artifacts/closeout/a1.json".into()),
                commit_sha: Some("abc123".into()),
            },
        )];
        let agg = build_aggregate("run-1", "test-loop", &actions, results);
        assert_eq!(agg.overall_status, "pass");
        assert!(!agg.partial);
        assert!(aggregate_passes(&agg));
    }

    #[test]
    fn test_build_aggregate_partial() {
        let actions = vec![
            LoopAction {
                action_id: "a1".into(),
                action_type: "fix".into(),
                scope_paths: vec![],
                safety: "L1".into(),
                description: None,
            },
            LoopAction {
                action_id: "a2".into(),
                action_type: "fix".into(),
                scope_paths: vec![],
                safety: "L2".into(),
                description: None,
            },
        ];
        let results = vec![
            ("a1".to_string(), AggregateActionResult::Skipped),
            ("a2".to_string(), AggregateActionResult::Interrupted),
        ];
        let agg = build_aggregate("run-1", "test-loop", &actions, results);
        assert_eq!(agg.overall_status, "partial");
        assert!(aggregate_has_partial(&agg));
    }
}
