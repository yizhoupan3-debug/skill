// Cross-host review-gate contract tests live in `hook_contract_matrix` (Epic E).
// Portable matrix owners (Cursor duplicates removed): deep_review_advisory, spawn_first_on/off,
// my_light_suppress/clear/assistant_only, narrow_review/disarm, review_gate_disabled_spawn,
// closeout_blocks, user_override, reject/rg_clear tokens, independent_reviewer_clears,
// fork_infer, second_deep_rearm, paper_prose_default — plus canonical/legacy disable rows.

#[test]
fn before_submit_review_and_implementx_same_prompt_suppresses_review_but_arms_goal() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "dual-review-implementx";
    let prompt = "请全面review这个仓库 /implementx 修复刚发现的问题";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, prompt));
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_required,
        "my-light + goal drive must not arm review; got {state:?}"
    );
    assert!(
        !state.goal_required,
        "my-light /implementx must not arm goal_required on Stop path; got {state:?}"
    );
}

/// 未命中「并行 review 候选」三元时仍注入同一行指针；不再追加第二段「≥3」以免刷屏。
#[test]
fn before_submit_review_prompt_compact_nudge_has_no_second_breadth_paragraph() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _spawn_on = SpawnFirstNudgeEnableEnvGuard::enable();
    let repo = fresh_repo();
    let sid = "s-review-no-breadth-scope";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "security code review"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("skills/code-review-deep/SKILL.md") && ac.contains("fork_context=false"),
        "expected spawn-first pointer; got {ac:?}"
    );
    assert!(
        !ac.contains("≥3"),
        "hook must not append a separate ≥3 breadth paragraph; got {ac:?}"
    );
}

#[test]
fn parallel_delegation_does_not_latch_delegation() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s2", "请前端后端测试并行分头执行"),
    );
    let state = load_state_for(&repo, "s2");
    assert_eq!(state.phase, 0);
}

#[test]
fn my_implement_entry_does_not_arm_delegation_or_review_from_fix_copy() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(
            "ap-del",
            "/implementx address all review findings from the last pass",
        ),
    );
    let state = load_state_for(&repo, "ap-del");
    assert!(
        !state.review_required,
        "My implement turn must not re-arm review from findings wording"
    );
    assert!(
        !state.goal_required,
        "my-light implementx must not arm goal_required"
    );
}

#[test]
fn before_submit_review_and_goal_drive_same_prompt_merges_mixing_hint() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    core_policy::hook_common::set_test_my_light_override(None);
    let repo = fresh_repo();
    let sid = "s-dual-review-pre-goal-hint";
    let prompt = "请全面review这个仓库 /implementx 修复刚发现的问题";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, prompt));
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("router-rs：本轮提交同时包含"),
        "my-light /implementx must not inject review+goal mixing nudge; got {ac:?}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        !state.review_required,
        "same-submit goal_drive must suppress review arming; got {state:?}"
    );
    assert!(
        !state.goal_required,
        "my-light implementx must not arm goal_required"
    );
}

// Cursor-only: my-light profile predicate — covered by `before_submit_review_*_mixing_*` / `my_implement_entry_*`

#[test]
fn before_submit_review_with_disk_goal_non_my_light_injects_mixing_hint() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-team-mix-hint";
    let cwd = repo.display().to_string();
    let payload = json!({
        "session_id": sid,
        "cwd": cwd,
        "prompt": "深度 review 整个路由系统 /implementx 继续"
    });
    // Re-assert after env-lock wait: parallel tests may reset the thread-local override.
    core_policy::hook_common::set_test_my_light_override(Some(false));
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("router-rs：本轮提交同时包含"),
        "non-my-light review+implementx must inject split hint; got {ac:?}"
    );
    let state = load_state(&repo, &json!({ "session_id": sid, "cwd": cwd }))
        .expect("load ok")
        .expect("state exists");
    assert!(
        !state.review_required,
        "same-submit implementx must disarm review arming; got {state:?}"
    );
    assert!(state.goal_required);
}

#[test]
fn before_submit_implementx_injects_one_breath_nudge() {
    let repo = fresh_repo();
    let sid = "s-implement-nudge";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("ALL waves") || ac.contains("WAVE_STATE"),
        "implementx must inject MY_IMPLEMENT nudge; got {ac:?}"
    );
}

#[test]
fn before_submit_implementx_injects_subagent_model_inherit_nudge() {
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let repo = fresh_repo();
    let sid = "s-model-nudge-impl";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("继承主会话"),
        "implementx must inject model inherit nudge; got {ac:?}"
    );
    let model_pos = ac.find("继承主会话").expect("model nudge");
    let goal_pos = ac
        .find("ALL waves")
        .or_else(|| ac.find("WAVE_STATE"))
        .expect("implement goal nudge");
    assert!(
        model_pos < goal_pos,
        "model inherit must precede implement one-breath nudge; got {ac:?}"
    );
}

#[test]
fn before_submit_spawn_first_and_model_inherit_not_duplicated() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let _gate = ReviewGateActiveGuard::new();
    let _spawn_on = SpawnFirstNudgeEnableEnvGuard::enable();
    let repo = fresh_repo();
    let sid = "s-spawn-model-dedup";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个路由系统"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    let count = ac.matches("继承主会话").count();
    assert_eq!(
        count, 1,
        "spawn-first already includes model inherit; must not duplicate; got {ac:?}"
    );
}

#[test]
#[serial]
fn before_submit_model_inherit_survives_output_policy_truncation() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let _cap_lock = hook_outbound_context_max_chars_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS");
    unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", "900") };
    let repo = fresh_repo();
    let sid = "s-model-trunc-survive";
    let mut out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    super::apply_cursor_hook_output_policy(&mut out);
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("继承主会话"),
        "model inherit must stay in prefix after outbound truncation; got {ac:?}"
    );
    match prev_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS") },
    }
}

#[test]
fn before_submit_my_light_review_still_injects_model_inherit_without_spawn_first() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let _env = SubagentModelInheritNudgeForceOnEnvGuard::new();
    let repo = fresh_repo();
    let sid = "s-model-nudge-my-light";
    let _nudge_off = SpawnFirstNudgeDisableEnvGuard::disable();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "/discussx then 全面review这个路由系统"),
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("继承主会话"),
        "my-light must still get model inherit nudge when spawn-first off; got {ac:?}"
    );
}

#[test]
fn subagent_model_inherit_nudge_disabled_injects_no_model_line() {
    let _off = SubagentModelInheritNudgeDisableEnvGuard::disable();
    let repo = fresh_repo();
    let sid = "s-model-nudge-off";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/implementx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("继承主会话"),
        "MODEL_INHERIT_NUDGE=0 must omit model line; got {ac:?}"
    );
}

#[test]
fn before_submit_verifyx_injects_generic_goal_nudge() {
    let repo = fresh_repo();
    let sid = "s-verify-nudge";
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/verifyx"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("skills/verifyx/SKILL.md"),
        "verifyx must inject MY_GOAL_DRIVE nudge; got {ac:?}"
    );
    assert!(
        !ac.contains("ALL waves") && !ac.contains("WAVE_STATE"),
        "verifyx must not inject implement one-breath nudge; got {ac:?}"
    );
}

// Cursor-only: phase / active_subagent_count re-arm — see `cursor_rearm_review_*`

#[test]
fn before_submit_implementx_returns_unreadable_when_hook_state_corrupt() {
    let repo = fresh_repo();
    let sid = "s-corrupt-impl-cursor";
    let payload = event(sid, "/implementx");
    if let Some(parent) = state_path(&repo, &payload).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(
        out.get("continue").and_then(Value::as_bool),
        Some(false),
        "corrupt hook-state UPS must fail-closed (symmetric with Stop); got {out:?}"
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        msg.contains(super::CURSOR_HOOK_STATE_UNREADABLE),
        "corrupt hook-state must surface unreadable; got {out:?}"
    );
    assert!(
        !msg.contains("ALL waves"),
        "must not mask corrupt state with implement nudge; got {out:?}"
    );
}

#[test]
fn rearm_review_resets_review_followup_count_after_soft_nag() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _spawn_on = SpawnFirstNudgeEnableEnvGuard::enable();
    let _cap_env = ReviewGateStopMaxNudgesEnvGuard::set("2");
    let repo = fresh_repo();
    let sid = "s-rearm-followup-reset";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "深度 review 这个 PR"),
    );
    for _ in 0..3 {
        let stop = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
        assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&stop));
    }
    assert!(
        load_state_for(&repo, sid).review_followup_count >= 3,
        "advisory Stop nudges must accumulate review_followup_count"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-first",
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-first",
        }),
    );
    assert_eq!(load_state_for(&repo, sid).phase, 3);
    let rearm_out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "Please do another code review of this change."),
    );
    let rearmed = load_state_for(&repo, sid);
    assert_eq!(
        rearmed.review_followup_count, 0,
        "re-arm must reset review_followup_count; got {rearmed:?}"
    );
    let ac = rearm_out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("skills/code-review-deep/SKILL.md"),
        "re-arm must reinject spawn-first nudge; got {ac:?}"
    );
    let stop = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "done"));
    let fm = stop
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains(REVIEW_GATE_FOLLOWUP_NEED_SEGMENT) && !fm.contains("mode=soft_nag"),
        "fresh review cycle must not inherit prior soft_nag on first Stop; fm={fm:?}"
    );
}

#[test]
fn before_submit_my_new_project_does_not_arm_goal_required() {
    let repo = fresh_repo();
    let sid = "my-new-project-pre-exec";
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "请 /discussx 做迁移后技术债审查"),
    );
    assert_eq!(out.get("continue").and_then(Value::as_bool), Some(true));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        ac.contains("My lifecycle pre-execution"),
        "expected pre-exec nudge; got {ac:?}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        !state.goal_required,
        "pre-exec /discussx must not arm goal_required; got {state:?}"
    );
}

#[test]
fn before_submit_my_plan_phase_does_not_arm_goal_required() {
    let repo = fresh_repo();
    let sid = "my-plan-pre-exec";
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event(sid, "/planx"));
    let state = load_state_for(&repo, sid);
    assert!(
        !state.goal_required,
        "/planx must not arm goal_required; got {state:?}"
    );
}

#[test]
fn plan_build_path_does_not_arm_goal() {
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    let plan_ref = format!("{cwd}/.cursor/plans/feature.plan.md");
    let payload = json!({
        "session_id": "plan-build",
        "cwd": cwd,
        "prompt": format!("Implement {plan_ref}"),
    });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let st_off = load_state_for(&repo, "plan-build");
    assert!(
        !st_off.goal_required,
        "plan path alone must not arm goal_required"
    );
}

#[test]
#[serial]
fn stop_closeout_uses_hydration_task_when_active_completed_and_focus_running() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
    unsafe { env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1") };

    let repo = fresh_repo();
    for id in ["done-active", "drive-focus"] {
        fs::create_dir_all(repo.join("artifacts/current").join(id)).expect("mkdir");
    }
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"done-active"}"#,
    )
    .expect("active ptr");
    fs::write(
        repo.join("artifacts/current/focus_task.json"),
        r#"{"task_id":"drive-focus"}"#,
    )
    .expect("focus ptr");
    // Pointer 机制已移除：写入 task_registry.json 供回退使用
    fs::write(
        repo.join("artifacts/current/task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"drive-focus","tasks":[{"task_id":"done-active"},{"task_id":"drive-focus"}]}"#,
    )
    .expect("task registry");
    fs::write(
        repo.join("artifacts/current/done-active/GOAL_STATE.json"),
        r#"{"schema_version":"router-rs-goal-v1","goal":"done","status":"completed","drive_until_done":false,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test"]}"#,
    )
    .expect("active goal");
    fs::write(
        repo.join("artifacts/current/drive-focus/GOAL_STATE.json"),
        r#"{"schema_version":"router-rs-goal-v1","goal":"drive","status":"running","drive_until_done":true,"non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test"]}"#,
    )
    .expect("focus goal");

    let msg = stop_hard_closeout_followup_for_assistant_response(&repo, "已完成")
        .expect("closeout followup");
    assert!(
        msg.contains("task_id=drive-focus"),
        "closeout must align with hydration pointer, not stale active; got {msg}"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
#[serial]
fn stop_completion_claim_allows_when_closeout_record_passes() {
    let _env = core_policy::test_env_sync::process_env_lock();
    use std::env;
    let _gate_disable_guard = ReviewGateDisableEnvClearGuard::new();
    let prev = env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
    unsafe { env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1") };

    let repo = fresh_repo();
    let tid = "t-closeout-ok";
    write_active_task(&repo, tid);
    write_goal_state_completed(&repo, tid);
    // Ensure evidence exists or provide commands_run in record (R7/R8 coverage).
    fs::write(
        repo.join("artifacts/current")
            .join(tid)
            .join("EVIDENCE_INDEX.json"),
        r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0,"success":true}]}"#,
    )
    .expect("write evidence");
    write_closeout_record(
        &repo,
        tid,
        r#"{
  "schema_version": "closeout-record-v1",
  "task_id": "t-closeout-ok",
  "summary": "已完成并验证",
  "verification_status": "passed",
  "commands_run": [{"command":"cargo test","exit_code":0}]
}"#,
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &json!({
            "session_id": "s-closeout-2",
            "cwd": repo.display().to_string(),
            "prompt": "/implementx do thing"
        }),
    );

    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s-closeout-2",
            "cwd": repo.display().to_string(),
            "prompt": "ok",
            "response": "已完成",
        }),
    );
    let msg = hook_user_visible_blob(&out);
    assert!(
        !msg.contains("CLOSEOUT_FOLLOWUP"),
        "expected no closeout followup; got {msg:?}"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT") },
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn completion_claim_detector_matches_basic_tokens() {
    assert!(completion_claimed_in_text("done"));
    assert!(completion_claimed_in_text("已完成"));
    assert!(!completion_claimed_in_text("验证通过"));
    assert!(
        core_policy::hook_common::GOAL_CHAT_VERIFY_ZH_PHRASES
            .iter()
            .any(|p| "验证通过".contains(p))
    );
    assert!(completion_claimed_in_text("tests passed"));
    assert!(!completion_claimed_in_text("still working"));
}

#[test]
fn completion_claim_detector_ignores_completion_as_substring_gossip() {
    assert!(!completion_claimed_in_text("方案的完成度还可以"));
    assert!(!completion_claimed_in_text("讨论完成任务拆分"));
}

#[test]
fn my_skips_pre_goal_nag_when_goal_state_on_disk() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    use std::env;
    let prev_strict = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
    unsafe { env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "0") };

    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gt1")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gt1"}"#,
    )
    .expect("active");
    // Pointer 机制已移除：写入 task_registry.json 供回退使用
    fs::write(
        repo.join("artifacts/current/task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"gt1","tasks":[{"task_id":"gt1"}]}"#,
    )
    .expect("task registry");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gt1",
        "goal": "close review findings",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ap-disk", "/implementx 继续实现"),
    );
    assert!(
        load_state_for(&repo, "ap-disk").pre_goal_review_satisfied,
        "existing GOAL_STATE implies execution lane already opened"
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("My implement (/implementx") && !msg.contains("independent-context"),
        "pre-goal nag should be skipped when GOAL_STATE exists; msg={msg:?}"
    );

    match prev_strict {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK") },
    }
}

#[test]
fn my_pre_goal_strict_disk_skips_hydrate_pre_goal_on_before_submit() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    use std::env;
    let prev = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
    unsafe { env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "1") };

    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gt-strict")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gt-strict"}"#,
    )
    .expect("active");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gt-strict",
        "goal": "close review findings",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ap-disk-strict", "/implementx 继续实现"),
    );
    assert!(
        !load_state_for(&repo, "ap-disk-strict").pre_goal_review_satisfied,
        "strict disk: disk GOAL alone must not satisfy pre-goal on beforeSubmit"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK") },
    }
}

#[test]
fn stop_goal_gate_hydrates_from_goal_state_and_evidence_without_keywords() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-ev")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-ev"}"#,
    )
    .expect("active");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "t-ev",
        "goal": "fix review findings",
        "non_goals": ["avoid unrelated refactors"],
        "done_when": ["tests green", "review checklist cleared"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "checkpoint",
        "task_id": "t-ev",
        "note": "applied patch",
    }))
    .expect("checkpoint");
    fs::write(
            repo.join("artifacts/current/t-ev/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test -q","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ev-gate", "/implementx finish fixes"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "ev-gate",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "ok",
            "response": "done; no Goal:/Checkpoint:/verified boilerplate in prose"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "goal gate should hydrate from disk; msg={msg:?} out={out:?}"
    );
}

#[test]
fn stop_hydrates_when_hook_state_lacks_goal_required_but_goal_on_disk() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-nof")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-nof"}"#,
    )
    .expect("active");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "t-nof",
        "goal": "stdio seeded goal",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("start");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "checkpoint",
        "task_id": "t-nof",
        "note": "step",
    }))
    .expect("cp");
    fs::write(
        repo.join("artifacts/current/t-nof/EVIDENCE_INDEX.json"),
        r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
    )
    .expect("ev");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("noflag", "hello"));
    assert!(
        !load_state_for(&repo, "noflag").goal_required,
        "plain prompt must not arm goal_required before hydrate"
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "noflag",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "bye",
            "response": "done without magic words"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "GOAL_STATE on disk must hydrate despite goal_required=false; msg={msg:?}"
    );
}

#[test]
fn stop_clears_stale_goal_required_when_goal_purged_from_disk() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    let sid = "purge-goal";
    let hook_ev = |session: &str, prompt: &str| {
        json!({ "session_id": session, "cwd": cwd, "prompt": prompt })
    };
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &hook_ev(sid, "/implementx ship"),
    );
    let loaded = load_state(&repo, &hook_ev(sid, ""))
        .expect("load ok")
        .expect("state exists");
    assert!(
        loaded.goal_required,
        "implementx must arm goal_required"
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": cwd,
            "prompt": "bye",
            "response": "summary without contract headings"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "post-purge Stop must not require chat goal_contract; msg={msg:?}"
    );
    let after = load_state(&repo, &hook_ev(sid, ""))
        .expect("load ok")
        .expect("state exists");
    assert!(
        !after.goal_required,
        "hydrate must disarm orphan goal_required"
    );
}

#[test]
fn stop_hydrates_when_active_task_missing_but_goal_on_disk() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-orph")).expect("mkdir");
    fs::write(
            repo.join("artifacts/current/t-orph/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","goal":"no active_task json","status":"running","non_goals":["n"],"checkpoints":[{"note":"step"}],"done_when":["ship","review checklist cleared"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
    fs::write(
        repo.join("artifacts/current/t-orph/EVIDENCE_INDEX.json"),
        r#"{"schema_version":"evidence-index-v2","artifacts":[{"exit_code":0}]}"#,
    )
    .expect("ev");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("orph", "hello"));
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "orph",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "bye",
            "response": "done"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "scan fallback must hydrate when active_task.json is missing; msg={msg:?}"
    );
}

#[test]
fn stop_goal_gate_hydrates_running_goal_without_checkpoints_or_keywords() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-run")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-run"}"#,
    )
    .expect("active");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "t-run",
        "goal": "minimal running goal only",
        "non_goals": ["avoid unrelated refactors"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
        "lifecycle_profile": "my-light",
    }))
    .expect("goal start");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "checkpoint",
        "task_id": "t-run",
        "note": "w0",
    }))
    .expect("cp");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("run-gate", "/implementx continue"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "run-gate",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "ok",
            "response": "no Goal/Checkpoint/Verification boilerplate"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "running GOAL_STATE with non-empty goal should hydrate progress+verify; msg={msg:?}"
    );
}

#[test]
fn stop_disarms_goal_drive_when_active_task_pointer_missing() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-noptr")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/t-noptr/GOAL_STATE.json"),
        r#"{"schema_version":"router-rs-goal-v1","goal":"orphan goal on disk","status":"running","non_goals":["n"],"checkpoints":[{"at":"t","note":"c"}],"done_when":["d1","d2"],"validation_commands":["cargo test -q"],"drive_until_done":true}"#,
    )
    .expect("goal");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("noptr", "/implementx continue"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "noptr",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "ok",
            "response": "ok"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "broken/missing active_task pointer must not hard-block Stop; msg={msg:?}"
    );
}

#[test]
fn stop_my_light_on_disk_suppresses_ag_followup_without_slash_in_prompt() {
    let repo = fresh_repo();
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "t-my-disk",
        "goal": "my-light goal",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": false,
        "lifecycle_profile": "my-light",
    }))
    .expect("start");
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "my-disk",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "wrap up",
            "response": "done"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "lifecycle_profile on disk should enable my-light suppress; msg={msg:?}"
    );
}

#[test]
fn stop_goal_gate_hydrates_when_goal_state_omits_status_field() {
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/t-nost")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-nost"}"#,
    )
    .expect("active");
    fs::write(
            repo.join("artifacts/current/t-nost/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","goal":"hand-written without status","non_goals":["n"],"checkpoints":[],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal json");
    fs::write(
            repo.join("artifacts/current/t-nost/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test -q","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("ns-gate", "/implementx continue"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "ns-gate",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "ok",
            "response": "no chat boilerplate"
        }),
    );
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("AG_FOLLOWUP"),
        "missing status + non-empty goal should hydrate; msg={msg:?}"
    );
}

#[test]
fn override_phrase_in_chinese_disables_arming() {
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s3", "全面review这个仓库，不要用子代理"),
    );
    assert!(out.get("followup_message").is_none());
    let state = load_state_for(&repo, "s3");
    assert!(state.review_override);
    assert_eq!(state.phase, 0);
}

#[test]
fn stop_does_not_set_review_override_from_assistant_echo_alone() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-ov-echo", "全面review这个仓库"),
    );
    assert!(
        !load_state_for(&repo, "s-ov-echo").review_override,
        "user prompt must not imply review_override"
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s-ov-echo",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "全面review这个仓库",
            "response": "用户坚持不要用子代理，我仅在主会话输出 findings。"
        }),
    );
    let state = load_state_for(&repo, "s-ov-echo");
    assert!(
        !state.review_override,
        "assistant echo of override-like wording must not set review_override"
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
}

#[test]
fn stop_does_not_set_delegation_override_from_assistant_echo_when_review_armed() {
    let repo = fresh_repo();
    let sid = "s-delov-echo";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(!load_state_for(&repo, sid).delegation_override);
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "全面review这个仓库",
            "response": "项目经理说不要并行分头推进，我只好先在主会话出 findings。"
        }),
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
    let st = load_state_for(&repo, sid);
    assert!(
        !st.delegation_override,
        "`has_delegation_override`-like wording must not be read from assistant response alone"
    );
    assert!(
        !st.review_override,
        "sanity: user prompt did not request review bypass",
    );
}

#[test]
fn stop_does_not_set_delegation_override_from_assistant_global_override_echo_when_review_armed() {
    let repo = fresh_repo();
    let sid = "s-globov-echo";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(!load_state_for(&repo, sid).delegation_override);
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "全面review这个仓库",
            "response": "Stand-up recap: we'll handle this locally and summarize in chat."
        }),
    );
    assert_followup_signals_review_gate_incomplete(&hook_user_visible_blob(&out));
    let st = load_state_for(&repo, sid);
    assert!(
        !st.delegation_override,
        "`has_override` wording on Stop must not originate from assistant response alone",
    );
    assert!(
        !st.review_override,
        "sanity: user prompt did not request review bypass",
    );
}

#[test]
fn stop_user_parallel_opt_out_matches_has_override_and_delegation_regex_coupling() {
    // `hook_common::has_override` 与 delegation 正则均含中文「不要…并行/分工」；用户写入 Stop prompt
    // 时两行 `handle_stop` if 可同时置位，`review_hard_armed` 为假并解除未完成 reviewer 随访。
    let repo = fresh_repo();
    let sid = "s-user-parov";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "prompt": "我们不要并行分工了，先主线程输出",
            "response": "明白。",
        }),
    );
    let st = load_state_for(&repo, sid);
    assert!(st.delegation_override);
    assert!(
        st.review_override,
        "同一 `has_override` 句式同时推高 review/disarm branch"
    );
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("router-rs REVIEW_GATE incomplete"),
        "combined overrides disarm reviewer stop follow-up; blob={blob:?}",
    );
}

#[test]
fn nested_payload_response_reject_reason_does_not_satisfy_review_gate_on_stop() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s13nest-r", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "stop",
        &json!({
            "session_id": "s13nest-r",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": {
                "prompt": "继续",
                "response": "reject reason: shared_context_heavy"
            }
        }),
    );
    assert_review_gate_stop_nudge_absent(&hook_user_visible_blob(&out));
}

#[test]
fn nested_payload_response_sets_reject_reason_on_after_agent_response() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s13nest-a", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "s13nest-a",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": { "response": "small_task" }
        }),
    );
    assert!(load_state_for(&repo, "s13nest-a").reject_reason_seen);
}

#[test]
fn emergency_review_gate_disable_cold_after_agent_response_persists_reject_reason_seen() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let _env_clear = ReviewGateDisableEnvClearGuard::new();
    let _rg_disable = ReviewGateDisableTestGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "s-cold-ara",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "response": "reject reason: small_task"
        }),
    );
    assert!(
        load_state_for(&repo, "s-cold-ara").reject_reason_seen,
        "应急门控下仍以 `handle_after_agent_response` 写入 hook-state；无 beforeSubmit 冷启动亦应落盘 reject_reason_seen"
    );
}

#[test]
fn hook_signal_uses_structured_text_unless_full_scrape_enabled() {
    let event = json!({
        "session_id": "scrape-mode",
        "payload": {
            "unknown_transcript": "small_task"
        }
    });
    let compact = hook_event_signal_text_with_scrape_mode(&event, "latest user", "", false);
    assert!(compact.contains("latest user"));
    assert!(
        !compact.contains("small_task"),
        "default hot path must not scrape arbitrary transcript fields"
    );
    let full = hook_event_signal_text_with_scrape_mode(&event, "latest user", "", true);
    assert!(
        full.contains("small_task"),
        "explicit fallback mode should preserve unknown-field compatibility"
    );
}

#[test]
fn before_submit_lock_failure_fails_closed_without_writing_state() {
    let _gate = ReviewGateActiveGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s14", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(out.get("continue"), Some(&json!(false)));
    assert!(
        hook_user_visible_blob(&out).contains("锁不可用"),
        "out={out:?}"
    );
    assert!(!state_path(&repo, &payload).exists());
}

#[test]
fn before_submit_lock_failure_allows_non_strict_prompt() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s14b", "帮我润色一句话");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(out.get("continue"), Some(&json!(true)));
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("降级"),
        "expected degraded lock copy; blob={blob}"
    );
}

#[test]
fn stop_lock_failure_reports_degraded_followup() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s15", "帮我润色一句话");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("锁不可用") && blob.contains("降级"),
        "expected degraded stop copy; blob={blob}"
    );
}

#[test]
fn stop_lock_failure_fail_closed_review_gate_when_review_armed() {
    let _gate = ReviewGateActiveGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let payload = event("s15-review-lock", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete") && fm.contains("hook_state_lock_unavailable"),
        "review-armed stop must fail-closed REVIEW_GATE on lock loss; fm={fm}"
    );
}

#[test]
fn subagent_start_lock_failure_denies_when_review_armed() {
    let _gate = ReviewGateActiveGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    let sid = "s-sub-lock-deny";
    let submit = event(sid, "全面review这个仓库");
    let mut armed = empty_state();
    armed.review_required = true;
    armed.phase = 1;
    if let Some(parent) = state_path(&repo, &submit).parent() {
        fs::create_dir_all(parent).expect("mkdir hook-state");
    }
    fs::write(
        state_path(&repo, &submit),
        serde_json::to_string(&armed).expect("serialize"),
    )
    .expect("seed review-armed state");
    let out = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({ "session_id": sid, "subagent_type": "general-purpose", "fork_context": false }),
    );
    assert_eq!(out.get("permission"), Some(&json!("deny")));
}

#[test]
fn stop_load_state_invalid_json_fail_closed_review_gate() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let sid = "s-stop-bad-json";
    let payload = event(sid, "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    fs::write(state_path(&repo, &payload), b"{not json").expect("bad state");
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete") && fm.contains("hook_state_read_failed"),
        "corrupt hook-state must fail-closed; fm={fm}"
    );
}

#[test]
fn stop_lock_failure_still_surfaces_goal_drive() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let _guard = ForceHookStateLockFailureGuard::new();
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/gl-stop-lock")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"gl-stop-lock"}"#,
    )
    .expect("active_task");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "gl-stop-lock",
        "goal": "lock-merge",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");

    let payload = event("s15b", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir lock parent");
    fs::write(&lock_path, b"locked").expect("seed lock");
    let out = dispatch_cursor_hook_event(&repo, "stop", &payload);
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("REVIEW_GATE incomplete") && blob.contains("hook_state_lock_unavailable"),
        "review-armed stop lock loss must hard REVIEW_GATE; blob={blob}"
    );
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "hard lock-failure followup must not merge GOAL_CONTINUE; blob={blob}"
    );
}

#[test]
fn stop_with_active_goal_does_not_inject_goal_continue() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    let tid = "existing-followup";
    fs::create_dir_all(repo.join("artifacts/current").join(tid)).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        format!(r#"{{"task_id":"{tid}"}}"#),
    )
    .expect("active_task");
    fs::write(
            repo
                .join("artifacts/current")
                .join(tid)
                .join("GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-goal-v1","goal":"drive while hard message exists","status":"running","drive_until_done":true,"non_goals":["n"],"done_when":["d1","d2"],"validation_commands":["cargo test -q"]}"#,
        )
        .expect("goal");
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("existing-followup", "hi"));
    let blob = hook_user_visible_blob(&out);
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "continuity removal: Stop must not inject GOAL_CONTINUE: {blob}"
    );
}

#[test]
fn before_submit_does_not_merge_goal_or_rfv_continuity() {
    let repo = fresh_repo();
    let tid = "merge-both";
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
            r#"{"schema_version":"router-rs-rfv-loop-v1","goal":"rfv-line","loop_status":"active","current_round":0,"max_rounds":3,"allow_external_research":false,"rounds":[]}"#,
        )
        .expect("rfv");
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("merge-t", "hello"));
    let msg = hook_user_visible_blob(&out);
    assert!(!msg.contains("GOAL_CONTINUE"), "{msg}");
    assert!(!msg.contains("RFV_LOOP_CONTINUE"), "{msg}");
    assert!(!msg.contains("## 续跑"), "{msg}");
}

#[test]
#[serial]
fn stop_active_goal_does_not_inject_goal_continue() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    fs::create_dir_all(repo.join("artifacts/current/default-ac")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"default-ac"}"#,
    )
    .expect("active_task");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "default-ac",
        "goal": "default additional context drive",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");

    let out = dispatch_cursor_hook_event(&repo, "stop", &event("default-ac", "hi"));
    assert!(
        out.get("followup_message").is_none(),
        "continuity nudge should not become hard followup: {out:?}"
    );
    let ctx = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ctx.contains("GOAL_CONTINUE"),
        "continuity removal: {ctx}"
    );
    assert!(
        ctx.contains("SESSION_CLOSE_STYLE"),
        "soft terminal closeout nudge still allowed: {ctx}"
    );
}

#[test]
#[serial]
fn stop_plain_session_injects_session_close_style_when_no_hard_followup() {
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("plain-close", "ok"));
    assert!(out.get("followup_message").is_none(), "{out:?}");
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(ac.contains("SESSION_CLOSE_STYLE"), "{ac}");
}

#[test]
fn stop_review_advisory_does_not_inject_session_close_style_paragraph() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-hard-g", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("s-hard-g", "继续"));
    assert!(
        out.get("followup_message").is_some(),
        "review-armed Stop must emit advisory followup; out={out:?}"
    );
    assert!(
        out.get("permission").is_none(),
        "advisory REVIEW_GATE must not hard-block Stop; out={out:?}"
    );
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("SESSION_CLOSE_STYLE"),
        "advisory Stop followup must not bundle soft closeout nudge: {out:?}"
    );
    assert!(
        !ac.contains("GOAL_CONTINUE"),
        "advisory Stop followup must suppress goal continuity merge: {out:?}"
    );
    assert!(
        !ac.contains("review-output-lint"),
        "advisory Stop followup must not merge review-output-lint: {out:?}"
    );
}

#[test]
fn stop_review_armed_with_active_goal_suppresses_my_continue() {
    let _inject_on = OperatorInjectEnabledGuard::new();
    let repo = fresh_repo();
    let sid = "s-review-goal-mutex";
    fs::create_dir_all(repo.join("artifacts/current/default-rg")).expect("mkdir goal");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"default-rg"}"#,
    )
    .expect("active_task");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "default-rg",
        "goal": "drive while review gate open",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "继续"));
    let blob = hook_user_visible_blob(&out);
    assert!(
        blob.contains("REVIEW_GATE") || blob.contains("AG_FOLLOWUP"),
        "expected advisory gate followup: {blob}"
    );
    assert!(
        out.get("permission").is_none(),
        "advisory REVIEW_GATE must not hard-block Stop; out={out:?}"
    );
    assert!(
        !blob.contains("GOAL_CONTINUE"),
        "advisory gate must not merge GOAL_CONTINUE: {blob}"
    );
    assert!(
        !blob.contains("review-output-lint"),
        "advisory gate must not merge review-output-lint: {blob}"
    );
}

#[test]
fn review_gate_disabled_before_submit_emits_only_continue_true() {
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let payload = event("rg-off-before-submit", "hello");
    let expected = json!({ "continue": true });
    let _rg = ReviewGateDisableTestGuard::new();
    let out_prompt = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    assert_eq!(
        out_prompt, expected,
        "beforeSubmitPrompt in review-gate-disabled mode must not attach before_submit nudges/state; got {out_prompt:?}"
    );
    let out_user = dispatch_cursor_hook_event(&repo, "userPromptSubmit", &payload);
    assert_eq!(
        out_user, expected,
        "userPromptSubmit must normalize like beforeSubmitPrompt; got {out_user:?}"
    );
}

#[test]
fn review_gate_disabled_after_agent_response_updates_state_after_before_submit_seeded() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let _rg_env = ReviewGateDisableEnvClearGuard::new();
    let repo = fresh_repo();
    let sid = "aar-rg-disabled-parity";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    assert!(
        !load_state_for(&repo, sid).reject_reason_seen,
        "precondition: reject_reason not set by beforeSubmit alone"
    );
    let payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": { "response": "small_task" }
    });
    {
        let _rg = ReviewGateDisableTestGuard::new();
        assert_eq!(
            dispatch_cursor_hook_event(&repo, "afterAgentResponse", &payload),
            json!({}),
            "afterAgentResponse shape unchanged under review-gate-disabled dispatch"
        );
    }
    assert!(
        load_state_for(&repo, sid).reject_reason_seen,
        "reject_reason must persist when afterAgentResponse runs on emergency dispatch table"
    );
}

#[test]
fn session_close_style_nudge_disabled_by_env() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE", "0") };
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("style-off", "x"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("SESSION_CLOSE_STYLE"),
        "env should disable soft close nudge: {ac}"
    );
    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE") },
    }
}

#[test]
fn session_close_style_nudge_suppressed_when_operator_inject_off() {
    let _lock = core_policy::test_env_sync::process_env_lock();
    let prev_inject = env::var_os("ROUTER_RS_OPERATOR_INJECT");
    unsafe { env::set_var("ROUTER_RS_OPERATOR_INJECT", "0") };
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(&repo, "stop", &event("plain-close-inject-off", "ok"));
    let ac = out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !ac.contains("SESSION_CLOSE_STYLE"),
        "ROUTER_RS_OPERATOR_INJECT=0 must suppress SESSION_CLOSE_STYLE: {ac}"
    );
    match prev_inject {
        Some(v) => unsafe { env::set_var("ROUTER_RS_OPERATOR_INJECT", v) },
        None => unsafe { env::remove_var("ROUTER_RS_OPERATOR_INJECT") },
    }
}
