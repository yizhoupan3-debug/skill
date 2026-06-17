#[test]
fn main_thread_compact_review_does_not_clear_gate_without_subagent() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let sid = "s-main-thread-review";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "response": "[P1] core/router-rs/src/cursor_hooks/handlers.rs:3000 — Stop 双信号 — 续跑与 REVIEW_GATE 并存"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "afterAgentResponse", &payload);
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "done"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "compact-only must not clear gate (P0-4); fm={fm}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        state.phase < 3,
        "compact-only must not reach phase 3; phase={}",
        state.phase
    );
}

#[test]
fn main_thread_compact_stop_only_does_not_clear_gate_without_subagent() {
    let repo = fresh_repo();
    let sid = "s-main-thread-stop-only";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let stop_payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "response": "[P1] core/router-rs/src/cursor_hooks/handlers.rs:3000 — Stop-only compact path — substantive finding line for gate clear"
        }
    });
    let out = dispatch_cursor_hook_event(&repo, "stop", &stop_payload);
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete"),
        "stop-only compact must not clear gate without subagent; fm={fm}"
    );
    let state = load_state_for(&repo, sid);
    assert!(state.phase < 3, "phase={}", state.phase);
}

#[test]
fn main_thread_deferential_compact_does_not_clear_gate_on_stop() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let sid = "s-main-thread-deferential";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let payload = json!({
        "session_id": sid,
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "response": "[P2] 见上文"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "afterAgentResponse", &payload);
    let out = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "done"));
    let fm = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        fm.contains("REVIEW_GATE incomplete") || fm.contains("AG_FOLLOWUP"),
        "deferential-only compact must not clear gate; fm={fm}"
    );
    let state = load_state_for(&repo, sid);
    assert!(
        state.phase < 3,
        "deferential-only compact must not reach phase 3; phase={}",
        state.phase
    );
}

#[test]
fn strict_disk_stop_pre_goal_not_satisfied_from_goal_file_alone() {
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let prev_pre = env::var_os("ROUTER_RS_PRE_GOAL_ENABLED");
    unsafe { env::set_var("ROUTER_RS_PRE_GOAL_ENABLED", "1") };
    let prev = env::var_os("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK");
    unsafe { env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", "1") };

    let repo = fresh_repo();
    let sid = "s-strict-disk-stop";
    fs::create_dir_all(repo.join("artifacts/current/strict-stop")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"strict-stop"}"#,
    )
    .expect("active_task");
    // Pointer 机制已移除：写入 task_registry.json 供回退使用
    fs::write(
        repo.join("artifacts/current/task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"strict-stop","tasks":[{"task_id":"strict-stop"}]}"#,
    )
    .expect("task registry");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": repo.display().to_string(),
        "operation": "start",
        "task_id": "strict-stop",
        "goal": "strict disk stop",
        "non_goals": ["n"],
        "done_when": ["d1", "d2"],
        "validation_commands": ["cargo test -q"],
    }))
    .expect("goal");
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "/implementx continue"),
    );
    let after_submit = load_state_for(&repo, sid);
    assert!(
        !after_submit.pre_goal_review_satisfied,
        "strict disk: beforeSubmit must not hydrate pre_goal from disk alone"
    );
    let _ = dispatch_cursor_hook_event(&repo, "stop", &event(sid, "continue"));
    let after_stop = load_state_for(&repo, sid);
    assert!(
        !after_stop.pre_goal_review_satisfied,
        "strict disk: Stop hydrate must not set pre_goal from disk GOAL alone"
    );
    assert!(
        after_stop.goal_required || after_stop.goal_drive_entry_active,
        "implementx must arm goal tracking on Stop"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK") },
    }
    match prev_pre {
        Some(v) => unsafe { env::set_var("ROUTER_RS_PRE_GOAL_ENABLED", v) },
        None => unsafe { env::remove_var("ROUTER_RS_PRE_GOAL_ENABLED") },
    }
}

#[test]
fn subagent_stop_must_match_open_reviewer_cycle() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s6c", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": "s6c",
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-1"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": "s6c", "subagent_type": "general-purpose", "subagent_id": "other" }),
    );
    let state = load_state_for(&repo, "s6c");
    assert_eq!(state.phase, 2);
    assert_eq!(state.subagent_stop_count, 0);
    assert_eq!(
        state.active_subagent_count, 0,
        "wrong-cycle subagentStop must still decrement open count (P0-1)"
    );
}

#[test]
fn duplicate_subagent_start_same_id_does_not_inflate_start_count() {
    let repo = fresh_repo();
    let sid = "s-dup-start";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event(sid, "全面review这个仓库"),
    );
    let start_payload = json!({
        "session_id": sid,
        "subagent_type": "general-purpose",
        "fork_context": false,
        "subagent_id": "review-dup"
    });
    let _ = dispatch_cursor_hook_event(&repo, "subagentStart", &start_payload);
    let _ = dispatch_cursor_hook_event(&repo, "subagentStart", &start_payload);
    let state = load_state_for(&repo, sid);
    assert_eq!(state.subagent_start_count, 1, "duplicate id: start must not double-count");
    assert_eq!(state.review_subagent_pending_cycle_keys.len(), 1);
    assert_eq!(
        state.active_subagent_count, 1,
        "duplicate id: open count must increment once"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-dup"
        }),
    );
    let after_stop = load_state_for(&repo, sid);
    assert_eq!(
        after_stop.active_subagent_count, 0,
        "single stop after duplicate start must zero open count"
    );
    assert!(after_stop.review_subagent_pending_cycle_keys.is_empty());
}

/// 两个不同 subagent id 并行 start：各自 stop 各核销一条 pending；**第二次** stop 排空 multiset 后才 phase 3。
#[test]
fn review_gate_two_distinct_subagent_ids_both_stops_clear_gate() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") };
    let repo = fresh_repo();
    let sid = "s-two-review-ids";
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
            "subagent_id": "review-a"
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStart",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "fork_context": false,
            "subagent_id": "review-b"
        }),
    );
    let mid = load_state_for(&repo, sid);
    assert_eq!(mid.phase, 2);
    assert_eq!(mid.review_subagent_pending_cycle_keys.len(), 2);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-a"
        }),
    );
    let after_first_stop = load_state_for(&repo, sid);
    assert_eq!(after_first_stop.phase, 2);
    assert_eq!(after_first_stop.subagent_stop_count, 0);
    assert_eq!(after_first_stop.review_subagent_pending_cycle_keys.len(), 1);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose",
            "subagent_id": "review-b"
        }),
    );
    let final_state = load_state_for(&repo, sid);
    assert_eq!(final_state.phase, 3);
    assert_eq!(final_state.subagent_stop_count, 1);
    assert!(final_state.review_subagent_pending_cycle_keys.is_empty());

    match prev_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}

/// 无 subagent id 时 cycle key 均为同一 `lane:`；两次并行 start 压入两条 multiset 记录，需**两次** stop 才清门。
#[test]
fn review_gate_parallel_lane_only_keys_two_stops_clear_gate() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_cap = env::var_os("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX");
    unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", "2") };

    let repo = fresh_repo();
    let sid = "s-parallel-lane-only";
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
            "fork_context": false
        }),
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
    let mid = load_state_for(&repo, sid);
    assert_eq!(mid.review_subagent_pending_cycle_keys.len(), 2);
    assert_eq!(
        mid.review_subagent_pending_cycle_keys[0],
        mid.review_subagent_pending_cycle_keys[1]
    );

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose"
        }),
    );
    let after_one = load_state_for(&repo, sid);
    assert_eq!(after_one.phase, 2);
    assert_eq!(after_one.review_subagent_pending_cycle_keys.len(), 1);
    assert_eq!(after_one.subagent_stop_count, 0);

    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "general-purpose"
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 3);
    assert!(state.review_subagent_pending_cycle_keys.is_empty());
    assert_eq!(state.subagent_stop_count, 1);

    match prev_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX") },
    }
}

#[test]
fn review_lane_only_cycle_stop_advances_phase_when_ids_absent() {
    let repo = fresh_repo();
    let sid = "s6-lane-only";
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
            "fork_context": false
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({ "session_id": sid, "subagent_type": "general-purpose" }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 3);
    assert_eq!(state.subagent_stop_count, 1);
}

#[test]
fn review_lane_only_cycle_mismatch_lane_on_stop_does_not_advance() {
    let repo = fresh_repo();
    let sid = "s6-lane-mismatch";
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
            "fork_context": false
        }),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "subagentStop",
        &json!({
            "session_id": sid,
            "subagent_type": "best-of-n-runner",
            "fork_context": false
        }),
    );
    let state = load_state_for(&repo, sid);
    assert_eq!(state.phase, 2);
    assert_eq!(state.subagent_stop_count, 0);
}

#[test]
fn subtracted_before_shell_default_noop_skips_terminal_ledger() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") };

    let repo = fresh_repo();
    let payload = json!({
        "session_id": "sub-shell-noop",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "command": "echo noop-test"
    });
    let ledger_path = state_dir(&repo).join(format!(
        "session-terminals-{}.json",
        super::session_key(&payload)
    ));
    let out = dispatch_cursor_hook_event(&repo, "beforeShellExecution", &payload);
    assert_eq!(
        out,
        json!({ "continue": true, "permission": "allow" }),
        "default subtracted dispatch must pass shell gate without side effects"
    );
    assert!(
        !ledger_path.exists(),
        "ledger file must not be created on subtracted noop: {}",
        ledger_path.display()
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") },
    }
}

#[test]
fn subtracted_after_agent_response_runs_handler_when_registered_in_hooks_json() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") };

    let repo = fresh_repo();
    let hooks_path = repo.join(".cursor/hooks.json");
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&hooks_path).unwrap_or_else(|_| r#"{"hooks":{}}"#.to_string()),
    )
    .unwrap();
    doc["hooks"]["afterAgentResponse"] = json!([{
        "command": "configs/framework/cursor-router-rs-hook.sh",
        "timeout": 20
    }]);
    fs::write(&hooks_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("sub-ara-reg", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "sub-ara-reg",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": { "response": "reject reason: small_task" }
        }),
    );
    assert!(
        load_state_for(&repo, "sub-ara-reg").reject_reason_seen,
        "registered subtracted event must run handler without LEGACY env"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") },
    }
}

#[test]
fn subtracted_empty_hooks_entry_stays_noop() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") };

    let repo = fresh_repo();
    let hooks_path = repo.join(".cursor/hooks.json");
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&hooks_path).unwrap_or_else(|_| r#"{"hooks":{}}"#.to_string()),
    )
    .unwrap();
    doc["hooks"]["afterAgentResponse"] = json!([]);
    fs::write(&hooks_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("sub-ara-empty", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": "sub-ara-empty",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": { "response": "reject reason: small_task" }
        }),
    );
    assert!(
        !load_state_for(&repo, "sub-ara-empty").reject_reason_seen,
        "empty hooks entry must not run handler"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") },
    }
}

#[test]
fn review_gate_disabled_registered_after_agent_response_persists_reject_reason() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_legacy = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") };
    let _env_clear = ReviewGateDisableEnvClearGuard::new();
    let _rg_disable = ReviewGateDisableTestGuard::new();

    let repo = fresh_repo();
    let hooks_path = repo.join(".cursor/hooks.json");
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&hooks_path).unwrap_or_else(|_| r#"{"hooks":{}}"#.to_string()),
    )
    .unwrap();
    doc["hooks"]["afterAgentResponse"] = json!([{
        "command": "configs/framework/cursor-router-rs-hook.sh",
        "timeout": 20
    }]);
    fs::write(&hooks_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let sid = "rg-dis-ara";
    let _ = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({
            "session_id": sid,
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "response": "reject reason: small_task"
        }),
    );
    assert!(
        load_state_for(&repo, sid).reject_reason_seen,
        "review-gate-disabled + registered afterAgentResponse must still run handler"
    );

    match prev_legacy {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") },
    }
}

#[test]
fn subtracted_after_agent_response_default_is_empty_object() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") };

    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "afterAgentResponse",
        &json!({ "session_id": "sub-ara", "response": "[P1] x" }),
    );
    assert_eq!(out, json!({}));

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS") },
    }
}

#[test]
fn pre_compact_emits_additional_context_summary() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s8", "全面review这个仓库"),
    );
    let out = dispatch_cursor_hook_event(
        &repo,
        "preCompact",
        &json!({ "session_id": "s8", "cwd": FRAMEWORK_HARNESS_TEST_CWD }),
    );
    assert!(out
        .get("additional_context")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .contains("phase=0"));
}

#[test]
fn session_end_clears_state_file() {
    let repo = fresh_repo();
    let payload = event("s9", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let path = state_path(&repo, &payload);
    assert!(path.exists());
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(!path.exists());
}

#[test]
fn session_end_cleans_stale_lock_if_present() {
    let repo = fresh_repo();
    let payload = event("s9b", "全面review这个仓库");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    fs::write(&lock_path, b"pid=1 ts=1").expect("seed lock");
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &payload);
    assert!(!lock_path.exists());
}

#[test]
fn session_end_preserves_other_session_hook_state_when_legacy_sweep_disabled() {
    let _env = core_policy::test_env_sync::process_env_lock();
    use std::env;
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP");
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP") };

    let repo = fresh_repo();
    let stale_payload = event("stale-session", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &stale_payload);
    let stale_state = state_path(&repo, &stale_payload);
    let stale_lock = state_lock_path(&repo, &stale_payload);
    let stale_loop = adversarial_loop_path(&repo, &stale_payload);
    fs::create_dir_all(stale_lock.parent().expect("parent")).expect("mkdir");
    fs::write(&stale_lock, b"pid=1 ts=1").expect("seed lock");
    fs::write(&stale_loop, b"{\"version\":1,\"completed_passes\":0}").expect("seed loop");
    assert!(stale_state.exists());

    // Unrelated SessionEnd：默认不得删其它 session_key 下的门控状态。
    let unrelated_payload = json!({ "session_id": "fresh-session-zzz" });
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &unrelated_payload);

    assert!(
        stale_state.exists(),
        "other session review-subagent state must be preserved without legacy sweep"
    );
    assert!(
        stale_lock.exists(),
        "other session review-subagent lock must be preserved without legacy sweep"
    );
    assert!(
        stale_loop.exists(),
        "other session adversarial-loop state must be preserved without legacy sweep"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP") },
    }
}

/// `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP=1` 时恢复全目录前缀清扫（session_id/cwd 漂移遗留）。
#[test]
fn session_end_legacy_full_sweep_removes_unrelated_session_hook_state() {
    let _env = core_policy::test_env_sync::process_env_lock();
    use std::env;
    let prev = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP");
    unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP", "1") };

    let repo = fresh_repo();
    let stale_payload = event("stale-session", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &stale_payload);
    let stale_state = state_path(&repo, &stale_payload);
    let stale_lock = state_lock_path(&repo, &stale_payload);
    let stale_loop = adversarial_loop_path(&repo, &stale_payload);
    fs::create_dir_all(stale_lock.parent().expect("parent")).expect("mkdir");
    fs::write(&stale_lock, b"pid=1 ts=1").expect("seed lock");
    fs::write(&stale_loop, b"{\"version\":1,\"completed_passes\":0}").expect("seed loop");
    assert!(stale_state.exists());

    let unrelated_payload = json!({ "session_id": "fresh-session-zzz" });
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &unrelated_payload);

    assert!(
        !stale_state.exists(),
        "stale review-subagent state must be swept under legacy full sweep"
    );
    assert!(
        !stale_lock.exists(),
        "stale review-subagent lock must be swept under legacy full sweep"
    );
    assert!(
        !stale_loop.exists(),
        "stale adversarial-loop state must be swept under legacy full sweep"
    );

    match prev {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP") },
    }
}

#[cfg(unix)]
fn set_path_mtime_days_ago(path: &std::path::Path, days: u64) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .saturating_sub(days.saturating_mul(86_400));
    let times = [
        libc::timespec {
            tv_sec: secs as libc::time_t,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: secs as libc::time_t,
            tv_nsec: 0,
        },
    ];
    let cpath = CString::new(path.as_os_str().as_bytes()).expect("path");
    unsafe {
        libc::utimensat(libc::AT_FDCWD, cpath.as_ptr(), times.as_ptr(), 0);
    }
}

/// Age sweep must not unlink `.lock` when holder PID is alive (even with old ts) if json is fresh.
#[test]
fn stale_sweep_preserves_alive_holder_lock_when_json_fresh() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let prev_days = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS");
    unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", "7") };
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP") };

    let repo = fresh_repo();
    let victim = event("victim-alive-lock", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &victim);
    let lock_path = state_lock_path(&repo, &victim);
    let stale_ts = now_millis().saturating_sub(120_000);
    fs::write(
        &lock_path,
        format!("pid={} ts={stale_ts}\n", std::process::id()),
    )
    .expect("seed alive-pid lock with old ts");

    let _ = dispatch_cursor_hook_event(
        &repo,
        "sessionEnd",
        &json!({ "session_id": "sweeper-other-session" }),
    );

    assert!(
        lock_path.is_file(),
        "sweep must not remove lock while holder pid is alive and json is fresh"
    );

    match prev_days {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS") },
    }
}

/// Default age sweep removes old session_key files but keeps recent parallel-session state.
#[cfg(unix)]
#[test]
fn session_end_stale_sweep_removes_old_orphan_preserves_recent() {
    let _env = core_policy::test_env_sync::process_env_lock();
    use std::env;
    let prev_days = env::var_os("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS");
    unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", "1") };
    unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP") };

    let repo = fresh_repo();
    let old_payload = event("old-session-key", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &old_payload);
    let old_state = state_path(&repo, &old_payload);
    assert!(old_state.exists());
    set_path_mtime_days_ago(&old_state, 10);
    let old_lock = state_lock_path(&repo, &old_payload);
    let old_ts = now_millis().saturating_sub(10 * 86_400 * 1000);
    fs::write(&old_lock, format!("pid=1 ts={old_ts}\n")).expect("seed old ts lock");
    set_path_mtime_days_ago(&old_lock, 10);

    let recent_payload = json!({ "session_id": "fresh-parallel-session" });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &recent_payload);
    let recent_state = state_path(&repo, &recent_payload);
    assert!(recent_state.exists());

    let end_payload = json!({ "session_id": "unrelated-end-session" });
    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &end_payload);

    assert!(!old_state.exists(), "10d-old hook-state must be age-swept");
    assert!(
        recent_state.exists(),
        "recent parallel session state must remain"
    );

    match prev_days {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS") },
    }
}

/// 清扫只覆盖本模块拥有的前缀，不应误伤未识别文件（避免与未来其它 hook 共用目录时冲突）。
#[test]
fn session_end_sweep_keeps_unrelated_files() {
    let repo = fresh_repo();
    let dir = state_dir(&repo);
    fs::create_dir_all(&dir).expect("mkdir state dir");
    let unrelated = dir.join("other-hook-state.json");
    fs::write(&unrelated, b"{}").expect("seed unrelated");

    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));
    assert!(unrelated.exists(), "unrelated hook state must be preserved");
}

/// SessionEnd sweep 必须回收 `save_state` 及历史 adversarial-loop 原子写入孤儿，
/// 避免长期累积消耗 `.cursor/hook-state/` 卫生。
#[test]
fn session_end_sweeps_atomic_write_orphans() {
    let repo = fresh_repo();
    let dir = state_dir(&repo);
    fs::create_dir_all(&dir).expect("mkdir state dir");

    let primary_tmp = dir.join(".tmp-99999-12345-review-subagent-deadbeef.json");
    let adv_tmp = dir.join(".tmp-adv-loop-99999-67890");
    let other_tmp = dir.join(".tmp-99999-12345-other-hook.json");
    fs::write(&primary_tmp, b"{}").expect("seed primary tmp");
    fs::write(&adv_tmp, b"{}").expect("seed adv tmp");
    fs::write(&other_tmp, b"{}").expect("seed other tmp");

    let _ = dispatch_cursor_hook_event(&repo, "sessionEnd", &json!({ "session_id": "any" }));

    assert!(
        !primary_tmp.exists(),
        "review-subagent atomic-write tmp must be swept"
    );
    assert!(
        !adv_tmp.exists(),
        "adversarial-loop atomic-write tmp must be swept"
    );
    assert!(
        other_tmp.exists(),
        "unrelated tmp must be preserved (sweep is module-scoped)"
    );
}

/// 文件名归属判断必须只接受本模块写入的命名（含原子写入孤儿前缀），其它名称一律排除。
#[test]
fn review_gate_state_file_owned_by_module_recognizes_known_names_only() {
    // 主状态：仅认 json|lock 扩展。
    assert!(review_gate_state_file_owned_by_module(
        "review-subagent-abc.json"
    ));
    assert!(review_gate_state_file_owned_by_module(
        "review-subagent-abc.lock"
    ));
    assert!(review_gate_state_file_owned_by_module(
        "adversarial-loop-abc.json"
    ));
    assert!(!review_gate_state_file_owned_by_module(
        "review-subagent-abc.bak"
    ));
    assert!(!review_gate_state_file_owned_by_module("review-subagent-"));
    // 原子写入孤儿。
    assert!(review_gate_state_file_owned_by_module(
        ".tmp-1-2-review-subagent-abc.json"
    ));
    assert!(review_gate_state_file_owned_by_module(".tmp-adv-loop-1-2"));
    // 未识别命名不应被清扫。
    assert!(!review_gate_state_file_owned_by_module(
        "other-hook-state.json"
    ));
    assert!(!review_gate_state_file_owned_by_module(
        ".tmp-1-2-other-hook.json"
    ));
    assert!(!review_gate_state_file_owned_by_module(".tmp-random"));
}

#[test]
fn v1_state_migrates_to_current_schema_phase() {
    let repo = fresh_repo();
    let payload = json!({ "session_id": "s11" });
    let path = state_path(&repo, &payload);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &path,
        r#"{"version":1,"review_required":true,"review_subagent_seen":true,"followup_count":2}"#,
    )
    .expect("write v1");
    let state = load_state(&repo, &payload).expect("load").expect("state");
    assert_eq!(state.version, STATE_VERSION);
    assert_eq!(state.phase, 2);
    assert_eq!(state.followup_count, 2);
}

#[test]
fn post_tool_use_subagent_sets_phase() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s12", "全面review这个仓库"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id":"s12",
            "tool_name":"functions.subagent",
            "tool_input":{"subagent_type":"general-purpose","fork_context":false}
        }),
    );
    let state = load_state_for(&repo, "s12");
    assert!(state.phase >= 2);
}

// Cursor-only: legacy RG_FOLLOWUP / breadth token scrub — see `review_gate_stdout_scrub_*`

#[test]
#[serial]
fn goal_stop_followup_is_short_code_only() {
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    use std::env;
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    fs::create_dir_all(repo.join("artifacts/current/t-s17")).expect("mkdir");
    fs::write(
        repo.join("artifacts/current/active_task.json"),
        r#"{"task_id":"t-s17"}"#,
    )
    .expect("active");
    // Pointer 机制已移除：写入 task_registry.json 供回退使用
    fs::write(
        repo.join("artifacts/current/task_registry.json"),
        r#"{"schema_version":"task-registry-v1","focus_task_id":"t-s17","tasks":[{"task_id":"t-s17"}]}"#,
    )
    .expect("task registry");
    core_state::state_manager::framework_goal_drive(json!({
        "repo_root": cwd,
        "operation": "start",
        "task_id": "t-s17",
        "goal": "short code stop test",
        "non_goals": ["scope creep"],
        "done_when": ["a", "b"],
        "validation_commands": ["cargo test -q"],
        "drive_until_done": true,
    }))
    .expect("goal start");
    let prev_close_style = env::var_os("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE");
    unsafe { env::set_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE", "0") };
    let hook_ev = |session: &str, prompt: &str| {
        json!({ "session_id": session, "cwd": cwd, "prompt": prompt })
    };
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &hook_ev("s17", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id":"s17",
            "cwd": cwd,
            "tool_name":"functions.subagent",
            "tool_input":{"subagent_type":"explore"}
        }),
    );
    let first = dispatch_cursor_hook_event(&repo, "stop", &hook_ev("s17", "继续"));
    let first_msg = hook_user_visible_blob(&first);
    assert!(
        first_msg.contains("router-rs AG_FOLLOWUP missing_parts="),
        "Stop uses short goal hint only; msg={first_msg:?}"
    );
    assert!(
        !first_msg.contains("Goal drive mode:"),
        "Stop must not dump full goal contract prose; msg={first_msg:?}"
    );
    let second = dispatch_cursor_hook_event(&repo, "stop", &hook_ev("s17", "继续"));
    let second_msg = hook_user_visible_blob(&second);
    // The invariant: Stop must keep the followup short. If a followup is emitted, it must
    // be the short AG_FOLLOWUP code, not long prose.
    if !second_msg.is_empty() {
        assert!(
            second_msg.contains("router-rs AG_FOLLOWUP missing_parts="),
            "expected short code when non-empty; second_msg={second_msg:?} second={second:?}"
        );
        assert!(
            !second_msg.contains("Goal drive mode:"),
            "Stop must not dump full goal contract prose; second_msg={second_msg:?}"
        );
    }
    match prev_close_style {
        Some(v) => unsafe { env::set_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE", v) },
        None => unsafe { env::remove_var("ROUTER_RS_CURSOR_SESSION_CLOSE_STYLE_NUDGE") },
    }
}

#[test]
fn stop_picks_assistant_goal_contract_from_messages_when_top_level_response_empty() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-msg-goal", "/implementx finish wiring"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-msg-goal",
            "tool_name": "functions.subagent",
            "tool_input": {"subagent_type": "general-purpose", "fork_context": false}
        }),
    );
    let assistant = concat!(
        "Goal: wire hook\n",
        "Non-goals: expand scope\n",
        "Validation commands: cargo test -q nl_route\n",
        "Done when:\n",
        "- a passes\n",
        "- b passes\n",
        "\n",
        "Checkpoint: merged handler.\n",
        "Verified: test passed.\n",
    );
    let stop_payload = json!({
        "session_id": "s-msg-goal",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "prompt": "continue",
        "messages": [
            {"role": "user", "content": "continue"},
            {"role": "assistant", "content": assistant}
        ]
    });
    let out = dispatch_cursor_hook_event(&repo, "stop", &stop_payload);
    let msg = hook_user_visible_blob(&out);
    assert!(
        !msg.contains("router-rs AG_FOLLOWUP missing_parts=goal_contract"),
        "assistant body only under messages[] must satisfy goal_contract; msg={msg:?}"
    );
}

#[test]
fn my_pre_goal_nudge_when_opt_in_enabled() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _pre_goal = MyPreGoalOptInEnvGuard::enable();
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let repo = fresh_repo();
    let out = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17b", "/implementx 完成任务"),
    );
    let msg = hook_user_visible_blob(&out);
    assert!(
        msg.contains("My implement (/implementx"),
        "expected My pre-goal nudge; surface={msg:?}"
    );
    assert!(load_state_for(&repo, "s17b").goal_required);
}

#[test]
fn my_pre_goal_auto_releases_when_nag_cap_reached() {
    let _env = core_policy::test_env_sync::process_env_lock();
    let _gate = ReviewGateActiveGuard::new();
    let _my_light = MyLightOverrideGuard::force_non_my_light();
    let prev_cap = env::var_os("ROUTER_RS_PRE_GOAL_MAX_NUDGES");
    let _pre_goal = MyPreGoalOptInEnvGuard::enable();
    unsafe { env::set_var("ROUTER_RS_PRE_GOAL_MAX_NUDGES", "2") };
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("cap-nag", "/implementx smoke"),
    );
    let mid = load_state_for(&repo, "cap-nag");
    assert_eq!(mid.pre_goal_nag_count, 1);
    assert!(!mid.pre_goal_review_satisfied);
    let out =
        dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &event("cap-nag", "continue"));
    let end = load_state_for(&repo, "cap-nag");
    assert!(end.pre_goal_review_satisfied);
    assert_eq!(end.pre_goal_nag_count, 0);
    let blob = hook_user_visible_blob(&out);
    assert!(blob.contains("pre-goal 提示已达上限"), "blob={blob:?}");
    match prev_cap {
        Some(v) => unsafe { env::set_var("ROUTER_RS_PRE_GOAL_MAX_NUDGES", v) },
        None => unsafe { env::remove_var("ROUTER_RS_PRE_GOAL_MAX_NUDGES") },
    }
}

#[test]
fn deep_json_strings_satisfy_pre_goal_reject_on_before_submit() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("deep-s1", "/implementx 任务"),
    );
    let deep = json!({
        "session_id": "deep-s1",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "messages": [{ "role": "user", "content": "small_task" }]
    });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &deep);
    assert!(load_state_for(&repo, "deep-s1").pre_goal_review_satisfied);
}

#[test]
fn messages_tail_user_text_clears_review_gate_when_top_level_prompt_empty() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-msg-only", "全面review这个仓库"),
    );
    let ev = json!({
        "session_id": "s-msg-only",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "messages": [
            { "role": "user", "content": "earlier" },
            { "role": "assistant", "content": "ok" },
            { "role": "user", "content": "rg_clear" }
        ]
    });
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &ev);
    let state = load_state_for(&repo, "s-msg-only");
    assert!(state.reject_reason_seen);
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !msg.contains(LEGACY_REVIEW_FOLLOWUP_TOKEN) && !msg.contains("Broad/deep review detected"),
        "expected gate clear from messages[].content; msg={msg:?} out={out:?}"
    );
    assert_eq!(state.followup_count, 0);
    assert_eq!(state.review_followup_count, 0);
}

#[test]
fn before_submit_reject_reason_token_in_user_prompt_satisfies_pre_goal() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17e", "/implementx 第一轮"),
    );
    let out = dispatch_cursor_hook_event(
            &repo,
            "beforeSubmitPrompt",
            &event(
                "s17e",
                "small_task\n\nGoal: smoke\nNon-goals: none\nDone when: ok\nValidation commands: cargo test",
            ),
        );
    let state = load_state_for(&repo, "s17e");
    assert!(state.reject_reason_seen);
    assert!(state.pre_goal_review_satisfied);
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("My implement (/implementx")
            && !msg.contains("independent-context reviewer"),
        "reject_reason on submit should skip pre-goal nag; msg={msg:?}"
    );
}

#[test]
fn nested_payload_prompt_reject_reason_satisfies_pre_goal_before_submit() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17nest", "/implementx 第一轮"),
    );
    let nested = json!({
        "session_id": "s17nest",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "prompt": "small_task\n\nGoal: smoke\nNon-goals: none\nDone when: ok\nValidation commands: cargo test"
        }
    });
    let out = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &nested);
    let state = load_state_for(&repo, "s17nest");
    assert!(state.reject_reason_seen);
    assert!(state.pre_goal_review_satisfied);
    let msg = out
        .get("followup_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !msg.contains("independent-context"),
        "nested payload prompt should satisfy pre_goal; msg={msg:?}"
    );
}

#[test]
fn nested_payload_prompt_reject_reason_updates_stop_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17stop-n", "/implementx 任务"),
    );
    let nested_stop = json!({
        "session_id": "s17stop-n",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "payload": {
            "prompt": "small_task\nGoal:\nNon-goals:\nDone when:\nValidation commands:"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "stop", &nested_stop);
    assert!(load_state_for(&repo, "s17stop-n").pre_goal_review_satisfied);
}

#[test]
fn post_tool_use_fork_context_true_does_not_satisfy_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17c", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id":"s17c",
            "tool_name":"functions.subagent",
            "tool_input":{"subagent_type":"explore","fork_context":true}
        }),
    );
    let state = load_state_for(&repo, "s17c");
    assert!(
        !state.pre_goal_review_satisfied,
        "shared fork_context must not count as independent pre-goal review"
    );
}

#[test]
fn post_tool_use_tool_input_type_field_satisfies_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17d", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s17d",
            "tool_name": "functions.subagent",
            "tool_input": {"type": "general-purpose", "fork_context": false}
        }),
    );
    assert!(
        load_state_for(&repo, "s17d").pre_goal_review_satisfied,
        "hosts may emit lane kind as tool_input.type instead of subagent_type"
    );
}

#[test]
fn post_tool_use_heuristic_mcp_subagent_tool_name_satisfies_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17mcp", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s17mcp",
            "tool_name": "mcp_cursor_agent_subagent",
            "tool_input": {"subagent_type": "general-purpose", "fork_context": false}
        }),
    );
    assert!(load_state_for(&repo, "s17mcp").pre_goal_review_satisfied);
}

#[test]
fn post_tool_use_nested_payload_tool_fields_satisfy_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s17nest-tu", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s17nest-tu",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "payload": {
                "tool_name": "functions.subagent",
                "tool_input": {"type": "general-purpose", "fork_context": false}
            }
        }),
    );
    assert!(load_state_for(&repo, "s17nest-tu").pre_goal_review_satisfied);
}

#[test]
fn post_tool_use_non_countable_lane_does_not_satisfy_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-lane", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-lane",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_name": "functions.subagent",
            "tool_input": {"lane": "my-custom-reviewer", "fork_context": false}
        }),
    );
    assert!(
        !load_state_for(&repo, "s-lane").pre_goal_review_satisfied,
        "custom lane is not a countable deep reviewer lane for pre-goal"
    );
}

#[test]
fn post_tool_use_fork_context_string_true_blocks_pre_goal() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-fkstr", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "postToolUse",
        &json!({
            "session_id": "s-fkstr",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_name": "functions.subagent",
            "tool_input": {"type": "explore", "fork_context": "true"}
        }),
    );
    assert!(
        !load_state_for(&repo, "s-fkstr").pre_goal_review_satisfied,
        "string fork_context=true must not count as independent pre-goal"
    );
}

#[test]
fn review_keyword_inside_codeblock_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s18", "```请 review 这段代码```"),
    );
    assert_eq!(load_state_for(&repo, "s18").phase, 0);
}

#[test]
fn review_keyword_inside_inline_code_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s19", "这是 `review` 函数"),
    );
    assert_eq!(load_state_for(&repo, "s19").phase, 0);
}

#[test]
fn review_keyword_inside_url_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s20", "https://example.com/review/123"),
    );
    assert_eq!(load_state_for(&repo, "s20").phase, 0);
}

#[test]
fn review_keyword_inside_blockquote_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s21", "> 用户说 review 一下"),
    );
    assert_eq!(load_state_for(&repo, "s21").phase, 0);
}

#[test]
fn quoted_review_token_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s22", r#"他说 "review hook""#),
    );
    assert_eq!(load_state_for(&repo, "s22").phase, 0);
}

#[test]
fn parallel_alone_does_not_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s23", "请解释 parallel 的含义"),
    );
    assert_eq!(load_state_for(&repo, "s23").phase, 0);
}

#[test]
fn parallel_with_task_verb_arms() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s24", "用 parallel workers 实现 X"),
    );
    assert_eq!(load_state_for(&repo, "s24").phase, 0);
}

#[test]
fn english_concurrent_alone_no_arm() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s25", "What does concurrent mean?"),
    );
    assert_eq!(load_state_for(&repo, "s25").phase, 0);
}

#[test]
fn resolve_cursor_hook_repo_root_finds_hooks_from_payload_cwd() {
    let root = fresh_repo();
    let nested = root.join("core/router-rs");
    fs::create_dir_all(&nested).expect("mkdir nested");
    let payload = json!({
        "session_id": "rk",
        "cwd": nested.display().to_string()
    });
    let wrong_cli = nested.join("ghost");
    let resolved = resolve_cursor_hook_repo_root(Some(wrong_cli.as_path()), &payload).expect("ok");
    assert_eq!(
        resolved,
        fs::canonicalize(&root).unwrap_or_else(|_| root.clone())
    );
}

#[test]
fn cursor_session_key_fallback_stable_for_cwd_without_session_id() {
    let payload = json!({ "cwd": "/tmp/abc-stable-fallback" });
    let a = session_key(&payload);
    let b = session_key(&payload);
    assert_eq!(a.len(), 32);
    assert_eq!(a, b, "cwd-only key must survive separate hook processes");
}

#[test]
fn cursor_session_key_reads_metadata_session_id() {
    let payload = json!({
        "cwd": "/tmp/x",
        "metadata": { "sessionId": "meta-sess-1" }
    });
    let from_meta = session_key(&payload);
    let flat = session_key(&json!({
        "session_id": "meta-sess-1",
        "cwd": "/tmp/x"
    }));
    assert_eq!(from_meta, flat);
}

#[test]
fn cursor_session_key_nested_payload_session_id_matches_top_level() {
    let nested = json!({
        "cwd": "/tmp/x",
        "payload": { "sessionId": "uuid-nested-pregoal" }
    });
    let flat = json!({
        "session_id": "uuid-nested-pregoal",
        "cwd": "/tmp/x"
    });
    assert_eq!(session_key(&nested), session_key(&flat));
}

#[test]
fn cursor_session_key_nested_workspace_folder_matches_top_cwd() {
    let nested = json!({
        "payload": { "workspaceFolder": "/tmp/ws-nested" }
    });
    let flat = json!({ "cwd": "/tmp/ws-nested" });
    assert_eq!(session_key(&nested), session_key(&flat));
}

#[test]
fn my_pre_goal_persists_when_session_id_only_nested_in_payload() {
    let _gate = ReviewGateActiveGuard::new();
    let repo = fresh_repo();
    let cwd = repo.display().to_string();
    let sid = "nested-sid-pregoal";
    let before = json!({
        "cwd": cwd,
        "payload": {
            "sessionId": sid,
            "prompt": "/implementx 完成任务"
        }
    });
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &before);
    let stop = json!({
        "cwd": cwd,
        "payload": {
            "sessionId": sid,
            "prompt": "small_task\nGoal: g\nNon-goals: n\nDone when: d\nValidation commands: cargo test"
        }
    });
    let out = dispatch_cursor_hook_event(&repo, "stop", &stop);
    let state = load_state(&repo, &json!({ "session_id": sid, "cwd": cwd }))
        .expect("load")
        .expect("state file");
    assert!(
        state.pre_goal_review_satisfied,
        "stop followup={:?}",
        out.get("followup_message")
    );
}

#[test]
fn subagent_start_pre_goal_requires_typed_subagent() {
    let repo = fresh_repo();
    let _ = dispatch_cursor_hook_event(
        &repo,
        "beforeSubmitPrompt",
        &event("s-sub-pre", "/implementx 完成任务"),
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "SubagentStart",
        &json!({
            "session_id": "s-sub-pre",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "tool_input": {"fork_context": false}
        }),
    );
    assert!(
        !load_state_for(&repo, "s-sub-pre").pre_goal_review_satisfied,
        "untyped SubagentStart must not satisfy pre-goal"
    );
    let _ = dispatch_cursor_hook_event(
        &repo,
        "SubagentStart",
        &json!({
            "session_id": "s-sub-pre",
            "cwd": FRAMEWORK_HARNESS_TEST_CWD,
            "subagent_type": "general-purpose",
            "tool_input": {"fork_context": false}
        }),
    );
    assert!(load_state_for(&repo, "s-sub-pre").pre_goal_review_satisfied);
}

#[test]
fn cursor_lock_writes_owner_metadata() {
    let repo = fresh_repo();
    let payload = event("s26", "review");
    let lock = acquire_state_lock(&repo, &payload).expect("acquire");
    let text = fs::read_to_string(state_lock_path(&repo, &payload)).expect("read lock");
    assert!(text.contains("pid="));
    assert!(text.contains("ts="));
    let mut guard = Some(lock);
    release_state_lock(&mut guard);
}

#[test]
fn cursor_lock_recovers_from_stale_timestamp() {
    let repo = fresh_repo();
    let payload = event("s27", "review");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    let stale_ts = now_millis().saturating_sub(60_000);
    fs::write(&lock_path, format!("pid=999999 ts={stale_ts}\n")).expect("seed stale lock");
    let mut lock = acquire_state_lock(&repo, &payload);
    assert!(lock.is_some());
    release_state_lock(&mut lock);
}

#[test]
fn cursor_lock_recovers_orphan_lock_file_without_remove_when_holder_alive() {
    let repo = fresh_repo();
    let payload = event("s27-alive", "review");
    let lock_path = state_lock_path(&repo, &payload);
    fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
    let stale_ts = now_millis().saturating_sub(60_000);
    fs::write(
        &lock_path,
        format!("pid={} ts={stale_ts}\n", std::process::id()),
    )
    .expect("seed stale lock metadata without flock holder");
    let mut lock = acquire_state_lock(&repo, &payload);
    assert!(
        lock.is_some(),
        "orphan lock file must be acquired via try_lock without remove_file on alive pid"
    );
    assert!(
        lock_path.is_file(),
        "must not remove_file lock path when holder pid is still alive"
    );
    release_state_lock(&mut lock);
}

#[test]
fn cursor_lock_concurrent_acquire_serializes() {
    let repo = Arc::new(fresh_repo());
    let sessions = ["s28-a", "s28-b"];
    let mut joins = Vec::new();
    for session in sessions {
        let repo = Arc::clone(&repo);
        joins.push(std::thread::spawn(move || {
            let payload = event(session, "review");
            for _ in 0..20 {
                let lock = acquire_state_lock(&repo, &payload).expect("acquire");
                let mut guard = Some(lock);
                release_state_lock(&mut guard);
            }
        }));
    }
    for join in joins {
        join.join().expect("join");
    }
}

#[test]
fn cursor_state_save_completes_with_fsync_unix() {
    let repo = fresh_repo();
    let payload = event("s29", "review");
    let mut state = empty_state();
    state.phase = 2;
    assert!(save_state(&repo, &payload, &mut state));
    let loaded = load_state(&repo, &payload).expect("load").expect("state");
    assert_eq!(loaded.phase, 2);
}

#[test]
fn prompt_from_nested_messages_reads_text_without_content_key() {
    let payload = json!({
        "session_id": "msg-text-only",
        "cwd": FRAMEWORK_HARNESS_TEST_CWD,
        "messages": [{"role": "user", "text": "small_task review ./foo.rs"}],
    });
    assert_eq!(
        super::prompt_text(&payload),
        "small_task review ./foo.rs"
    );
}

#[test]
fn cursor_hook_rejects_non_object_stdin() {
    let mut reader = Cursor::new(b"[]".to_vec());
    let err = super::stdin::read_stdin_json_from_reader(&mut reader).expect_err("must reject");
    assert_eq!(err, "stdin_json_not_object");
}

#[test]
fn cursor_hook_rejects_oversized_stdin() {
    let large = "a".repeat(5 * 1024 * 1024);
    let mut reader = Cursor::new(large.into_bytes());
    let err = super::stdin::read_stdin_json_from_reader(&mut reader).expect_err("must reject");
    assert_eq!(err, "stdin_too_large");
}

#[test]
fn pre_compact_does_not_mutate_state() {
    let _legacy = LegacySubtractedEventsGuard::enable();
    let repo = fresh_repo();
    let payload = event("s30", "全面review这个仓库");
    let _ = dispatch_cursor_hook_event(&repo, "beforeSubmitPrompt", &payload);
    let path = state_path(&repo, &payload);
    let before = fs::read_to_string(&path).expect("read before");
    let _ = dispatch_cursor_hook_event(&repo, "preCompact", &payload);
    let after = fs::read_to_string(&path).expect("read after");
    assert_eq!(before, after);
}

