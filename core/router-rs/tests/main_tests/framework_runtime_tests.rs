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
fn post_tool_evidence_appends_cargo_test_after_continuity_seed() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE");
    unsafe { std::env::set_var("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE", "1") };

    let repo_root = temp_dir_path("post-tool-evidence-append");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "evidence-task",
        "task": "Verify evidence append",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed continuity",
        "focus": true,
        "next_actions": ["Run tests"]
    }))
    .expect("seed artifacts");

    let event = json!({
        "tool_name": "Bash",
        "tool_input": { "command": "cd core/router-rs && cargo test -q" },
        "session_id": "sess-post-tool-1",
        "tool_output": { "exit_code": 0 },
    });
    crate::framework_runtime::try_append_post_tool_shell_evidence(
        &repo_root,
        &event,
        "codex_post_tool_verification",
    )
    .expect("append");

    let evidence_path = repo_root
        .join("artifacts/current/evidence-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read evidence"))
            .expect("parse evidence");
    let artifacts = evidence["artifacts"].as_array().expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0]["kind"], json!("codex_post_tool_verification"));
    assert_eq!(artifacts[0]["exit_code"], json!(0));
    assert_eq!(artifacts[0]["success"], json!(true));
    assert!(artifacts[0]["command_preview"]
        .as_str()
        .unwrap()
        .contains("cargo test"));

    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE") },
    }
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn cursor_post_tool_evidence_appends_cargo_test_after_continuity_seed() {
    let _env = crate::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE");
    unsafe { std::env::set_var("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE", "1") };

    let repo_root = temp_dir_path("cursor-post-tool-evidence-append");
    let output_dir = repo_root.join("artifacts").join("current");
    let _ = write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": "cursor-evidence-task",
        "task": "Cursor shell evidence",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed continuity",
        "focus": true,
        "next_actions": ["Run tests"]
    }))
    .expect("seed artifacts");

    let event = json!({
        "tool_name": "run_terminal_cmd",
        "tool_input": { "command": "cd core/router-rs && cargo test -q" },
        "session_id": "sess-cursor-post-tool-1",
        "tool_output": { "exit_code": 0 },
    });
    crate::framework_runtime::try_append_post_tool_shell_evidence(
        &repo_root,
        &event,
        "cursor_post_tool_verification",
    )
    .expect("append");

    let evidence_path = repo_root
        .join("artifacts/current/cursor-evidence-task")
        .join("EVIDENCE_INDEX.json");
    let evidence: Value =
        serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read evidence"))
            .expect("parse evidence");
    let artifacts = evidence["artifacts"].as_array().expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0]["kind"], json!("cursor_post_tool_verification"));
    assert_eq!(artifacts[0]["exit_code"], json!(0));
    assert_eq!(artifacts[0]["success"], json!(true));
    assert!(artifacts[0]["command_preview"]
        .as_str()
        .unwrap()
        .contains("cargo test"));

    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE") },
    }
    let _ = fs::remove_dir_all(&repo_root);
}


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
    assert_eq!(done["operation"], json!("completed"),
        "goal complete should return operation=completed; got: {done}");
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
    let eval = crate::framework_runtime::evaluate_closeout_record_file_for_task(
        &repo_root,
        "closeout-hook",
        &record_path,
    )
    .expect("evaluate closeout");
    assert_eq!(eval["closeout_allowed"], json!(true));
    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn post_tool_evidence_no_ops_without_continuity_seed() {
    let repo_root = temp_dir_path("post-tool-evidence-skip");
    let event = json!({
        "tool_name": "Bash",
        "tool_input": { "command": "cargo test" },
    });
    crate::framework_runtime::try_append_post_tool_shell_evidence(
        &repo_root,
        &event,
        "codex_post_tool_verification",
    )
    .expect("noop");
    assert!(
        !repo_root
            .join("artifacts/current/EVIDENCE_INDEX.json")
            .exists(),
        "evidence file should not be created without continuity anchors"
    );
    let _ = fs::remove_dir_all(&repo_root);
}


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
    let stale_hash = crate::framework_runtime::hash_file_for_test(&focus_path).expect("focus hash");
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
        crate::framework_runtime::hash_file_for_test(&active_path).expect("active hash");
    let focus_hash = crate::framework_runtime::hash_file_for_test(&focus_path).expect("focus hash");
    let supervisor_hash =
        crate::framework_runtime::hash_file_for_test(&supervisor_path).expect("supervisor hash");

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

    let changed = crate::framework_runtime::write_framework_session_artifacts(json!({
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
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("build snapshot");
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
        build_framework_runtime_snapshot_envelope(&repo_root, None, None).expect("snapshot");
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
    assert_eq!(body["quality_gate_superseded"], json!(false));

    let path = repo_root.join("artifacts/current/ag-stdio-task/GOAL_STATE.json");
    assert!(path.is_file(), "missing {}", path.display());

    let _ = fs::remove_dir_all(&repo_root);
}


#[test]
fn stdio_framework_quality_gate_roundtrip() {
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
            "max_rounds": 100u64,
            "allow_external_research": true,
            "verify_commands": ["cargo test -q"],
        }
    });
    let line = serde_json::to_string(&start).expect("serialize");
    let response = handle_stdio_json_line(&line);
    assert!(response.ok, "{:?}", response.error);
    let body = response.payload.expect("payload");
    assert_eq!(body["goal_state_cleared"], json!(false));

    let path = repo_root.join("artifacts/current/rfv-stdio-task/RFV_LOOP_STATE.json");
    assert!(path.is_file(), "missing {}", path.display());

    assert_eq!(
        body["quality_gate_state"]["prefer_structured_external_research"],
        json!(true),
        "prefer_structured defaults true when allow_external_research=true"
    );
    assert_eq!(
        body["quality_gate_state"]["external_research_strict"],
        json!(true),
        "external_research_strict defaults true in persisted RFV state"
    );

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
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["schema_version"],
        Value::String("runtime-sandbox-lifecycle-v1".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["cleanup_mode"],
        Value::String("async-drain-and-recycle".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["control_operations"][1],
        Value::String("cleanup".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["control_operations"][2],
        Value::String("admit".to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["event_schema_version"],
        Value::String(SANDBOX_EVENT_SCHEMA_VERSION.to_string())
    );
    assert_eq!(
        payload["services"]["execution"]["sandbox_lifecycle_contract"]["event_tracing"]
            ["response_flag"],
        Value::String("event_written".to_string())
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
    use crate::framework_runtime::build_framework_runtime_snapshot_envelope_with_level;

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


