fn handle_post_tool_use(repo_root: &Path, event: &Value) -> Value {
    let name = normalize_tool_name(Some(&crate::hosts::hook_dispatch::extract_tool_name(event)));
    let tool_origin = core_policy::hook_common::classify_tool_origin(&name);
    let _ = &tool_origin;

    // Shared tool call telemetry (4-host unified)
    crate::hosts::hook_dispatch::record_tool_call_emission(
        repo_root,
        &name,
        hooks::extract_post_tool_duration_ms(event).unwrap_or(0),
        hooks::post_tool_call_succeeded(event),
    );
    let review_armed_peek = peek_review_hard_armed(repo_root, event);

    if review_armed_peek {
        if !post_tool_use_needs_work(repo_root, event, &name, None) {
            return json!({});
        }
        let mut lock = acquire_state_lock(repo_root, event);
        if lock.is_none() {
            return post_tool_armed_hook_state_lock_fail_closed_json();
        }
        let state = load_state(repo_root, event)
            .ok()
            .flatten()
            .unwrap_or_else(empty_state);
        if !post_tool_use_needs_work(repo_root, event, &name, Some(&state)) {
            release_state_lock(&mut lock);
            return json!({});
        }
        return handle_post_tool_use_with_lock(repo_root, event, &name, &mut lock, state);
    }

    if !post_tool_use_needs_work(repo_root, event, &name, None) {
        return json!({});
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
    let tool_input = crate::hosts::hook_dispatch::extract_tool_input(event);
    let (sub_type, agent_type) = subagent_type_pair(&tool_input, event);
    let pre_goal_kind = pre_goal_subagent_kind_ok(&sub_type, &agent_type);
    let fork = fork_context_from_tool_with_inference(event, &tool_input, &sub_type, &agent_type);
    let review_kind = review_subagent_kind_ok(&sub_type, &agent_type);
    let independent_fork_review =
        core_policy::review_gate_engine::review_independent_fork(fork, review_kind);
    let independent_fork_pre_goal =
        core_policy::review_gate_engine::review_independent_fork(fork, pre_goal_kind);
    let mut mutated = false;
    if tool_name_matches_subagent_lane(name)
        && pre_goal_kind
        && crate::hosts::hook_dispatch::shared_tracks_goal(state.goal_required, state.core.goal_drive_entry_active)
        && independent_fork_pre_goal
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
        mutated = true;
    }
    if tool_name_matches_subagent_lane(name) && review_kind && armed && independent_fork_review {
        if let Some(limit) = max_open_subagents() {
            let pending_open = state
                .review_subagent_pending_cycle_keys
                .len()
                .saturating_add(state.review_lite_pending_cycle_keys.len()) as u32;
            if pending_open.saturating_add(state.active_subagent_count) >= limit {
                release_state_lock(lock);
                return subagent_limit_denial(
                    state.active_subagent_count.saturating_add(pending_open),
                    limit,
                );
            }
        }
        let start_key = review_subagent_cycle_key(event, &tool_input, &sub_type, &agent_type);
        let lite_stable_id = !stable_subagent_id(event, &tool_input).is_empty();
        let push = push_review_pending_cycle_key(&mut state, start_key.clone(), true, lite_stable_id);
        if push != PendingCyclePush::AtCap {
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
        // Task/subagent PostTool 返回即一轮 lane 结束；弥补宿主漏发或 id 漂移的 subagentStop。
        if try_settle_review_subagent_cycle(&mut state, &start_key, review_kind) {
            if state.active_subagent_count > 0 {
                state.active_subagent_count -= 1;
                if state.active_subagent_count == 0 {
                    state.active_subagent_last_started_at = None;
                }
            }
            mutated = true;
        }
    }
    // Agent `Shell` 工具：在释放 session 锁之前更新终端账本（ADR-003）。
    if name == "shell" {
        post_tool_shell_terminal_track(repo_root, event);
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);

    // 与 Codex PostTool 对齐：终端执行验证类命令时写入 EVIDENCE_INDEX（连续性就绪且未关闭 POSTTOOL_EVIDENCE）。
    let syn = hooks::synthetic_post_tool_evidence_shape(event);
    if let Err(err) = hooks::try_append_post_tool_shell_evidence(
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
    crate::hosts::hook_dispatch::extract_tool_name(event).trim().to_string()
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
    if !hooks::router_rs_cargo_check_sync_enabled() {
        return None;
    }
    const TIMEOUT_S: u64 = 25;
    const MAX_ERROR_LINES: usize = 20;

    let tool_name = payload_tool_name(event);
    if !tool_name_is_rust_file_write_tool(&tool_name) {
        return None;
    }
    let path = payload_tool_path(event)?;
    if !core_state::utils::path_guard::path_is_within_repo_root(repo_root, &path) {
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
    if !core_state::utils::path_guard::path_is_within_repo_root(repo_root, &cargo_dir) {
        return None;
    }

    let cargo_start = std::time::Instant::now();
    let (rc, output) =
        cargo_check_with_timeout(&cargo_dir, std::time::Duration::from_secs(TIMEOUT_S));
    hooks::add_cargo_check_ms(cargo_start.elapsed().as_millis() as u64);

    // Continuity: append cargo check outcome to artifacts/current/EVIDENCE_INDEX.json (no-op if continuity not seeded).
    let cmd_preview = format!(
        "(cd {} && cargo check --message-format=short)",
        cargo_dir.display()
    );
    let _ = hooks::framework_hook_evidence_append(json!({
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
    let frame = core_state::task_state::resolve_continuity_frame(repo_root);
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
    let prompt = crate::hosts::hook_dispatch::extract_prompt_text(event);
    let text = agent_response_text(event);
    let signal = hook_event_signal_text(event, &prompt, &text);
    let disk_goal = frame.hydration_goal.is_some();
    let mut dirty = false;
    if saw_reject_reason(&signal, &prompt) {
        state.core.reject_reason_seen = true;
        if crate::hosts::hook_dispatch::shared_tracks_goal(state.goal_required, state.core.goal_drive_entry_active) {
            state.pre_goal_review_satisfied = true;
        }
        clear_review_gate_escalation_counters(&mut state);
        dirty = true;
    }
    if track_goal && !disk_goal {
        if has_structured_goal_contract(&signal) {
            state.core.goal_contract_seen = true;
            dirty = true;
        }
        if has_goal_progress_signal(&signal) {
            state.core.goal_progress_seen = true;
            dirty = true;
        }
        if has_goal_verify_or_block_signal(&signal) {
            state.core.goal_verify_or_block_seen = true;
            dirty = true;
        }
    }
    if track_goal {
        hydrate_goal_gate_from_disk(repo_root, &mut state, false, &frame, false);
        dirty = true;
    }
    let tail = core_policy::hook_common::hook_assistant_tail_window(
        &text,
        core_policy::hook_common::HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
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

