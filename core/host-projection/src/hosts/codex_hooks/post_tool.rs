//! E7: Codex PostToolUse handler — subagent evidence, review gate arming, shell verification.

use router_rs::framework_runtime::try_append_post_tool_shell_evidence;
use serde_json::Value;
use std::path::Path;

use super::lifecycle::{
    clear_codex_review_gate_hook_state, codex_deep_independent_reviewer_evidence,
    codex_fresh_lifecycle_context_from_event, codex_hook_state_persist_block_payload,
    codex_prompt_text, codex_recognized_subagent_kind, codex_review_gate_suppressed,
    codex_subagent_lane_bits_from_kind, codex_tool_input, codex_tool_name, saw_subagent_codex,
};
use super::state::with_codex_state_lock;

/// Codex PostToolUse: `None` → allow; `Some` → block on hook-state persist failure.
pub fn evaluate_codex_post_tool_use(repo_root: &Path, event: &Value) -> Option<Value> {
    let prompt_for_profile = codex_prompt_text(event);
    if codex_review_gate_suppressed(repo_root, &prompt_for_profile) {
        clear_codex_review_gate_hook_state(repo_root, event);
        return None;
    }
    if let Err(err) =
        try_append_post_tool_shell_evidence(repo_root, event, "codex_post_tool_verification")
    {
        eprintln!("[router-rs] post-tool evidence append failed (non-fatal): {err}");
    }
    let tool_name = codex_tool_name(event);
    let tool_input = codex_tool_input(event);
    if let Err(e) = router_rs::session_call_tracker::record_tool_call(repo_root, &tool_name) {
        eprintln!("[router-rs] session tracker record_tool_call failed (non-fatal): {e}");
    }
    if !saw_subagent_codex(&tool_name, &tool_input) {
        return None;
    }
    match with_codex_state_lock(repo_root, event, |loaded| {
        let mut state = match loaded {
            Some(value) => value,
            None => codex_fresh_lifecycle_context_from_event(event),
        };
        state.generic_subagent_seen = true;
        let recognized = codex_recognized_subagent_kind(&tool_input);
        let tool_label = recognized
            .as_ref()
            .map(|kind| format!("{tool_name}#{kind}"))
            .unwrap_or_else(|| format!("{tool_name}#untyped"));
        state.review_subagent_tool = Some(tool_label);
        let (review_lane, parallel_lane) =
            codex_subagent_lane_bits_from_kind(recognized.as_deref());
        if review_lane {
            state.review_lane_seen = true;
        }
        if parallel_lane {
            state.parallel_lane_seen = true;
        }
        state.review_subagent_seen = true;
        if codex_deep_independent_reviewer_evidence(recognized.as_deref(), &tool_input, event) {
            state.independent_review_subagent_seen = true;
            state.subagent_start_count = state.subagent_start_count.saturating_add(1);
            state.phase = state.phase.max(2);
            let post_facts = router_rs::review_gate_engine::ReviewGateFacts::from_prompt(&prompt_for_profile);
            let should_arm_review = state.review_required || post_facts.review_required;
            if should_arm_review
                && !router_rs::hook_common::my_light_profile_active(Some(repo_root), &prompt_for_profile)
            {
                state.review_required = true;
            }
        }
        Ok((Some(state), ()))
    }) {
        Ok(()) => None,
        Err(err) => {
            eprintln!("[router-rs] codex subagent evidence persist failed (fail-closed): {err}");
            Some(codex_hook_state_persist_block_payload())
        }
    }
}
