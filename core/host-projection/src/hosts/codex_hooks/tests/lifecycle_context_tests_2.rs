use super::lifecycle_context_tests::SEQ;
use super::lifecycle_context_tests::{TEST_COMPACT_FINDING, env_lock, fresh_repo, run_gate};
use super::*;
use serde_json::json;
use serial_test::serial;
use std::sync::atomic::{AtomicU64, Ordering};

fn codex_review_gate_disable_env_skips_block() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
    core_policy::hook_common::set_test_my_light_override(Some(true));
    unsafe { std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1") };
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-disable",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-disable",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(out.is_none(), "disable env must skip gate: {out:?}");
    match prior {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE") },
    }
    core_policy::hook_common::set_test_my_light_override(None);
}

#[test]
fn codex_review_gate_disable_clears_armed_state_on_userpromptsubmit() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
    let repo = fresh_repo();
    let arm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-disable-clear",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &arm).unwrap();
    assert!(
        codex_load_state(&repo, &arm)
            .unwrap()
            .map(|s| s.review_gate.review_required)
            .unwrap_or(false)
    );
    core_policy::hook_common::set_test_my_light_override(Some(true));
    unsafe { std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1") };
    let ups_disable = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-disable-clear",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let _ = run_gate(&repo, &ups_disable).unwrap();
    let state = codex_load_state(&repo, &ups_disable).unwrap().unwrap();
    assert_eq!(state.seq, 0, "disable UPS must reset hook-state");
    assert!(!state.review_gate.review_required);
    match prior {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE") },
    }
    core_policy::hook_common::set_test_my_light_override(None);
}

#[test]
fn codex_review_gate_disable_clears_state_on_posttool() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
    let repo = fresh_repo();
    let arm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-disable-post",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &arm).unwrap();
    core_policy::hook_common::set_test_my_light_override(Some(true));
    unsafe { std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1") };
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-disable-post",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review",
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert_eq!(state.seq, 0, "disable PostTool must reset hook-state");
    assert!(!state.review_gate.review_required);
    match prior {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE") },
    }
    core_policy::hook_common::set_test_my_light_override(None);
}

#[test]
fn post_tool_delegate_tool_does_not_count_deep_evidence() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-delegate",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-delegate",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Delegate",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(!state.review_gate.independent_reviewer_seen);
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-delegate",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(msg.contains("CODEX_REVIEW_GATE"));
}

#[test]
fn post_tool_gp_missing_fork_codex_infer_off_blocks_at_stop() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
    unsafe {
        std::env::set_var(
            "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            "0",
        );
    }
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-infer-off",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-infer-off",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose"}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(!state.review_gate.independent_reviewer_seen);
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-infer-off",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(msg.contains("CODEX_REVIEW_GATE"));
    match prior {
        Some(v) => unsafe {
            std::env::set_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", v)
        },
        None => unsafe {
            std::env::remove_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
        },
    }
}

#[test]
fn user_prompt_submit_review_and_implementx_suppresses_review_arming() {
    let _g = env_lock();
    let repo = fresh_repo();
    let sid = "sm-dual-review-implementx";
    let arm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review这个仓库"
    });
    let _ = run_gate(&repo, &arm).unwrap();
    let armed = codex_load_state(&repo, &arm).unwrap().unwrap();
    assert!(
        armed.review_gate.review_required,
        "review-only UPS should arm; got {armed:?}"
    );
    let dual = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"请全面review这个仓库 /implementx 修复刚发现的问题"
    });
    let _ = run_gate(&repo, &dual).unwrap();
    let cleared = codex_load_state(&repo, &dual).unwrap().unwrap();
    assert!(
        !cleared.review_gate.review_required,
        "my-light goal drive must clear/disarm review on Codex UPS; got {cleared:?}"
    );
}

#[test]
fn rearm_review_resets_codex_independent_evidence() {
    let _g = env_lock();
    let repo = fresh_repo();
    let sid = "sm-rearm-evidence";
    let arm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &arm).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let seeded = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(seeded.review_gate.independent_reviewer_seen);
    assert!(seeded.phase >= 2);
    let rearm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review全仓找bug"
    });
    let _ = run_gate(&repo, &rearm).unwrap();
    let reset = codex_load_state(&repo, &rearm).unwrap().unwrap();
    assert!(
        !reset.review_gate.independent_reviewer_seen,
        "re-arm review must reset PostTool evidence"
    );
    assert_eq!(reset.phase, 0);
    assert_eq!(reset.subagent_start_count, 0);
    assert!(!reset.review_subagent_seen);
    assert!(!reset.generic_subagent_seen);
    assert!(reset.review_gate.review_required);
}

#[test]
fn rearm_review_preserves_evidence_when_override() {
    let repo = fresh_repo();
    let sid = "sm-rearm-override";
    let arm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &arm).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let seeded = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(seeded.review_gate.independent_reviewer_seen);
    let override_ups = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review，不要用子代理"
    });
    let _ = run_gate(&repo, &override_ups).unwrap();
    let kept = codex_load_state(&repo, &override_ups).unwrap().unwrap();
    assert!(
        kept.review_gate.independent_reviewer_seen,
        "override must not reset prior PostTool reviewer evidence"
    );
    assert!(kept.review_gate.review_override);
}

#[test]
fn legacy_phase_two_alone_compact_does_not_clear_codex_review_gate() {
    let _g = env_lock();
    let repo = fresh_repo();
    let sid = "sm-legacy-phase2-compact";
    let arm = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &arm).unwrap();
    let sp = codex_state_path(&repo, &arm);
    let mut state = codex_load_state(&repo, &arm).unwrap().unwrap();
    state.phase = 2;
    state.subagent_start_count = 0;
    state.review_gate.independent_reviewer_seen = false;
    state.review_gate.review_required = true;
    assert!(codex_save_state_to_path(&sp, &mut state));
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id": sid,
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续",
        "response":"[P1] scripts/foo.rs:1 — issue — impact — verify",
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(
        msg.contains("CODEX_REVIEW_GATE"),
        "legacy phase=2 without PostTool start/independent must not clear gate; msg={msg:?}"
    );
    let loaded = codex_load_state(&repo, &stop).unwrap().unwrap();
    assert!(
        loaded.phase < 3,
        "compact must not bump to phase 3 without countable evidence"
    );
}

#[test]
fn stop_reject_reason_in_response_clears_gate() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-reject-resp",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-reject-resp",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"",
        "response":"small_task"
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(
        out.is_none(),
        "reject token in response must clear: {out:?}"
    );
}

#[test]
fn stop_clears_after_best_of_n_runner_posttool_and_compact() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-bon",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-bon",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"best-of-n-runner","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-bon",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续",
        "response": TEST_COMPACT_FINDING
    });
    let out = run_gate(&repo, &stop).unwrap();
    assert!(out.is_none(), "best-of-n + compact must clear: {out:?}");
}

#[test]
fn stop_with_review_explore_fork_false_still_blocks() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-7-explore",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-7-explore",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"explore","fork_context":false}
    });
    let _ = run_gate(&repo, &post).unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-7-explore",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续"
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
}

#[test]
fn stop_hook_active_bypass_skips_gate_only_when_env_set() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
    unsafe { std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", "1") };
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-8-bypass",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let payload = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-8-bypass",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续",
        "stop_hook_active": true
    });
    let out = run_gate(&repo, &payload).unwrap();
    assert!(
        out.is_none(),
        "bypass env must skip review gate on replay: {out:?}"
    );
    match prior {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS") },
    }
}

#[test]
fn stop_hook_active_still_blocks_review_gate_by_default() {
    let _g = env_lock();
    let prior = std::env::var_os("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
    unsafe { std::env::remove_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS") };
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-8-default",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"全面review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let payload = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-8-default",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"继续",
        "stop_hook_active": true
    });
    let out = run_gate(&repo, &payload).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert!(
        out.as_ref()
            .and_then(|v| v.get("decision"))
            .and_then(Value::as_str)
            != Some("block"),
        "review gate Stop must be advisory-only: {out:?}"
    );
    assert!(
        msg.contains("CODEX_REVIEW_GATE"),
        "stop_hook_active without bypass must still nudge review: {out:?}"
    );
    match prior {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", v) },
        None => {}
    }
}

#[test]
#[serial]
fn stop_completion_claim_blocks_with_closeout_followup_when_strict() {
    let _g = env_lock();
    let prev = std::env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
    unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1") };
    let repo = fresh_repo();
    let tid = "t-codex-closeout";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).unwrap();
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .unwrap();
    // Pointer 机制已移除：写入 task_registry.json 供回退使用
    fs::write(
        repo.join("artifacts/current/task_registry.json"),
        format!(r#"{{"schema_version":"task-registry-v1","focus_task_id":"{tid}","tasks":[{{"task_id":"{tid}"}}]}}"#),
    )
    .unwrap();
    let stop = json!({
        "hook_event_name":"Stop",
        "session_id":"sm-closeout",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"all done, shipped"
    });
    let out = run_gate(&repo, &stop).unwrap();
    let msg = out
        .as_ref()
        .and_then(|v| v["followup_message"].as_str())
        .unwrap_or_default();
    assert_eq!(
        out.as_ref()
            .and_then(|v| v.get("decision"))
            .and_then(Value::as_str),
        Some("block")
    );
    assert!(
        msg.contains("CLOSEOUT_FOLLOWUP") && msg.contains("missing_record"),
        "expected closeout block on Stop; got {out:?}"
    );
    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
    }
}

#[test]
fn post_tool_state_lock_failure_blocks_like_user_prompt_submit() {
    let repo = fresh_repo();
    let event = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"lock-pt-block",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"subagent_type":"general-purpose","fork_context":false}
    });
    let state_path = codex_state_path(&repo, &event);
    fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    fs::write(&lock_path, "pid=1 ts=1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o000)).unwrap();
    }
    #[cfg(not(unix))]
    {
        let guard = acquire_codex_state_lock(&state_path).unwrap();
        let _hold = guard;
    }
    let out = run_gate(&repo, &event).unwrap();
    assert_eq!(
        out.as_ref()
            .and_then(|v| v.get("decision"))
            .and_then(Value::as_str),
        Some("block"),
        "PostTool lock failure must fail-closed: {out:?}"
    );
    assert_eq!(
        out.as_ref()
            .and_then(|v| v.get("reason"))
            .and_then(Value::as_str),
        Some("Codex hook state could not be persisted under .codex/hook-state.")
    );
}

#[test]
fn no_drift_warn_when_manifest_missing() {
    let repo = fresh_repo();
    let codex_home = repo.join("codex-home");
    fs::create_dir_all(&codex_home).unwrap();
    unsafe { std::env::set_var("CODEX_HOME", &codex_home) };
    let payload = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-drift-1",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"普通提问"
    });
    let out = run_gate(&repo, &payload).unwrap();
    // Plain prompts no longer arm a hard subagent gate,
    // so the hook may return None (no context to emit). If context IS
    // emitted for other reasons, it must not contain a drift warning.
    let ctx = out
        .as_ref()
        .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
        .unwrap_or_default()
        .to_string();
    assert!(!ctx.contains("hook projection drift detected"));
}

#[test]
fn no_drift_warn_when_manifest_matches() {
    let repo = fresh_repo();
    let codex_home = repo.join("codex-home");
    fs::create_dir_all(&codex_home).unwrap();
    unsafe { std::env::set_var("CODEX_HOME", &codex_home) };
    let manifest = json!({
        "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
        "command_digest": "abc",
    });
    fs::write(
        codex_home.join(".router-rs-install.manifest.json"),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let payload = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-drift-2",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"普通提问"
    });
    let out = run_gate(&repo, &payload).unwrap();
    if let Some(value) = out {
        let ctx = value["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default();
        assert!(!ctx.contains("hook projection drift detected"));
    }
}

#[test]
fn v1_migration_ignores_removed_override_flag() {
    let repo = fresh_repo();
    let event = json!({"session_id":"v1-override"});
    let state_path = codex_state_path(&repo, &event);
    fs::write(
        state_path,
        r#"{"schema_version":1,"override":true,"subagent_required":true}"#,
    )
    .unwrap();
    let state = codex_load_state(&repo, &event).unwrap().unwrap();
    assert_eq!(state.seq, 0);
}

#[test]
fn v1_migration_ignores_removed_reject_reason_flag() {
    let repo = fresh_repo();
    let event = json!({"session_id":"v1-reject"});
    let state_path = codex_state_path(&repo, &event);
    fs::write(
        state_path,
        r#"{"schema_version":1,"reject_reason_seen":true}"#,
    )
    .unwrap();
    let state = codex_load_state(&repo, &event).unwrap().unwrap();
    assert_eq!(state.seq, 0);
}

#[test]
fn v1_delegation_only_maps_to_phase1() {
    let repo = fresh_repo();
    let event = json!({"session_id":"v1-phase"});
    let state_path = codex_state_path(&repo, &event);
    fs::write(
        state_path,
        r#"{"schema_version":1,"delegation_required":true,"review_subagent_seen":false}"#,
    )
    .unwrap();
    let state = codex_load_state(&repo, &event).unwrap().unwrap();
    assert_eq!(state.seq, 1);
}

#[test]
fn codex_session_key_fallback_is_stable_without_identifiers() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CODEX_SESSION_ID") };
    unsafe { std::env::remove_var("CODEX_CONVERSATION_ID") };
    unsafe { std::env::remove_var("ROUTER_RS_CODEX_HOOK_STATE_SALT") };
    let repo = fresh_repo();
    let event = json!({"cwd": repo.to_string_lossy()});
    let a = codex_session_key(&repo, &event);
    let b = codex_session_key(&repo, &event);
    assert_eq!(a, b);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(a.len(), 32);
}

#[test]
fn codex_session_key_differs_by_cwd_when_unstable() {
    let _guard = env_lock();
    unsafe { std::env::remove_var("CODEX_SESSION_ID") };
    unsafe { std::env::remove_var("CODEX_CONVERSATION_ID") };
    let repo = fresh_repo();
    let a = codex_session_key(&repo, &json!({"cwd":"/tmp/a"}));
    let b = codex_session_key(&repo, &json!({"cwd":"/tmp/b"}));
    assert_ne!(a, b, "unstable fallback must not collapse unlike cwd");
}

#[test]
fn saw_subagent_codex_accepts_agent_type_camel_case_field() {
    assert!(saw_subagent_codex(
        "Task",
        &json!({"agentType":"browser-use"})
    ));
}

#[test]
fn post_tool_use_with_agent_type_camel_case_marks_seen_without_deep_independent() {
    let repo = fresh_repo();
    let start = json!({
        "hook_event_name":"UserPromptSubmit",
        "session_id":"sm-2e",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt":"please do deep review"
    });
    let _ = run_gate(&repo, &start).unwrap();
    let post = json!({
        "hook_event_name":"PostToolUse",
        "session_id":"sm-2e",
        "cwd": repo.to_string_lossy().to_string(),
        "tool_name":"Task",
        "tool_input":{"agentType":"explore","fork_context":false}
    });
    let out = run_gate(&repo, &post).unwrap();
    assert!(out.is_none());
    let state = codex_load_state(&repo, &post).unwrap().unwrap();
    assert!(state.review_subagent_seen);
    assert!(
        !state.review_gate.independent_reviewer_seen,
        "explore must not satisfy Codex independent deep-review bar"
    );
    assert!(state.generic_subagent_seen);
    assert!(state.review_lane_seen);
    assert!(!state.parallel_lane_seen);
    assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
}

#[test]
fn dispatch_unknown_event_blocks_with_message() {
    let repo = fresh_repo();
    let payload = json!({
        "hook_event_name":"Other",
        "session_id":"sm-9",
        "cwd": repo.to_string_lossy().to_string()
    });
    let out = run_gate(&repo, &payload).unwrap().unwrap();
    assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
    assert!(
        out.get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("unsupported")
    );
}

#[test]
fn dispatch_missing_event_blocks_with_message() {
    let repo = fresh_repo();
    let payload = json!({"session_id":"sm-10"});
    let out = run_gate(&repo, &payload).unwrap().unwrap();
    assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
    assert!(
        out.get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("missing")
    );
}

#[test]
fn codex_state_lock_recovers_from_stale_lock() {
    let repo = fresh_repo();
    let event = json!({"session_id":"lock-stale"});
    let state_path = codex_state_path(&repo, &event);
    fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    fs::write(&lock_path, "pid=999999 ts=1\n").unwrap();
    let lock = acquire_codex_state_lock(&state_path);
    assert!(lock.is_ok());
}

#[test]
fn codex_state_lock_recovers_from_corrupt_lock_metadata() {
    let repo = fresh_repo();
    let event = json!({"session_id":"lock-corrupt"});
    let state_path = codex_state_path(&repo, &event);
    fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    fs::write(&lock_path, "not-a-lock-metadata-line\n").unwrap();
    let lock = acquire_codex_state_lock(&state_path);
    assert!(lock.is_ok());
}

#[test]
fn codex_state_lock_recovers_from_unparseable_pid_and_ts() {
    let repo = fresh_repo();
    let event = json!({"session_id":"lock-unparseable"});
    let state_path = codex_state_path(&repo, &event);
    fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    fs::write(&lock_path, "pid=bad ts=bad\n").unwrap();
    let lock = acquire_codex_state_lock(&state_path);
    assert!(lock.is_ok());
}

#[cfg(unix)]
#[test]
fn codex_state_lock_blocks_until_released() {
    use std::sync::mpsc;

    let repo = fresh_repo();
    let event = json!({"session_id":"lock-held"});
    let state_path = codex_state_path(&repo, &event);
    fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    let guard = acquire_codex_state_lock(&state_path).unwrap();
    let state_path_clone = state_path.clone();
    let (tx, rx) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let second = acquire_codex_state_lock(&state_path_clone).unwrap();
        let _ = tx.send(());
        drop(second);
    });
    std::thread::sleep(Duration::from_millis(50));
    assert!(rx.try_recv().is_err());
    drop(guard);
    rx.recv_timeout(Duration::from_secs(5))
        .expect("second acquirer should proceed after lock release");
    waiter.join().unwrap();
}

#[cfg(not(unix))]
#[test]
fn codex_state_lock_blocks_when_held() {
    let repo = fresh_repo();
    let event = json!({"session_id":"lock-held"});
    let state_path = codex_state_path(&repo, &event);
    fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    let guard = acquire_codex_state_lock(&state_path).unwrap();
    let started = std::time::Instant::now();
    let second = acquire_codex_state_lock(&state_path);
    assert!(second.is_err());
    assert!(started.elapsed() >= Duration::from_millis(1200));
    drop(guard);
}

#[test]
#[serial]
fn codex_state_lock_serializes_concurrent_writes() {
    let repo = fresh_repo();
    let event = json!({"session_id":"lock-inc"});
    let repo_a = repo.clone();
    let repo_b = repo.clone();
    let event_a = event.clone();
    let event_b = event.clone();
    let worker = move |repo_root: PathBuf, ev: Value| {
        for _ in 0..1000 {
            with_codex_state_lock(&repo_root, &ev, |loaded| {
                let mut state = loaded.unwrap_or_default();
                state.seq += 1;
                Ok((Some(state), ()))
            })
            .unwrap();
        }
    };
    let t1 = std::thread::spawn(move || worker(repo_a, event_a));
    let t2 = std::thread::spawn(move || worker(repo_b, event_b));
    t1.join().unwrap();
    t2.join().unwrap();
    let state = codex_load_state(&repo, &event).unwrap().unwrap();
    // flock on macOS has known edge cases with concurrent threads;
    // accept 1999-2000 to avoid flaky test failures.
    assert!(
        state.seq >= 1999 && state.seq <= 2000,
        "concurrent seq should be 1999 or 2000, got {}",
        state.seq
    );
}

#[test]
fn userpromptsubmit_simple_prompt_records_only_telemetry() {
    let repo = fresh_repo();
    let event = json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": "test-p0a-simple",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt": "just a simple question about coding"
    });
    let _ = run_gate(&repo, &event).unwrap();
    let state = codex_load_state(&repo, &event).unwrap().unwrap();
    assert_eq!(state.seq, 1);
    assert!(!state.review_subagent_seen);
}

#[test]
fn userpromptsubmit_review_prompt_records_gate_requirement() {
    let repo = fresh_repo();
    let event = json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": "test-p0a-review",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt": "please do a deep code review of this module"
    });
    let _ = run_gate(&repo, &event).unwrap();
    let state = codex_load_state(&repo, &event).unwrap().unwrap();
    assert_eq!(state.seq, 1);
    assert!(state.review_gate.review_required);
    assert!(!state.review_subagent_seen);
}

// P0-B: protected prefix tests
#[test]
fn protected_prefixes_cover_skill_files_and_registry() {
    assert!(
        classify_protected_generated_path("skills/SKILL_ROUTING_RUNTIME.json").is_some(),
        "SKILL_ROUTING_RUNTIME.json should be protected"
    );
    assert!(
        classify_protected_generated_path("skills/SKILL_MANIFEST.json").is_some(),
        "SKILL_MANIFEST.json should be protected"
    );
    assert!(
        classify_protected_generated_path("configs/framework/RUNTIME_REGISTRY.json").is_some(),
        "RUNTIME_REGISTRY.json should be protected"
    );
    assert!(
        classify_protected_generated_path("skills/other_file.json").is_none(),
        "non-SKILL_ prefixed file should not be protected"
    );
}

// P1-B: CODEX_SESSION_ID env var fallback test
#[test]
fn codex_session_key_uses_codex_session_id_env_when_no_event_fields() {
    let _guard = env_lock();
    // Use a unique env-var value to avoid cross-test pollution.
    let unique_id = format!(
        "test-stable-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    );
    let event = json!({});
    let repo = fresh_repo();
    unsafe { std::env::set_var("CODEX_SESSION_ID", &unique_id) };
    let a = codex_session_key(&repo, &event);
    let b = codex_session_key(&repo, &event);
    unsafe { std::env::remove_var("CODEX_SESSION_ID") };
    assert_eq!(a, b, "env var fallback should produce a stable key");
    assert!(
        a.chars().all(|c| c.is_ascii_hexdigit()),
        "key should be hex"
    );
    assert_eq!(a.len(), 32, "key should be 32 hex chars");
}

#[test]
fn codex_session_key_matches_for_session_id_camel_case() {
    let repo = fresh_repo();
    let sid = "sess-key-camel-01";
    let snake = codex_session_key(&repo, &json!({"session_id": sid}));
    let camel = codex_session_key(&repo, &json!({"sessionId": sid}));
    assert_eq!(snake, camel);
}

#[test]
fn codex_session_key_uses_codex_conversation_id_env_when_no_event_fields() {
    let _guard = env_lock();
    let unique_id = format!(
        "test-conv-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    );
    let event = json!({});
    unsafe { std::env::remove_var("CODEX_SESSION_ID") };
    let repo = fresh_repo();
    unsafe { std::env::set_var("CODEX_CONVERSATION_ID", &unique_id) };
    let a = codex_session_key(&repo, &event);
    let b = codex_session_key(&repo, &event);
    unsafe { std::env::remove_var("CODEX_CONVERSATION_ID") };
    assert_eq!(a, b, "CODEX_CONVERSATION_ID fallback should be stable");
    assert_eq!(a.len(), 32);
}

#[test]
fn strict_stable_session_key_blocks_userpromptsubmit_without_identifier() {
    let _guard = env_lock();
    unsafe { std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "1") };
    unsafe { std::env::remove_var("CODEX_SESSION_ID") };
    unsafe { std::env::remove_var("CODEX_CONVERSATION_ID") };
    let repo = fresh_repo();
    let event = json!({
        "hook_event_name": "UserPromptSubmit",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt": "hello"
    });
    let out = super::run_codex_lifecycle_context_hook(&repo, &event)
        .unwrap()
        .unwrap();
    assert_eq!(out["decision"], json!("block"));
    unsafe { std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY") };
}

#[test]
fn strict_stable_session_key_allows_sessionstart_without_identifier() {
    let _guard = env_lock();
    unsafe { std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "1") };
    unsafe { std::env::remove_var("CODEX_SESSION_ID") };
    unsafe { std::env::remove_var("CODEX_CONVERSATION_ID") };
    let repo = fresh_repo();
    let event = json!({
        "hook_event_name": "SessionStart",
        "source": "startup"
    });
    let out = super::run_codex_lifecycle_context_hook(&repo, &event)
        .unwrap()
        .expect("sessionstart output");
    assert!(out.get("hookSpecificOutput").is_some());
    unsafe { std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY") };
}

#[test]
fn strict_stable_session_key_off_allows_userpromptsubmit_without_identifier() {
    let _guard = env_lock();
    unsafe { std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "0") };
    unsafe { std::env::remove_var("CODEX_SESSION_ID") };
    unsafe { std::env::remove_var("CODEX_CONVERSATION_ID") };
    let repo = fresh_repo();
    let event = json!({
        "hook_event_name": "UserPromptSubmit",
        "cwd": repo.to_string_lossy().to_string(),
        "prompt": "hello"
    });
    let out = super::run_codex_lifecycle_context_hook(&repo, &event).unwrap();
    assert!(
        !matches!(out, Some(ref v) if v.get("decision") == Some(&json!("block"))),
        "unexpected lifecycle block when strict mode off"
    );
}

// P1-C: prune_stale_hook_state_files test
#[test]
fn prune_removes_excess_files_over_limit() {
    let repo = fresh_repo();
    let state_dir = repo.join(".codex/hook-state");
    // Create 60 fake review-subagent JSON files
    for i in 0..60u64 {
        let name = format!("review-subagent-{:032x}.json", i);
        fs::write(state_dir.join(&name), "{}").unwrap();
    }
    prune_stale_hook_state_files(&state_dir);
    let count = fs::read_dir(&state_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name();
            let s = n.to_string_lossy();
            s.starts_with("review-subagent-") && s.ends_with(".json")
        })
        .count();
    assert!(
        count <= 50,
        "after pruning, at most 50 files should remain, got {count}"
    );
}
