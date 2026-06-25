use super::common::*;
use super::*;

use serde_json::{json, Value};

#[test]
fn framework_session_artifact_write_rejects_stale_focus_update() {
    let repo_root = temp_dir_path("framework-session-cas");
    let output_dir = repo_root.join("artifacts").join("current");
    let first = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-task",
        "task": "CAS task",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Initial write.",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("first write");
    assert_eq!(first["task_id"], json!("cas-task"));

    let focus_path = repo_root.join("artifacts/current/focus_task.json");
    let stale_hash = fr_utils::util::hash_file_for_test(&focus_path).expect("focus hash");
    write_text_fixture(
        &focus_path,
        r#"{"task_id":"other-task","task":"Other task","updated_at":"2026-04-25T00:00:00+08:00"}"#,
    );

    let err = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-task",
        "task": "CAS task",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Stale write.",
        "focus": true,
        "expected_focus_task_hash": stale_hash,
        "next_actions": ["Continue"]
    }))
    .expect_err("stale focus update should fail");
    assert!(err.contains("stale focus task pointer update rejected"));

    let focus = read_json(&focus_path).expect("read focus");
    assert_eq!(focus["task_id"], json!("other-task"));
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_preserves_existing_roundtrip() {
    let _lock = crate::test_env_sync::process_env_lock();
    let _strict = CloseoutStrictEnvGuard::new();
    let repo_root = temp_dir_path("framework-session-cas-roundtrip");
    let output_dir = repo_root.join("artifacts").join("current");
    let first = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-roundtrip",
        "task": "CAS roundtrip",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Initial write.",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("first write");
    assert_eq!(first["task_id"], json!("cas-roundtrip"));

    let active_path = repo_root.join("artifacts/current/active_task.json");
    let focus_path = repo_root.join("artifacts/current/focus_task.json");
    let supervisor_path = repo_root.join(".supervisor_state.json");
    let active_hash =
        fr_utils::util::hash_file_for_test(&active_path).expect("active hash");
    let focus_hash = fr_utils::util::hash_file_for_test(&focus_path).expect("focus hash");
    let supervisor_hash =
        fr_utils::util::hash_file_for_test(&supervisor_path).expect("supervisor hash");

    let second = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cas-roundtrip",
        "task": "CAS roundtrip",
        "phase": "validation",
        "status": "passed",
        "summary": "Validated write.",
        "focus": true,
        "expected_active_task_hash": active_hash,
        "expected_focus_task_hash": focus_hash,
        "expected_supervisor_state_hash": supervisor_hash,
        "next_actions": [],
        // Completion claims (status in CLOSEOUT_COMPLETION_STATUSES)
        // require a closeout record so closeout_enforcement can verify
        // evidence. Provide a minimal passed record here.
        "closeout_record": {
            "schema_version": "closeout-record-v1",
            "task_id": "cas-roundtrip",
            "verification_status": "passed",
            "summary": "Validated write.",
            "commands_run": [
                {"command": "cargo test --manifest-path core/router-rs/Cargo.toml", "exit_code": 0}
            ],
            "artifacts_checked": [
                {"path": "README.md", "exists": true}
            ]
        }
    }))
    .expect("roundtrip write");
    assert_eq!(second["task_id"], json!("cas-roundtrip"));
    assert_eq!(
        second["closeout_evaluation"]["closeout_allowed"],
        json!(true)
    );

    let supervisor = read_json(&supervisor_path).expect("read supervisor");
    assert_eq!(supervisor["active_phase"], json!("validation"));
    assert_eq!(
        supervisor["verification"]["verification_status"],
        json!("passed")
    );
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn framework_session_artifact_write_omitted_evidence_preserves_existing_file() {
    let repo_root = temp_dir_path("framework-session-preserve-evidence");
    let output_dir = repo_root.join("artifacts").join("current");
    write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "preserve-evidence",
        "task": "Preserve evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Initial write.",
        "focus": true,
        "next_actions": ["Continue"],
        "evidence": [
            {"command_preview": "cargo test -q", "exit_code": 0, "success": true}
        ]
    }))
    .expect("first write");
    let evidence_path = repo_root
        .join("artifacts/current/preserve-evidence")
        .join("EVIDENCE_INDEX.json");
    let before = fs::read_to_string(&evidence_path).expect("read before");

    write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "preserve-evidence",
        "task": "Preserve evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Automatic checkpoint without evidence payload.",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("second write");
    let after = fs::read_to_string(&evidence_path).expect("read after");
    assert_eq!(
        after, before,
        "omitted evidence must not overwrite existing file"
    );

    let changed = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "preserve-evidence",
        "task": "Preserve evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "Explicit evidence reset.",
        "focus": true,
        "next_actions": ["Continue"],
        "evidence": []
    }))
    .expect("explicit reset");
    assert!(changed["changed_paths"]
        .as_array()
        .unwrap()
        .iter()
        .any(|path| path
            .as_str()
            .is_some_and(|p| p.ends_with("EVIDENCE_INDEX.json"))));
    let reset: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read reset"))
            .expect("parse reset");
    assert_eq!(reset["artifacts"], json!([]));

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn task_registry_normalization_dedupes_and_limits_old_tasks() {
    let repo_root = temp_dir_path("framework-registry-compact");
    let current_root = repo_root.join("artifacts").join("current");
    let mut tasks = Vec::new();
    for index in 0..140 {
        tasks.push(json!({
            "task_id": format!("task-{index:03}"),
            "task": format!("Task {index:03}"),
            "updated_at": format!("2026-04-24T12:{:02}:00+08:00", index % 60),
            "status": "completed",
            "phase": "closeout",
            "resume_allowed": false
        }));
    }
    tasks.push(json!({
        "task_id": "focus-task",
        "task": "Focus task",
        "updated_at": "2026-04-24T13:00:00+08:00",
        "status": "in_progress",
        "phase": "implementation",
        "resume_allowed": true
    }));
    write_text_fixture(
        &current_root.join("task_registry.json"),
        &json!({
            "schema_version": "task-registry-v1",
            "focus_task_id": "focus-task",
            "tasks": tasks
        })
        .to_string(),
    );

    let changed = framework_extra::session_artifacts::write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": current_root,
        "task_id": "focus-task",
        "task": "Focus task",
        "phase": "implementation",
        "status": "in_progress",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("write focused task");
    assert_eq!(changed["task_id"], json!("focus-task"));

    let registry = serde_json::from_str::<Value>(
        &fs::read_to_string(current_root.join("task_registry.json")).expect("read registry"),
    )
    .expect("parse registry");
    let tasks = registry["tasks"].as_array().expect("tasks");
    assert_eq!(tasks.len(), 128);
    assert_eq!(registry["truncated"], json!(true));
    assert_eq!(registry["focus_task_id"], json!("focus-task"));
    assert_eq!(tasks[0]["task_id"], json!("focus-task"));
    assert_eq!(registry["recoverable_task_count"], json!(1));

    let _ = fs::remove_dir_all(&repo_root);
}
