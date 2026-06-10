fn handle_subagent_start(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_state_lock_failure_output(repo_root, event);
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let tool_input = tool_input_of(event);
    let stale_reset = apply_subagent_stale_hygiene(&mut state);
    if let Some(limit) = cursor_max_open_subagents() {
        if state.active_subagent_count >= limit {
            release_state_lock(&mut lock);
            return subagent_limit_denial(state.active_subagent_count, limit);
        }
    }
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let fork = cursor_fork_context_from_tool(event, &tool_input, &sub_type, &agent_type);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
    let independent_fork_pre_goal =
        core_policy::review_gate_engine::review_independent_fork(fork, pre_goal_kind);
    let independent_fork_review =
        core_policy::review_gate_engine::review_independent_fork(fork, review_kind);
    let cycle_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
    let lite_stable_id = !stable_subagent_id(event, &tool_input).is_empty();
    let armed = review_hard_armed(&state);
    let mut track_open_subagent = true;
    let mut mutated = false;
    // 与 PostToolUse 对齐：pre-goal 在独立 fork 且存在 lane 类型证据时满足（含非白名单 lane 名）。
    if tracks_goal_or_drive_entry(&state) && pre_goal_kind && independent_fork_pre_goal {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if armed && independent_fork_review && review_kind {
        if state.review_pending_cap_refused {
            let _ = save_state(repo_root, event, &mut state);
            release_state_lock(&mut lock);
            return review_pending_cycle_cap_denial(
                hooks::router_rs_cursor_review_pending_cycle_max() as usize,
            );
        }
        match push_review_pending_cycle_key(&mut state, cycle_key, false, lite_stable_id) {
            PendingCyclePush::NewlyInserted => {
            let was_below_2 = state.phase < 2;
            bump_phase(&mut state, 2);
            // 仅 SubagentStart 事件计数；PostToolUse 入 multiset 不递增（见 `push_review_pending_cycle_key` 模块注释）。
            state.subagent_start_count += 1;
            state.lane_intent_matches = Some(true);
            state.last_subagent_type = Some(if !sub_type.is_empty() {
                sub_type.clone()
            } else {
                agent_type.clone()
            });
            if was_below_2 {
                clear_review_gate_escalation_counters(&mut state);
            }
            mutated = true;
            }
            PendingCyclePush::AlreadyPresent => {
                // Duplicate `id:` subagentStart: multiset deduped; do not inflate start_count or open count.
                track_open_subagent = false;
            }
            PendingCyclePush::AtCap => {
            let _ = save_state(repo_root, event, &mut state);
            release_state_lock(&mut lock);
            return review_pending_cycle_cap_denial(
                hooks::router_rs_cursor_review_pending_cycle_max() as usize,
            );
            }
        }
    }
    if track_open_subagent {
        state.active_subagent_count = state.active_subagent_count.saturating_add(1);
        state.active_subagent_last_started_at = Some(Utc::now().to_rfc3339());
        mutated = true;
    }
    if stale_reset {
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

fn handle_subagent_stop(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_state_lock_failure_output(repo_root, event);
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let mut mutated = false;
    let tool_input = tool_input_of(event);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
    let cycle_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
    let cycle_matches = !state.review_subagent_pending_cycle_keys.is_empty()
        && cycle_key.as_ref().is_some_and(|k| {
            state
                .review_subagent_pending_cycle_keys
                .iter()
                .any(|p| p == k)
        });
    // Always decrement open count on stop when tracked (P0-1); pending multiset核销与 open 计数解耦。
    if state.active_subagent_count > 0 {
        state.active_subagent_count -= 1;
        if state.active_subagent_count == 0 {
            state.active_subagent_last_started_at = None;
        }
        mutated = true;
    }
    if review_hard_armed(&state) {
        if try_settle_review_subagent_cycle(&mut state, &cycle_key, review_kind) {
            mutated = true;
        } else if state.phase < 2 || !review_kind || !cycle_matches {
            if mutated {
                let _ = save_state(repo_root, event, &mut state);
            }
            release_state_lock(&mut lock);
            return json!({});
        }
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

/// PostToolUse fast-path: skip tracker, hook-state, evidence, and rust-lint for tools that
/// cannot affect review multiset / shell ledger / pre-goal.
/// When `state` is `Some`, caller holds the session lock (P1-3: no TOCTOU on review armed).
fn post_tool_use_needs_work(
    repo_root: &Path,
    event: &Value,
    name: &str,
    state: Option<&ReviewGateState>,
) -> bool {
    if tool_name_matches_subagent_lane(name) {
        return true;
    }
    if name.eq_ignore_ascii_case("shell") {
        return true;
    }
    if tool_name_is_rust_file_write_tool(name) {
        if let Some(path) = payload_tool_path(event) {
            if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && path.is_file()
                && core_state::utils::path_guard::path_is_within_repo_root(repo_root, &path)
            {
                return true;
            }
        }
    }
    if let Some(state) = state {
        if review_hard_armed(state) {
            return review_armed_posttool_requires_l3(state, name);
        }
    }
    false
}

/// Armed review: `Read` skips L3 only when no lite/strict pending and no open subagents (D6/D12).
fn review_armed_posttool_requires_l3(state: &ReviewGateState, name: &str) -> bool {
    if name.eq_ignore_ascii_case("read") {
        return !state.review_lite_pending_cycle_keys.is_empty()
            || !state.review_subagent_pending_cycle_keys.is_empty()
            || state.active_subagent_count > 0;
    }
    true
}

