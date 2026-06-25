#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::json;
use super::*;

fn record_with(summary: &str, status: &str) -> CloseoutRecord {
    CloseoutRecord {
        schema_version: CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
        task_id: "t-1".to_string(),
        summary: summary.to_string(),
        verification_status: status.to_string(),
        ..Default::default()
    }
}

fn has_rule(resp: &CloseoutEnforcementResponse, rule: &str) -> bool {
    resp.violations.iter().any(|v| v.rule == rule)
}

#[test]
fn passed_clean_record_is_allowed() {
    let mut record = record_with("已完成 deck rebuild and verified output", "passed");
    record.commands_run.push(CloseoutCommandRecord {
        command: "python build_deck.py".to_string(),
        exit_code: 0,
        ..Default::default()
    });
    record.artifacts_checked.push(CloseoutArtifactRecord {
        path: "ppt/deck_v3.pptx".to_string(),
        exists: true,
        ..Default::default()
    });
    record.changed_files.push("ppt/build_deck.py".to_string());
    let resp = evaluate_closeout_record(&record);
    assert!(
        resp.closeout_allowed,
        "expected allowed, got {:?}",
        resp.violations
    );
    assert!(resp.claimed_completion);
}

#[test]
fn closeout_record_schema_snapshot() {
    // Snapshot default CloseoutRecord structure for schema regression detection.
    let record = CloseoutRecord::default();
    insta::assert_debug_snapshot!(record);
}

#[test]
fn claimed_done_without_evidence_is_blocked() {
    let record = record_with("已完成", "not_run");
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "claimed_done_without_evidence"));
}

#[test]
fn changed_files_without_command_or_risk_is_blocked() {
    let mut record = record_with("Refactored builder", "partial");
    record.changed_files.push("ppt/build_deck.py".to_string());
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "changed_files_without_command_or_risk"));
}

#[test]
fn changed_files_with_risk_is_allowed() {
    let mut record = record_with("Refactored builder; tests not run", "partial");
    record.changed_files.push("ppt/build_deck.py".to_string());
    record
        .risks
        .push("did not execute python build_deck.py because PIL missing".to_string());
    let resp = evaluate_closeout_record(&record);
    assert!(
        resp.closeout_allowed,
        "expected allowed, violations: {:?}",
        resp.violations
    );
}

#[test]
fn verification_passed_with_failed_command_is_blocked() {
    let mut record = record_with("done", "passed");
    record.commands_run.push(CloseoutCommandRecord {
        command: "pytest".to_string(),
        exit_code: 1,
        ..Default::default()
    });
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "verification_passed_with_failed_command"));
}

#[test]
fn verification_passed_with_missing_artifact_is_blocked() {
    let mut record = record_with("done", "passed");
    record.artifacts_checked.push(CloseoutArtifactRecord {
        path: "ppt/deck_v3.pptx".to_string(),
        exists: false,
        ..Default::default()
    });
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "verification_passed_with_missing_artifact"));
}

#[test]
fn not_run_without_blockers_is_blocked() {
    let record = record_with("Investigating but not yet done", "not_run");
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(
        has_rule(&resp, "not_run_without_blockers_or_risks")
            || has_rule(&resp, "claimed_done_without_evidence")
    );
}

#[test]
fn not_run_with_blocker_is_allowed() {
    let mut record = record_with("Paused awaiting user", "not_run");
    record
        .blockers
        .push("Need user to approve schema migration".to_string());
    let resp = evaluate_closeout_record(&record);
    assert!(resp.closeout_allowed, "violations: {:?}", resp.violations);
}

#[test]
fn failed_status_with_done_summary_is_blocked() {
    let mut record = record_with("已完成", "failed");
    // No risks/blockers.
    record.commands_run.push(CloseoutCommandRecord {
        command: "pytest".to_string(),
        exit_code: 2,
        ..Default::default()
    });
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "claimed_done_with_failed_verification"));
}

#[test]
fn empty_summary_and_status_blocks() {
    let record = CloseoutRecord {
        schema_version: CLOSEOUT_RECORD_SCHEMA_VERSION.to_string(),
        task_id: "t-empty".to_string(),
        ..Default::default()
    };
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "summary_missing"));
    assert!(has_rule(&resp, "verification_status_missing"));
}

#[test]
fn schema_version_mismatch_blocks() {
    let record = CloseoutRecord {
        schema_version: "wrong-v0".to_string(),
        task_id: "t".to_string(),
        summary: "ok".to_string(),
        verification_status: "partial".to_string(),
        ..Default::default()
    };
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "schema_version_mismatch"));
}

#[test]
fn schema_version_missing_blocks_value_evaluator() {
    let response = evaluate_closeout_record_value(json!({
        "task_id": "t",
        "summary": "ok",
        "verification_status": "partial"
    }))
    .expect("evaluate closeout");
    assert_eq!(response["closeout_allowed"], json!(false));
    assert!(
        response["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .any(|v| v["rule"] == "schema_version_mismatch")
    );
}

#[test]
fn empty_command_record_does_not_count_as_success_evidence() {
    let response = evaluate_closeout_record_value(json!({
        "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
        "task_id": "t",
        "summary": "done",
        "verification_status": "passed",
        "commands_run": [{}]
    }))
    .expect("evaluate closeout");
    assert_eq!(response["closeout_allowed"], json!(false));
    assert!(
        response["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .any(|v| v["rule"] == "invalid_command_evidence")
    );
}

#[test]
fn context_task_id_mismatch_blocks_value_evaluator() {
    let ctx = CloseoutEvidenceContext {
        task_id: Some("expected-task".to_string()),
        _evidence_rows_non_empty: true,
        has_successful_verification: true,
        ..Default::default()
    };
    let response = evaluate_closeout_record_value_with_context(
        json!({
            "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
            "task_id": "other-task",
            "summary": "done",
            "verification_status": "passed",
            "commands_run": [{"command": "cargo test", "exit_code": 0}]
        }),
        &ctx,
    )
    .expect("evaluate closeout");
    assert_eq!(response["closeout_allowed"], json!(false));
    assert!(
        response["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .any(|v| v["rule"] == "task_id_context_mismatch")
    );
}

#[test]
fn invalid_status_is_blocked() {
    let record = record_with("ok", "maybe");
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed);
    assert!(has_rule(&resp, "verification_status_invalid"));
}

#[test]
fn contract_payload_lists_rules() {
    let payload = closeout_enforcement_contract();
    let rules = payload["rules"].as_array().expect("rules array");
    assert!(rules.iter().any(|v| v == "claimed_done_without_evidence"));
    assert!(
        rules
            .iter()
            .any(|v| v == "verification_passed_with_missing_artifact")
    );
    assert!(rules.iter().any(|v| v == "claimed_passed_without_evidence"));
    assert!(
        rules
            .iter()
            .any(|v| v == "claimed_passed_without_evidence_index_rows")
    );
    assert_eq!(
        payload["record_schema_version"],
        CLOSEOUT_RECORD_SCHEMA_VERSION
    );
}

/// P0-B / R7: passed + nothing else recorded → block.
#[test]
fn passed_with_no_evidence_or_acknowledgement_is_blocked() {
    let record = record_with("已完成 verification skipped", "passed");
    let resp = evaluate_closeout_record(&record);
    assert!(!resp.closeout_allowed, "violations: {:?}", resp.violations);
    assert!(has_rule(&resp, "claimed_passed_without_evidence"));
}

/// R7 must not fire when an artifact-existence check is recorded (R4 still owns missing-artifact case).
#[test]
fn passed_with_only_artifact_check_is_allowed_by_r7() {
    let mut record = record_with("done", "passed");
    record.artifacts_checked.push(CloseoutArtifactRecord {
        path: "out/release.tar.gz".to_string(),
        exists: true,
        ..Default::default()
    });
    let resp = evaluate_closeout_record(&record);
    // R7 scope = empty {commands_run, artifacts_checked, risks, blockers}; artifact present → R7 silent.
    assert!(!has_rule(&resp, "claimed_passed_without_evidence"));
}

/// R8: passed + commands_run empty + EVIDENCE rollup empty → block by context-aware path.
#[test]
fn r8_blocks_passed_when_evidence_rollup_empty() {
    let mut record = record_with("done", "passed");
    // Acknowledge a risk so R7 stays silent and we isolate R8.
    record
        .risks
        .push("did not run verifier locally".to_string());
    let ctx = CloseoutEvidenceContext {
        task_id: Some(record.task_id.clone()),
        _evidence_rows_non_empty: false,
        has_successful_verification: false,
        ..Default::default()
    };
    let resp = evaluate_closeout_record_with_context(&record, &ctx);
    assert!(!resp.closeout_allowed, "violations: {:?}", resp.violations);
    assert!(has_rule(
        &resp,
        "claimed_passed_without_evidence_index_rows"
    ));
}

#[test]
fn prediction_mismatch_is_warn_not_block() {
    use core_state_types::goal_prediction::GoalStatePrediction;
    let mut record = record_with("已完成 router-rs green", "failed");
    record.commands_run.push(CloseoutCommandRecord {
        command: "cargo test -p router-rs".to_string(),
        exit_code: 1,
        ..Default::default()
    });
    record.risks.push("tests failed unexpectedly".to_string());
    let ctx = CloseoutEvidenceContext {
        goal_prediction: Some(GoalStatePrediction {
            expected_verification_status: Some("passed".to_string()),
            hypothesis: Some("router-rs green".to_string()),
        }),
        ..Default::default()
    };
    let resp = evaluate_closeout_record_with_context(&record, &ctx);
    assert!(
        resp.closeout_allowed,
        "prediction dry-run must not block closeout: {:?}",
        resp.violations
    );
    assert!(has_rule(&resp, "prediction_verification_status_mismatch"));
    assert!(
        resp.prediction_verification
            .iter()
            .any(|p| p.rule == "prediction_hypothesis_reflected")
    );
}

/// R8 silent when EVIDENCE rollup has at least one successful row.
#[test]
fn r8_allows_passed_when_evidence_has_successful_row() {
    let mut record = record_with("done", "passed");
    record.risks.push(
        "commands_run intentionally empty; relying on hook-appended evidence".to_string(),
    );
    let ctx = CloseoutEvidenceContext {
        task_id: Some(record.task_id.clone()),
        _evidence_rows_non_empty: true,
        has_successful_verification: true,
        ..Default::default()
    };
    let resp = evaluate_closeout_record_with_context(&record, &ctx);
    assert!(resp.closeout_allowed, "violations: {:?}", resp.violations);
    assert!(!has_rule(
        &resp,
        "claimed_passed_without_evidence_index_rows"
    ));
}

/// P0-F-style invariant: typo'd field rejected by deny_unknown_fields, not silently ignored.
/// After fix: serde parse failure returns Ok(response) with closeout_allowed=false and
/// a parse_error violation (plus any raw_shape_violations), rather than Err(String).
#[test]
fn unknown_field_in_record_is_rejected_at_parse() {
    let bad = json!({
        "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
        "task_id": "t",
        "summary": "ok",
        "verification_state": "passed"
    });
    let response = evaluate_closeout_record_value(bad)
        .expect("should return enforcement response, not Err");
    assert_eq!(response["closeout_allowed"], json!(false));
    let violations = response["violations"].as_array().expect("violations");
    assert!(
        violations.iter().any(|v| v["rule"] == "parse_error"),
        "should have parse_error violation: {:?}",
        violations
    );
    let parse_detail = violations
        .iter()
        .find(|v| v["rule"] == "parse_error")
        .and_then(|v| v["detail"].as_str())
        .unwrap_or("");
    assert!(
        parse_detail.contains("unknown field"),
        "parse_error detail should mention unknown field: {}",
        parse_detail
    );
}
