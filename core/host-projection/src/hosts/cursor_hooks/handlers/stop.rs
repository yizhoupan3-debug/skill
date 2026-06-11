// Stop / preCompact hook handlers (P4 handlers split; closeout in stop_closeout.rs).
fn handle_stop(repo_root: &Path, event: &Value) -> Value {
    let frame = core_state::task_state::resolve_cursor_continuity_frame(repo_root);
    let stop_prompt_for_profile = prompt_text(event);
    if cursor_review_gate_suppressed(repo_root, &stop_prompt_for_profile) {
        let response_text = agent_response_text(event);
        let closeout_msg =
            stop_hard_closeout_followup_for_assistant_response(repo_root, &response_text);
        let mut out = json!({});
        if let Some(msg) = closeout_msg {
            out["followup_message"] = Value::String(msg);
        }
        let _skip_review_output_lint = out.get("followup_message").is_some();
        finalize_stop_hook_outputs(repo_root, &mut out, &frame);
        return out;
    }
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let _skip_review_output_lint = true;
        let mut out = if stop_lock_failure_is_fail_closed(repo_root, event) {
            json!({
                "followup_message": review_gate_stop_lock_unavailable_line()
            })
        } else {
            json!({
                "followup_message": lock_failure_followup_for_stop(repo_root, event)
            })
        };
        finalize_stop_hook_outputs(repo_root, &mut out, &frame);
        return out;
    }
    let loaded = load_state(repo_root, event);
    let text = prompt_text(event);
    let response_full = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_full);
    let response_for_lint = core_policy::hook_common::hook_assistant_tail_window(
        &response_full,
        core_policy::hook_common::CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
    );

    // Completion claim guard must not depend on hook-state existence: a strict closeout violation
    // is a hard-stop even when the review gate state was never initialized for this session.
    if let Some(msg) = stop_hard_closeout_followup_for_assistant_response(repo_root, &response_full)
    {
        let mut out = json!({ "followup_message": msg });
        release_lock_then_finalize_stop(repo_root, &mut out, &frame, &mut lock);
        return out;
    }
    let (mut output, skip_review_output_lint) = match loaded {
        Ok(None) => (json!({}), false),
        Err(io_error) => {
            let msg = format!(
                "router-rs REVIEW_GATE incomplete phase=0 {} hook_state_read_failed={io_error} {}",
                REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
            );
            (json!({ "followup_message": msg }), true)
        }
        Ok(Some(mut state)) => {
            let _stale_reset = apply_subagent_stale_hygiene(&mut state);
            state.delegation_required = false;
            // Override 句式仅承认用户本轮 prompt（与 beforeSubmit 一致）；勿用含助手输出的
            // `signal_text`，避免助手复述「不要用子代理」类话术误清空 REVIEW_GATE。
            if has_override(&text) {
                state.review_override = true;
                state.delegation_override = true;
            }
            let disk_goal = frame.hydration_goal.is_some();
            if !disk_goal {
                if has_structured_goal_contract(&signal_text) {
                    state.goal_contract_seen = true;
                }
                if has_goal_progress_signal(&signal_text) {
                    state.goal_progress_seen = true;
                }
                if has_goal_verify_or_block_signal(&signal_text) {
                    state.goal_verify_or_block_seen = true;
                }
            }
            if saw_reject_reason(&signal_text, &text) {
                state.reject_reason_seen = true;
                if tracks_goal_or_drive_entry(&state) {
                    state.pre_goal_review_satisfied = true;
                }
                clear_review_gate_escalation_counters(&mut state);
            }
            let goal_drive_entrypoint =
                is_framework_goal_drive_entry_prompt(&text, &signal_text);
            hydrate_goal_gate_from_disk(
                repo_root,
                &mut state,
                true,
                &frame,
                goal_drive_entrypoint,
            );
            if maybe_bump_review_phase_for_main_thread_compact_findings(
                &mut state,
                &response_for_lint,
            ) {
                let _ = save_state(repo_root, event, &mut state);
            }
            let gate_suppresses_review_lint = stop_review_output_lint_suppressed(&state);
            if review_stop_followup_needed(&state) {
                state.followup_count += 1;
                state.review_followup_count += 1;
                let cap =
                    hooks::router_rs_cursor_review_gate_stop_max_nudges_cap();
                let use_full = match cap {
                    None => true,
                    Some(n) => state.review_followup_count <= n,
                };
                // soft_nag 超 cap：仍注入 REVIEW 提示，但不阻断 My/RFV 续跑（ADR / P1-4）。
                let skip_review_output_lint = if use_full {
                    gate_suppresses_review_lint
                } else {
                    tracks_goal_or_drive_entry(&state) && !goal_is_satisfied(&state)
                };
                let out = if use_full {
                    json!({ "followup_message": review_stop_followup_line(&state) })
                } else {
                    let full_cap = cap.expect("soft branch implies cap=Some");
                    let soft = review_stop_followup_soft_line(&state, full_cap);
                    let need_line = review_stop_followup_line(&state);
                    let followup_message = format!("{soft}\n{need_line}");
                    let mut soft_out = json!({ "followup_message": followup_message });
                    core_state::state_manager::merge_hook_nudge_paragraph(
                        &mut soft_out,
                        &review_stop_followup_detail_paragraph(&state),
                        REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX,
                        false,
                    );
                    soft_out
                };
                let _ = save_state(repo_root, event, &mut state);
                (out, skip_review_output_lint)
            } else if tracks_goal_or_drive_entry(&state) && !goal_is_satisfied(&state) {
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                // Stop 只给短码，避免把整段 Autopilot 契约说明塞进会话收尾（细则见 beforeSubmit / AGENTS）。
                let message = goal_stop_followup_line(&state);
                (
                    json!({ "followup_message": message }),
                    gate_suppresses_review_lint,
                )
            } else {
                // Do not clear gate state on Stop for sessions that still track goal/review:
                // the next Stop should still enforce the same requirements until satisfied/overridden.
                if state.review_required
                    || tracks_goal_or_drive_entry(&state)
                    || state.reject_reason_seen
                {
                    let _ = save_state(repo_root, event, &mut state);
                } else {
                    let mut reset = empty_state();
                    let _ = save_state(repo_root, event, &mut reset);
                }
                (json!({}), false)
            }
        }
    };
    // Advisory: lint review output format (compact envelope checks).
    // Skip when Stop already carries a hard followup or review-output-lint is suppressed.
    let hard_stop_followup = output
        .get("followup_message")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    if !hard_stop_followup
        && !skip_review_output_lint
        && !response_for_lint.trim().is_empty()
        && response_for_lint.contains("[P")
    {
        let lint_findings = lint_review_output(&response_for_lint);
        if !lint_findings.is_empty() {
            let warning_count = lint_findings
                .iter()
                .filter(|f| f.severity == LintSeverity::Warning)
                .count();
            if warning_count > 0 {
                let msg = format!(
                    "review-output-lint: {} compact envelope warning(s) — check `skills/code-review-deep/SKILL.md` §Compact envelope",
                    warning_count
                );
                core_state::state_manager::merge_hook_nudge_paragraph(
                    &mut output,
                    &msg,
                    "review-output-lint",
                    false,
                );
            }
        }
    }
    release_lock_then_finalize_stop(repo_root, &mut output, &frame, &mut lock);
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
            let summary = format!(
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
    let max_bytes = hooks::router_rs_cursor_sessionstart_context_max_bytes();
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

