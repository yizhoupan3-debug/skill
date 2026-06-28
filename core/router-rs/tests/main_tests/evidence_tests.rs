use super::common::*;
use super::*;

use serde_json::{Value, json};

#[test]
fn hook_evidence_append_cli_writes_cursor_cargo_check() {
    let repo_root = temp_dir_path("hook-evidence-cursor");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cursor-ev-task",
        "task": "cursor hook evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Continue"]
    }))
    .expect("seed");

    let payload = json!({
        "repo_root": repo_root,
        "command_preview": "(cd core/router-rs && cargo check --message-format=short)",
        "exit_code": 1,
        "source": "cursor_rust_lint",
    });
    let out = framework_hook_evidence_append(payload).expect("append");
    assert_eq!(out["ok"], json!(true));
    assert_eq!(out["skipped"], json!(false));

    let evidence_path = repo_root
        .join("artifacts/current/cursor-ev-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read")).expect("parse");
    let artifacts = evidence["artifacts"].as_array().expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0]["kind"], json!("external_hook_verification"));
    assert_eq!(artifacts[0]["exit_code"], json!(1));
    assert_eq!(artifacts[0]["success"], json!(false));

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn stdio_framework_hook_evidence_append_dispatches() {
    let repo_root = temp_dir_path("stdio-framework-hook-evidence");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "stdio-he-task",
        "task": "stdio hook evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": []
    }))
    .expect("seed");

    let rr = repo_root.display().to_string();
    let req = json!({
        "id": "stdio-he-1",
        "op": "framework_hook_evidence_append",
        "payload": {
            "repo_root": rr,
            "command_preview": "cargo test -q",
            "exit_code": 0,
            "source": "stdio_integration_test",
        }
    });
    let line = serde_json::to_string(&req).expect("serialize stdio line");
    let response = handle_stdio_json_line(&line);
    assert!(response.ok, "{:?}", response.error);
    let body = response.payload.expect("payload");
    assert_eq!(body["ok"], json!(true));

    let evidence_path = repo_root
        .join("artifacts/current/stdio-he-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read")).expect("parse");
    assert_eq!(
        evidence["artifacts"][0]["kind"],
        json!("external_hook_verification")
    );

    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn hook_evidence_append_allows_goal_drive_complete() {
    let repo_root = temp_dir_path("hook-evidence-goal-drive-complete");
    fs::create_dir_all(repo_root.join("artifacts/current")).expect("mkdir current");
    fs::write(
        repo_root.join("artifacts/current/active_task.json"),
        r#"{"task_id":"hook-goal"}"#,
    )
    .expect("active pointer");
    crate::goal_drive::framework_goal_drive(json!({
        "repo_root": repo_root,
        "operation": "start",
        "task_id": "hook-goal",
        "goal": "finish with hook evidence",
        "non_goals": ["no unrelated cleanup"],
        "done_when": ["implementation complete", "tests pass"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");

    framework_hook_evidence_append(json!({
        "repo_root": repo_root,
        "task_id": "hook-goal",
        "command_preview": "cargo test -q",
        "exit_code": 0,
        "source": "stdio_integration_test",
    }))
    .expect("append evidence");

    let done = crate::goal_drive::framework_goal_drive(json!({
        "repo_root": repo_root,
        "operation": "complete",
        "task_id": "hook-goal",
    }))
    .expect("goal complete should see task-local hook evidence");
    assert_eq!(
        done["operation"],
        json!("completed"),
        "goal complete should return operation=completed; got: {done}"
    );
    let _ = fs::remove_dir_all(&repo_root);
}

#[test]
fn hook_evidence_append_feeds_closeout_context() {
    let repo_root = temp_dir_path("hook-evidence-closeout-context");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "closeout-hook",
        "task": "closeout hook evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Validate"]
    }))
    .expect("seed");
    framework_hook_evidence_append(json!({
        "repo_root": repo_root,
        "task_id": "closeout-hook",
        "command_preview": "cargo test -q",
        "exit_code": 0,
        "source": "stdio_integration_test",
    }))
    .expect("append evidence");
    let record_path = repo_root.join("closeout-hook-record.json");
    fs::write(
        &record_path,
        serde_json::to_string(&json!({
            "schema_version": "closeout-record-v1",
            "task_id": "closeout-hook",
            "verification_status": "passed",
            "summary": "done",
            "risks": ["commands_run intentionally empty; relying on hook-appended evidence"]
        }))
        .expect("record json"),
    )
    .expect("write record");
    let eval = framework_extra::closeout::evaluate_closeout_record_file_for_task(
        &repo_root,
        "closeout-hook",
        &record_path,
    )
    .expect("evaluate closeout");
    assert_eq!(eval["closeout_allowed"], json!(true));
    let _ = fs::remove_dir_all(&repo_root);
}
