//! Integration tests for agent-orchestrator: worker lifecycle management.

use agent_orchestrator::handle_orchestrator_operation;
use serde_json::json;
use std::path::PathBuf;

fn temp_state_path(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("orchestrator-test-{label}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&p);
    p
}

#[test]
fn inspect_returns_error_for_missing_worker() {
    let state_path = temp_state_path("inspect-missing");
    let result = handle_orchestrator_operation(json!({
        "operation": "inspect",
        "state_path": state_path,
        "worker_id": "nonexistent-worker",
    }));
    assert!(result.is_err(), "inspect of missing worker should error");
    let _ = std::fs::remove_dir_all(&state_path);
}

#[test]
fn classify_block_accepts_rate_limit_evidence() {
    let state_path = temp_state_path("classify-evidence");
    let result = handle_orchestrator_operation(json!({
        "operation": "classify_block",
        "state_path": state_path,
        "host": "claude",
        "evidence_text": "HTTP 429 Too Many Requests",
    }))
    .expect("classify_block should succeed");
    assert_eq!(result["operation"], "classify_block");
    let _ = std::fs::remove_dir_all(&state_path);
}
