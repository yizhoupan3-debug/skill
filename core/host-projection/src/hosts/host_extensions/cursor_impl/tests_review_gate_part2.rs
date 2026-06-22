#[test]
fn review_gate_disabled_stop_still_merges_goal_drive() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gl-rgoff")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gl-rgoff"}"#,
    )
    .expect("active_task");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gl-rgoff",
        "goal": "rg-off-merge",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");

    let mut out = {
        let _rg = ReviewGateDisableTestGuard::new();
        dispatch_cursor_hook_event(&repo, "stop", &event("sg1", "hi"))
    };
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: {blob}"
    );

    apply_cursor_hook_output_policy(&mut out);
    let preserved = hook_user_visible_blob(&out);
    assert!(
        !preserved.contains("GOAL_CONTINUE"),
        "continuity removal: {preserved}"
    );
}

#[test]
fn stop_goal_and_rfv_do_not_emit_continuity_followups() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let tid = "stop-both";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .expect("active");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","goal":"goal-line","status":"running","drive_until_done":true,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("RFV_LOOP_STATE.json"),
            r#"{"schema_version":"router-rs-rfv-loop-v1","goal":"rfv-line","loop_status":"active","current_round":1,"max_rounds":3,"allow_external_research":false,"rounds":[]}"#,
        )
        .expect("rfv");

    let cwd = repo.display().to_string();
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "stop-both",
            "cwd": cwd,
            "prompt": "hello",
        }),
    );
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: {blob}"
    );
    assert!(
        !blob.contains("RFV_LOOP_CONTINUE"),
        "continuity removal: {blob}"
    );
}

#[test]
fn stop_goal_and_rfv_do_not_merge_schema_hint_into_continue() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _advisory_env = AdvisoryOperatorEnvClearGuard::new();
    let repo = fresh_repo();
    let tid = "stop-both-struct";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .expect("active");
    fs::write(
        repo
            .join("artifacts/current")
            .join(tid)
            .join("GOAL_STATE.json"),
        r#"{"schema_version":"router-rs-goal-v1","goal":"goal-line","status":"running","drive_until_done":true,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
    )
    .expect("goal");
    fs::write(
        repo
            .join("artifacts/current")
            .join(tid)
            .join("RFV_LOOP_STATE.json"),
        r#"{"schema_version":"router-rs-rfv-loop-v1","goal":"rfv-line","loop_status":"active","current_round":1,"max_rounds":3,"allow_external_research":true,"prefer_structured_external_research":true,"rounds":[{"round":1,"verify_result":"PASS"}]}"#,
    )
    .expect("rfv");

    let cwd = repo.display().to_string();
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "stop-both-struct",
            "cwd": cwd,
            "prompt": "hello",
        }),
    );
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: {blob}"
    );
    assert!(
        !blob.contains(hooks::RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH),
        "continuity removal must not inject RFV schema pointer via Stop: {blob}"
    );
}

#[test]
#[serial]
fn hook_output_policy_truncates_additional_context_under_env_budget() {
    let _env_lock = hook_outbound_context_max_chars_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", "1500") };
    let pad = "Z".repeat(8000);
    let mut out = json!({
        "additional_context": format!("GOAL_HEAD\nGOAL_DRIVE_MARKER\n{}", pad),
    });
    apply_cursor_hook_output_policy(&mut out);
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") };
    if let Some(v) = prev {
        unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v) };
    }

    let s = out["additional_context"].as_str().expect("str");
    assert!(
        s.len() <= 1500,
        "len={}, s.prefix={:?}",
        s.len(),
        &s[..s.len().min(80)]
    );
    assert!(
        s.starts_with("GOAL_HEAD")
            && s.contains("GOAL_DRIVE_MARKER")
            && s.ends_with(super::CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX),
        "prefer prefix preservation: {s:?}"
    );
}

#[test]
#[serial]
fn hook_output_policy_truncates_followup_after_absurd_length() {
    let _env_lock = hook_outbound_context_max_chars_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") };
    let max_out = hooks::router_rs_hook_outbound_context_max_bytes();
    let absurd = vec![b'Q'; max_out.saturating_mul(5).max(32 * 1024)];
    let absurd_str = String::from_utf8(absurd).expect("ascii");
    let mut out = json!({ "followup_message": absurd_str });
    apply_cursor_hook_output_policy(&mut out);
    match prev_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") },
    }
    let s = out["followup_message"].as_str().expect("str");
    assert!(s.len() <= max_out, "truncated={}, max={}", s.len(), max_out);
    assert!(s.ends_with(super::CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX));
    assert!(s.starts_with('Q'));
}

#[test]
#[serial]
fn hook_output_policy_is_noop_for_review_gate_advisory_lines() {
    let hard = format!(
        "router-rs REVIEW_GATE incomplete phase=0 {} {}",
        REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    );
    let mut out = json!({
        "followup_message": hard.clone()
    });
    apply_cursor_hook_output_policy(&mut out);
    assert_eq!(out["followup_message"], json!(hard));
    assert_eq!(out["router_rs_observation"]["gate"]["code"], "review_gate");
    assert_eq!(out["router_rs_observation"]["gate"]["blocking"], true);
}

#[test]
fn hook_outbound_trunc_respects_byte_cap_and_marker() {
    let body = "x".repeat(9000);
    let max_out = 8192usize;
    let got = super::truncate_cursor_hook_outbound_context(&body, max_out);
    assert!(got.len() <= max_out, "len {} max {}", got.len(), max_out);
    assert!(got.ends_with(super::CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX));
}

#[test]
#[serial]
fn outbound_truncation_preserves_review_gate_and_continuity_suppressed_lines() {
    let _env_lock = hook_outbound_context_max_chars_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", "512") };

    let filler = "z".repeat(2000);
    let gate_line = format!(
        "router-rs REVIEW_GATE incomplete phase=2 {} {}",
        super::REVIEW_GATE_FOLLOWUP_NEED_SEGMENT,
        super::REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    );
    let body = format!("{filler}\ncontinuity_suppressed=review_soft_nag\n{gate_line}\n{filler}");
    let max_out = hooks::router_rs_hook_outbound_context_max_bytes();
    let got = super::truncate_cursor_hook_outbound_context_preserving_gate(&body, max_out);
    assert!(got.len() <= max_out);
    assert!(got.contains("continuity_suppressed=review_soft_nag"));
    assert!(got.contains(super::REVIEW_GATE_FOLLOWUP_NEED_SEGMENT));

    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") },
    }
}

#[test]
fn review_gate_disabled_post_tool_use_does_not_advance_review_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("srg-pu2", "全面review这个仓库"),
    );
    assert!(load_state_for(&repo, "srg-pu2").phase < 2);

    let out = {
        let _rg = ReviewGateDisableTestGuard::new();
        dispatch_cursor_hook_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id": "srg-pu2",
                "cwd": FRAMEWORK_HARNESS_TEST_CWD,
                "tool_name": "functions.subagent",
                "tool_input": { "subagent_type": "explore", "fork_context": false }
            }),
        )
    };

    assert_eq!(out, json!({}));
    let state = load_state_for(&repo, "srg-pu2");
    assert!(
        state.phase < 2,
        "DISABLE must clear review state and not advance phase via postToolUse; phase={}",
        state.phase
    );
}

#[test]
fn hook_output_policy_is_noop() {
    let mut keep = json!({ "followup_message": "keep" });
    apply_cursor_hook_output_policy(&mut keep);
    assert_eq!(keep["followup_message"], json!("keep"));
    assert!(keep["router_rs_observation"]["gate"].is_null());

    let mut strip = json!({
        "continue": false,
        "followup_message": "nag",
        "additional_context": "ctx"
    });
    apply_cursor_hook_output_policy(&mut strip);
    assert_eq!(strip["continue"], json!(false));
    assert_eq!(strip["followup_message"], json!("nag"));
    assert_eq!(strip["additional_context"], json!("ctx"));
    assert!(strip["router_rs_observation"]["gate"].is_null());
}

#[test]
fn subagent_start_promotes_phase_to_2() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s5", "subagent_type": "general-purpose", "fork_context": false }),
    );
    let state = load_state_for(&repo, "s5");
    assert_eq!(state.phase, 2);
    assert_eq!(state.subagent_start_count, 1);
}

#[test]
fn review_lane_subagent_start_does_not_count_toward_review_gate() {
    let repo = fresh_repo();
    let sid = "s5-review-lane";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "review",
            "fork_context": false
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(
        state.subagent_start_count, 1,
        "review lane is in registry reviewer_lanes"
    );
    assert!(
        !state.review_subagent_pending_cycle_keys.is_empty()
            || !state.review_lite_pending_cycle_keys.is_empty(),
        "review lane with independent fork enqueues pending cycle keys"
    );
}

#[test]
fn review_subagent_start_with_shared_fork_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5-shared", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": "s5-shared",
            "subagent_type": "explore",
            "fork_context": true
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s5-shared", "subagent_type": "explore" }),
    );
    let state = load_state_for(&repo, "s5-shared");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_start_count, 0);
    assert_eq!(state.subagent_stop_count, 0);
}

#[test]
fn review_subagent_start_without_explicit_fork_false_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5-missing-fork", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s5-missing-fork", "subagent_type": "explore" }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s5-missing-fork", "subagent_type": "explore" }),
    );
    let state = load_state_for(&repo, "s5-missing-fork");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_start_count, 0);
    assert_eq!(state.subagent_stop_count, 0);
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("s5-missing-fork", "继续"));
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn stop_releases_l3_before_continuity_checkpoint() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "s-stop-l3-release";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let out = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    assert_ne!(
        out.get("permission").and_then(Value::as_str),
        Some("deny"),
        "stop must release L3 before returning so a later hook can acquire; out={out:?}"
    );
}

#[test]
fn review_gate_soft_nag_includes_need_segment() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _cap_env = ReviewGateStopMaxNudgesEnvGuard::set("1");
    assert_eq!(
        hooks::router_rs_review_gate_stop_max_nudges_cap(),
        Some(1)
    );
    let repo = fresh_repo();
    let sid = "s-soft-need";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("mode=soft_nag") && fm.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "soft-nag followup must include need= segment; fm={fm:?}"
    );
}

#[test]
fn session_end_acquires_lock_before_state_delete() {
    let repo = fresh_repo();
    let payload = event("s-end-lock", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let sp = state_path(&repo, &payload);
    assert!(sp.exists());
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(!sp.exists(), "sessionEnd must remove review gate state");
    let mut lock = acquire_state_lock(&repo, &payload);
    assert!(lock.is_some(), "sessionEnd must not leave L3 wedged");
    release_state_lock(&mut lock);
}

#[test]
fn post_tool_skips_cargo_check_when_env_off() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC");
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC", "0") };
    assert!(
        !hooks::router_rs_cargo_check_sync_enabled(),
        "env off must disable sync cargo check gate"
    );
    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_CARGO_CHECK_SYNC") },
    }
}

#[test]
fn review_gate_stop_softens_after_max_nudges_env_cap() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _cap_env = ReviewGateStopMaxNudgesEnvGuard::set("2");
    assert_eq!(
        hooks::router_rs_review_gate_stop_max_nudges_cap(),
        Some(2)
    );
    let repo = fresh_repo();
    let sid = "s-rg-stop-nudge-cap";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let out1 = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm1 = out1
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out1));
    assert!(
        fm1.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "first stop should keep full need= in followup_message; out1={out1:?}"
    );

    let out2 = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm2 = out2
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out2));
    assert!(
        fm2.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "second stop still within cap=2; out2={out2:?}"
    );

    let out3 = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm3 = out3
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    let blob3 = hook_user_visible_blob(&out3);
    assert!(
        fm3.contains("mode=soft_nag") && fm3.contains("router-rs REVIEW_GATE"),
        "third stop should shorten followup_message; fm3={fm3:?}"
    );
    assert!(
        fm3.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT),
        "need= must stay in followup_message after cap (not only additional_context); fm3={fm3:?}"
    );
    assert!(
        fm3.contains(REVIEW_GATE_FOLLOWUP_HINT_SEGMENT),
        "hint= must stay in followup_message after cap; fm3={fm3:?}"
    );
    assert!(
        !blob3.contains("continuity_suppressed=review_soft_nag"),
        "soft-nag over cap must not block My/RFV merge (P1-4); blob3={blob3:?}"
    );
}

#[test]
fn session_end_skips_state_delete_when_lock_unavailable() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s-end-no-lock", "全面review这个仓库");
    let sp = state_path(&repo, &payload);
    if let Some(parent) = sp.parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(
        &sp,
        serde_json::to_string(&empty_state()).expect("serialize"),
    )
    .expect("seed state");
    assert!(sp.exists());
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(
        sp.exists(),
        "sessionEnd must skip delete when lock unavailable (D7)"
    );
}

#[test]
fn hook_silent_strips_additional_context_keeps_review_gate_followup() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_HOOK_SILENT");
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", "1") };
    let mut out = json!({
        "followup_message": format!(
            "router-rs REVIEW_GATE incomplete phase=0 {} {}",
            REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
        ),
        "additional_context": "Continuity digest: noisy advisory text",
    });
    apply_cursor_hook_silent_policy(&mut out);
    assert!(out.get("additional_context").is_none());
    let fm = out["followup_message"].as_str().unwrap_or("");
    assert!(fm.contains("router-rs REVIEW_GATE"));
    assert!(fm.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT));
    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_HOOK_SILENT", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_HOOK_SILENT") },
    }
}

#[test]
fn review_pending_not_cleared_when_stale_after_disabled() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS");
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", "0") };

    let repo = fresh_repo();
    let sid = "s-pending-no-prune-off";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-keep-pending",
        }),
    );
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "继续"));
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_subagent_pending_cycle_keys.is_empty(),
        "STALE_AFTER=0 must not clear pending via prune_stale_review_pending_cycle_keys"
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "STALE_AFTER=0 must not prune pending on stop; fm={fm}"
    );

    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS") },
    }
}

#[test]
fn review_pending_cycle_keys_respects_env_cap() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "2") };
    assert_eq!(
        hooks::router_rs_review_pending_cycle_max(),
        2,
        "env cap must be visible before dispatch"
    );

    let repo = fresh_repo();
    let sid = "s-pending-cap";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    for i in 0..3 {
        let _ = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({
                "session_id": sid,
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": format!("sa-cap-{i}"),
            }),
        );
    }
    let state = load_state_for(&repo, sid);
    assert_eq!(
        state.review_subagent_pending_cycle_keys.len(),
        2,
        "cap=2 must refuse third push, got {:?}",
        state.review_subagent_pending_cycle_keys
    );
    assert!(
        !state
            .review_subagent_pending_cycle_keys
            .iter()
            .any(|k| k == "id:sa-cap-2"),
        "third key must be refused at cap"
    );

    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}

#[test]
fn v1_migrate_pending_preserved_when_no_started_at_timestamp() {
    let repo = fresh_repo();
    let sid = "s-pending-orphan";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);

    let mut state = empty_state();
    state.core.review_required = true;
    state.phase = 2;
    state.review_subagent_pending_cycle_keys = vec!["id:orphan".to_string()];
    state.active_subagent_count = 0;
    state.active_subagent_last_started_at = None;
    assert!(save_state(&repo, &payload, &mut state));

    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let loaded = load_state_for(&repo, sid);
    assert_eq!(
        loaded.review_subagent_pending_cycle_keys,
        vec!["id:orphan".to_string()],
        "v1 migrate fixture must not clear pending without timestamp"
    );
}

#[test]
fn review_pending_cycle_pruned_when_no_open_subagents_and_stale_start() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS");
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", "1") };

    let repo = fresh_repo();
    let sid = "s-pending-prune";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-prune-1",
        }),
    );

    let sp = state_path(&repo, &payload);
    let raw = fs::read_to_string(&sp).expect("read state");
    let mut state: Value = serde_json::from_str(&raw).expect("parse state");
    state["phase"] = json!(3);
    state["review_subagent_pending_cycle_keys"] = json!(["id:sa-prune-1"]);
    state["active_subagent_count"] = json!(0);
    state["active_subagent_last_started_at"] = json!("2000-01-01T00:00:00+00:00");
    fs::write(&sp, serde_json::to_string(&state).expect("serialize")).expect("write state");

    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "stale pending without qualifying stop must not clear REVIEW_GATE; fm={fm}"
    );
    let st = load_state_for(&repo, sid);
    assert_eq!(st.phase, 2, "phase must downgrade from 3 when pruning without stop");

    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS") },
    }
}

#[test]
fn review_subagent_start_without_reviewer_lane_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s5-untyped", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s5-untyped", "fork_context": false }),
    );
    let state = load_state_for(&repo, "s5-untyped");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_start_count, 0);
}

#[test]
fn subagent_start_blocks_when_active_limit_reached() {
    let repo = fresh_repo();
    for _ in 0..DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS {
        let out = dispatch_cursor_hook_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s-open-limit", "subagent_type": "explore" }),
        );
        assert_eq!(out, json!({}));
    }

    let out = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s-open-limit", "subagent_type": "explore" }),
    );

    assert_eq!(out.get("permission").and_then(Value::as_str), Some("deny"));
    assert!(out
        .get("user_message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .contains("仍标记为打开"));
    let state = load_state_for(&repo, "s-open-limit");
    assert_eq!(
        state.active_subagent_count,
        DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS
    );
}

#[test]
fn subagent_start_recovers_stale_active_count() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let repo = fresh_repo();
    let payload = json!({ "session_id": "s-open-stale", "subagent_type": "explore" });
    let stale_started_at =
        Utc::now() - chrono::Duration::seconds(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS + 1);
    let mut state = empty_state();
    state.active_subagent_count = DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS;
    state.active_subagent_last_started_at = Some(stale_started_at.to_rfc3339());
    assert!(save_state(&repo, &payload, &mut state));

    let out = dispatch_cursor_hook_event(&repo, "subagentStart", &payload);

    assert_eq!(out, json!({}));
    let state = load_state_for(&repo, "s-open-stale");
    assert_eq!(state.active_subagent_count, 1);
    assert!(state.active_subagent_last_started_at.is_some());
}

#[test]
fn subagent_stop_decrements_active_count_without_review_gate() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": "s-open-stop", "subagent_type": "explore" }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s-open-stop", "subagent_type": "explore" }),
    );

    let state = load_state_for(&repo, "s-open-stop");
    assert_eq!(state.active_subagent_count, 0);
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_stop_count, 0);
}

#[test]
fn subagent_stop_without_start_does_not_promote_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s6", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s6", "subagent_type": "explore" }),
    );
    let state = load_state_for(&repo, "s6");
    assert_eq!(state.phase, 0);
    assert_eq!(state.subagent_stop_count, 0);
}

/// `explore` + `fork_context=false` 在门控启用时不得当作深度审稿 lane；随后的 `general-purpose` 完整周期仍可清相位。
#[test]
fn armed_review_explore_posttool_then_general_purpose_cycle_clears_phase() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-explore-then-gp";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(load_state_for(&repo, sid).core.review_required);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {"subagent_type":"explore","fork_context":false}
        }),
    );
    let after_explore = load_state_for(&repo, sid);
    assert!(
        after_explore.phase < 2,
        "explore must not bump review gate phase; phase={}",
        after_explore.phase
    );

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false
        }),
    );
    assert_eq!(load_state_for(&repo, sid).phase, 2);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({"session_id": sid, "subagent_type": "general-purpose"}),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 3);
    assert_eq!(state.subagent_stop_count, 1);
}

/// PostTool 返回后应核销 pending（无需 subagentStop），避免长期卡在 phase=2。
#[test]
fn post_tool_settles_review_cycle_without_subagent_stop() {
    let repo = fresh_repo();
    let sid = "s-posttool-settle";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "deep-reviewer",
            "fork_context": false,
            "subagent_id": "settle-only",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "Task",
            "tool_input": {
                "subagent_type": "deep-reviewer",
                "fork_context": false,
                "subagent_id": "settle-only",
            }
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 3, "PostTool must settle pending to phase 3");
    assert!(
        state.review_subagent_pending_cycle_keys.is_empty(),
        "pending must drain after PostTool settle"
    );
    let stop = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "收尾"));
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !fm.contains("REVIEW_GATE incomplete"),
        "gate must clear after PostTool settle; fm={fm}"
    );
}

// Cursor-only: phase / subagent_stop_count telemetry — see `review_gate_two_distinct_*`

#[test]
fn subagent_start_then_stop_promotes_to_phase3() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s6b", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": "s6b",
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-1"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s6b", "subagent_type": "general-purpose", "subagent_id": "review-1" }),
    );
    let state = load_state_for(&repo, "s6b");
    assert_eq!(state.phase, 3);
    assert_eq!(state.subagent_stop_count, 1);
}

#[test]
fn post_tool_use_fast_path_skips_tracker_for_read() {
    let repo = fresh_repo();
    let sid = "s-fast-read";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "Read",
            "tool_input": { "path": "README.md" }
        }),
    );
    let tracker = repo.join("artifacts/current/SESSION_CALL_TRACKER.json");
    assert!(
        !tracker.is_file(),
        "fast-path Read must not write SESSION_CALL_TRACKER"
    );
}

#[test]
fn post_tool_armed_lock_failure_fails_closed_with_continue_false() {
    let _gate = ReviewGateActiveGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let sid = "s-posttool-armed-lock";
    let submit = event(sid, "全面review这个仓库");
    let mut armed = empty_state();
    armed.core.review_required = true;
    armed.phase = 1;
    if let Some(parent) = state_path(&repo, &submit).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(
        state_path(&repo, &submit),
        serde_json::to_string(&armed).expect("serialize"),
    )
    .expect("seed review-armed state");
    let lock_path = state_lock_path(&repo, &submit);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "Task",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "pt-lock"
            }
        }),
    );
    assert_eq!(out.get("continue"), Some(&json!(false)));
    assert!(
        hook_user_visible_blob(&out).contains("锁不可用"),
        "out={out:?}"
    );
}

#[test]
fn post_tool_use_still_records_subagent_on_task_tool() {
    let _guard = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-fast-task";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "Task",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "pt-only"
            }
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(
        state.phase, 3,
        "Task postToolUse must enqueue then settle review cycle when gate armed"
    );
    assert!(
        state.review_subagent_pending_cycle_keys.is_empty(),
        "PostTool settle must drain pending"
    );
}

/// `subagentStart` 与随后同一 `subagent_id` 的 `PostToolUse` 不应对 **`id:`** multiset 双入队。
#[test]
fn review_gate_posttool_skips_duplicate_id_after_subagent_start() {
    let repo = fresh_repo();
    let sid = "s-dedupe-id";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "same-id"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "same-id"
            }
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(
        mid.subagent_start_count, 1,
        "PostTool must not bump subagent_start_count"
    );
    assert_eq!(mid.phase, 3, "PostTool must settle same-id cycle after subagentStart");
    assert!(
        mid.review_subagent_pending_cycle_keys.is_empty(),
        "PostTool must not leave duplicate pending for same id"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "same-id"
        }),
    );
    let end = load_state_for(&repo, sid);
    assert_eq!(end.phase, 3);
    assert!(end.review_subagent_pending_cycle_keys.is_empty());
}

/// 同 session：`subagentStart` + `PostToolUse`（同 `lane:`、无 id）仅一条 pending；单次 stop 清门。
#[test]
fn review_gate_dual_event_lane_dedup_single_stop_clears() {
    let repo = fresh_repo();
    let sid = "s-dedupe-lane";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": { "subagent_type": "general-purpose", "fork_context": false }
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(mid.phase, 3, "PostTool must settle lane-key cycle");
    assert!(
        mid.review_subagent_pending_cycle_keys.is_empty(),
        "dual-event lane must not double pending"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": sid, "subagent_type": "general-purpose" }),
    );
    let end = load_state_for(&repo, sid);
    assert_eq!(end.phase, 3);
    assert!(end.review_subagent_pending_cycle_keys.is_empty());
}

#[test]
fn before_submit_fail_closed_when_hook_state_dir_readonly() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _rg = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "save-fail-closed";
    let dir = repo.join(".cursor/hook-state");
    fs::create_dir_all(&dir).expect("mkdir hook-state");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dir).expect("meta").permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&dir, perms).expect("chmod");
    }
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面深度 review 这段代码"),
    );
    assert_eq!(
        out.get("continue").and_then(Value::as_bool),
        Some(false),
        "fail-closed (ADR-002): {out:?}"
    );
    let blocked = out
        .get("user_message")
        .and_then(Value::as_str)
        .or_else(|| out.get("followup_message").and_then(Value::as_str))
        .unwrap_or("");
    assert!(
        blocked.contains("未能持久化")
            || blocked.contains("锁不可用")
            || blocked.contains("已拦截"),
        "expected blocked persist or lock message: {out:?}"
    );
}

#[test]
fn before_submit_planx_persist_fail_soft_warning_not_block() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _rg = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "save-fail-soft-planx";
    let dir = repo.join(".cursor/hook-state");
    fs::create_dir_all(&dir).expect("mkdir hook-state");
    #[cfg(unix)]
    let _readonly = HookStateDirReadonlyGuard::readonly(dir.clone());
    #[cfg(not(unix))]
    {
        let _ = &dir;
    }
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "/planx"),
    );
    assert_eq!(
        out.get("continue").and_then(Value::as_bool),
        Some(true),
        "pre-exec /planx must not fail-closed on persist when review/goal unarmed: {out:?}"
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("未能持久化") || ac.contains("锁不可用"),
        "expected soft persist/lock degrade warning in additional_context: {out:?}"
    );
}

#[test]
fn rearm_review_resets_active_subagent_count_after_start_without_stop() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-rearm-open-subagent";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "深度 review 这个 PR"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-open",
        }),
    );
    let open = load_state_for(&repo, sid);
    assert!(
        open.active_subagent_count > 0,
        "subagentStart must increment open count; got {open:?}"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "Please do another code review of this change."),
    );
    let rearmed = load_state_for(&repo, sid);
    assert_eq!(
        rearmed.active_subagent_count, 0,
        "re-arm must reset open subagent count (P1-16); got {rearmed:?}"
    );
    assert_eq!(rearmed.phase, 0, "re-arm must reset phase; got {rearmed:?}");
}

#[test]
fn pending_cap_refused_survives_review_rearm_ups() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1") };
    let repo = fresh_repo();
    let sid = "s-cap-rearm";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-cap-1",
        }),
    );
    let cap_denied = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-cap-2",
        }),
    );
    assert_eq!(
        cap_denied.get("permission").and_then(Value::as_str),
        Some("deny"),
        "cap must deny second spawn: {cap_denied:?}"
    );
    assert!(
        load_state_for(&repo, sid).review_pending_cap_refused,
        "cap denial must latch review_pending_cap_refused"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "Please do another code review of this change."),
    );
    assert!(
        load_state_for(&repo, sid).review_pending_cap_refused,
        "review re-arm UPS must not clear cap refusal"
    );
    let cap_denied_again = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-cap-3",
        }),
    );
    assert_eq!(
        cap_denied_again.get("permission").and_then(Value::as_str),
        Some("deny"),
        "cap refusal must survive re-arm: {cap_denied_again:?}"
    );
    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}

#[test]
fn legacy_phase_two_alone_compact_does_not_clear_review_gate() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-legacy-phase2";
    let state_path = state_path(&repo, &event(sid, ""));
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(
        &state_path,
        r#"{"version":1,"review_required":true,"review_subagent_seen":true}"#,
    )
    .expect("write v1 legacy state");
    let stop = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "status": "completed",
            "loop_count": 0,
            "response": "[P1] scripts/foo.rs:1 — issue — impact — verify",
        }),
    );
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "legacy phase=2 without live subagent evidence must not clear gate; fm={fm:?}"
    );
    assert!(
        load_state_for(&repo, sid).phase < 3,
        "compact must not bump to phase 3 without live evidence"
    );
}

struct MyLightOverrideGuard {
    _env: core_policy::test_env_sync::ProcessEnvLockGuard,
}

impl MyLightOverrideGuard {
    fn force_non_my_light() -> Self {
        let guard = core_policy::test_env_sync::process_env_lock();
        core_policy::hook_common::set_test_my_light_override(Some(false));
        Self { _env: guard }
    }
}

impl Drop for MyLightOverrideGuard {
    fn drop(&mut self) {
        core_policy::hook_common::set_test_my_light_override(None);
    }
}

#[test]
fn stale_hygiene_compact_does_not_clear_review_gate() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = std::env::var_os("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS");
    unsafe { std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", "1") };
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-stale-compact";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "stale-compact-1",
        }),
    );
    let sp = state_path(&repo, &payload);
    let mut state: Value =
        serde_json::from_str(&fs::read_to_string(&sp).expect("read state")).expect("parse state");
    state["active_subagent_count"] = json!(1);
    state["active_subagent_last_started_at"] = json!("2000-01-01T00:00:00+00:00");
    fs::write(
        &sp,
        serde_json::to_string(&state).expect("serialize"),
    )
    .expect("write stale state");
    let stop = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "status": "completed",
            "loop_count": 0,
            "response": "[P1] scripts/foo.rs:1 — issue — impact — verify",
        }),
    );
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "stale hygiene + compact must not clear gate; fm={fm:?}"
    );
    let st = load_state_for(&repo, sid);
    assert_eq!(
        st.subagent_start_count, 0,
        "stale hygiene must invalidate orphan start_count"
    );
    assert!(st.phase < 3, "compact must not bump phase without stop/pending");
    match prev {
        Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS", v) },
        None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS") },
    }
}

#[test]
fn posttool_at_pending_cap_persists_refused_latch() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1") };
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-posttool-cap-persist";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "cap-1",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "cap-2"
            }
        }),
    );
    let st = load_state_for(&repo, sid);
    assert!(
        st.review_pending_cap_refused,
        "PostTool cap refusal must persist review_pending_cap_refused; got {st:?}"
    );
    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}

#[test]
fn before_submit_review_and_implementx_injects_mixing_nudge_when_not_my_light() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    let sid = "dual-review-implementx-non-my-light";
    let prompt = "深度 review 整个路由系统 /implementx 修复刚发现的问题";
    let payload = json!({ "session_id": sid, "cwd": cwd, "prompt": prompt });
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("router-rs：本轮提交同时包含"),
        "non-my-light dual prompt must inject mixing nudge; got {ac:?}"
    );
    let state = load_state(&repo, &json!({ "session_id": sid, "cwd": cwd }))
        .expect("load ok")
        .expect("state exists");
    assert!(!state.core.review_required);
    assert!(state.goal_required);
}

#[test]
fn before_submit_benign_ups_returns_unreadable_when_hook_state_corrupt() {
    let repo = fresh_repo();
    let sid = "s-corrupt-benign-ups";
    let payload = event(sid, "hello there");
    if let Some(parent) = state_path(&repo, &payload).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(
        out.get("continue").and_then(Value::as_bool),
        Some(false),
        "corrupt hook-state benign UPS must fail-closed; got {out:?}"
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        msg.contains(super::CURSOR_HOOK_STATE_UNREADABLE),
        "corrupt hook-state must surface unreadable for benign UPS; got {out:?}"
    );
    let raw = fs::read_to_string(state_path(&repo, &payload)).expect("state still corrupt");
    assert!(
        raw.starts_with("{not json"),
        "benign UPS must not overwrite corrupt hook-state with empty_state"
    );
}

#[test]
fn deep_reviewer_lane_counts_for_review_gate() {
    let _gate = ReviewGateActiveGuard::new();
    assert!(core_policy::hook_common::is_reviewer_lane_normalized("deep-reviewer"));
    let repo = fresh_repo();
    let sid = "s-deep-reviewer-lane";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "deep-reviewer",
            "fork_context": false,
            "subagent_id": "dr-1",
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.subagent_start_count, 1);
    assert_eq!(state.phase, 2);
}

#[test]
fn before_submit_discussx_returns_unreadable_when_hook_state_corrupt() {
    let repo = fresh_repo();
    let sid = "s-corrupt-discussx-cursor";
    let payload = event(sid, "/discussx");
    if let Some(parent) = state_path(&repo, &payload).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(
        out.get("continue").and_then(Value::as_bool),
        Some(false),
        "corrupt hook-state /discussx UPS must fail-closed; got {out:?}"
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        msg.contains(super::CURSOR_HOOK_STATE_UNREADABLE),
        "corrupt hook-state must surface unreadable for /discussx; got {out:?}"
    );
    assert!(
        !msg.contains("pre-execution"),
        "must not mask corrupt state with discussx nudge; got {out:?}"
    );
}

#[test]
fn pending_cap_denial_does_not_increment_active_subagent_count() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1") };
    assert_eq!(
        hooks::router_rs_review_pending_cycle_max(),
        1
    );

    let repo = fresh_repo();
    let sid = "s-cap-atomic";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-1",
        }),
    );
    let cap_denied = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "sa-2",
        }),
    );
    assert_eq!(
        cap_denied.get("permission").and_then(Value::as_str),
        Some("deny"),
        "ADR-004: cap refusal must deny subagentStart: {cap_denied:?}"
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(
        mid.review_subagent_pending_cycle_keys.len(),
        1,
        "cap=1 must refuse second pending: {:?}",
        mid.review_subagent_pending_cycle_keys
    );
    assert_eq!(
        mid.active_subagent_count, 1,
        "cap refusal must not bump open count"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "sa-1",
        }),
    );
    let after_stop = load_state_for(&repo, sid);
    assert_eq!(
        after_stop.active_subagent_count, 0,
        "stop must not leave phantom open count"
    );
    assert_ne!(
        after_stop.phase, 2,
        "must not stick at phase 2 with zero open subagents after sole stop"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}

#[test]
fn posttool_at_pending_cap_does_not_bump_phase() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "1") };

    let repo = fresh_repo();
    let sid = "s-posttool-cap";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "cap-1",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": sid,
            "tool_name": "functions.subagent",
            "tool_input": {
                "subagent_type": "general-purpose",
                "fork_context": false,
                "subagent_id": "cap-1"
            }
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(
        mid.phase, 3,
        "postTool on existing pending must settle, not leave stuck phase=2"
    );
    assert!(
        mid.review_subagent_pending_cycle_keys.is_empty(),
        "cap full: postTool must not add phantom pending: {:?}",
        mid.review_subagent_pending_cycle_keys
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}
