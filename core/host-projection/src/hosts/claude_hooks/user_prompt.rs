//! E7 step 2: UserPromptSubmit handler — review gate sync, lifecycle nudges, paper hooks.

use router_rs::framework_error::{FrameworkError, FrameworkResult};
use router_rs::hook_common::{
    has_override, is_narrow_review_prompt, is_review_prompt,
};
use serde_json::Value;
use std::path::Path;

use super::{
    active_stdio_agent_hook_host, add_context, agent_review_gate_disabled,
};
use super::session::{
    load_review_gate_disk, review_state_path, with_claude_review_state_lock,
    write_review_state_unlocked, AgentDiskState, ReviewGateState,
};

pub fn claude_user_prompt_text(payload: &Value) -> String {
    payload
        .get("prompt")
        .or_else(|| payload.get("user_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub fn claude_stop_signal_text(payload: &Value) -> String {
    claude_user_prompt_text(payload)
}

fn claude_review_gate_incomplete_stop_reason() -> String {
    if router_rs::router_env_flags::router_rs_claude_review_fork_context_missing_infer_false_enabled()
    {
        "router-rs CLAUDE_REVIEW_GATE incomplete fix=Task subagent_type=general-purpose prompt=\"深度 review（只读 findings）\" | omit fork_context when ROUTER_RS_CLAUDE_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE=1"
            .to_string()
    } else {
        active_stdio_agent_hook_host()
            .review_gate_incomplete_stop_reason()
            .to_string()
    }
}

fn should_sync_review_gate_on_user_prompt(repo_root: &Path, prompt: &str) -> bool {
    router_rs::hook_common::my_light_profile_active(Some(repo_root), prompt)
        || router_rs::hook_common::is_framework_goal_entry_prompt(prompt)
        || router_rs::hook_common::is_my_pre_execution_entry_prompt(prompt)
        || is_narrow_review_prompt(prompt)
        || is_review_prompt(prompt)
        || has_override(prompt)
}

pub fn apply_claude_review_gate_user_prompt(
    repo_root: &Path,
    payload: &Value,
    prompt: &str,
) -> FrameworkResult<ReviewGateState> {
    let path = review_state_path(repo_root, payload);
    let my_light = router_rs::hook_common::my_light_profile_active(Some(repo_root), prompt);
    let narrow = is_narrow_review_prompt(prompt);
    let goal_drive = router_rs::hook_common::is_framework_goal_entry_prompt(prompt);
    let review_arms = is_review_prompt(prompt) && !goal_drive;
    let override_now = has_override(prompt);
    with_claude_review_state_lock(&path, || {
        let mut state = match load_review_gate_disk(repo_root, payload) {
            AgentDiskState::Unreadable => {
                eprintln!(
                    "[router-rs] {} review_gate state unreadable: {}",
                    active_stdio_agent_hook_host().log_label(),
                    path.display()
                );
                return Err(FrameworkError::other("review_gate_unreadable"));
            }
            AgentDiskState::Absent => ReviewGateState::default(),
            AgentDiskState::Ok(s) => s,
        };
        if my_light || goal_drive || narrow {
            state.review_required = false;
            state.independent_reviewer_seen = false;
        } else {
            if review_arms && !override_now {
                state.independent_reviewer_seen = false;
            }
            state.review_required = state.review_required || review_arms;
        }
        state.review_override = state.review_override || override_now;
        write_review_state_unlocked(&path, &state)?;
        Ok(state)
    })
}

/// Claude UserPromptSubmit: `None` → silent; `Some` → additionalContext injection.
pub fn evaluate_claude_user_prompt_submit(
    repo_root: &Path,
    payload: &Value,
) -> Option<Value> {
    let prompt = claude_user_prompt_text(payload);
    let review_sync = if !agent_review_gate_disabled()
        && should_sync_review_gate_on_user_prompt(repo_root, &prompt)
    {
        Some(apply_claude_review_gate_user_prompt(repo_root, payload, &prompt))
    } else {
        None
    };
    if let Some(Err(_)) = review_sync {
        let path = review_state_path(repo_root, payload);
        return add_context(
            "UserPromptSubmit",
            &format!(
                "{} (path {}). Repair JSON or permissions before continuing.",
                active_stdio_agent_hook_host().hook_state_unreadable(),
                path.display()
            ),
        );
    }
    if router_rs::hook_common::is_my_pre_execution_entry_prompt(&prompt) {
        return add_context(
            "UserPromptSubmit",
            router_rs::hook_common::MY_PRE_EXECUTION_HOOK_NUDGE,
        );
    }
    if router_rs::hook_common::is_framework_goal_entry_prompt(&prompt) {
        return add_context(
            "UserPromptSubmit",
            router_rs::hook_common::my_goal_drive_hook_nudge_for_prompt(&prompt),
        );
    }
    let mut contexts: Vec<String> = Vec::new();
    if let Some(Ok(state)) = review_sync {
        if state.review_required
            && !state.review_override
            && router_rs::hook_common::should_inject_spawn_first_review_nudge(Some(repo_root), &prompt)
        {
            contexts.push(router_rs::runtime_registry::review_spawn_first_nudge_line(
                Some(repo_root),
                "claude-code",
            ));
        }
    }
    // I7: inject heterogeneous adversarial review hint for broad/deep review prompts.
    if router_rs::review::heterogeneous::should_plan_heterogeneous_adversarial_lane(&prompt, true) {
        if let Some(hint) = router_rs::review::heterogeneous::heterogeneous_review_hint_for_lane() {
            contexts.push(hint);
        }
    }
    router_rs::paper_adversarial_hook::maybe_append_paper_adversarial_context(
        repo_root,
        &prompt,
        &mut contexts,
        router_rs::paper_prose_hook::PaperProseHookHost::Claude,
    );
    router_rs::paper_prose_hook::maybe_append_paper_prose_context(
        repo_root,
        &prompt,
        &mut contexts,
        router_rs::paper_prose_hook::PaperProseHookHost::Claude,
    );
    if contexts.is_empty() {
        return None;
    }
    add_context("UserPromptSubmit", &contexts.join("\n"))
}

pub fn claude_review_gate_incomplete_stop_reason_for_stop() -> String {
    claude_review_gate_incomplete_stop_reason()
}
