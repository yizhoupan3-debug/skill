//! Roadmap v5 §6.4 cat.5: sandbox close path drain -> cleanup -> recycled (registry contract).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::framework_runtime::{
    build_runtime_control_plane_payload, build_sandbox_control_response,
    dispatch_stdio_json_request,
};
use crate::runtime_envelope_ids::SANDBOX_CONTROL_SCHEMA_VERSION;
use crate::session_supervisor::handle_session_supervisor_operation;
use crate::stdio_payload_types::SandboxControlRequestPayload;

fn temp_supervisor_state_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-smoke-{name}-{nonce}.json"))
}

fn sandbox_lifecycle_contract() -> Value {
    build_runtime_control_plane_payload()["services"]["execution"]["sandbox_lifecycle_contract"]
        .clone()
}

fn contract_allows_transition(contract: &Value, from: &str, to: &str) -> bool {
    contract["allowed_transitions"]
        .as_array()
        .expect("allowed_transitions array")
        .iter()
        .any(|edge| {
            edge.as_array()
                .map(|pair| {
                    pair.len() == 2
                        && pair[0].as_str() == Some(from)
                        && pair[1].as_str() == Some(to)
                })
                .unwrap_or(false)
        })
}

fn sandbox_request(operation: &str, extra: Value) -> SandboxControlRequestPayload {
    let mut payload = SandboxControlRequestPayload {
        schema_version: SANDBOX_CONTROL_SCHEMA_VERSION.to_string(),
        operation: operation.to_string(),
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
    };
    if let Some(current_state) = extra.get("current_state").and_then(Value::as_str) {
        payload.current_state = Some(current_state.to_string());
    }
    if let Some(next_state) = extra.get("next_state").and_then(Value::as_str) {
        payload.next_state = Some(next_state.to_string());
    }
    if let Some(cleanup_failed) = extra.get("cleanup_failed").and_then(Value::as_bool) {
        payload.cleanup_failed = Some(cleanup_failed);
    }
    payload
}

/// busy -> draining -> cleanup -> recycled matches runtime control-plane contract.
#[test]
fn graceful_drain_cleanup_recycle_smoke() {
    let contract = sandbox_lifecycle_contract();
    assert_eq!(
        contract["cleanup_mode"].as_str(),
        Some("async-drain-and-recycle")
    );
    assert!(contract_allows_transition(&contract, "busy", "draining"));
    assert!(contract_allows_transition(&contract, "draining", "recycled"));

    let drain = build_sandbox_control_response(sandbox_request(
        "execution_result",
        json!({ "current_state": "busy" }),
    ))
    .expect("execution_result drain");
    assert!(drain.allowed);
    assert_eq!(drain.reason, "execution-completed");
    assert_eq!(drain.resolved_state.as_deref(), Some("draining"));
    assert_eq!(drain.cleanup_required, Some(true));

    let transition = build_sandbox_control_response(sandbox_request(
        "transition",
        json!({
            "current_state": "busy",
            "next_state": "draining",
        }),
    ))
    .expect("busy to draining transition");
    assert!(transition.allowed);
    assert_eq!(transition.resolved_state.as_deref(), Some("draining"));

    let recycled = build_sandbox_control_response(sandbox_request(
        "cleanup",
        json!({
            "current_state": "draining",
            "cleanup_failed": false,
        }),
    ))
    .expect("cleanup recycled");
    assert!(recycled.allowed);
    assert_eq!(recycled.reason, "cleanup-completed");
    assert_eq!(recycled.resolved_state.as_deref(), Some("recycled"));

    let stdio_recycled = dispatch_stdio_json_request(
        "sandbox_control",
        json!({
            "schema_version": SANDBOX_CONTROL_SCHEMA_VERSION,
            "operation": "cleanup",
            "current_state": "draining",
            "cleanup_failed": false,
        }),
    )
    .expect("stdio sandbox_control cleanup");
    assert_eq!(
        stdio_recycled["resolved_state"].as_str(),
        Some("recycled"),
        "stdio sandbox_control must match in-process drain-close path"
    );
    assert_eq!(
        stdio_recycled["reason"].as_str(),
        Some("cleanup-completed")
    );
}

/// draining -> cleanup(cleanup_failed=true) -> failed matches control-plane force-close path.
#[test]
fn force_drain_to_failed_smoke() {
    let contract = sandbox_lifecycle_contract();
    assert!(contract_allows_transition(&contract, "draining", "failed"));

    let drain = build_sandbox_control_response(sandbox_request(
        "execution_result",
        json!({ "current_state": "busy" }),
    ))
    .expect("execution_result drain");
    assert!(drain.allowed);
    assert_eq!(drain.resolved_state.as_deref(), Some("draining"));
    assert_eq!(drain.cleanup_required, Some(true));

    let failed = build_sandbox_control_response(sandbox_request(
        "cleanup",
        json!({
            "current_state": "draining",
            "cleanup_failed": true,
        }),
    ))
    .expect("cleanup failed");
    assert!(failed.allowed);
    assert_eq!(failed.reason, "cleanup-failed");
    assert_eq!(failed.resolved_state.as_deref(), Some("failed"));
    assert_eq!(failed.quarantined, Some(true));
    assert_eq!(
        failed.event_kind.as_deref(),
        Some("sandbox.cleanup_failed")
    );

    let stdio_failed = dispatch_stdio_json_request(
        "sandbox_control",
        json!({
            "schema_version": SANDBOX_CONTROL_SCHEMA_VERSION,
            "operation": "cleanup",
            "current_state": "draining",
            "cleanup_failed": true,
        }),
    )
    .expect("stdio sandbox_control cleanup_failed");
    assert_eq!(
        stdio_failed["resolved_state"].as_str(),
        Some("failed"),
        "stdio sandbox_control must match in-process force-close path"
    );
    assert_eq!(stdio_failed["reason"].as_str(), Some("cleanup-failed"));
    assert_eq!(stdio_failed["quarantined"].as_bool(), Some(true));
}

/// Concurrent drain/cleanup + supervisor list/terminate must not corrupt close paths.
#[test]
fn concurrent_close_safety_smoke() {
    const N: usize = 8;
    let drain_handles: Vec<_> = (0..N)
        .map(|_| {
            std::thread::spawn(|| {
                let drain = build_sandbox_control_response(sandbox_request(
                    "execution_result",
                    json!({ "current_state": "busy" }),
                ))
                .expect("concurrent drain");
                assert!(drain.allowed);
                assert_eq!(drain.resolved_state.as_deref(), Some("draining"));
                assert_eq!(drain.cleanup_required, Some(true));

                let stdio_drain = dispatch_stdio_json_request(
                    "sandbox_control",
                    json!({
                        "schema_version": SANDBOX_CONTROL_SCHEMA_VERSION,
                        "operation": "execution_result",
                        "current_state": "busy",
                    }),
                )
                .expect("stdio concurrent drain");
                assert_eq!(stdio_drain["resolved_state"].as_str(), Some("draining"));
            })
        })
        .collect();
    for handle in drain_handles {
        handle.join().expect("drain thread join");
    }

    let cleanup_handles: Vec<_> = (0..N)
        .map(|idx| {
            std::thread::spawn(move || {
                let cleanup_failed = idx % 2 == 0;
                let recycled = build_sandbox_control_response(sandbox_request(
                    "cleanup",
                    json!({
                        "current_state": "draining",
                        "cleanup_failed": cleanup_failed,
                    }),
                ))
                .expect("concurrent cleanup");
                assert!(recycled.allowed);
                let expected = if cleanup_failed { "failed" } else { "recycled" };
                assert_eq!(recycled.resolved_state.as_deref(), Some(expected));

                let stdio_cleanup = dispatch_stdio_json_request(
                    "sandbox_control",
                    json!({
                        "schema_version": SANDBOX_CONTROL_SCHEMA_VERSION,
                        "operation": "cleanup",
                        "current_state": "draining",
                        "cleanup_failed": cleanup_failed,
                    }),
                )
                .expect("stdio concurrent cleanup");
                assert_eq!(stdio_cleanup["resolved_state"].as_str(), Some(expected));
            })
        })
        .collect();
    for handle in cleanup_handles {
        handle.join().expect("cleanup thread join");
    }

    let state_path = Arc::new(temp_supervisor_state_path("concurrent-close"));
    let launch_now = "2026-04-23T10:00:00Z";
    let close_now = "2026-04-23T10:01:00Z";
    for idx in 0..N {
        let state_path = Arc::clone(&state_path);
        handle_session_supervisor_operation(json!({
            "operation": "launch",
            "state_path": state_path.as_ref(),
            "worker_id": format!("close-{idx}"),
            "host": "codex",
            "cwd": "/tmp/project",
            "prompt": format!("concurrent close lane {idx}"),
            "dry_run": true,
            "now": launch_now,
        }))
        .expect("launch worker for concurrent close");
    }

    let mut close_handles = Vec::new();
    for idx in 0..N {
        let state_path = Arc::clone(&state_path);
        close_handles.push(std::thread::spawn(move || {
            handle_session_supervisor_operation(json!({
                "operation": "terminate",
                "state_path": state_path.as_ref(),
                "worker_id": format!("close-{idx}"),
                "dry_run": true,
                "now": close_now,
            }))
            .expect("concurrent terminate");
        }));
    }
    for _ in 0..N {
        let state_path = Arc::clone(&state_path);
        close_handles.push(std::thread::spawn(move || {
            handle_session_supervisor_operation(json!({
                "operation": "list",
                "state_path": state_path.as_ref(),
                "now": close_now,
            }))
            .expect("concurrent list during close");
        }));
    }
    for handle in close_handles {
        handle.join().expect("supervisor close thread join");
    }

    let listed = handle_session_supervisor_operation(json!({
        "operation": "list",
        "state_path": state_path.as_ref(),
        "now": close_now,
    }))
    .expect("final list after concurrent close");
    let workers = listed["workers"]
        .as_array()
        .expect("workers array");
    assert_eq!(workers.len(), N, "workers={workers:?}");
    for worker in workers {
        assert_eq!(worker["status"], json!("interrupted"));
    }

    let store_text = fs::read_to_string(state_path.as_ref()).expect("read supervisor store");
    let store: Value = serde_json::from_str(&store_text).expect("parse supervisor store after concurrent close");
    assert!(
        store.get("workers").and_then(Value::as_array).is_some(),
        "store must remain valid JSON after concurrent terminate/list"
    );

    let _ = fs::remove_file(state_path.as_ref());
}
