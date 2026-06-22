// Stop / preCompact hook handlers (P4 handlers split; closeout in stop_closeout.rs).
// ADR §2.1: delegates core pipeline to unified stop_dispatch; host-specific
// state management (file locking, goal signal scanning) handled here.
fn handle_stop(repo_root: &Path, event: &Value) -> Value {
    let frame = core_state::task_state::resolve_continuity_frame(repo_root);
    let stop_prompt_for_profile = crate::hosts::hook_dispatch::extract_prompt_text(event);

    // ── 1. Suppression check (shared) ──
    if crate::hosts::hook_dispatch::is_review_gate_suppressed("cursor", Some(repo_root), &stop_prompt_for_profile) {
        let response_text = agent_response_text(event);
        let closeout_msg = stop_hard_closeout_followup_for_assistant_response(repo_root, &response_text);
        let mut out = json!({});
        if let Some(msg) = closeout_msg {
            out["followup_message"] = Value::String(msg);
        }
        finalize_stop_hook_outputs(repo_root, &mut out, &frame);
        return out;
    }

    // ── 2. File lock acquisition (Cursor-specific) ──
    let mut lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        let mut out = if stop_lock_failure_is_fail_closed(repo_root, event) {
            json!({ "followup_message": review_gate_stop_lock_unavailable_line() })
        } else {
            json!({ "followup_message": lock_failure_followup_for_stop(repo_root, event) })
        };
        finalize_stop_hook_outputs(repo_root, &mut out, &frame);
        return out;
    }

    // ── 3. Load state (Cursor-specific) ──
    let loaded = load_state(repo_root, event);
    let text = crate::hosts::hook_dispatch::extract_prompt_text(event);
    let response_full = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_full);
    let response_for_lint = core_policy::hook_common::hook_assistant_tail_window(
        &response_full, core_policy::hook_common::HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
    );

    // ── 4. Hard closeout check (shared) ──
    if let Some(msg) = stop_hard_closeout_followup_for_assistant_response(repo_root, &response_full) {
        let mut out = json!({ "followup_message": msg });
        release_lock_then_finalize_stop(repo_root, &mut out, &frame, &mut lock);
        return out;
    }

    // ── 5. Core decision pipeline (shared via hook_dispatch) ──
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
            crate::hosts::hook_dispatch::apply_override_and_reject(&mut state.core, &text, &signal_text);

            // Cursor-specific: reject reason arms pre_goal_review_satisfied
            if state.core.reject_reason_seen
                && crate::hosts::hook_dispatch::shared_tracks_goal(state.goal_required, state.core.goal_drive_entry_active) {
                state.pre_goal_review_satisfied = true;
                clear_review_gate_escalation_counters(&mut state);
            }

            // Goal gate signal scanning (Cursor-specific)
            let disk_goal = frame.hydration_goal.is_some();
            if !disk_goal {
                if has_structured_goal_contract(&signal_text) { state.core.goal_contract_seen = true; }
                if has_goal_progress_signal(&signal_text) { state.core.goal_progress_seen = true; }
                if has_goal_verify_or_block_signal(&signal_text) { state.core.goal_verify_or_block_seen = true; }
            }
            let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
            hydrate_goal_gate_from_disk(repo_root, &mut state, true, &frame, goal_drive_entrypoint);

            if maybe_bump_review_phase_for_main_thread_compact_findings(&mut state, &response_for_lint) {
                let _ = save_state(repo_root, event, &mut state);
            }

            // ── Shared stop decision ──
            match crate::hosts::hook_dispatch::evaluate_stop_decision(
                &mut state.core, &text, &response_full,
                &format!("{text}\n{response_full}"),
                &response_full, repo_root, "cursor",
            ) {
                crate::hosts::hook_dispatch::StopDecision::Closeout { message }
                | crate::hosts::hook_dispatch::StopDecision::ReviewGateNudge { message } => {
                    state.core.followup_count += 1;
                    state.core.review_followup_count += 1;
                    let _ = save_state(repo_root, event, &mut state);
                    (json!({ "followup_message": message }), false)
                }
                crate::hosts::hook_dispatch::StopDecision::GoalFollowup { message } => {
                    state.core.followup_count += 1;
                    state.core.goal_followup_count += 1;
                    let _ = save_state(repo_root, event, &mut state);
                    (json!({ "followup_message": message }), false)
                }
                crate::hosts::hook_dispatch::StopDecision::Clean => {
                    if state.core.review_required
                        || crate::hosts::hook_dispatch::shared_tracks_goal(state.goal_required, state.core.goal_drive_entry_active)
                        || state.core.reject_reason_seen
                    {
                        let _ = save_state(repo_root, event, &mut state);
                    } else {
                        let mut reset = empty_state();
                        let _ = save_state(repo_root, event, &mut reset);
                    }
                    (json!({}), false)
                }
            }
        }
    };

    // ── 6. Review output lint (shared) ──
    let hard_stop_followup = output.get("followup_message").and_then(Value::as_str).is_some_and(|s| !s.trim().is_empty());
    if !hard_stop_followup && !skip_review_output_lint && !response_for_lint.trim().is_empty() && response_for_lint.contains("[P") {
        let lint_findings = lint_review_output(&response_for_lint);
        if !lint_findings.is_empty() {
            let warning_count = lint_findings.iter().filter(|f| f.severity == LintSeverity::Warning).count();
            if warning_count > 0 {
                let msg = format!("review-output-lint: {} compact envelope warning(s)", warning_count);
                core_state::state_manager::merge_hook_nudge_paragraph(&mut output, &msg, "review-output-lint", false);
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
                "router-rs 门控快照：phase={} review={} override={} reject={} pre_goal_ok={} subagentStart_n={} subagent_stop={}",
                state.phase,
                state.core.review_required,
                (state.core.review_override || state.core.delegation_override),
                state.core.reject_reason_seen,
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

