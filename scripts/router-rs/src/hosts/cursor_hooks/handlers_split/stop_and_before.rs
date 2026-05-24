fn handle_before_submit(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let (allow_continue, followup) = lock_failure_followup_for_before_submit(event);
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
        let mut out = json!({ "continue": true });
        merge_additional_context(
            &mut out,
            &format!(
                "{} (path {}, err={load_err}). Repair JSON or permissions before continuing.",
                CURSOR_HOOK_STATE_UNREADABLE,
                path.display()
            ),
        );
        return out;
    }
    let mut state = state_load.ok().flatten().unwrap_or_else(empty_state);
    let _stale_reset = apply_subagent_stale_hygiene(&mut state);
    // delegation 启发式不再持久化进 hook-state，避免与 review 相位门控长期粘连。
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
    state.goal_required = state.goal_required || goal_drive_entrypoint;
    state.goal_contract_seen =
        state.goal_contract_seen || has_structured_goal_contract(&signal_text);
    state.goal_progress_seen = state.goal_progress_seen || has_goal_progress_signal(&signal_text);
    state.goal_verify_or_block_seen =
        state.goal_verify_or_block_seen || has_goal_verify_or_block_signal(&signal_text);
    // 用户在本轮提交里写出 reject_reason token 时须即时生效；否则仅能在助手回复或 Stop 里识别，导致 autopilot pre-goal 与 AG_FOLLOWUP 循环。
    // `signal_text` 含整树字符串，覆盖仅出现在 `messages[].content` 等深层路径的 token。
    if saw_reject_reason(&signal_text, &text) {
        state.reject_reason_seen = true;
        if state.goal_required {
            state.pre_goal_review_satisfied = true;
        }
        clear_review_gate_escalation_counters(&mut state);
    }
    hydrate_goal_gate_from_disk(repo_root, &mut state, false, &frame);
    if review || delegation || goal_drive_entrypoint {
        state.last_prompt = Some(text.chars().take(500).collect());
    }

    let pre_goal_auto_release_note = maybe_autopilot_pre_goal_nag_cap_release(&mut state);

    let persisted = save_state(repo_root, event, &mut state);

    // Review：首次武装门控时注入默认「深度+广度」契约指针（短）；相位仍只靠 subagent/PostToolUse（仅 review_hard_armed）。
    let needs_autopilot_pre_goal =
        crate::router_env_flags::router_rs_cursor_autopilot_pre_goal_enabled()
            && state.goal_required
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
    if state.goal_required
        && !my_light
        && !cursor_review_gate_suppressed(repo_root, &text)
        && (review_arms_for_gate || (review && goal_drive_entrypoint))
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
        crate::review_gate_engine::cursor_review_independent_fork(fork, pre_goal_kind);
    let independent_fork_review =
        crate::review_gate_engine::cursor_review_independent_fork(fork, review_kind);
    let cycle_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
    let armed = review_hard_armed(&state);
    let track_open_subagent = true;
    let mut mutated = false;
    // 与 PostToolUse 对齐：pre-goal 在独立 fork 且存在 lane 类型证据时满足（含非白名单 lane 名）。
    if state.goal_required && pre_goal_kind && independent_fork_pre_goal {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if armed && independent_fork_review && review_kind {
        if state.review_pending_cap_refused {
            let _ = save_state(repo_root, event, &mut state);
            release_state_lock(&mut lock);
            return review_pending_cycle_cap_denial(
                crate::router_env_flags::router_rs_cursor_review_pending_cycle_max() as usize,
            );
        }
        if push_review_pending_cycle_key(&mut state, cycle_key, false) {
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
        } else {
            let _ = save_state(repo_root, event, &mut state);
            release_state_lock(&mut lock);
            return review_pending_cycle_cap_denial(
                crate::router_env_flags::router_rs_cursor_review_pending_cycle_max() as usize,
            );
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
    let decrement_open_count = if review_hard_armed(&state) && review_kind {
        cycle_matches
    } else {
        state.active_subagent_count > 0
    };
    if decrement_open_count && state.active_subagent_count > 0 {
        state.active_subagent_count -= 1;
        if state.active_subagent_count == 0 {
            state.active_subagent_last_started_at = None;
        }
        mutated = true;
    }
    if review_hard_armed(&state) {
        // Stop：命中 pending  multiset 中**一条**同 key 的 start 记录则移除该条；**仅当** pending 排空时升 phase 3
        // 并记 `subagent_stop_count`（并行多路需各路各一次 qualifying stop，同 lane 无 id 时依赖重复 `lane:` key）。
        if state.phase < 2 || !review_kind || !cycle_matches {
            if mutated {
                let _ = save_state(repo_root, event, &mut state);
            }
            release_state_lock(&mut lock);
            return json!({});
        }
        if let Some(ref k) = cycle_key {
            if let Some(pos) = state
                .review_subagent_pending_cycle_keys
                .iter()
                .position(|p| p == k)
            {
                state.review_subagent_pending_cycle_keys.remove(pos);
            }
        }
        sync_review_cycle_legacy_fields(&mut state);
        if state.review_subagent_pending_cycle_keys.is_empty() {
            state.review_pending_cap_refused = false;
            bump_phase(&mut state, 3);
            state.subagent_stop_count += 1;
            state.lane_intent_matches = Some(true);
        }
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

/// PostToolUse fast-path: skip tracker, hook-state, evidence, and rust-lint for tools that
/// cannot affect review multiset / shell ledger / pre-goal (see plan: Cursor memory P0).
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
                && crate::path_guard::path_is_within_repo_root(repo_root, &path)
            {
                return true;
            }
        }
    }
    if let Some(state) = state {
        if review_hard_armed(state) {
            return true;
        }
    }
    false
}

fn handle_post_tool_use(repo_root: &Path, event: &Value) -> Value {
    let name = normalize_tool_name(Some(&tool_name_of(event)));
    let review_armed_peek = peek_review_hard_armed(repo_root, event);

    if review_armed_peek {
        let mut lock = acquire_state_lock(repo_root, event);
        if lock.is_none() {
            return hook_state_lock_fail_closed_for_review_json();
        }
        let state = load_state(repo_root, event)
            .ok()
            .flatten()
            .unwrap_or_else(empty_state);
        if !post_tool_use_needs_work(repo_root, event, &name, Some(&state)) {
            release_state_lock(&mut lock);
            return json!({});
        }
        if let Err(e) = crate::session_call_tracker::record_tool_call(repo_root, &name) {
            eprintln!("[router-rs] session tracker record_tool_call failed (non-fatal): {e}");
        }
        return handle_post_tool_use_with_lock(repo_root, event, &name, &mut lock, state);
    }

    if !post_tool_use_needs_work(repo_root, event, &name, None) {
        return json!({});
    }
    if let Err(e) = crate::session_call_tracker::record_tool_call(repo_root, &name) {
        eprintln!("[router-rs] session tracker record_tool_call failed (non-fatal): {e}");
    }
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    handle_post_tool_use_with_lock(repo_root, event, &name, &mut lock, state)
}

fn handle_post_tool_use_with_lock(
    repo_root: &Path,
    event: &Value,
    name: &str,
    lock: &mut Option<LockGuard>,
    mut state: ReviewGateState,
) -> Value {
    let armed = review_hard_armed(&state);
    let tool_input = tool_input_of(event);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let fork = cursor_fork_context_from_tool(event, &tool_input, &sub_type, &agent_type);
    let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
    let independent_fork_review =
        crate::review_gate_engine::cursor_review_independent_fork(fork, review_kind);
    let independent_fork_pre_goal =
        crate::review_gate_engine::cursor_review_independent_fork(fork, pre_goal_kind);
    let mut mutated = false;
    if tool_name_matches_subagent_lane(&name)
        && pre_goal_kind
        && state.goal_required
        && independent_fork_pre_goal
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if tool_name_matches_subagent_lane(&name) && review_kind && armed && independent_fork_review {
        if let Some(limit) = cursor_max_open_subagents() {
            let pending_open = state.review_subagent_pending_cycle_keys.len() as u32;
            if pending_open.saturating_add(state.active_subagent_count) >= limit {
                release_state_lock(lock);
                return subagent_limit_denial(
                    state.active_subagent_count.saturating_add(pending_open),
                    limit,
                );
            }
        }
        let start_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
        if push_review_pending_cycle_key(&mut state, start_key, true) {
            let was_below_2 = state.phase < 2;
            bump_phase(&mut state, 2);
            if state.active_subagent_last_started_at.is_none() {
                state.active_subagent_last_started_at = Some(Utc::now().to_rfc3339());
            }
            state.last_subagent_tool = Some(name.to_string());
            if !sub_type.is_empty() || !agent_type.is_empty() {
                state.last_subagent_type = Some(if !sub_type.is_empty() {
                    sub_type
                } else {
                    agent_type
                });
            }
            state.lane_intent_matches = Some(true);
            if was_below_2 {
                clear_review_gate_escalation_counters(&mut state);
            }
            mutated = true;
        } else if state.review_pending_cap_refused {
            mutated = true;
        }
    }
    // Agent `Shell` 工具：在释放 session 锁之前更新终端账本（ADR-003）。
    if name == "shell" {
        cursor_post_tool_shell_terminal_track(repo_root, event);
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);

    // 与 Codex PostTool 对齐：终端执行验证类命令时写入 EVIDENCE_INDEX（连续性就绪且未关闭 POSTTOOL_EVIDENCE）。
    let syn = crate::hook_posttool_normalize::synthetic_post_tool_evidence_shape(event);
    if let Err(err) = crate::framework_runtime::try_append_post_tool_shell_evidence(
        repo_root,
        &syn,
        "cursor_post_tool_verification",
    ) {
        eprintln!("[router-rs] cursor post-tool evidence append failed (non-fatal): {err}");
    }

    let mut out = json!({});
    if let Some(ctx) = maybe_run_cursor_rust_lint(repo_root, event) {
        merge_additional_context(&mut out, &ctx);
    }
    out
}

fn payload_tool_name(event: &Value) -> String {
    tool_name_of(event).trim().to_string()
}

fn payload_tool_path(event: &Value) -> Option<PathBuf> {
    event
        .get("tool_input")
        .and_then(|t| t.get("path"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| {
            event
                .get("file_path")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
}

fn tool_name_is_rust_file_write_tool(name: &str) -> bool {
    let n = name.trim();
    matches!(n, "Write" | "StrReplace" | "write" | "str_replace")
}

fn find_cargo_dir(start: &Path) -> Option<PathBuf> {
    let mut cur = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    for _ in 0..64 {
        if cur.join("Cargo.toml").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

fn truncate_lines(s: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    s.lines().take(max_lines).collect::<Vec<_>>().join("\n")
}

fn cargo_check_with_timeout(cargo_dir: &Path, timeout: std::time::Duration) -> (i32, String) {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = match Command::new("cargo")
        .arg("check")
        .arg("--message-format=short")
        .current_dir(cargo_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => return (127, format!("rust-lint: failed to spawn cargo: {err}")),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(1);
                let mut buf = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    use std::io::Read;
                    let _ = stderr.read_to_string(&mut buf);
                }
                return (code, buf);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return (124, "rust-lint: cargo check exceeded timeout".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(err) => return (1, format!("rust-lint: cargo check wait error: {err}")),
        }
    }
}

fn maybe_run_cursor_rust_lint(repo_root: &Path, event: &Value) -> Option<String> {
    if !crate::router_env_flags::router_rs_cursor_cargo_check_sync_enabled() {
        return None;
    }
    const TIMEOUT_S: u64 = 25;
    const MAX_ERROR_LINES: usize = 20;

    let tool_name = payload_tool_name(event);
    if !tool_name_is_rust_file_write_tool(&tool_name) {
        return None;
    }
    let path = payload_tool_path(event)?;
    if !crate::path_guard::path_is_within_repo_root(repo_root, &path) {
        return None;
    }
    if path.extension().and_then(|e| e.to_str()) != Some("rs") {
        return None;
    }
    if !path.is_file() {
        return None;
    }
    if which::which("cargo").is_err() {
        return None;
    }
    let cargo_dir = find_cargo_dir(&path)?;
    if !crate::path_guard::path_is_within_repo_root(repo_root, &cargo_dir) {
        return None;
    }

    let cargo_start = std::time::Instant::now();
    let (rc, output) =
        cargo_check_with_timeout(&cargo_dir, std::time::Duration::from_secs(TIMEOUT_S));
    crate::hook_timing::add_cargo_check_ms(cargo_start.elapsed().as_millis() as u64);

    // Continuity: append cargo check outcome to artifacts/current/EVIDENCE_INDEX.json (no-op if continuity not seeded).
    let cmd_preview = format!(
        "(cd {} && cargo check --message-format=short)",
        cargo_dir.display()
    );
    let _ = crate::framework_runtime::framework_hook_evidence_append(json!({
        "repo_root": repo_root.display().to_string(),
        "command_preview": cmd_preview,
        "exit_code": rc,
        "source": "cursor_rust_lint",
    }));

    if rc == 0 {
        return None;
    }
    if rc == 124 {
        let base = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file.rs");
        return Some(format!(
            "cargo check timed out after {TIMEOUT_S}s while checking {base} (crate: {}). Consider running cargo check manually.",
            cargo_dir.display()
        ));
    }

    let errors: String = output
        .lines()
        .filter(|l| l.starts_with("error") || l.starts_with("warning"))
        .take(MAX_ERROR_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    let fallback = truncate_lines(&output, MAX_ERROR_LINES);
    let picked = if !errors.trim().is_empty() {
        errors
    } else {
        fallback
    };
    let base = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file.rs");
    Some(format!(
        "cargo check failed after editing {base}:\n{picked}\n\nFix these errors before finalizing. Run `cargo check` to verify."
    ))
}

fn handle_after_agent_response(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = review_hard_armed(&state);
    let track_goal = state.goal_required || armed;
    let prompt = prompt_text(event);
    let text = agent_response_text(event);
    let signal = hook_event_signal_text(event, &prompt, &text);
    let mut dirty = false;
    if saw_reject_reason(&signal, &prompt) {
        state.reject_reason_seen = true;
        if state.goal_required {
            state.pre_goal_review_satisfied = true;
        }
        clear_review_gate_escalation_counters(&mut state);
        dirty = true;
    }
    if track_goal && has_structured_goal_contract(&signal) {
        state.goal_contract_seen = true;
        dirty = true;
    }
    if track_goal && has_goal_progress_signal(&signal) {
        state.goal_progress_seen = true;
        dirty = true;
    }
    if track_goal && has_goal_verify_or_block_signal(&signal) {
        state.goal_verify_or_block_seen = true;
        dirty = true;
    }
    let tail = crate::hook_common::hook_assistant_tail_window(
        &text,
        crate::hook_common::CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
    );
    if maybe_bump_review_phase_for_main_thread_compact_findings(&mut state, &tail) {
        dirty = true;
    }
    if dirty {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

