use super::common::*;
use super::*;

use serde_json::{json, Value};

fn write_framework_alias_registry_fixture(repo_root: &Path) {
    let registry_dir = repo_root.join("configs").join("framework");
    fs::create_dir_all(&registry_dir).expect("create registry dir");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../configs/framework/RUNTIME_REGISTRY.json");
    fs::copy(source, registry_dir.join("RUNTIME_REGISTRY.json")).expect("copy runtime registry");
}




#[test]
fn framework_alias_builds_compact_deepinterview_payload() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-deepinterview-alias-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("active-bootstrap-repair-20260418210000");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
        task_root.join("TRACE_METADATA.json"),
        r#"{"task":"active bootstrap repair","matched_skills":["deepinterview"]}"#,
    )
    .expect("write trace metadata");
    write_framework_alias_registry_fixture(&repo_root);
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
    )
    .expect("write active task");
    fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

    let payload = build_framework_alias_envelope(
        &repo_root,
        "deepinterview",
        FrameworkAliasBuildOptions {
            max_lines: 5,
            compact: false,
            host_id: None,
        },
    )
    .expect("build alias payload");
    let alias = payload
        .get("alias")
        .and_then(Value::as_object)
        .expect("alias payload");
    let prompt = alias
        .get("entry_prompt")
        .and_then(Value::as_str)
        .expect("entry prompt");

    assert_eq!(
        payload["schema_version"],
        json!(FRAMEWORK_ALIAS_SCHEMA_VERSION)
    );
    assert_eq!(alias["name"], json!("deepinterview"));
    assert_eq!(alias["host_entrypoint"], json!("/deepinterview"));
    assert_eq!(alias["compact"], json!(false));
    assert_eq!(alias["canonical_owner"], json!("deepinterview"));
    assert_eq!(
        alias["entry_contract"]["route_rules"][0],
        json!("主 owner -> `deepinterview`")
    );
    assert!(prompt.contains("进入 deepinterview"));
    assert!(prompt.contains("每轮只问一个问题"));
    assert!(prompt.contains("review lanes ->"));

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_alias_fails_closed_for_missing_alias_record() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-retired-alias-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let registry_dir = repo_root.join("configs").join("framework");
    fs::create_dir_all(&registry_dir).expect("create registry dir");
    fs::write(
        registry_dir.join("RUNTIME_REGISTRY.json"),
        r#"{"schema_version":"framework-runtime-registry-v2","framework_commands":{"gitx":{"canonical_owner":"gitx","skill_path":"skills/gitx/SKILL.md"}}}"#,
    )
    .expect("write registry");

    let err = build_framework_alias_envelope(
        &repo_root,
        "team",
        FrameworkAliasBuildOptions {
            max_lines: 3,
            compact: true,
            host_id: None,
        },
    )
    .expect_err("missing alias should fail closed");
    assert!(err.contains("Unknown framework alias `team`"));

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_alias_compact_payload_omits_duplicate_prompt_fields() {
    let repo_root = std::env::temp_dir().join(format!(
        "router-rs-compact-alias-fixture-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let task_root = repo_root
        .join("artifacts")
        .join("current")
        .join("active-bootstrap-repair-20260418210000");
    fs::create_dir_all(&task_root).expect("create task root");
    fs::create_dir_all(repo_root.join("artifacts").join("current")).expect("create current root");
    fs::write(
        task_root.join("SESSION_SUMMARY.md"),
        "- task: active bootstrap repair\n- phase: implementation\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        task_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Patch classifier","Run MCP regression tests"]}"#,
    )
    .expect("write next actions");
    fs::write(task_root.join("EVIDENCE_INDEX.json"), r#"{"artifacts":[]}"#)
        .expect("write evidence index");
    fs::write(
        task_root.join("TRACE_METADATA.json"),
        r#"{"task":"active bootstrap repair","matched_skills":["gitx"]}"#,
    )
    .expect("write trace metadata");
    write_framework_alias_registry_fixture(&repo_root);
    fs::write(
        repo_root
            .join("artifacts")
            .join("current")
            .join("active_task.json"),
        r#"{"task_id":"active-bootstrap-repair-20260418210000","task":"active bootstrap repair"}"#,
    )
    .expect("write active task");
    fs::write(
            repo_root.join(".supervisor_state.json"),
            r#"{
                "task_id":"active-bootstrap-repair-20260418210000",
                "task_summary":"active bootstrap repair",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true},
                "execution_contract":{"acceptance_criteria":["completed tasks never appear as current execution"]}
            }"#,
        )
        .expect("write supervisor state");

    let payload = build_framework_alias_envelope(
        &repo_root,
        "gitx",
        FrameworkAliasBuildOptions {
            max_lines: 3,
            compact: true,
            host_id: None,
        },
    )
    .expect("build alias payload");
    let alias = payload
        .get("alias")
        .and_then(Value::as_object)
        .expect("alias payload");

    assert_eq!(alias["compact"], json!(true));
    assert!(alias.get("entry_prompt").is_none());
    assert!(alias.get("entry_prompt_token_estimate").is_none());
    assert!(alias.get("upstream_source").is_none());
    assert_eq!(alias["state_machine"]["evidence_missing"], json!(true));
    assert_eq!(
        alias["entry_contract"]["context"]["execution_readiness"],
        json!("use-alias-default")
    );
    assert_eq!(
        alias["state_machine"]["required_anchors"],
        json!([
            "SESSION_SUMMARY",
            "NEXT_ACTIONS",
            "TRACE_METADATA",
            "SUPERVISOR_STATE"
        ])
    );
    assert!(alias["state_machine"]["resume"].get("task").is_none());

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_snapshot_reconciles_stale_supervisor_against_current_pointers() {
    let repo_root = temp_dir_path("runtime-anchor-reconcile");
    let artifacts_root = repo_root.join("artifacts");
    let current_root = artifacts_root.join("current");
    let fresh_root = current_root.join("fresh-task");
    let stale_root = current_root.join("stale-task");
    fs::create_dir_all(&fresh_root).expect("create fresh task root");
    fs::create_dir_all(&stale_root).expect("create stale task root");
    write_text_fixture(
        &fresh_root.join("SESSION_SUMMARY.md"),
        "- task: fresh task\n- phase: implementation\n- status: in_progress\n",
    );
    write_text_fixture(
        &fresh_root.join("NEXT_ACTIONS.json"),
        r#"{"next_actions":["Continue fresh task"]}"#,
    );
    write_text_fixture(
        &fresh_root.join("EVIDENCE_INDEX.json"),
        r#"{"artifacts":[]}"#,
    );
    write_text_fixture(
        &fresh_root.join("TRACE_METADATA.json"),
        r#"{"task":"fresh task","matched_skills":["goal_drive"]}"#,
    );
    write_text_fixture(
        &stale_root.join("SESSION_SUMMARY.md"),
        "- task: stale task\n- phase: implementation\n- status: in_progress\n",
    );
    write_text_fixture(
        &current_root.join("active_task.json"),
        r#"{"task_id":"fresh-task"}"#,
    );
    write_text_fixture(
        &current_root.join("focus_task.json"),
        r#"{"task_id":"fresh-task"}"#,
    );
    write_text_fixture(
        &current_root.join("task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"fresh-task","tasks":[{"task_id":"fresh-task"}]}"#,
    );
    write_text_fixture(
        &repo_root.join(".supervisor_state.json"),
        r#"{
                "task_id":"stale-task",
                "task_summary":"stale task",
                "active_phase":"implementation",
                "verification":{"verification_status":"in_progress"},
                "continuity":{"story_state":"active","resume_allowed":true}
            }"#,
    );

    let payload =
        build_framework_runtime_snapshot_envelope_with_level(&repo_root, None, None, "summary").expect("build snapshot");
    let snapshot = &payload["runtime_snapshot"];
    assert_eq!(snapshot["active_task_id"], json!("fresh-task"));
    assert_eq!(snapshot["continuity"]["state"], json!("inconsistent"));
    let reasons = snapshot["continuity"]["inconsistency_reasons"]
        .as_array()
        .expect("inconsistency reasons")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(reasons
        .iter()
        .any(|reason| reason.contains("supervisor task_id 'stale-task' disagrees")));

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_runtime_snapshot_surfaces_invalid_task_registry_json() {
    let repo_root = temp_dir_path("framework-runtime-invalid-registry");
    let current_root = repo_root.join("artifacts/current");
    fs::create_dir_all(&current_root).unwrap();
    fs::write(current_root.join("task_registry.json"), "{truncated").unwrap();
    let payload =
        build_framework_runtime_snapshot_envelope_with_level(&repo_root, None, None, "summary").expect("snapshot");
    let reasons = payload["runtime_snapshot"]["control_plane_inconsistency_reasons"]
        .as_array()
        .expect("control plane reasons");
    let joined = reasons
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(joined.contains("invalid control-plane json"));
    assert!(joined.contains("task_registry.json"));
    let _ = fs::remove_dir_all(&repo_root);
}
