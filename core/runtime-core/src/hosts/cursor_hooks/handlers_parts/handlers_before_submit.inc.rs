fn handle_before_submit(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let (allow_continue, followup) = lock_failure_followup_for_before_submit(repo_root, event);
        let mut out = json!({ "continue": allow_continue });
        if !allow_continue {
            out["followup_message"] = Value::String(followup);
        } else {
            merge_additional_context(&mut out, &followup);
        }
        return out;
    }
    let text = prompt_text(event);
    let signal_text = hook_event_signal_text(event, &text, "");
    let state_load = load_state(repo_root, event);
    if let Err(ref load_err) = state_load {
        release_state_lock(&mut lock);
        let path = state_path(repo_root, event);
        let mut out = json!({ "continue": false });
        out["followup_message"] = Value::String(format!(
            "{} (path {}, err={load_err}). Repair JSON or permissions before submitting.",
            CURSOR_HOOK_STATE_UNREADABLE,
            path.display()
        ));
        return out;
    }
    let mut state = state_load.ok().flatten().unwrap_or_else(empty_state);
    let _stale_reset = apply_subagent_stale_hygiene(&mut state);
    // `delegation_required` is legacy serde only; always cleared (see ReviewGateState rustdoc).
    state.delegation_required = false;
    let review = is_review_prompt(&text);
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms_for_gate = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let user_gate_override = has_override(&text);

    let prior_review_required = state.review_required;
    let my_light = crate::hook_common::my_light_profile_active(Some(repo_root), &text);
    let review_gate_live = !cursor_review_gate_suppressed(repo_root, &text);
    let mut fresh_review_cycle = false;
    if my_light {
        state.review_required = false;
        reset_review_cycle_progress(&mut state, false);
    } else {
        if review_arms_for_gate && !user_gate_override && review_gate_live {
            reset_review_cycle_progress(&mut state, true);
            fresh_review_cycle = true;
        }
        state.review_required = state.review_required || review_arms_for_gate;
    }
    if goal_drive_entrypoint && !review_arms_for_gate {
        state.review_required = false;
        clear_review_gate_escalation_counters(&mut state);
        reset_review_cycle_progress(&mut state, false);
    }
    state.review_override = state.review_override || user_gate_override;
    state.delegation_override = state.delegation_override || user_gate_override;
    if goal_drive_entrypoint {
        state.goal_drive_entry_active = true;
    }
    state.goal_required =
        state.goal_required || (goal_drive_entrypoint && !my_light);
    let disk_goal = frame.hydration_goal.is_some();
    if !disk_goal {
        state.goal_contract_seen =
            state.goal_contract_seen || has_structured_goal_contract(&signal_text);
        state.goal_progress_seen =
            state.goal_progress_seen || has_goal_progress_signal(&signal_text);
        state.goal_verify_or_block_seen = state.goal_verify_or_block_seen
            || has_goal_verify_or_block_signal(&signal_text);
    }
    // 用户在本轮提交里写出 reject_reason token 时须即时生效；否则仅能在助手回复或 Stop 里识别，导致 autopilot pre-goal 与 AG_FOLLOWUP 循环。
    // `signal_text` 含整树字符串，覆盖仅出现在 `messages[].content` 等深层路径的 token。
    if saw_reject_reason(&signal_text, &text) {
        state.reject_reason_seen = true;
        if tracks_goal_or_drive_entry(&state) {
            state.pre_goal_review_satisfied = true;
        }
        clear_review_gate_escalation_counters(&mut state);
    }
    hydrate_goal_gate_from_disk(
        repo_root,
        &mut state,
        false,
        &frame,
        goal_drive_entrypoint,
    );
    if review || delegation || goal_drive_entrypoint {
        state.last_prompt = Some(text.chars().take(500).collect());
    }

    let pre_goal_auto_release_note = maybe_autopilot_pre_goal_nag_cap_release(&mut state);

    let persisted = save_state(repo_root, event, &mut state);

    // Review：首次武装门控时注入默认「深度+广度」契约指针（短）；相位仍只靠 subagent/PostToolUse（仅 review_hard_armed）。
    let needs_autopilot_pre_goal =
        crate::router_env_flags::router_rs_cursor_autopilot_pre_goal_enabled()
            && tracks_goal_or_drive_entry(&state)
            && !state.pre_goal_review_satisfied
            && !is_overridden(&state)
            && !state.reject_reason_seen;
    let mut output = json!({ "continue": true });
    let mut spawn_first_line: Option<String> = None;
    if review_arms_for_gate
        && (fresh_review_cycle || !prior_review_required)
        && !cursor_review_gate_suppressed(repo_root, &text)
        && !state.review_override
        && crate::hook_common::should_inject_spawn_first_review_nudge(Some(repo_root), &text)
    {
        let nudge =
            crate::runtime_registry::review_spawn_first_nudge_line(Some(repo_root), "cursor");
        spawn_first_line = Some(nudge.clone());
        merge_additional_context(&mut output, &nudge);
    }
    let skip_model_nudge = spawn_first_line.is_some()
        && crate::runtime_registry::spawn_first_includes_model_inherit_for_host(
            Some(repo_root),
            "cursor",
        );
    if !my_light
        && !cursor_review_gate_suppressed(repo_root, &text)
        && review
        && goal_drive_entrypoint
    {
        merge_additional_context(&mut output, CURSOR_REVIEW_MY_SAME_ROUND_NUDGE);
    }
    if !skip_model_nudge
        && crate::hook_common::should_inject_subagent_model_inherit_nudge(
            &text,
            user_gate_override,
            goal_drive_entrypoint,
            delegation,
            review,
        )
    {
        let model_nudge = crate::runtime_registry::review_subagent_model_inherit_nudge_line(
            Some(repo_root),
            "cursor",
        );
        merge_additional_context(&mut output, &model_nudge);
    }
    if needs_autopilot_pre_goal {
        // 仅计入总 follow-up 次数；不要把 goal_followup_count 算进去，否则首次 stop 会误判成「非首条」而跳过完整 goal 提示。
        state.followup_count += 1;
        let pre = my_pre_goal_followup_message();
        crate::autopilot_goal::merge_hook_nudge_paragraph(
            &mut output,
            &pre,
            "My implement (/implementx)",
            false,
        );
    }
    if let Some(note) = pre_goal_auto_release_note {
        merge_additional_context(&mut output, note);
    }
    if crate::hook_common::is_my_pre_execution_entry_prompt(&text) {
        merge_additional_context(&mut output, crate::hook_common::MY_PRE_EXECUTION_HOOK_NUDGE);
    }
    if goal_drive_entrypoint {
        merge_additional_context(
            &mut output,
            crate::hook_common::my_goal_drive_hook_nudge_for_prompt(&text),
        );
    }
    crate::paper_adversarial_hook::maybe_merge_paper_adversarial_before_submit(
        repo_root,
        &mut output,
        &text,
        false,
    );
    crate::paper_prose_hook::maybe_merge_paper_prose_before_submit(
        repo_root,
        &mut output,
        &text,
        false,
    );
    let persisted_after_followup = if needs_autopilot_pre_goal {
        save_state(repo_root, event, &mut state)
    } else {
        persisted
    };
    let gate_needs_persist = review_arms_for_gate
        || state.goal_required
        || needs_autopilot_pre_goal;
    if !persisted || !persisted_after_followup {
        if gate_needs_persist
            && !crate::router_env_flags::router_rs_cursor_hook_state_fail_open_enabled()
        {
            release_state_lock(&mut lock);
            let mut out = json!({ "continue": false });
            merge_fail_closed_user_messages(
                &mut out,
                "router-rs：hook-state 未能持久化，review/goal 门控本回合已拦截提交。请检查 .cursor/hook-state 目录权限或磁盘空间；应急可设 ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1。",
            );
            return out;
        }
        let warning = "router-rs：hook-state 未能持久化，review/委托门控本回合可能降级。";
        merge_additional_context(&mut output, warning);
    }
    release_state_lock(&mut lock);
    output
}

