use super::common::*;
use super::*;

use serde_json::{Value, json};

#[test]
fn background_control_enqueue_rejects_invalid_strategy_and_capacity() {
    let invalid = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "enqueue".to_string(),
        multitask_strategy: Some("pause".to_string()),
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: Some(0),
        capacity_limit: Some(4),
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("invalid strategy response");
    assert_eq!(invalid.authority, BACKGROUND_CONTROL_AUTHORITY);
    assert!(!invalid.strategy_supported);
    assert_eq!(invalid.accepted, Some(false));

    let capacity = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "enqueue".to_string(),
        multitask_strategy: Some("interrupt".to_string()),
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: Some(2),
        capacity_limit: Some(2),
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("capacity response");
    assert!(capacity.strategy_supported);
    assert_eq!(capacity.accepted, Some(false));
    assert_eq!(capacity.requires_takeover, Some(true));
    assert_eq!(capacity.reason, "capacity-rejected");
}

#[test]
fn background_control_batch_plan_resolves_group_and_lane_assignments() {
    let planned = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "batch-plan".to_string(),
        requested_parallel_group_id: Some("pgroup-contract".to_string()),
        request_parallel_group_ids: Some(vec![
            Some("pgroup-contract".to_string()),
            Some("pgroup-contract".to_string()),
        ]),
        request_lane_ids: Some(vec![Some("lane-a".to_string()), None]),
        lane_id_prefix: Some("lane".to_string()),
        batch_size: Some(2),
        ..background_control_request_defaults()
    })
    .expect("batch plan response");
    assert_eq!(planned.accepted, Some(true));
    assert_eq!(
        planned.resolved_parallel_group_id.as_deref(),
        Some("pgroup-contract")
    );
    assert_eq!(
        planned.lane_ids,
        Some(vec!["lane-a".to_string(), "lane-2".to_string()])
    );
    assert_eq!(planned.reason, "batch-plan-resolved");
    assert_eq!(planned.effect_plan.next_step, "plan_batch");

    let rejected = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "batch-plan".to_string(),
        request_parallel_group_ids: Some(vec![
            Some("pgroup-a".to_string()),
            Some("pgroup-b".to_string()),
        ]),
        batch_size: Some(2),
        ..background_control_request_defaults()
    })
    .expect("rejected batch plan response");
    assert_eq!(rejected.accepted, Some(false));
    assert_eq!(rejected.reason, "batch-plan-misaligned-parallel-group");
    assert_eq!(
        rejected.error.as_deref(),
        Some(
            "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
        )
    );
}

#[test]
fn background_control_retry_computes_backoff_and_terminal_status() {
    let retry = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry".to_string(),
        multitask_strategy: None,
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: None,
        capacity_limit: None,
        attempt: Some(1),
        retry_count: Some(0),
        max_attempts: Some(2),
        backoff_base_seconds: Some(0.5),
        backoff_multiplier: Some(2.0),
        max_backoff_seconds: Some(1.0),
        ..background_control_request_defaults()
    })
    .expect("retry response");
    assert_eq!(retry.should_retry, Some(true));
    assert_eq!(retry.next_retry_count, Some(1));
    assert_eq!(retry.backoff_seconds, Some(0.5));
    assert_eq!(retry.terminal_status.as_deref(), Some("retry_scheduled"));
    assert_eq!(retry.effect_plan.next_step, "schedule_retry");
    assert_eq!(retry.effect_plan.next_retry_count, Some(1));
    assert_eq!(retry.effect_plan.backoff_seconds, Some(0.5));

    let exhausted = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry".to_string(),
        multitask_strategy: None,
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: None,
        capacity_limit: None,
        attempt: Some(2),
        retry_count: Some(1),
        max_attempts: Some(2),
        backoff_base_seconds: Some(0.5),
        backoff_multiplier: Some(2.0),
        max_backoff_seconds: Some(1.0),
        ..background_control_request_defaults()
    })
    .expect("retry exhausted response");
    assert_eq!(exhausted.should_retry, Some(false));
    assert_eq!(
        exhausted.terminal_status.as_deref(),
        Some("retry_exhausted")
    );
    assert_eq!(exhausted.effect_plan.next_step, "finalize_terminal");
}

#[test]
fn background_control_interrupt_resolves_finalize_and_cancel_paths() {
    let queued = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "interrupt".to_string(),
        multitask_strategy: None,
        current_status: Some("queued".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("queued interrupt response");
    assert_eq!(
        queued.resolved_status.as_deref(),
        Some("interrupt_requested")
    );
    assert_eq!(queued.finalize_immediately, Some(true));
    assert_eq!(queued.cancel_running_task, Some(false));
    assert_eq!(queued.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(queued.effect_plan.next_step, "finalize_interrupted");

    let running = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "interrupt".to_string(),
        multitask_strategy: None,
        current_status: Some("running".to_string()),
        task_active: Some(true),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("running interrupt response");
    assert_eq!(running.finalize_immediately, Some(false));
    assert_eq!(running.cancel_running_task, Some(true));
    assert_eq!(
        running.terminal_status.as_deref(),
        Some("interrupt_requested")
    );
    assert_eq!(running.effect_plan.next_step, "request_interrupt");
}

#[test]
fn background_control_claim_resolves_running_and_suppressed_paths() {
    let queued = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "claim".to_string(),
        multitask_strategy: None,
        current_status: Some("queued".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("queued claim response");
    assert_eq!(queued.resolved_status.as_deref(), Some("running"));
    assert_eq!(queued.reason, "claim-running");
    assert_eq!(queued.effect_plan.next_step, "claim_execution");

    let interrupted = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "claim".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("interrupt claim response");
    assert_eq!(interrupted.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(interrupted.reason, "claim-suppressed-interrupted");
    assert_eq!(interrupted.effect_plan.next_step, "finalize_interrupted");
}

#[test]
fn background_control_complete_and_completion_race_resolve_terminal_status() {
    let complete = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "complete".to_string(),
        multitask_strategy: None,
        current_status: Some("running".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("complete response");
    assert_eq!(complete.terminal_status.as_deref(), Some("completed"));
    assert_eq!(complete.resolved_status.as_deref(), Some("completed"));
    assert_eq!(complete.effect_plan.next_step, "finalize_completed");

    let race_won = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "completion-race".to_string(),
        multitask_strategy: None,
        current_status: Some("running".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("completion race won response");
    assert_eq!(race_won.terminal_status.as_deref(), Some("completed"));
    assert_eq!(race_won.reason, "completion-race-won");
    assert_eq!(race_won.effect_plan.next_step, "finalize_completed");

    let race_lost = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "completion-race".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("completion race lost response");
    assert_eq!(race_lost.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(race_lost.resolved_status.as_deref(), Some("interrupted"));
    assert_eq!(race_lost.reason, "completion-race-lost");
    assert_eq!(race_lost.effect_plan.next_step, "finalize_interrupted");
}

#[test]
fn background_control_retry_claim_and_interrupt_finalize_cover_retry_lifecycle() {
    let claimed = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry-claim".to_string(),
        multitask_strategy: None,
        current_status: Some("retry_scheduled".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("retry claim response");
    assert_eq!(claimed.terminal_status.as_deref(), Some("retry_claimed"));
    assert_eq!(claimed.resolved_status.as_deref(), Some("retry_claimed"));
    assert_eq!(claimed.finalize_immediately, Some(false));
    assert_eq!(claimed.effect_plan.next_step, "claim_retry");

    let interrupted = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "retry-claim".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(false),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("retry claim interrupted response");
    assert_eq!(interrupted.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(interrupted.resolved_status.as_deref(), Some("interrupted"));
    assert_eq!(interrupted.reason, "retry-claim-interrupted");
    assert_eq!(interrupted.effect_plan.next_step, "finalize_interrupted");

    let finalize = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "interrupt-finalize".to_string(),
        multitask_strategy: None,
        current_status: Some("interrupt_requested".to_string()),
        task_active: Some(false),
        task_done: Some(true),
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("interrupt finalize response");
    assert_eq!(finalize.terminal_status.as_deref(), Some("interrupted"));
    assert_eq!(finalize.resolved_status.as_deref(), Some("interrupted"));
    assert_eq!(finalize.reason, "interrupt-finalized");
    assert_eq!(finalize.effect_plan.next_step, "finalize_interrupted");
}

#[test]
fn background_control_session_release_exposes_wait_plan() {
    let release = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "session-release".to_string(),
        multitask_strategy: None,
        current_status: None,
        task_active: None,
        task_done: None,
        active_job_count: None,
        capacity_limit: None,
        attempt: None,
        retry_count: None,
        max_attempts: None,
        backoff_base_seconds: None,
        backoff_multiplier: None,
        max_backoff_seconds: None,
        ..background_control_request_defaults()
    })
    .expect("session release response");
    assert_eq!(release.reason, "session-release-wait");
    assert_eq!(release.effect_plan.next_step, "wait_for_release");
    assert_eq!(release.effect_plan.wait_timeout_seconds, Some(5.0));
    assert_eq!(release.effect_plan.wait_poll_interval_seconds, Some(0.02));

    let backed_off = build_background_control_response(BackgroundControlRequestPayload {
        schema_version: BACKGROUND_CONTROL_SCHEMA_VERSION.to_string(),
        operation: "session-release".to_string(),
        retry_count: Some(3),
        ..background_control_request_defaults()
    })
    .expect("session release backoff response");
    assert_eq!(
        backed_off.effect_plan.wait_poll_interval_seconds,
        Some(0.0675)
    );
}

#[test]
fn background_state_operation_persists_control_plane_projection_and_health() {
    let state_path = temp_json_path("background-state-filesystem");
    let response = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "control_plane_descriptor": {
            "schema_version": "router-rs-runtime-control-plane-v1",
            "authority": "rust-runtime-control-plane",
            "services": {
                "state": {
                    "authority": "rust-runtime-control-plane",
                    "role": "durable-background-state",
                    "projection": "rust-native-projection",
                    "delegate_kind": "filesystem-state-store"
                },
                "trace": {
                    "authority": "rust-runtime-control-plane",
                    "role": "trace-and-handoff",
                    "projection": "rust-native-projection",
                    "delegate_kind": "filesystem-trace-store"
                }
            }
        },
        "job_id": "job-filesystem-1",
        "mutation": {
            "status": "queued",
            "session_id": "session-filesystem-1"
        }
    }))
    .expect("filesystem background state response");

    assert_eq!(
        response["schema_version"],
        Value::String("router-rs-background-state-store-v1".to_string())
    );
    assert_eq!(
        response["authority"],
        Value::String("rust-background-state-store".to_string())
    );
    assert_eq!(
        response["health"]["runtime_control_plane_authority"],
        Value::String("rust-runtime-control-plane".to_string())
    );
    assert_eq!(
        response["health"]["runtime_control_plane_schema_version"],
        Value::String("router-rs-runtime-control-plane-v1".to_string())
    );
    assert_eq!(
        response["health"]["control_plane_projection"],
        Value::String("rust-native-projection".to_string())
    );
    assert_eq!(
        response["health"]["control_plane_delegate_kind"],
        Value::String("filesystem-state-store".to_string())
    );
    assert_eq!(
        response["health"]["backend_family"],
        Value::String("filesystem".to_string())
    );
    assert_eq!(
        response["health"]["supports_atomic_replace"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_compaction"],
        Value::Bool(false)
    );
    assert_eq!(
        response["health"]["supports_snapshot_delta"],
        Value::Bool(false)
    );
    assert_eq!(
        response["health"]["supports_remote_event_transport"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_consistent_append"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_sqlite_wal"],
        Value::Bool(false)
    );

    let persisted = read_json(&state_path).expect("read persisted state");
    assert_eq!(
        persisted["control_plane"]["authority"],
        Value::String("rust-runtime-control-plane".to_string())
    );
    assert_eq!(
        persisted["control_plane"]["projection"],
        Value::String("rust-native-projection".to_string())
    );
    assert_eq!(
        persisted["control_plane"]["delegate_kind"],
        Value::String("filesystem-state-store".to_string())
    );
    assert_eq!(
        persisted["control_plane"]["supports_atomic_replace"],
        Value::Bool(true)
    );
    assert_eq!(
        persisted["control_plane"]["supports_consistent_append"],
        Value::Bool(true)
    );
    assert_eq!(
        persisted["jobs"][0]["status"],
        Value::String("queued".to_string())
    );

    let recovered = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "snapshot",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem"
    }))
    .expect("recovered background state snapshot");
    assert_eq!(
        recovered["health"]["control_plane_delegate_kind"],
        Value::String("filesystem-state-store".to_string())
    );
    assert_eq!(
        recovered["state"]["jobs"][0]["job_id"],
        Value::String("job-filesystem-1".to_string())
    );

    fs::remove_file(&state_path).expect("cleanup filesystem background state");
}

#[test]
fn background_state_operation_compacts_terminal_jobs_over_capacity() {
    let state_path = temp_json_path("background-state-capacity");
    for (job_id, status) in [
        ("job-1", "completed"),
        ("job-2", "failed"),
        ("job-3", "queued"),
    ] {
        handle_background_state_operation(json!({
            "schema_version": "router-rs-background-state-request-v1",
            "operation": "apply_mutation",
            "state_path": state_path.display().to_string(),
            "backend_family": "filesystem",
            "job_id": job_id,
            "mutation": {"status": status}
        }))
        .expect("write background state fixture");
    }

    let response = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "snapshot",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "capacity_limit": 2
    }))
    .expect("capacity-compacted snapshot");
    let jobs = response["state"]["jobs"].as_array().expect("jobs");
    assert_eq!(jobs.len(), 2);
    assert!(
        jobs.iter()
            .any(|job| job["job_id"] == Value::String("job-3".to_string()))
    );
    assert_eq!(response["health"]["max_background_jobs"], json!(16));
    assert_eq!(response["health"]["max_background_jobs_limit"], json!(64));

    fs::remove_file(&state_path).expect("cleanup capacity background state");
}

#[test]
fn background_state_operation_reports_sqlite_backend_capabilities() {
    let temp_dir = temp_json_path("background-state-sqlite-root")
        .parent()
        .expect("temp root parent")
        .join(format!(
            "router-rs-bg-sqlite-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
    fs::create_dir_all(&temp_dir).expect("create sqlite temp dir");
    let canonical_temp_dir = temp_dir
        .canonicalize()
        .expect("canonicalize sqlite temp dir");
    let state_path = canonical_temp_dir.join("runtime_background_jobs.json");
    let sqlite_db_path = canonical_temp_dir.join("runtime_background_jobs.sqlite");

    let response = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "sqlite",
        "sqlite_db_path": sqlite_db_path.display().to_string(),
        "control_plane_descriptor": {
            "schema_version": "router-rs-runtime-control-plane-v1",
            "authority": "rust-runtime-control-plane",
            "services": {
                "state": {
                    "authority": "rust-runtime-control-plane",
                    "role": "durable-background-state",
                    "projection": "rust-native-projection",
                    "delegate_kind": "filesystem-state-store"
                }
            }
        },
        "job_id": "job-sqlite-1",
        "mutation": {
            "status": "completed",
            "session_id": "session-sqlite-1"
        }
    }))
    .expect("sqlite background state response");

    assert_eq!(
        response["health"]["control_plane_delegate_kind"],
        Value::String("sqlite-state-store".to_string())
    );
    assert_eq!(
        response["health"]["backend_family"],
        Value::String("sqlite".to_string())
    );
    assert_eq!(
        response["health"]["supports_atomic_replace"],
        Value::Bool(true)
    );
    assert_eq!(response["health"]["supports_compaction"], Value::Bool(true));
    assert_eq!(
        response["health"]["supports_snapshot_delta"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_remote_event_transport"],
        Value::Bool(true)
    );
    assert_eq!(
        response["health"]["supports_consistent_append"],
        Value::Bool(true)
    );
    assert_eq!(response["health"]["supports_sqlite_wal"], Value::Bool(true));
    assert!(!state_path.exists());
    assert!(sqlite_db_path.exists());

    let recovered = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "snapshot",
        "state_path": state_path.display().to_string(),
        "backend_family": "sqlite",
        "sqlite_db_path": sqlite_db_path.display().to_string()
    }))
    .expect("recovered sqlite background state snapshot");
    assert_eq!(
        recovered["state"]["jobs"][0]["job_id"],
        Value::String("job-sqlite-1".to_string())
    );
    assert_eq!(
        recovered["health"]["control_plane_delegate_kind"],
        Value::String("sqlite-state-store".to_string())
    );

    fs::remove_dir_all(&canonical_temp_dir).expect("cleanup sqlite background state dir");
}

#[test]
fn background_state_arbitration_dispatch_requires_explicit_operation() {
    let state_path = temp_json_path("background-state-arbitration-dispatch");

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "job_id": "job-1",
        "mutation": {
            "status": "running",
            "session_id": "shared-session"
        }
    }))
    .expect("seed active owner");

    let reserved = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "arbitrate_session_takeover",
        "arbitration_operation": "reserve",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("dispatch reserve arbitration");
    assert_eq!(reserved["takeover"]["operation"], json!("reserve"));
    assert_eq!(reserved["takeover"]["outcome"], json!("pending"));

    let missing = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "arbitrate_session_takeover",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-3"
    }))
    .expect_err("missing arbitration operation should fail closed");
    assert!(
        missing.to_string().contains("missing arbitration_operation"),
        "expected arbitration operation error, got: {missing}"
    );

    fs::remove_file(&state_path).expect("cleanup arbitration dispatch state");
}

#[test]
fn background_state_operation_arbitrates_takeover_across_persisted_roundtrip() {
    let state_path = temp_json_path("background-state-takeover");
    let control_plane_descriptor = json!({
        "schema_version": "router-rs-runtime-control-plane-v1",
        "authority": "rust-runtime-control-plane",
        "services": {
            "state": {
                "authority": "rust-runtime-control-plane",
                "role": "durable-background-state",
                "projection": "rust-native-projection",
                "delegate_kind": "filesystem-state-store"
            }
        }
    });

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "control_plane_descriptor": control_plane_descriptor,
        "job_id": "job-1",
        "mutation": {
            "status": "running",
            "session_id": "shared-session",
            "claimed_by": "job-1"
        }
    }))
    .expect("seed active owner");

    let reserved = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "reserve",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("reserve takeover");
    assert_eq!(
        reserved["takeover"]["outcome"],
        Value::String("pending".to_string())
    );
    assert_eq!(reserved["takeover"]["changed"], Value::Bool(true));
    assert_eq!(
        reserved["takeover"]["previous_active_job_id"],
        Value::String("job-1".to_string())
    );
    assert_eq!(
        reserved["takeover"]["pending_job_id"],
        Value::String("job-2".to_string())
    );
    assert_eq!(reserved["health"]["pending_session_takeovers"], json!(1));

    let completed = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "job_id": "job-1",
        "mutation": {
            "status": "completed",
            "session_id": "shared-session",
            "claimed_by": "job-1"
        }
    }))
    .expect("complete previous owner");
    assert_eq!(completed["health"]["active_job_count"], json!(0));

    let claimed = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "claim",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("claim takeover");
    assert_eq!(
        claimed["takeover"]["outcome"],
        Value::String("claimed".to_string())
    );
    assert_eq!(claimed["takeover"]["changed"], Value::Bool(true));
    assert_eq!(
        claimed["takeover"]["active_job_id"],
        Value::String("job-2".to_string())
    );
    assert_eq!(claimed["takeover"]["pending_job_id"], Value::Null);
    assert_eq!(claimed["health"]["pending_session_takeovers"], json!(0));

    let active = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "get_active_job",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session"
    }))
    .expect("get active job after claim");
    assert_eq!(active["active_job_id"], Value::String("job-2".to_string()));

    let persisted = read_json(&state_path).expect("read persisted takeover state");
    assert_eq!(persisted["pending_session_takeovers"], Value::Array(vec![]));
    assert_eq!(
        persisted["active_sessions"],
        Value::Array(vec![json!({
            "session_id": "shared-session",
            "job_id": "job-2"
        })])
    );

    fs::remove_file(&state_path).expect("cleanup takeover background state");
}

#[test]
fn background_state_operation_release_keeps_current_owner_when_only_pending_takeover_exists() {
    let state_path = temp_json_path("background-state-release");

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "apply_mutation",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "job_id": "job-1",
        "mutation": {
            "status": "running",
            "session_id": "shared-session"
        }
    }))
    .expect("seed release owner");

    handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "reserve",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("seed pending takeover");

    let released = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "release",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session",
        "incoming_job_id": "job-2"
    }))
    .expect("release pending takeover");
    assert_eq!(
        released["takeover"]["outcome"],
        Value::String("released".to_string())
    );
    assert_eq!(released["takeover"]["changed"], Value::Bool(true));
    assert_eq!(
        released["takeover"]["active_job_id"],
        Value::String("job-1".to_string())
    );
    assert_eq!(released["takeover"]["pending_job_id"], Value::Null);
    assert_eq!(released["health"]["pending_session_takeovers"], json!(0));

    let active = handle_background_state_operation(json!({
        "schema_version": "router-rs-background-state-request-v1",
        "operation": "get_active_job",
        "state_path": state_path.display().to_string(),
        "backend_family": "filesystem",
        "session_id": "shared-session"
    }))
    .expect("get active job after release");
    assert_eq!(active["active_job_id"], Value::String("job-1".to_string()));

    fs::remove_file(&state_path).expect("cleanup release background state");
}
