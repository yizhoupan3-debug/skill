//! E7: Codex Stop handler — review gate, closeout enforcement, hook-state cleanup.

use router_rs::hook_common::has_override;
use router_rs::review_gate_engine::{
    codex_review_gate_satisfied, maybe_bump_codex_review_phase_for_compact_findings,
    ReviewGateFacts,
};
use serde_json::{json, Value};
use std::path::Path;

use super::lifecycle::{
    codex_agent_response_text, codex_closeout_completion_text, codex_hook_state_persist_block_payload,
    codex_prompt_text, codex_reset_hook_state, codex_review_gate_suppressed,
    codex_review_stop_followup_line, codex_stop_hook_active_bypass_enabled,
    codex_stop_hook_active_replay, codex_stop_signal_text,
};
use super::lifecycle_host;
use super::state::{codex_load_state, with_codex_state_lock};

/// Codex Stop: `None` → allow; `Some` → block with followup / repair message.
pub fn evaluate_codex_stop(repo_root: &Path, event: &Value) -> Option<Value> {
    if codex_stop_hook_active_replay(event) && codex_stop_hook_active_bypass_enabled() {
        return None;
    }

    let stop_signal = codex_stop_signal_text(event);
    let prompt_text = codex_prompt_text(event);
    let response_full = codex_agent_response_text(event);

    if codex_review_gate_suppressed(repo_root, &stop_signal) {
        if let Some(msg) = router_rs::framework_runtime::closeout_stop_followup_for_completion_text(
            repo_root,
            &codex_closeout_completion_text(event),
        ) {
            return Some(json!({
                "decision": "block",
                "followup_message": msg
            }));
        }
        codex_reset_hook_state(repo_root, event);
        return None;
    }

    if let Some(msg) = router_rs::framework_runtime::closeout_stop_followup_for_completion_text(
        repo_root,
        &codex_closeout_completion_text(event),
    ) {
        return Some(json!({
            "decision": "block",
            "followup_message": msg
        }));
    }

    match codex_load_state(repo_root, event) {
        Err(reason) => {
            eprintln!("[router-rs] codex hook-state unreadable: {reason}");
            return Some(json!({
                "decision": "block",
                "followup_message": format!(
                    "router-rs {} need=repair_hook_state_json fix=rm -f {}/hook-state/*.json",
                    lifecycle_host().hook_state_unreadable_tag(),
                    lifecycle_host().state_dir_leaf
                )
            }));
        }
        Ok(Some(mut state)) => {
            let persist = with_codex_state_lock(repo_root, event, |_loaded| {
                if has_override(&prompt_text) {
                    state.review_override = true;
                }
                if router_rs::hook_common::saw_reject_reason(&stop_signal, &prompt_text) {
                    state.reject_reason_seen = true;
                }
                let assistant_tail = router_rs::hook_common::hook_assistant_tail_window(
                    &response_full,
                    router_rs::hook_common::CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
                );
                if let Some(phase) = maybe_bump_codex_review_phase_for_compact_findings(
                    state.review_required,
                    state.review_override,
                    state.phase,
                    state.subagent_start_count,
                    state.independent_review_subagent_seen,
                    &assistant_tail,
                ) {
                    state.phase = phase;
                }
                let phase = state.phase;
                let blocks = !codex_review_gate_satisfied(
                    state.review_required,
                    state.review_override,
                    state.reject_reason_seen,
                    state.independent_review_subagent_seen,
                    state.phase,
                );
                Ok((Some(state), (blocks, phase)))
            });
            match persist {
                Err(_) => return Some(codex_hook_state_persist_block_payload()),
                Ok((blocks, phase)) => {
                    if blocks {
                        return Some(json!({
                            "decision": "block",
                            "followup_message": codex_review_stop_followup_line(phase)
                        }));
                    }
                }
            }
        }
        Ok(None) => {
            let stop_facts = ReviewGateFacts::from_prompt(&prompt_text);
            let reject = router_rs::hook_common::saw_reject_reason(&stop_signal, &prompt_text);
            let assistant_tail = router_rs::hook_common::hook_assistant_tail_window(
                &response_full,
                router_rs::hook_common::CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
            );
            let phase = if maybe_bump_codex_review_phase_for_compact_findings(
                stop_facts.review_required,
                stop_facts.review_override,
                0,
                0,
                false,
                &assistant_tail,
            )
            .is_some()
            {
                3
            } else {
                0
            };
            if !codex_review_gate_satisfied(
                stop_facts.review_required,
                stop_facts.review_override,
                reject,
                false,
                phase,
            ) {
                return Some(json!({
                    "decision": "block",
                    "followup_message": codex_review_stop_followup_line(phase)
                }));
            }
        }
    }

    // I6: 关键任务 RFV 默认启用时注入自动续跑提示
    if let Some(nudge) = router_rs::framework_runtime::rfv_nudge_for_key_task(repo_root) {
        return Some(json!({
            "decision": "block",
            "followup_message": nudge
        }));
    }

    codex_reset_hook_state(repo_root, event);
    None
}

/// Test / legacy alias for Stop evaluation.
#[cfg_attr(not(test), allow(dead_code))]
pub fn handle_codex_stop(repo_root: &Path, event: &Value) -> Option<Value> {
    evaluate_codex_stop(repo_root, event)
}
