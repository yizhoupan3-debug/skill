/// 首次武装时注入一行指针，避免 `additional_context` 过长刷屏；细则见 skill / harness §5.0。
const CURSOR_DEEP_REVIEW_DEFAULT_NUDGE: &str = "深度审稿：`skills/code-review-deep/SKILL.md`（默认≥2 路只读并行；lane 仅 general-purpose / best-of-n-runner；每路 JSON 布尔 fork_context=false）。";

/// 同一条用户提交里同时出现 review 信号与 `/autopilot` 入口时追加；与 `review_arms_for_gate` 语义对齐。
const CURSOR_REVIEW_AUTOPILOT_SAME_ROUND_NUDGE: &str = "router-rs：本轮提交同时包含「代码审查 / review」信号与 `/autopilot` 入口；门控下 **不会** 在本回合因 review 措辞新武装 `REVIEW_GATE`。若需先跑独立审稿，请拆开用户消息（先发 review-only，再发 `/autopilot`）或先落盘 `GOAL_STATE`。详见 `docs/framework_operator_primer.md`。";

/// 将一条 `review_subagent_cycle_key` 压入 multiset 并同步 legacy 字段。
///
/// **双事件去重**：宿主可能对同一子代理先发 `subagentStart` 再发 `PostToolUse`（同一 `subagent_id`）。对 **`id:`** 前缀的稳定 key，若 pending 已含该字符串，则 **PostToolUse 路径不再 push**，避免「一次 stop 只核销一条」语义下出现双 pending。
///
/// **`subagent_start_count`** 仅在 **`handle_subagent_start`** 的 qualifying review 分支递增；PostToolUse 仅负责 multiset 入队（及 phase bump），**不**增加该计数，以免与宿主双事件重复计数。
fn push_review_pending_cycle_key(
    state: &mut ReviewGateState,
    cycle_key: Option<String>,
    from_posttool: bool,
) {
    let Some(k) = cycle_key else {
        return;
    };
    if from_posttool
        && k.starts_with("id:")
        && state.review_subagent_pending_cycle_keys.contains(&k)
    {
        return;
    }
    state.review_subagent_pending_cycle_keys.push(k);
    sync_review_cycle_legacy_fields(state);
}

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
    let user_gate_override = has_override(&text);

    let prior_review_required = state.review_required;
    state.review_required = state.review_required || review_arms_for_gate;
    state.review_override = state.review_override || user_gate_override;
    state.delegation_override = state.delegation_override || user_gate_override;
    state.goal_required = state.goal_required || autopilot_entrypoint;
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
    if review && autopilot_entrypoint && !cursor_review_gate_disabled_by_env() {
        merge_additional_context(&mut output, CURSOR_REVIEW_AUTOPILOT_SAME_ROUND_NUDGE);
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
    let independent_fork = independent_context_fork(fork);
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
    if armed && independent_fork && review_kind {
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        // 仅 SubagentStart 事件计数；PostToolUse 入 multiset 不递增（见 `push_review_pending_cycle_key` 模块注释）。
        state.subagent_start_count += 1;
        state.lane_intent_matches = Some(true);
        push_review_pending_cycle_key(&mut state, cycle_key, false);
        if was_below_2 {
            clear_review_gate_escalation_counters(&mut state);
        }
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
        let cycle_matches = !state.review_subagent_pending_cycle_keys.is_empty()
            && cycle_key.as_ref().is_some_and(|k| {
                state
                    .review_subagent_pending_cycle_keys
                    .iter()
                    .any(|p| p == k)
            });
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
    let independent_fork = independent_context_fork(fork);
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
        && independent_fork
    {
        let start_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
        let was_below_2 = state.phase < 2;
        bump_phase(&mut state, 2);
        state.last_subagent_tool = Some(name.clone());
        push_review_pending_cycle_key(&mut state, start_key, true);
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
        if let Some(msg) = closeout_msg {
            out["followup_message"] = Value::String(msg);
        }
        finalize_stop_hook_outputs(repo_root, &mut out, &frame, false);
        return out;
    }
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let msg = lock_failure_followup_for_stop(event);
        let mut out = json!({ "followup_message": msg });
        finalize_stop_hook_outputs(repo_root, &mut out, &frame, false);
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
        finalize_stop_hook_outputs(repo_root, &mut out, &frame, false);
        release_state_lock(&mut lock);
        return out;
    }
    let (mut output, skip_continuity_merge) = match loaded {
        Ok(None) => (json!({}), false),
        Err(io_error) => {
            let msg = format!(
                "router-rs：hook-state 不可读（{io_error}），门控降级。请检查权限与 JSON。"
            );
            (json!({ "followup_message": msg }), false)
        }
        Ok(Some(mut state)) => {
            state.delegation_required = false;
            // Override 句式仅承认用户本轮 prompt（与 beforeSubmit 一致）；勿用含助手输出的
            // `signal_text`，避免助手复述「不要用子代理」类话术误清空 REVIEW_GATE。
            if has_override(&text) {
                state.review_override = true;
                state.delegation_override = true;
            }
            if has_structured_goal_contract(&signal_text) {
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
                let cap =
                    crate::router_env_flags::router_rs_cursor_review_gate_stop_max_nudges_cap();
                let use_full = match cap {
                    None => true,
                    Some(n) => state.review_followup_count <= n,
                };
                let skip_continuity_merge = !use_full;
                let out = if use_full {
                    json!({ "followup_message": review_stop_followup_line(&state) })
                } else {
                    let full_cap = cap.expect("soft branch implies cap=Some");
                    let soft = review_stop_followup_soft_line(&state, full_cap);
                    let mut soft_out = json!({ "followup_message": soft });
                    crate::autopilot_goal::merge_hook_nudge_paragraph(
                        &mut soft_out,
                        &review_stop_followup_detail_paragraph(&state),
                        REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX,
                        false,
                    );
                    soft_out
                };
                let _ = save_state(repo_root, event, &mut state);
                (out, skip_continuity_merge)
            } else if !goal_is_satisfied(&state) {
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                // Stop 只给短码，避免把整段 Autopilot 契约说明塞进会话收尾（细则见 beforeSubmit / AGENTS）。
                let message = goal_stop_followup_line(&state);
                (json!({ "followup_message": message }), false)
            } else {
                // Do not clear gate state on Stop for sessions that still track goal/review:
                // the next Stop should still enforce the same requirements until satisfied/overridden.
                if state.review_required || state.goal_required || state.reject_reason_seen {
                    let _ = save_state(repo_root, event, &mut state);
                } else {
                    let mut reset = empty_state();
                    let _ = save_state(repo_root, event, &mut reset);
                }
                (json!({}), false)
            }
        }
    };
    finalize_stop_hook_outputs(repo_root, &mut output, &frame, skip_continuity_merge);
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
                "router-rs 门控快照：phase={} review={} delegation={} override={} reject={} pre_goal_ok={} subagentStart_n={} subagent_stop={}",
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

fn truncate_cursor_sessionstart_context(text: &str) -> String {
    let max_bytes = crate::router_env_flags::router_rs_cursor_sessionstart_context_max_bytes();
    truncate_cursor_hook_outbound_context(text, max_bytes)
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
    // Align with Codex `handle_codex_session_start`: advisory continuity text must honor
    // `ROUTER_RS_OPERATOR_INJECT` kill-switch (terminal baseline init above is not advisory).
    if !crate::router_env_flags::router_rs_operator_inject_globally_enabled() {
        return json!({ "additional_context": "" });
    }
    let mut sections = Vec::new();
    let task_view = crate::task_state::resolve_task_view(repo_root, None);
    if crate::task_state::task_view_has_active_goal_focus_mismatch_note(&task_view) {
        sections.push(crate::task_state::CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH.to_string());
    }
    // Raw SESSION_SUMMARY body (prefix-stable under SessionStart byte cap); see
    // `session_start_additional_context_observes_router_rs_sessionstart_max_env`.
    let session_summary_path = repo_root.join("artifacts/current/SESSION_SUMMARY.md");
    if let Ok(raw) = fs::read_to_string(&session_summary_path) {
        let block = raw.trim();
        if !block.is_empty() {
            sections.push(block.to_string());
        }
    }
    if let Ok(digest) =
        crate::framework_runtime::build_framework_continuity_digest_prompt_ex(repo_root, 4, true)
    {
        let trimmed = digest.trim();
        if !trimmed.is_empty() {
            sections.push(trimmed.to_string());
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
