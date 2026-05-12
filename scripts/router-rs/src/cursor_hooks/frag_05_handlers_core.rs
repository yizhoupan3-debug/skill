const CURSOR_DEEP_REVIEW_DEFAULT_NUDGE: &str = "默认审稿=深度+广度（非轻度）：只读、`fork_context=false` 子代理须为 **general-purpose**（或 **best-of-n-runner**）；explore/CI/guide 不计入 REVIEW_GATE。按 `skills/code-review-deep/SKILL.md`：verdict-first、五透镜、P0/P1 须路径/符号锚点（或外链）。";

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
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    // delegation 启发式不再持久化进 hook-state，避免与 review 相位门控长期粘连。
    state.delegation_required = false;
    let text = prompt_text(event);
    let signal_text = hook_event_signal_text(event, &text, "");
    let review = is_review_prompt(&text);
    let autopilot_entrypoint = is_autopilot_goal_entry_prompt(&text, &signal_text);
    let review_arms_for_gate = review && !autopilot_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let review_override = has_review_override(&text) || has_override(&text);
    let delegation_override = has_delegation_override(&text) || has_override(&text);

    let prior_review_required = state.review_required;
    state.review_required = state.review_required || review_arms_for_gate;
    state.review_override = state.review_override || review_override;
    state.delegation_override = state.delegation_override || delegation_override;
    state.goal_required = state.goal_required || autopilot_entrypoint;
    state.goal_contract_seen = state.goal_contract_seen || has_goal_contract_signal(&signal_text);
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
    if review || delegation || autopilot_entrypoint {
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
    if review_arms_for_gate
        && !prior_review_required
        && !cursor_review_gate_disabled_by_env()
        && !state.review_override
    {
        merge_additional_context(&mut output, CURSOR_DEEP_REVIEW_DEFAULT_NUDGE);
    }
    if needs_autopilot_pre_goal {
        // 仅计入总 follow-up 次数；不要把 goal_followup_count 算进去，否则首次 stop 会误判成「非首条」而跳过完整 goal 提示。
        state.followup_count += 1;
        let pre = autopilot_pre_goal_followup_message();
        crate::autopilot_goal::merge_hook_nudge_paragraph(
            &mut output,
            &pre,
            "Autopilot (/autopilot)",
            false,
        );
    }
    if let Some(note) = pre_goal_auto_release_note {
        merge_additional_context(&mut output, note);
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
    release_state_lock(&mut lock);
    if !persisted || !persisted_after_followup {
        let warning = "router-rs：hook-state 未能持久化，review/委托门控本回合可能降级。";
        merge_additional_context(&mut output, warning);
    }
    output
}

fn handle_subagent_start(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let tool_input = tool_input_of(event);
    let stale_reset = reset_stale_active_subagents(&mut state);
    if let Some(limit) = cursor_max_open_subagents() {
        if state.active_subagent_count >= limit {
            release_state_lock(&mut lock);
            return subagent_limit_denial(state.active_subagent_count, limit);
        }
    }
    let fork = fork_context_from_tool(event, &tool_input);
    let independent_fork = counts_as_independent_context_fork(fork);
    let explicit_independent_fork = fork_context_explicit_false(fork);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
    let cycle_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
    let armed = review_hard_armed(&state);
    state.active_subagent_count = state.active_subagent_count.saturating_add(1);
    state.active_subagent_last_started_at = Some(Utc::now().to_rfc3339());
    let mut mutated = true;
    // 与 PostToolUse 对齐：pre-goal 在独立 fork 且存在 lane 类型证据时满足（含非白名单 lane 名）。
    if state.goal_required && pre_goal_kind && independent_fork {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if armed && explicit_independent_fork && review_kind {
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.lane_intent_matches = Some(true);
        state.review_subagent_cycle_open = true;
        state.review_subagent_cycle_key = cycle_key;
        if was_below_2 {
            clear_review_gate_escalation_counters(&mut state);
        }
        mutated = true;
    }
    if armed && explicit_independent_fork && review_kind {
        state.last_subagent_type = Some(if !sub_type.is_empty() {
            sub_type.clone()
        } else {
            agent_type.clone()
        });
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
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let mut mutated = false;
    if state.active_subagent_count > 0 {
        state.active_subagent_count -= 1;
        if state.active_subagent_count == 0 {
            state.active_subagent_last_started_at = None;
        }
        mutated = true;
    }
    if review_hard_armed(&state) {
        let tool_input = tool_input_of(event);
        let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
        let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
        let cycle_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
        let cycle_matches = state.review_subagent_cycle_open
            && cycle_key.is_some()
            && cycle_key == state.review_subagent_cycle_key;
        // Stop evidence only counts for the same explicit reviewer cycle that opened the gate.
        if state.phase < 2 || !review_kind || !cycle_matches {
            if mutated {
                let _ = save_state(repo_root, event, &mut state);
            }
            release_state_lock(&mut lock);
            return json!({});
        }
        bump_phase(&mut state, 3);
        state.subagent_stop_count += 1;
        state.lane_intent_matches = Some(true);
        state.review_subagent_cycle_open = false;
        state.review_subagent_cycle_key = None;
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

fn handle_post_tool_use(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return hook_lock_unavailable_notice_json();
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = review_hard_armed(&state);
    let name = normalize_tool_name(Some(&tool_name_of(event)));
    let tool_input = tool_input_of(event);
    let (sub_type, agent_type) = cursor_subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let fork = fork_context_from_tool(event, &tool_input);
    let independent_fork = counts_as_independent_context_fork(fork);
    let explicit_independent_fork = fork_context_explicit_false(fork);
    let mut mutated = false;
    if tool_name_matches_subagent_lane(&name)
        && pre_goal_kind
        && state.goal_required
        && independent_fork
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if tool_name_matches_subagent_lane(&name)
        && review_subagent_kind_ok(&sub_type, &agent_type)
        && armed
        && explicit_independent_fork
    {
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.last_subagent_tool = Some(name.clone());
        state.review_subagent_cycle_open = true;
        state.review_subagent_cycle_key =
            review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
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
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);

    // Agent `Shell` 工具：`before/afterShellExecution` 可能不与 Task 工具一一对应；PostToolUse 再补记归属。
    if name == "shell" {
        cursor_post_tool_shell_terminal_track(repo_root, event);
    }

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
    const TIMEOUT_S: u64 = 25;
    const MAX_ERROR_LINES: usize = 20;

    let tool_name = payload_tool_name(event);
    if !tool_name_is_rust_file_write_tool(&tool_name) {
        return None;
    }
    let path = payload_tool_path(event)?;
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

    let (rc, output) =
        cargo_check_with_timeout(&cargo_dir, std::time::Duration::from_secs(TIMEOUT_S));

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
    if track_goal && has_goal_contract_signal(&signal) {
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
    if dirty {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(&mut lock);
    json!({})
}

fn handle_stop(repo_root: &Path, event: &Value) -> Value {
    let frame = crate::task_state::resolve_cursor_continuity_frame(repo_root);
    if cursor_review_gate_disabled_by_env() {
        let response_text = agent_response_text(event);
        let closeout_msg =
            stop_hard_closeout_followup_for_assistant_response(repo_root, &response_text);
        let mut out = json!({});
        merge_continuity_followups(repo_root, &mut out, &frame);
        if closeout_msg.is_none() {
            merge_session_close_style_nudge_when_soft_terminal(&mut out);
        }
        if let Some(msg) = closeout_msg {
            out["followup_message"] = Value::String(msg);
        }
        return out;
    }
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let msg = lock_failure_followup_for_stop(event);
        let mut out = json!({ "followup_message": msg });
        finalize_stop_hook_outputs(repo_root, &mut out, &frame);
        return out;
    }
    let loaded = load_state(repo_root, event);
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_text);

    // Completion claim guard must not depend on hook-state existence: a strict closeout violation
    // is a hard-stop even when the review gate state was never initialized for this session.
    if let Some(msg) = stop_hard_closeout_followup_for_assistant_response(repo_root, &response_text)
    {
        let mut out = json!({ "followup_message": msg });
        finalize_stop_hook_outputs(repo_root, &mut out, &frame);
        release_state_lock(&mut lock);
        return out;
    }
    let mut output = match loaded {
        Ok(None) => json!({}),
        Err(io_error) => {
            let msg = format!(
                "router-rs：hook-state 不可读（{io_error}），门控降级。请检查权限与 JSON。"
            );
            json!({ "followup_message": msg })
        }
        Ok(Some(mut state)) => {
            state.delegation_required = false;
            if has_review_override(&signal_text) || has_override(&signal_text) {
                state.review_override = true;
            }
            if has_delegation_override(&signal_text) || has_override(&signal_text) {
                state.delegation_override = true;
            }
            if has_goal_contract_signal(&signal_text) {
                state.goal_contract_seen = true;
            }
            if has_goal_progress_signal(&signal_text) {
                state.goal_progress_seen = true;
            }
            if has_goal_verify_or_block_signal(&signal_text) {
                state.goal_verify_or_block_seen = true;
            }
            if saw_reject_reason(&signal_text, &text) {
                state.reject_reason_seen = true;
                if state.goal_required {
                    state.pre_goal_review_satisfied = true;
                }
                clear_review_gate_escalation_counters(&mut state);
            }
            hydrate_goal_gate_from_disk(repo_root, &mut state, true, &frame);
            if review_stop_followup_needed(&state) {
                state.followup_count += 1;
                state.review_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                let message = review_stop_followup_line(&state);
                json!({ "followup_message": message })
            } else if !goal_is_satisfied(&state) {
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                // Stop 只给短码，避免把整段 Autopilot 契约说明塞进会话收尾（细则见 beforeSubmit / AGENTS）。
                let message = goal_stop_followup_line(&state);
                json!({ "followup_message": message })
            } else {
                // Do not clear gate state on Stop for sessions that still track goal/review:
                // the next Stop should still enforce the same requirements until satisfied/overridden.
                if state.review_required || state.goal_required || state.reject_reason_seen {
                    let _ = save_state(repo_root, event, &mut state);
                } else {
                    let mut reset = empty_state();
                    let _ = save_state(repo_root, event, &mut reset);
                }
                json!({})
            }
        }
    };
    finalize_stop_hook_outputs(repo_root, &mut output, &frame);
    release_state_lock(&mut lock);
    output
}

fn handle_pre_compact(repo_root: &Path, event: &Value) -> Value {
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({
            "additional_context": "router-rs：hook-state 锁不可用，preCompact 未读到持久化门控状态。"
        });
    }
    let mut out = match load_state(repo_root, event) {
        Ok(Some(state)) => {
            let mut summary = format!(
                "router-rs 门控快照：phase={} review={} delegation={} override={} reject={} pre_goal_ok={} subagent_start={} subagent_stop={}",
                state.phase,
                state.review_required,
                state.delegation_required,
                is_overridden(&state),
                state.reject_reason_seen,
                state.pre_goal_review_satisfied,
                state.subagent_start_count,
                state.subagent_stop_count
            );
            if let Some(hint) = crate::rfv_loop::rfv_loop_precompact_hint(repo_root) {
                summary.push_str(" | ");
                summary.push_str(&hint);
            }
            json!({ "additional_context": summary })
        }
        _ => json!({}),
    };
    // Token awareness (ported from .cursor/hooks/precompact-notice.sh)
    let usage = event
        .get("context_usage_percent")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("context_usage_percent")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let tokens = event
        .get("context_tokens")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("context_tokens")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let size = event
        .get("context_window_size")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("context_window_size")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let msgs = event
        .get("message_count")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("message_count")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let compact = event
        .get("messages_to_compact")
        .and_then(Value::as_i64)
        .map(|v| v.to_string())
        .or_else(|| {
            event
                .get("messages_to_compact")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".to_string());
    let trigger = event
        .get("trigger")
        .and_then(Value::as_str)
        .unwrap_or("auto")
        .to_string();
    let first = event
        .get("is_first_compaction")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut notice = format!(
        "⚡ Context compacting ({trigger}): {usage}% used · {tokens}/{size} tokens · {msgs} messages · {compact} being summarised."
    );
    if first {
        notice.push_str(" First compaction — earlier details may be summarised.");
    }
    notice.push_str(" Consider starting a new session if the current task scope is complete.");
    out["user_message"] = Value::String(notice);
    release_state_lock(&mut lock);
    out
}

fn read_file_head_lines(path: &Path, max_lines: usize) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(text.lines().take(max_lines).collect::<Vec<_>>().join("\n"))
}

fn read_json_value_strict(path: &Path) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()
}

fn truncate_cursor_sessionstart_context(text: &str) -> String {
    let max_bytes = crate::router_env_flags::router_rs_cursor_sessionstart_context_max_bytes();
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let budget = max_bytes.saturating_sub(3);
    let mut cut = budget.min(text.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    if let Some(pos) = text[..cut].rfind('\n') {
        let trimmed = text[..pos].trim_end();
        if !trimmed.is_empty() {
            return format!("{trimmed}...");
        }
    }
    format!("{}...", text[..cut].trim_end())
}

fn compact_cursor_sessionstart_context(parts: Vec<String>) -> Option<String> {
    let joined = parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if joined.trim().is_empty() {
        None
    } else {
        Some(truncate_cursor_sessionstart_context(&joined))
    }
}

fn handle_session_start(repo_root: &Path, event: &Value) -> Value {
    maybe_init_session_terminal_ledger(repo_root, event);
    let mut sections = Vec::new();

    let active_task = repo_root.join("artifacts/current/active_task.json");
    if active_task.is_file() {
        if let Some(v) = read_json_value_strict(&active_task) {
            if let Some(tid) = v.get("task_id").and_then(Value::as_str) {
                if !tid.trim().is_empty() && tid.trim() != "null" {
                    let gs = repo_root
                        .join("artifacts/current")
                        .join(tid)
                        .join("GOAL_STATE.json");
                    let rv = repo_root
                        .join("artifacts/current")
                        .join(tid)
                        .join("RFV_LOOP_STATE.json");
                    if gs.is_file() {
                        if let Some(gv) = read_json_value_strict(&gs) {
                            let status = gv.get("status").and_then(Value::as_str).unwrap_or("?");
                            let drive = gv
                                .get("drive_until_done")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            let goal = gv.get("goal").and_then(Value::as_str).unwrap_or("-");
                            sections.push(format!(
                                "Goal: {status} · drive={drive} · {}",
                                crate::framework_runtime::truncate_utf8_chars_with_ellipsis(
                                    goal, 140,
                                )
                            ));
                        }
                    }
                    if rv.is_file() {
                        if let Some(rvj) = read_json_value_strict(&rv) {
                            let loop_status = rvj
                                .get("loop_status")
                                .and_then(Value::as_str)
                                .unwrap_or("?");
                            let cur = rvj
                                .get("current_round")
                                .and_then(Value::as_i64)
                                .unwrap_or(0);
                            let max = rvj.get("max_rounds").and_then(Value::as_i64).unwrap_or(0);
                            let goal = rvj.get("goal").and_then(Value::as_str).unwrap_or("-");
                            sections.push(format!(
                                "RFV: {loop_status} · round {cur}/{max} · {}",
                                crate::framework_runtime::truncate_utf8_chars_with_ellipsis(
                                    goal, 100,
                                )
                            ));
                        }
                    }
                }
            }
        }
    }
    let summary_path = repo_root.join("artifacts/current/SESSION_SUMMARY.md");
    if summary_path.is_file() {
        if let Some(head) = read_file_head_lines(&summary_path, 12) {
            let head = head.trim();
            if !head.is_empty() {
                sections.push(format!(
                    "Continuity: {}",
                    crate::framework_runtime::truncate_utf8_chars_with_ellipsis(head, 420)
                ));
            }
        }
    }
    sections.push(format!("Repo: {}", repo_root.display()));
    let ctx = compact_cursor_sessionstart_context(sections).unwrap_or_default();
    json!({ "additional_context": ctx })
}

fn shell_event_command(event: &Value) -> Option<String> {
    first_nonempty_event_str(event, &["command"])
        .split('\n')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn shell_event_cwd(event: &Value) -> Option<PathBuf> {
    let cwd = first_nonempty_event_str(event, &["cwd"]);
    if cwd.trim().is_empty() {
        return None;
    }
    Some(PathBuf::from(cwd))
}

fn maybe_init_session_terminal_ledger(repo_root: &Path, event: &Value) {
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return;
    };
    let observations = collect_terminal_observations(&terminals_dir);
    let mut baseline: Vec<u32> = observations.iter().map(|o| o.pid).collect();
    baseline.sort_unstable();
    baseline.dedup();
    let ledger = SessionTerminalLedger {
        version: SESSION_TERMINAL_LEDGER_VERSION,
        baseline_pids: baseline,
        owned_pids: Vec::new(),
        pending_shells: Vec::new(),
    };
    save_session_terminal_ledger(repo_root, event, &ledger);
}

fn maybe_track_shell_owned_terminals(
    repo_root: &Path,
    event: &Value,
    matched_after_ms: Option<u64>,
) {
    let Some(terminals_dir) = resolve_cursor_terminals_dir(repo_root) else {
        return;
    };
    let observations = collect_terminal_observations(&terminals_dir);
    if observations.is_empty() {
        return;
    }
    let mut ledger = load_session_terminal_ledger(repo_root, event);
    if ledger.version != SESSION_TERMINAL_LEDGER_VERSION {
        ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
    }
    let baseline: HashSet<u32> = ledger.baseline_pids.iter().copied().collect();
    let mut owned: HashSet<u32> = ledger.owned_pids.iter().copied().collect();
    let cwd_filter = shell_event_cwd(event);
    let cmd_filter = shell_event_command(event).map(|s| normalize_shell_command(&s));
    for obs in observations {
        if baseline.contains(&obs.pid) {
            continue;
        }
        if let Some(t0) = matched_after_ms {
            if let Some(sa) = obs.started_at_ms {
                let floor = t0.saturating_sub(SHELL_TERMINAL_TIME_MATCH_SLACK_MS);
                if sa < floor {
                    continue;
                }
            }
        }
        if !obs.cwd.is_absolute() {
            continue;
        }
        if let Some(ref cwd) = cwd_filter {
            let obs_canon = obs.cwd.canonicalize().unwrap_or_else(|_| obs.cwd.clone());
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if !obs_canon.starts_with(&cwd_canon) && !cwd_canon.starts_with(&obs_canon) {
                continue;
            }
        }
        if let Some(ref cmd) = cmd_filter {
            let active = obs
                .active_command
                .as_deref()
                .map(normalize_shell_command)
                .unwrap_or_default();
            let last = obs
                .last_command
                .as_deref()
                .map(normalize_shell_command)
                .unwrap_or_default();
            if !active.is_empty()
                && !last.is_empty()
                && !active.contains(cmd)
                && !cmd.contains(&active)
                && !last.contains(cmd)
                && !cmd.contains(&last)
            {
                continue;
            }
        }
        owned.insert(obs.pid);
    }
    let mut owned_vec: Vec<u32> = owned.into_iter().collect();
    owned_vec.sort_unstable();
    ledger.owned_pids = owned_vec;
    save_session_terminal_ledger(repo_root, event, &ledger);
}

fn handle_before_shell_execution(repo_root: &Path, event: &Value) -> Value {
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let cmd_norm = shell_event_command(event)
        .map(|s| normalize_shell_command(&s))
        .unwrap_or_default();
    if !cmd_norm.is_empty() {
        let mut ledger = load_session_terminal_ledger(repo_root, event);
        ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
        let cwd_raw = shell_event_cwd(event)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        ledger.pending_shells.push(PendingShellRecord {
            command_norm: cmd_norm,
            cwd_raw,
            queued_ms: now_millis(),
        });
        trim_pending_shell_records(&mut ledger);
        save_session_terminal_ledger(repo_root, event, &ledger);
        // Shell 仍未真正启动 PID 前：仅用 baseline-diff + 指令/cwd 启发式扩展 owned（不关时间窗）。
        maybe_track_shell_owned_terminals(repo_root, event, None);
    }
    json!({
        "continue": true,
        "permission": "allow"
    })
}

fn handle_after_shell_execution(repo_root: &Path, event: &Value) -> Value {
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let cmd_norm = shell_event_command(event)
        .map(|s| normalize_shell_command(&s))
        .unwrap_or_default();
    let cwd_buf = shell_event_cwd(event);
    let cwd_hint = cwd_buf.as_deref();
    let mut ledger = load_session_terminal_ledger(repo_root, event);
    ledger.version = SESSION_TERMINAL_LEDGER_VERSION;
    let matched_after_ms = pop_matching_pending_shell(&mut ledger, &cmd_norm, cwd_hint);
    save_session_terminal_ledger(repo_root, event, &ledger);
    // 配对成功则用 pending 队列时间压低「它仓并发 terminal」误判；配对失败退回纯启发式（None）。
    maybe_track_shell_owned_terminals(repo_root, event, matched_after_ms);
    json!({})
}
