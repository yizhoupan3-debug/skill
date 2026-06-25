use crate::state::closeout_path;
use crate::types::{
    AggregateActionEntry, LoopAction, LoopActionRecord, LoopCloseoutAggregate, LoopError,
};
use fr_contracts::closeout_enforcement::evaluate_closeout_record_value;
use std::fs;
use std::path::Path;

const LOOP_CLOSEOUT_AGGREGATE_SCHEMA_VERSION: &str = "loop-closeout-aggregate-v1";

#[derive(Debug, Clone)]
/// Response from a closeout verification, indicating whether closeout is allowed
/// and listing any violations found.
pub struct CloseoutVerificationResponse {
    pub closeout_allowed: bool,
    pub violations: Vec<String>,
}

/// Verify a closeout JSON record by delegating to fr-contracts
/// `evaluate_closeout_record_value` implementation (single source of truth).
///
/// This replaces the former independent 6-rule implementation, ensuring
/// the same closeout JSON produces the same `closeout_allowed` on both paths.
pub fn verify_closeout_value(record: &serde_json::Value) -> CloseoutVerificationResponse {
    delegate_to_framework_runtime(record)
}

/// Internal helper: call fr-contracts' `evaluate_closeout_record_value`
/// and translate the result into a `CloseoutVerificationResponse`.
fn delegate_to_framework_runtime(record: &serde_json::Value) -> CloseoutVerificationResponse {
    match evaluate_closeout_record_value(record.clone()) {
        Ok(response_value) => {
            // Parse the structured CloseoutEnforcementResponse from JSON.
            let closeout_allowed = response_value
                .get("closeout_allowed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let violations: Vec<String> = response_value
                .get("violations")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|v| {
                            // Each violation is a CloseoutViolation { rule, severity, category, detail }.
                            // Format as "rule: detail" for backward-compatible string output.
                            let rule = v.get("rule").and_then(|r| r.as_str()).unwrap_or("unknown");
                            let detail = v.get("detail").and_then(|d| d.as_str()).unwrap_or("");
                            if detail.is_empty() {
                                rule.to_string()
                            } else {
                                format!("{rule}: {detail}")
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            CloseoutVerificationResponse {
                closeout_allowed,
                violations,
            }
        }
        Err(err) => CloseoutVerificationResponse {
            closeout_allowed: false,
            violations: vec![format!("framework_runtime_error: {err}")],
        },
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
        response
            .violations
            .push("evidence_index_missing_or_empty".to_string());
        response.closeout_allowed = false;
    }
    response
}

/// Verify RFV convergence state for paper-revision tasks.
/// Checks that the RFV loop has closed with proper convergence (min_rounds met,
/// consecutive_stable_count >= required, loop_status == "closed").
/// Returns Ok(()) if converged or if no RFV state exists (non-paper tasks).
/// Returns Err(violations) if RFV state indicates incomplete convergence.
pub fn verify_rfv_convergence(repo_root: &Path, task_id: &str) -> Result<(), Vec<String>> {
    let qg_path = repo_root
        .join("artifacts/current")
        .join(task_id)
        .join(core_state::state_manager::QUALITY_GATE_STATE_FILENAME);
    if !qg_path.is_file() {
        // No quality gate state — not a paper-revision task, or QG not started. Pass through.
        return Ok(());
    }
    let raw = match fs::read_to_string(&qg_path) {
        Ok(r) => r,
        Err(_) => return Ok(()), // Can't read — not a hard block for non-paper tasks
    };
    let val: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let mut violations = Vec::new();

    let loop_status = val
        .get("loop_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if loop_status != "closed" {
        violations.push(format!("quality_gate_not_closed: status={}", loop_status));
    }

    let current_round = val
        .get("current_round")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let min_rounds = val.get("min_rounds").and_then(|v| v.as_u64()).unwrap_or(0);
    if current_round < min_rounds {
        violations.push(format!(
            "quality_gate_below_min_rounds: current={} min={}",
            current_round, min_rounds
        ));
    }

    let stable_count = val
        .get("consecutive_stable_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let stable_required = val
        .get("consecutive_stable_required")
        .and_then(|v| v.as_u64())
        .unwrap_or(2);
    if stable_count < stable_required {
        violations.push(format!(
            "rfv_convergence_not_met: stable_count={} required={}",
            stable_count, stable_required
        ));
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
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
        schema_version: LOOP_CLOSEOUT_AGGREGATE_SCHEMA_VERSION.to_string(),
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
            Some((
                _,
                AggregateActionResult::Committed {
                    closeout_path,
                    commit_sha,
                },
            )) => AggregateActionEntry {
                action_id: action.action_id.clone(),
                safety_level: action.safety.clone(),
                execution: "committed".to_string(),
                closeout_path: closeout_path.clone(),
                verification: Some("pass".to_string()),
                commit_sha: commit_sha.clone(),
                merged: Some(false),
            },
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
            "schema_version": "closeout-record-v1",
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
            "schema_version": "closeout-record-v1",
            "summary": "fix",
            "verification_status": "passed",
            "changed_files": ["a.rs"],
            "commands_run": []
        });
        let resp = verify_closeout_value(&record);
        assert!(!resp.closeout_allowed);
        assert!(
            resp.violations
                .iter()
                .any(|v| v.contains("task_id_missing"))
        );
    }

    #[test]
    fn test_verify_closeout_fails_not_run() {
        let record = json!({
            "schema_version": "closeout-record-v1",
            "task_id": "a1",
            "summary": "fix",
            "verification_status": "not_run",
            "changed_files": ["a.rs"],
            "commands_run": []
        });
        let resp = verify_closeout_value(&record);
        assert!(!resp.closeout_allowed);
        // fr-contracts emits "not_run_without_blockers_or_risks" or
        // "claimed_done_without_evidence" for not_run records.
        assert!(resp.violations.iter().any(|v| v.contains("not_run")));
    }

    #[test]
    fn test_verify_closeout_fails_command_failure() {
        let record = json!({
            "schema_version": "closeout-record-v1",
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
        // fr-contracts emits "verification_passed_with_failed_command" for this case.
        assert!(
            resp.violations
                .iter()
                .any(|v| v.contains("failed_command") || v.contains("command_failed"))
        );
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

    /// B9 cross-path equivalence: the same closeout JSON must produce the same
    /// `closeout_allowed` when evaluated by both loop-engine's `verify_closeout_value`
    /// (which now delegates to fr-contracts) and the raw
    /// `evaluate_closeout_record_value` function directly.
    #[test]
    fn test_cross_path_equivalence_pass() {
        let record = json!({
            "schema_version": "closeout-record-v1",
            "task_id": "b9-equiv",
            "summary": "cross-path equivalence test",
            "verification_status": "passed",
            "changed_files": ["src/closeout.rs"],
            "commands_run": [{"command": "cargo test -p loop-engine", "exit_code": 0}],
            "blockers": [],
            "risks": []
        });
        // Loop-engine path (delegates to fr-contracts)
        let le_resp = verify_closeout_value(&record);
        // Direct fr-contracts path
        let fr_resp =
            fr_contracts::closeout_enforcement::evaluate_closeout_record_value(record.clone())
                .expect("fr-contracts should return Ok");
        let fr_allowed = fr_resp
            .get("closeout_allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert_eq!(
            le_resp.closeout_allowed, fr_allowed,
            "both paths must agree on closeout_allowed for a valid record"
        );
    }

    #[test]
    fn test_cross_path_equivalence_fail() {
        let record = json!({
            "schema_version": "closeout-record-v1",
            "task_id": "b9-equiv-fail",
            "summary": "should fail on command failure",
            "verification_status": "passed",
            "changed_files": ["src/closeout.rs"],
            "commands_run": [{"command": "cargo test", "exit_code": 1}],
            "blockers": [],
            "risks": []
        });
        let le_resp = verify_closeout_value(&record);
        let fr_resp =
            fr_contracts::closeout_enforcement::evaluate_closeout_record_value(record.clone())
                .expect("fr-contracts should return Ok");
        let fr_allowed = fr_resp
            .get("closeout_allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert_eq!(
            le_resp.closeout_allowed, fr_allowed,
            "both paths must agree on closeout_allowed for a failing record"
        );
        assert!(
            !le_resp.closeout_allowed,
            "this record should be disallowed"
        );
        assert!(!fr_allowed, "this record should be disallowed");
    }
}
