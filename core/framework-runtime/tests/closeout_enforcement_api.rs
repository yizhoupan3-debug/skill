//! Integration tests for framework-runtime public API.
//! Tests the closeout enforcement lifecycle from an external consumer perspective.

use framework_runtime::closeout_enforcement::evaluate_closeout_record_value;
use serde_json::json;

#[test]
fn evaluate_closeout_accepts_valid_passed_record() {
    let result = evaluate_closeout_record_value(json!({
        "schema_version": "closeout-record-v1",
        "task_id": "test-passed",
        "verification_status": "passed",
        "summary": "All tests passed",
        "commands_run": [{"command": "cargo test", "exit_code": 0}],
        "artifacts_checked": [],
    }))
    .expect("valid passed record should not error");
    assert!(
        result["closeout_allowed"].as_bool().unwrap_or(false),
        "expected closeout_allowed=true, got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );
}

#[test]
fn evaluate_closeout_rejects_missing_schema_version() {
    let result = evaluate_closeout_record_value(json!({
        "task_id": "test-no-schema",
        "verification_status": "passed",
        "summary": "No schema version",
        "commands_run": [{"command": "cargo test", "exit_code": 0}],
        "artifacts_checked": [],
    }))
    .expect("closeout should return a response");
    assert!(!result["closeout_allowed"].as_bool().unwrap_or(true));
}

#[test]
fn evaluate_closeout_rejects_failed_command_in_passed_record() {
    let result = evaluate_closeout_record_value(json!({
        "schema_version": "closeout-record-v1",
        "task_id": "test-failed-cmd",
        "verification_status": "passed",
        "summary": "Passed but command failed",
        "commands_run": [
            {"command": "cargo test", "exit_code": 1}
        ],
        "artifacts_checked": [],
    }))
    .expect("closeout should return a response");
    assert!(!result["closeout_allowed"].as_bool().unwrap_or(true));
    let violations = result["violations"].as_array().unwrap();
    assert!(!violations.is_empty(), "expected violations for failed command in passed record");
}

#[test]
fn evaluate_closeout_accepts_failed_record() {
    let result = evaluate_closeout_record_value(json!({
        "schema_version": "closeout-record-v1",
        "task_id": "test-failed",
        "verification_status": "failed",
        "summary": "Tests failed but record acknowledged",
        "commands_run": [{"command": "cargo test", "exit_code": 0}],
        "artifacts_checked": [],
    }))
    .expect("valid failed record should not error");
    assert!(
        result["closeout_allowed"].as_bool().unwrap_or(false),
        "expected closeout_allowed=true for explicit failed record, got: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );
}

#[test]
fn evaluate_closeout_returns_response_for_deny_unknown_fields() {
    // Typo field "verification_state" should be caught by deny_unknown_fields.
    let result = evaluate_closeout_record_value(json!({
        "schema_version": "closeout-record-v1",
        "task_id": "test-typo",
        "verification_state": "passed",
        "summary": "Typo field",
        "commands_run": [{"command": "cargo test", "exit_code": 0}],
        "artifacts_checked": [],
    }))
    .expect("closeout should return a response, not crash");
    // The deny_unknown_fields serde attribute means this parse error should
    // produce a block-level violation and closeout_allowed=false.
    assert!(!result["closeout_allowed"].as_bool().unwrap_or(true));
    let violations = result["violations"].as_array().unwrap();
    assert!(!violations.is_empty(), "expected violations for unknown field");
}
