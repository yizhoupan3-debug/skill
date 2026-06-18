use super::common::*;
use super::*;

use serde_json::Value;


#[test]
fn sandbox_control_accepts_known_edges_and_rejects_invalid_edges() {
    let accepted = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "transition".to_string(),
        current_state: Some("warm".to_string()),
        next_state: Some("busy".to_string()),
        ..sandbox_control_request_defaults()
    })
    .expect("accepted transition");
    assert_eq!(accepted.authority, SANDBOX_CONTROL_AUTHORITY);
    assert!(accepted.allowed);
    assert_eq!(accepted.reason, "transition-accepted");
    assert_eq!(accepted.resolved_state.as_deref(), Some("busy"));

    let rejected = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "transition".to_string(),
        current_state: Some("busy".to_string()),
        next_state: Some("warm".to_string()),
        ..sandbox_control_request_defaults()
    })
    .expect("rejected transition");
    assert!(!rejected.allowed);
    assert_eq!(rejected.reason, "invalid-transition");
    assert_eq!(
        rejected.error.as_deref(),
        Some("invalid sandbox transition: \"busy\" -> \"warm\"")
    );
}


#[test]
fn sandbox_control_cleanup_resolves_recycled_and_failed_targets() {
    let recycled = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "cleanup".to_string(),
        current_state: Some("draining".to_string()),
        cleanup_failed: Some(false),
        ..sandbox_control_request_defaults()
    })
    .expect("cleanup recycled response");
    assert!(recycled.allowed);
    assert_eq!(recycled.reason, "cleanup-completed");
    assert_eq!(recycled.resolved_state.as_deref(), Some("recycled"));

    let failed = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "cleanup".to_string(),
        current_state: Some("draining".to_string()),
        cleanup_failed: Some(true),
        ..sandbox_control_request_defaults()
    })
    .expect("cleanup failed response");
    assert!(failed.allowed);
    assert_eq!(failed.reason, "cleanup-failed");
    assert_eq!(failed.resolved_state.as_deref(), Some("failed"));
}


#[test]
fn sandbox_control_records_durable_event_when_requested() {
    let path = temp_trace_path("sandbox-events");
    let response = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "admit".to_string(),
        sandbox_id: Some("sandbox-1".to_string()),
        profile_id: Some("workspace".to_string()),
        current_state: Some("warm".to_string()),
        tool_category: Some("workspace_mutating".to_string()),
        capability_categories: Some(vec![
            "read_only".to_string(),
            "workspace_mutating".to_string(),
        ]),
        budget_cpu: Some(1.0),
        budget_memory: Some(1024),
        budget_wall_clock: Some(5.0),
        budget_output_size: Some(4096),
        event_log_path: Some(path.display().to_string()),
        trace_event: Some(true),
        ..sandbox_control_request_defaults()
    })
    .expect("sandbox event response");

    assert!(response.allowed);
    assert!(response.event_written);
    assert_eq!(
        response.event_schema_version.as_deref(),
        Some(SANDBOX_EVENT_SCHEMA_VERSION)
    );
    assert_eq!(
        response.effective_capabilities,
        Some(vec![
            "read_only".to_string(),
            "workspace_mutating".to_string()
        ])
    );

    let line = fs::read_to_string(&path).expect("sandbox event log");
    let event: Value = serde_json::from_str(line.trim()).expect("sandbox event json");
    assert_eq!(
        event["schema_version"],
        Value::String(SANDBOX_EVENT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        event["kind"],
        Value::String("sandbox.execution_started".to_string())
    );
    assert_eq!(event["sandbox_id"], Value::String("sandbox-1".to_string()));
    assert_eq!(
        event["effective_capabilities"][1],
        Value::String("workspace_mutating".to_string())
    );
}


#[test]
fn sandbox_event_append_preserves_jsonl_records_under_concurrency() {
    let event_path = temp_trace_path("sandbox-events-concurrent");
    let mut workers = Vec::new();
    for seq in 0..32 {
        let path = event_path.clone();
        workers.push(spawn(move || {
            build_sandbox_control_response(SandboxControlRequestPayload {
                schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
                operation: "admit".to_string(),
                sandbox_id: Some(format!("sandbox-{seq}")),
                profile_id: Some("workspace".to_string()),
                current_state: Some("warm".to_string()),
                tool_category: Some("read_only".to_string()),
                capability_categories: Some(vec!["read_only".to_string()]),
                budget_cpu: Some(1.0),
                budget_memory: Some(1024),
                budget_wall_clock: Some(5.0),
                budget_output_size: Some(4096),
                event_log_path: Some(path.display().to_string()),
                trace_event: Some(true),
                ..sandbox_control_request_defaults()
            })
            .expect("sandbox event response");
        }));
    }
    for worker in workers {
        worker.join().expect("join sandbox worker");
    }

    let persisted = fs::read_to_string(&event_path).expect("read sandbox jsonl");
    let lines = persisted.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 32);
    let mut seen = HashSet::new();
    for line in lines {
        let event = serde_json::from_str::<Value>(line).expect("parse sandbox jsonl");
        seen.insert(
            event["sandbox_id"]
                .as_str()
                .expect("sandbox id")
                .to_string(),
        );
    }
    assert_eq!(seen.len(), 32);

    fs::remove_file(&event_path).expect("cleanup sandbox path");
}


#[test]
fn sandbox_control_rejects_admission_from_invalid_state() {
    let response = build_sandbox_control_response(SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "admit".to_string(),
        current_state: Some("failed".to_string()),
        tool_category: Some("read_only".to_string()),
        capability_categories: Some(vec!["read_only".to_string()]),
        budget_cpu: Some(1.0),
        budget_memory: Some(1024),
        budget_wall_clock: Some(5.0),
        budget_output_size: Some(4096),
        ..sandbox_control_request_defaults()
    })
    .expect("sandbox invalid admission response");

    assert!(!response.allowed);
    assert_eq!(response.reason, "admission-rejected");
    assert_eq!(response.resolved_state.as_deref(), Some("failed"));
    assert_eq!(response.quarantined, Some(true));
    assert_eq!(
        response.failure_reason.as_deref(),
        Some("invalid sandbox admission state: \"failed\" -> \"busy\"")
    );
}

fn sandbox_control_request_defaults() -> SandboxControlRequestPayload {
    SandboxControlRequestPayload {
        schema_version: String::new(),
        operation: String::new(),
        sandbox_id: None,
        profile_id: None,
        current_state: None,
        next_state: None,
        cleanup_failed: None,
        tool_category: None,
        capability_categories: None,
        dedicated_profile: None,
        budget_cpu: None,
        budget_memory: None,
        budget_wall_clock: None,
        budget_output_size: None,
        probe_cpu: None,
        probe_memory: None,
        probe_wall_clock: None,
        probe_output_size: None,
        error_kind: None,
        event_log_path: None,
        trace_event: None,
    }
}


