use super::common::*;
use super::*;

use serde_json::{json, Value};


#[test]
fn framework_statusline_uses_rust_runtime_view() {
    let repo_root = temp_dir_path("framework-statusline");
    let task_id = "statusline-task-20260424120000";
    let task_root = repo_root.join("artifacts").join("current").join(task_id);
    write_text_fixture(
            &task_root.join("SESSION_SUMMARY.md"),
            "# SESSION_SUMMARY\n\n- task: Validate status line\n- phase: integration\n- status: in_progress\n",
        );
    write_text_fixture(
        &task_root.join("NEXT_ACTIONS.json"),
        &json!({"next_actions": ["Ship it"]}).to_string(),
    );
    write_text_fixture(
        &task_root.join("EVIDENCE_INDEX.json"),
        &json!({"artifacts": []}).to_string(),
    );
    write_text_fixture(
        &task_root.join("TRACE_METADATA.json"),
        &json!({"matched_skills": ["goal_drive", "skill-framework-developer"]}).to_string(),
    );
    write_text_fixture(
        &repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        &json!({"task_id": task_id, "task": "Validate status line"}).to_string(),
    );
    write_text_fixture(
        &repo_root
            .join("artifacts")
            .join("current")
            .join("focus_task.json"),
        &json!({"task_id": task_id, "task": "Validate status line"}).to_string(),
    );
    write_text_fixture(
        &repo_root
            .join("artifacts")
            .join("current")
            .join("task_registry.json"),
        &json!({
            "schema_version": "task-registry-v1",
            "focus_task_id": task_id,
            "tasks": [
                {
                    "task_id": task_id,
                    "task": "Validate status line",
                    "phase": "integration",
                    "status": "in_progress",
                    "resume_allowed": true
                }
            ]
        })
        .to_string(),
    );
    write_text_fixture(
        &repo_root.join(".supervisor_state.json"),
        &json!({
            "task_id": task_id,
            "task_summary": "Validate status line",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": true}
        })
        .to_string(),
    );

    let statusline = build_framework_statusline(&repo_root).expect("build statusline");

    assert!(statusline.contains("task=Validate status line"));
    assert!(statusline.contains("next=NEXT_ACTIONS"));
    assert!(statusline.contains("integration/in_progress"));
    assert!(statusline.contains("route=goal_drive+1"));
    assert!(statusline.contains("others=0"));
    assert!(statusline.contains("resumable=0"));
    assert!(
        statusline.contains("depth=d0 | "),
        "statusline should surface depth rollup; got {statusline:?}"
    );
    assert!(statusline.contains("git=nogit"));
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_snapshot_missing_recovery_anchors_is_not_resumable() {
    let repo_root = temp_dir_path("framework-missing-recovery-anchors");
    let current_root = repo_root.join("artifacts").join("current");
    write_text_fixture(
        &current_root.join("EVIDENCE_INDEX.json"),
        &json!({"artifacts": []}).to_string(),
    );

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
    let continuity = &snapshot["runtime_snapshot"]["continuity"];
    let missing_anchors = continuity["missing_recovery_anchors"]
        .as_array()
        .expect("missing anchors array");

    assert_eq!(continuity["state"], json!("missing"));
    assert_eq!(continuity["can_resume"], json!(false));
    assert_eq!(continuity["current_execution"], Value::Null);
    assert!(missing_anchors.contains(&json!("SESSION_SUMMARY")));
    assert!(missing_anchors.contains(&json!("NEXT_ACTIONS")));
    assert!(missing_anchors.contains(&json!("TRACE_METADATA")));

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_writer_materializes_complete_focus_continuity() {
    let _env = crate::test_env_sync::process_env_lock();
    let repo_root = temp_dir_path("framework-session-writer-continuity");
    let output_dir = repo_root.join("artifacts").join("current");
    let payload = json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "continuity-polish-20260424120000",
        "task": "continuity polish",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Make Rust continuity recoverable without manual mirror repair.",
        "focus": true,
        "next_actions": ["Run targeted tests"],
        "evidence": [],
        "matched_skills": ["goal_drive"],
        "execution_contract": {
            "goal": "Improve continuity artifacts",
            "acceptance_criteria": ["writer emits all recovery anchors"]
        },
        "blockers": ["none"]
    });

    let result = write_framework_session_artifacts(payload).expect("write artifacts");
    let task_id = result["task_id"].as_str().expect("task id");
    let task_root = repo_root.join("artifacts").join("current").join(task_id);

    for path in [
        task_root.join("SESSION_SUMMARY.md"),
        task_root.join("NEXT_ACTIONS.json"),
        task_root.join("EVIDENCE_INDEX.json"),
        task_root.join("TRACE_METADATA.json"),
        repo_root.join(".supervisor_state.json"),
        repo_root.join("artifacts/current/active_task.json"),
        repo_root.join("artifacts/current/focus_task.json"),
        repo_root.join("artifacts/current/task_registry.json"),
    ] {
        assert!(path.is_file(), "missing {}", path.display());
    }

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
    let runtime = &snapshot["runtime_snapshot"];
    assert_eq!(runtime["active_task_id"], json!(task_id));
    assert_eq!(runtime["continuity"]["state"], json!("active"));
    assert_eq!(runtime["continuity"]["can_resume"], json!(true));
    assert_eq!(runtime["continuity"]["missing_recovery_anchors"], json!([]));

    let supervisor = serde_json::from_str::<Value>(
        &fs::read_to_string(repo_root.join(".supervisor_state.json")).expect("read supervisor"),
    )
    .expect("parse supervisor");
    assert_eq!(supervisor["continuity"]["resume_allowed"], json!(true));
    assert_eq!(
        supervisor["verification"]["verification_status"],
        json!("in_progress")
    );
    assert_eq!(
        supervisor["trace_metadata"]["matched_skills"],
        json!(["goal_drive"])
    );
    assert_eq!(
        supervisor["artifact_refs"]["task_root"],
        json!(task_root.display().to_string())
    );
    let active_pointer = serde_json::from_str::<Value>(
        &fs::read_to_string(repo_root.join("artifacts/current/active_task.json"))
            .expect("read active pointer"),
    )
    .expect("parse active pointer");
    assert_eq!(
        active_pointer["session_summary"],
        json!(task_root.join("SESSION_SUMMARY.md").display().to_string())
    );
    for path in [
        repo_root.join("SESSION_SUMMARY.md"),
        repo_root.join("NEXT_ACTIONS.json"),
        repo_root.join("EVIDENCE_INDEX.json"),
        repo_root.join("TRACE_METADATA.json"),
        repo_root.join("artifacts/current/SESSION_SUMMARY.md"),
        repo_root.join("artifacts/current/NEXT_ACTIONS.json"),
        repo_root.join("artifacts/current/EVIDENCE_INDEX.json"),
        repo_root.join("artifacts/current/TRACE_METADATA.json"),
    ] {
        assert!(!path.exists(), "unexpected mirror {}", path.display());
    }
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn continuity_audit_flags_ephemeral_registry_row() {
    let repo_root = temp_dir_path("continuity-audit-ephemeral");
    let current = repo_root.join("artifacts/current");
    fs::create_dir_all(&current).expect("mkdir");
    fs::write(
        current.join("task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"real","tasks":[{"task_id":"cursor-stop-999T0000000000"},{"task_id":"real"}]}"#,
    )
    .expect("registry");
    fs::write(current.join("focus_task.json"), r#"{"task_id":"real"}"#).expect("focus");

    let report = run_continuity_audit(&repo_root).expect("audit");
    let warnings = report["warnings"].as_array().expect("warnings");
    let joined = warnings
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("EPHEMERAL CHECKPOINT ROW"),
        "expected ephemeral warning; report={report}"
    );

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn resolve_task_view_notes_active_goal_missing_on_focus() {
    use crate::task_state::RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL;

    let repo_root = temp_dir_path("goal-mismatch-note");
    let current = repo_root.join("artifacts/current");
    fs::create_dir_all(current.join("active-task")).expect("active dir");
    fs::create_dir_all(current.join("focus-task")).expect("focus dir");
    fs::write(
        current.join("active_task.json"),
        r#"{"task_id":"active-task"}"#,
    )
    .expect("active");
    fs::write(
        current.join("focus_task.json"),
        r#"{"task_id":"focus-task"}"#,
    )
    .expect("focus");
    fs::write(
        current.join("focus-task/GOAL_STATE.json"),
        r#"{"schema_version":1,"task_id":"focus-task","status":"in_progress","goal":"x","non_goals":[],"done_when":["d"],"validation_commands":["c"],"drive_until_done":false}"#,
    )
    .expect("focus goal");

    let view = resolve_task_view(&repo_root, None);
    assert_eq!(view.task_id.as_deref(), Some("active-task"));
    let notes = view.resolution_notes.join(" ");
    assert!(
        notes.contains(RESOLUTION_NOTE_ACTIVE_GOAL_MISSING_FOCUS_HAS_GOAL),
        "notes={notes}"
    );

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn runtime_view_active_task_id_matches_resolve_task_view() {
    let repo_root = temp_dir_path("runtime-view-task-id-align");
    let current = repo_root.join("artifacts/current");
    fs::create_dir_all(current.join("active-task")).expect("active dir");
    fs::create_dir_all(current.join("focus-only-task")).expect("focus dir");
    fs::write(
        current.join("active_task.json"),
        r#"{"task_id":"active-task"}"#,
    )
    .expect("write active");
    fs::write(
        current.join("focus_task.json"),
        r#"{"task_id":"focus-only-task"}"#,
    )
    .expect("write focus");
    fs::write(
        current.join("task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"focus-only-task","tasks":[{"task_id":"active-task"},{"task_id":"focus-only-task"}]}"#,
    )
    .expect("write registry");

    let snapshot =
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
    let resolved = resolve_task_view(&repo_root, None);
    assert_eq!(
        snapshot["runtime_snapshot"]["active_task_id"],
        json!("active-task")
    );
    assert_eq!(resolved.task_id.as_deref(), Some("active-task"));

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn stdio_framework_goal_drive_roundtrip() {
    let repo_root = temp_dir_path("stdio-goal-drive");
    let _ = fs::remove_dir_all(&repo_root);
    let output_dir = repo_root.join("artifacts").join("current");
    write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "ag-stdio-task",
        "task": "goal drive stdio",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("seed session");

    let rr = repo_root.display().to_string();
    let req = json!({
        "id": "ag-1",
        "op": "framework_goal_drive",
        "payload": {
            "repo_root": rr,
            "operation": "start",
            "task_id": "ag-stdio-task",
            "goal": "finish macro task",
            "non_goals": ["avoid scope creep"],
            "done_when": ["ci green", "review checklist cleared"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true
        }
    });
    let line = serde_json::to_string(&req).expect("serialize stdio line");
    let response = handle_stdio_json_line(&line);
    assert!(response.ok, "{:?}", response.error);
    let body = response.payload.expect("payload");
    assert_eq!(body["ok"], json!(true));

    let path = repo_root.join("artifacts/current/ag-stdio-task/GOAL_STATE.json");
    assert!(path.is_file(), "missing {}", path.display());

    let _ = fs::remove_dir_all(&repo_root);
}


use runtime_core::qg_route::init_qg_route;
#[test]
fn stdio_framework_quality_gate_roundtrip() {
    init_qg_route();
    let repo_root = temp_dir_path("stdio-rfv-loop");
    let _ = fs::remove_dir_all(&repo_root);
    let output_dir = repo_root.join("artifacts").join("current");
    write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "rfv-stdio-task",
        "task": "rfv stdio",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("seed session");

    let rr = repo_root.display().to_string();
    let start = json!({
        "id": "rfv-1",
        "op": "framework_quality_gate",
        "payload": {
            "repo_root": rr,
            "operation": "start",
            "task_id": "rfv-stdio-task",
            "goal": "deepen RFV",
        }
    });
    let line = serde_json::to_string(&start).expect("serialize");
    let response = handle_stdio_json_line(&line);
    assert!(response.ok, "{:?}", response.error);
    let body = response.payload.expect("payload");
    // Old QG state machine deleted (Wave 4a-ii). Now returns GateVerdict from QG Route.
    assert!(body.get("passed").is_some(), "GateVerdict should have passed field");
    assert!(body.get("checkers_ran").is_some(), "GateVerdict should have checkers_ran");

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn runtime_control_plane_payload_is_rust_owned() {
    let payload = build_runtime_control_plane_payload();

    assert_eq!(
        payload["schema_version"],
        Value::String(RUNTIME_CONTROL_PLANE_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["default_route_mode"],
        Value::String("rust".to_string())
    );
    assert_eq!(
        payload["default_route_authority"],
        Value::String(ROUTE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["runtime_status"]["runtime_primary_owner"],
        Value::String("rust-control-plane".to_string())
    );
    assert_eq!(
        payload["runtime_status"]["hot_path_projection_mode"],
        Value::String("descriptor-driven".to_string())
    );
    assert!(payload["runtime_status"]
        .get("framework_runtime_package_status")
        .is_none());
    assert_eq!(
        payload["runtime_status"]["framework_runtime_replacement"],
        Value::String("router-rs::framework_runtime".to_string())
    );
    assert_eq!(
        payload["runtime_host"]["role"],
        Value::String("runtime-orchestration".to_string())
    );
    assert_eq!(
        payload["runtime_host"]["startup_order"][0],
        Value::String("router".to_string())
    );
    assert_eq!(
        payload["runtime_host"]["concurrency_contract"]["router_stdio_pool_default_size"],
        json!(DEFAULT_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["runtime_host"]["concurrency_contract"]["router_stdio_pool_max_size"],
        json!(MAX_ROUTER_STDIO_POOL_SIZE)
    );
    assert_eq!(
        payload["services"]["middleware"]["subagent_limit_contract"]
            ["max_concurrent_subagents_limit"],
        json!(framework_kernel::stdio_payload_types::MAX_CONCURRENT_SUBAGENTS_LIMIT_LOCAL)
    );
    assert_eq!(
        payload["runtime_host"]["shutdown_order"][0],
        Value::String("background".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["delegate_kind"],
        Value::String("rust-execution-kernel-slice".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_live_backend_impl"],
        Value::String("router-rs".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]["execution_kernel_delegate_impl"],
        Value::String("router-rs".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]
            ["execution_kernel_metadata_schema_version"],
        Value::String(EXECUTION_METADATA_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]["execution_kernel_fallback_policy"],
        Value::String(EXECUTION_KERNEL_FALLBACK_POLICY.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract"]["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract_by_mode"]
            [EXECUTION_RESPONSE_SHAPE_DRY_RUN]["execution_kernel_response_shape"],
        Value::String(EXECUTION_RESPONSE_SHAPE_DRY_RUN.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_contract_by_mode"]
            [EXECUTION_RESPONSE_SHAPE_DRY_RUN]["execution_kernel_prompt_preview_owner"],
        Value::String(EXECUTION_PROMPT_PREVIEW_OWNER.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["schema_version"],
        Value::String(EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["authority"],
        Value::String(EXECUTION_KERNEL_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["runtime_fields"]
            ["live_primary_required"][2],
        Value::String("execution_mode".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["runtime_fields"]
            ["live_primary_passthrough"][1],
        Value::String("diagnostic_route_mode".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_metadata_contract"]["defaults"]
            ["live_primary_model_id_source"],
        Value::String(EXECUTION_MODEL_ID_SOURCE.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["kernel_live_delegate_authority"],
        Value::String("rust-execution-cli".to_string())
    );
    assert_eq!(
        payload["services"]["checkpoint"]["delegate_kind"],
        Value::String("filesystem-checkpointer".to_string())
    );
    assert_eq!(
        payload["services"]["checkpoint"]["backend_family_catalog"]
            ["strongest_local_backend_family"],
        Value::String("sqlite".to_string())
    );
    assert!(
        !payload["services"]["checkpoint"]["backend_family_catalog"]["families"]
            .as_array()
            .expect("backend family catalog")
            .iter()
            .any(|family| family["backend_family"] == "memory")
    );
    assert_eq!(
        payload["services"]["checkpoint"]["backend_family_catalog"]["test_only_backend_families"]
            [0],
        Value::String("memory".to_string())
    );
    assert_eq!(
        payload["services"]["checkpoint"]["backend_family_parity"]["aligned"],
        Value::Bool(true)
    );
    assert_eq!(
        payload["services"]["background"]["authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["services"]["background"]["delegate_kind"],
        Value::String("rust-background-control-policy".to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["policy_schema_version"],
        Value::String(BACKGROUND_CONTROL_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["active_statuses"][4],
        Value::String("retry_claimed".to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["policy_operations"][0],
        Value::String("batch-plan".to_string())
    );
    assert_eq!(
        payload["services"]["background"]["orchestration_contract"]["policy_operations"][5],
        Value::String("retry".to_string())
    );
    assert!(payload["services"].get("agent_factory").is_none());
}


#[test]
fn runtime_observability_exporter_descriptor_is_rust_owned() {
    let payload = build_runtime_observability_exporter_descriptor();

    assert_eq!(
        payload["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["metric_catalog_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
    );
    assert_eq!(
        payload["dashboard_schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["producer_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["exporter_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
    assert_eq!(
        payload["export_path"],
        Value::String("jsonl-plus-otel".to_string())
    );
}


#[test]
fn runtime_observability_dashboard_and_metric_record_follow_contract() {
    let catalog = build_runtime_observability_metric_catalog_payload();
    let metrics = catalog["metrics"].as_array().expect("metric catalog array");
    assert_eq!(
        catalog["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        catalog["metric_catalog_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
    );
    assert!(metrics
        .iter()
        .all(|metric| metric.get("dimensions").is_some()));
    assert!(metrics
        .iter()
        .all(|metric| metric.get("base_dimensions").is_none()));

    let dashboard = runtime_observability_dashboard_schema();
    let resource_dimensions = dashboard["resource_dimensions"]
        .as_array()
        .expect("resource dimensions array");
    assert_eq!(
        dashboard["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
    );
    assert!(resource_dimensions
        .iter()
        .any(|value| value == "service.name"));
    assert!(resource_dimensions
        .iter()
        .any(|value| value == "runtime.generation"));
    assert!(resource_dimensions
        .iter()
        .any(|value| value == "runtime.schema_version"));

    let record = build_runtime_metric_record(json!({
        "metric_name": "runtime.route_mismatch_total",
        "value": 3,
        "service_name": "codex-runtime",
        "service_version": "v1",
        "runtime_instance_id": "runtime-123",
        "route_engine_mode": "rust",
        "job_id": "job-1",
        "session_id": "session-1",
        "attempt": 2,
        "worker_id": "worker-7",
        "generation": "gen-a",
    }))
    .expect("metric record");
    assert_eq!(
        record["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(record["metric_type"], Value::String("counter".to_string()));
    assert_eq!(record["unit"], Value::String("1".to_string()));
    assert_eq!(
        record["dimensions"]["runtime.stage"],
        Value::String("runtime.metric".to_string())
    );
    assert_eq!(
        record["dimensions"]["runtime.status"],
        Value::String("ok".to_string())
    );
    assert_eq!(
        record["dimensions"]["runtime.schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        record["ownership"]["exporter_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );

    let err = build_runtime_metric_record(json!({
        "metric_name": "runtime.unknown_total",
        "value": 1,
        "service_name": "codex-runtime",
        "service_version": "v1",
        "runtime_instance_id": "runtime-123",
        "route_engine_mode": "rust",
        "job_id": "job-1",
        "session_id": "session-1",
        "attempt": 1,
        "worker_id": "worker-7",
        "generation": "gen-a",
    }))
    .expect_err("unknown metric should fail closed");
    assert_eq!(err, "unsupported runtime metric: runtime.unknown_total");

    let err = build_runtime_metric_record(json!({
        "metric_name": "runtime.route_mismatch_total",
        "value": 1,
        "service_name": "codex-runtime",
        "service_version": "v1",
        "runtime_instance_id": "runtime-123",
        "route_engine_mode": "rust",
        "job_id": "job-1",
        "session_id": "session-1",
        "attempt": -1,
        "worker_id": "worker-7",
        "generation": "gen-a",
    }))
    .expect_err("negative attempts should fail closed");
    assert_eq!(
        err,
        "runtime metric record requires non-negative integer field attempt"
    );
}


#[test]
fn runtime_observability_health_snapshot_is_rust_owned() {
    let payload = build_runtime_observability_health_snapshot();

    assert_eq!(
        payload["schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["metric_catalog_schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["dashboard_schema_version"],
        Value::String(RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["metric_catalog_version"],
        Value::String(RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION.to_string())
    );
    assert_eq!(
        payload["dashboard_panel_count"],
        Value::Number(serde_json::Number::from(6))
    );
    assert_eq!(
        payload["dashboard_alert_count"],
        Value::Number(serde_json::Number::from(3))
    );
    let metric_names = payload["metric_names"].as_array().expect("metric names");
    assert_eq!(metric_names.len(), 6);
    assert!(metric_names
        .iter()
        .any(|value| value == "runtime.route_mismatch_total"));
    assert_eq!(
        payload["exporter"]["exporter_authority"],
        Value::String(RUNTIME_CONTROL_PLANE_AUTHORITY.to_string())
    );
}


#[test]
fn write_text_payload_uses_unique_temp_paths_under_concurrency() {
    let output_path = temp_json_path("atomic-write-concurrent");
    let mut workers = Vec::new();
    for index in 0..32 {
        let path = output_path.clone();
        workers.push(spawn(move || {
            write_text_payload(&path, &format!("payload-{index}"))
                .expect("concurrent atomic write");
        }));
    }
    for worker in workers {
        worker.join().expect("join writer");
    }

    let persisted = fs::read_to_string(&output_path).expect("read final payload");
    assert!(persisted.starts_with("payload-"));
    let tmp_entries = fs::read_dir(output_path.parent().expect("output parent"))
        .expect("read temp dir")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_name().to_string_lossy().starts_with(
                output_path
                    .file_name()
                    .expect("file name")
                    .to_string_lossy()
                    .as_ref(),
            ) && entry.file_name().to_string_lossy().ends_with(".tmp")
        })
        .count();
    assert_eq!(tmp_entries, 0);

    fs::remove_file(&output_path).expect("cleanup concurrent write output");
}

#[test]
fn framework_snapshot_summary_mode_is_smaller_than_full() {
use framework_extra::snapshot::build_framework_runtime_snapshot_envelope_with_level;

    let repo_root = temp_dir_path("framework-snapshot-detail-level");
    let task_id = "detail-level-task";
    let task_root = repo_root.join("artifacts").join("current").join(task_id);
    write_text_fixture(
        &task_root.join("SESSION_SUMMARY.md"),
        "- task: detail level test\n- phase: implementation\n- status: in_progress\n",
    );
    write_text_fixture(
        &task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Verify"]}"#,
    );
    write_text_fixture(
        &task_root.join("EVIDENCE_INDEX.json"),
        r#"{"artifacts":[]}"#,
    );
    write_text_fixture(
        &task_root.join("TRACE_METADATA.json"),
        r#"{"task":"detail level test","matched_skills":["goal_drive"]}"#,
    );
    fs::create_dir_all(repo_root.join("artifacts/current")).expect("mkdir");
    write_text_fixture(
        &repo_root.join("artifacts/current/task_registry.json"),
        &json!({
            "schema_version": "task-registry-v1",
            "focus_task_id": task_id,
            "tasks": [
                {"task_id": "detail-level-task", "task": "detail level test", "status": "in_progress", "updated_at": "2026-06-12T10:00:00+08:00"},
                {"task_id": "other-task-1", "task": "other task 1", "status": "completed", "updated_at": "2026-06-11T10:00:00+08:00"},
                {"task_id": "other-task-2", "task": "other task 2", "status": "completed", "updated_at": "2026-06-10T10:00:00+08:00"},
                {"task_id": "other-task-3", "task": "other task 3 that has a very long description which should be truncated in summary mode to save tokens", "status": "completed", "updated_at": "2026-06-09T10:00:00+08:00"},
            ]
        }).to_string(),
    );
    write_text_fixture(
        &repo_root.join(".supervisor_state.json"),
        &json!({
            "task_id": task_id,
            "task_summary": "detail level test",
            "active_phase": "implementation",
        })
        .to_string(),
    );

    let summary =
        build_framework_runtime_snapshot_envelope_with_level(&repo_root, None, None, "summary")
            .expect("summary snapshot");
    let full = build_framework_runtime_snapshot_envelope_with_level(&repo_root, None, None, "full")
        .expect("full snapshot");

    assert_eq!(summary["runtime_snapshot"]["_truncated"], json!(true));
    assert!(full["runtime_snapshot"].get("_truncated").is_none());

    assert_eq!(
        summary["runtime_snapshot"]["detail_level"],
        json!("summary")
    );
    assert_eq!(full["runtime_snapshot"]["detail_level"], json!("full"));

    let summary_ids = summary["runtime_snapshot"]["known_task_ids"]
        .as_array()
        .expect("known_task_ids array in summary");
    assert!(summary_ids.len() <= 3);

    let full_ids = full["runtime_snapshot"]["known_task_ids"]
        .as_array()
        .expect("known_task_ids array in full");
    assert_eq!(full_ids.len(), 4);

    assert!(
        summary["runtime_snapshot"]["paths"]
            .get("session_summary")
            .is_none()
    );
    assert!(
        full["runtime_snapshot"]["paths"]
            .get("session_summary")
            .is_some()
    );

    let summary_tasks = summary["runtime_snapshot"]["registered_tasks"]["tasks"]
        .as_array()
        .expect("registered_tasks.tasks array in summary");
    let long_task = summary_tasks
        .iter()
        .find(|t| t["task_id"] == "other-task-3")
        .expect("find long task");
    let task_desc = long_task["task"].as_str().expect("task description");
    assert!(task_desc.len() <= 80, "got {} chars", task_desc.len());

    assert!(
        full["runtime_snapshot"]["continuity"]
            .get("paths")
            .is_some()
    );
    assert!(
        summary["runtime_snapshot"]["continuity"]
            .get("paths")
            .is_none()
    );

    let summary_size = serde_json::to_string(&summary).unwrap().len();
    let full_size = serde_json::to_string(&full).unwrap().len();
    assert!(
        summary_size < full_size,
        "summary ({summary_size} bytes) should be smaller than full ({full_size} bytes)"
    );

    let _ = fs::remove_dir_all(&repo_root);
}


