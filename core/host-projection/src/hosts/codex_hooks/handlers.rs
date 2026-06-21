//! Event handlers for the Codex lifecycle hook system.
//!
//! Contains handler functions for each lifecycle event (UserPromptSubmit, PostToolUse,
//! Stop, SubagentStart, SubagentStop, SessionStart) and the main dispatch logic
//! (`run_codex_lifecycle_context_hook_for_state_dir`).

use super::state::load_state;
use super::{
    CODEX_ADDITIONAL_CONTEXT_MAX_BYTES, CODEX_REVIEW_SUBAGENT_TYPES, CodexLifecycleContextState,
    CodexLifecycleHostKind, lifecycle_host,
};
use crate::hooks;
use crate::hooks::{
    router_rs_operator_inject_globally_enabled, try_append_post_tool_shell_evidence,
};
use crate::hosts::hook_dispatch;
use crate::hosts::hook_dispatch::{extract_prompt_text, extract_tool_name, extract_tool_input};
use core_policy::HookReviewDiskCore;
use core_policy::hook_common::{
    has_override, is_reviewer_lane_normalized, normalize_subagent_type, normalize_tool_name,
};
use core_policy::review_gate_engine::{
    ReviewGateFacts, fork_context_from_values, maybe_bump_codex_review_phase_for_compact_findings,
    review_independent_reviewer_evidence,
};
use serde_json::{Value, json};
use std::path::Path;

use super::drift::projection_drift_warning;
use super::state::with_codex_state_lock;

// ---------------------------------------------------------------------------
// Helper functions used by handlers
// ---------------------------------------------------------------------------

// Codex uses shared hook_dispatch extractors (no local duplicates).

#[cfg(test)]
pub(super) fn first_nonempty_prompt_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

pub(crate) fn saw_subagent_codex(tool_name: &str, _tool_input: &Value) -> bool {
    let name = normalize_tool_name(Some(tool_name));
    core_policy::subagent::is_subagent_tool(&name)
}

fn recognized_subagent_kind(tool_input: &Value) -> Option<String> {
    let typed_fields = [
        tool_input.get("subagent_type").and_then(Value::as_str),
        tool_input.get("agent_type").and_then(Value::as_str),
        tool_input.get("agentType").and_then(Value::as_str),
        tool_input.get("type").and_then(Value::as_str),
    ];
    typed_fields
        .into_iter()
        .map(|field| normalize_subagent_type(field))
        .find(|normalized| CODEX_REVIEW_SUBAGENT_TYPES.contains(&normalized.as_str()))
}

fn subagent_lane_bits_from_kind(kind: Option<&str>) -> (bool, bool) {
    let Some(k) = kind else {
        return (false, false);
    };
    let review_lane = matches!(
        k,
        "explore"
            | "explorer"
            | "general-purpose"
            | "generalpurpose"
            | "ci-investigator"
            | "ciinvestigator"
            | "cursor-guide"
            | "cursorguide"
            | "best-of-n-runner"
            | "bestofnrunner"
            | "default"
    );
    let parallel_lane = matches!(k, "worker" | "shell" | "browser-use" | "browseruse");
    (review_lane, parallel_lane)
}

fn tool_fork_context(tool_input: &Value, event: &Value) -> Option<bool> {
    fork_context_from_values(tool_input, Some(event))
}

/// 与 Cursor `REVIEW_GATE` 深度 lane 对齐：`general-purpose` / `best-of-n-runner`（已 normalize）；缺字段推断见 [`review_independent_fork`].
fn deep_independent_reviewer_evidence(
    recognized_kind: Option<&str>,
    tool_input: &Value,
    event: &Value,
) -> bool {
    let reviewer_lane = recognized_kind.is_some_and(is_reviewer_lane_normalized);
    review_independent_reviewer_evidence(tool_fork_context(tool_input, event), reviewer_lane)
}

fn hook_state_persist_block_payload() -> Value {
    let host = lifecycle_host();
    json!({
        "decision": "block",
        "reason": format!(
            "{} hook state could not be persisted under {}/hook-state.",
            host.lifecycle_label(),
            host.state_dir_leaf
        ),
    })
}

fn stop_hook_active_replay(event: &Value) -> bool {
    event
        .get("stop_hook_active")
        .or(event.get("stopHookActive"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Codex-internal Stop replays (`stop_hook_active`): skip gate enforcement only when explicitly opted in.
fn stop_hook_active_bypass_enabled() -> bool {
    crate::hooks::router_rs_env_enabled_default_false(
        lifecycle_host().stop_hook_active_bypass_env(),
    )
}

fn clear_codex_review_gate_hook_state(repo_root: &Path, event: &Value) {
    super::reset_hook_state(repo_root, event);
}

fn stop_signal_text(event: &Value) -> String {
    hook_dispatch::stop_signal_text_from_payload(event)
}

fn closeout_completion_text(event: &Value) -> String {
    stop_signal_text(event)
}

fn review_stop_advisory_payload(fields: &core_policy::HookReviewGateFields) -> Option<Value> {
    core_policy::hook_review_stop_advisory_needed(fields, lifecycle_host().review_gate_tag())
        .map(|followup_message| json!({ "followup_message": followup_message }))
}

// ---------------------------------------------------------------------------
// Context compaction
// ---------------------------------------------------------------------------

/// Upper bound for merged `additionalContext` UTF-8 **bytes** (not Unicode scalar count).
///
/// Reads `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` first when set; otherwise
/// `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` (legacy name; still interpreted as bytes).
/// Value is clamped to [256, 8192].
pub(crate) fn additional_context_max_bytes() -> usize {
    const MIN: usize = 256;
    const MAX: usize = 8192;
    std::env::var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .or_else(|| {
            std::env::var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX")
                .ok()
                .and_then(|raw| raw.trim().parse::<usize>().ok())
        })
        .map(|n| n.clamp(MIN, MAX))
        .unwrap_or(CODEX_ADDITIONAL_CONTEXT_MAX_BYTES)
}

pub(crate) fn compact_contexts_shared(parts: Vec<String>) -> Option<String> {
    // Trim + filter empty before dedup; then delegate to shared compaction.
    let trimmed: Vec<String> = parts
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    let max_bytes = additional_context_max_bytes();
    crate::hosts::hook_dispatch::compact_contexts_with_suffix(
        trimmed,
        max_bytes,
        "...(截断)",
    )
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

pub(super) fn handle_codex_userpromptsubmit(repo_root: &Path, event: &Value) -> Option<Value> {
    let prompt = extract_prompt_text(event);
    if hook_dispatch::is_review_gate_suppressed("codex", Some(repo_root), &prompt) {
        clear_codex_review_gate_hook_state(repo_root, event);
        return None;
    }
    let my_light = core_policy::hook_common::is_interactive_profile(Some(repo_root), &prompt);
    let mut facts = ReviewGateFacts::from_prompt(&prompt);
    if my_light {
        facts.review_required = false;
    }
    let state = CodexLifecycleContextState {
        seq: 0,
        review_gate: HookReviewDiskCore {
            review_required: facts.review_required,
            review_override: facts.review_override,
            ..HookReviewDiskCore::default()
        },
        ..CodexLifecycleContextState::default()
    };

    let narrow = core_policy::hook_common::is_narrow_review_prompt(&prompt);
    let review_arms = facts.review_required;
    let override_now = facts.review_override;
    let write_result = with_codex_state_lock(repo_root, event, |loaded| {
        let mut next = state.clone();
        if let Some(prev) = loaded {
            next.seq = prev.seq.saturating_add(1);
            if my_light || narrow {
                next.review_gate.review_required = false;
                next.review_gate.independent_reviewer_seen = false;
                next.phase = 0;
                next.subagent_start_count = 0;
            } else {
                if review_arms && !override_now {
                    next.review_gate.independent_reviewer_seen = false;
                    next.phase = 0;
                    next.subagent_start_count = 0;
                    next.review_subagent_seen = false;
                    next.generic_subagent_seen = false;
                } else {
                    next.review_gate.independent_reviewer_seen =
                        prev.review_gate.independent_reviewer_seen;
                    next.phase = prev.phase;
                    next.subagent_start_count = prev.subagent_start_count;
                }
                next.review_gate.review_required = prev.review_gate.review_required || review_arms;
            }
            next.review_gate.review_override = prev.review_gate.review_override || override_now;
            next.review_gate.reject_reason_seen = prev.review_gate.reject_reason_seen;
        } else {
            next.seq = 1;
        }
        Ok((Some(next), ()))
    });
    if write_result.is_err() {
        return Some(hook_state_persist_block_payload());
    }

    if !router_rs_operator_inject_globally_enabled() {
        return None;
    }

    let mut contexts: Vec<String> = Vec::new();
    if let Some(warning) = projection_drift_warning(repo_root) {
        contexts.push(warning);
    }
    // Shared context injection (4-host unified)
    let paper_host = lifecycle_host().paper_prose_hook_host();
    contexts.extend(hook_dispatch::build_user_prompt_context_injection(
        repo_root,
        &prompt,
        lifecycle_host().spawn_first_host_id(),
        paper_host,
        facts.review_required,
        facts.review_override,
    ));
    let additional_context = compact_contexts_shared(contexts);
    if additional_context.is_none() {
        None
    } else {
        Some(json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": additional_context,
            }
        }))
    }
}

pub(super) fn handle_codex_posttooluse(repo_root: &Path, event: &Value) -> Option<Value> {
    let tool_name = extract_tool_name(event);
    let tool_origin = core_policy::hook_common::classify_tool_origin(&tool_name);
    let _ = &tool_origin;

    // Shared tool call telemetry (4-host unified)
    hook_dispatch::record_tool_call_emission(
        repo_root,
        &tool_name,
        hooks::extract_post_tool_duration_ms(event).unwrap_or(0),
        hooks::post_tool_call_succeeded(event),
    );

    let prompt_for_profile = extract_prompt_text(event);
    if hook_dispatch::is_review_gate_suppressed("codex", Some(repo_root), &prompt_for_profile) {
        clear_codex_review_gate_hook_state(repo_root, event);
        return None;
    }
    if let Err(err) =
        try_append_post_tool_shell_evidence(repo_root, event, "codex_post_tool_verification")
    {
        eprintln!("[router-rs] post-tool evidence append failed (non-fatal): {err}");
    }
    let tool_input = extract_tool_input(event);
    if !saw_subagent_codex(&tool_name, &tool_input) {
        return None;
    }
    match with_codex_state_lock(repo_root, event, |loaded| {
        let mut state = match loaded {
            Some(value) => value,
            None => {
                let prompt = extract_prompt_text(event);
                let facts = ReviewGateFacts::from_prompt(&prompt);
                CodexLifecycleContextState {
                    seq: 1,
                    review_gate: HookReviewDiskCore {
                        review_required: facts.review_required,
                        review_override: facts.review_override,
                        ..HookReviewDiskCore::default()
                    },
                    ..CodexLifecycleContextState::default()
                }
            }
        };
        state.generic_subagent_seen = true;

        // Codex-specific subagent type recognition (preserves Codex review type set)
        let recognized = recognized_subagent_kind(&tool_input);
        let tool_label = recognized
            .as_ref()
            .map(|kind| format!("{tool_name}#{kind}"))
            .unwrap_or_else(|| format!("{tool_name}#untyped"));
        state.review_subagent_tool = Some(tool_label);
        let (review_lane, parallel_lane) =
            subagent_lane_bits_from_kind(recognized.as_deref());
        if review_lane {
            state.review_lane_seen = true;
        }
        if parallel_lane {
            state.parallel_lane_seen = true;
        }
        state.review_subagent_seen = true;

        // Codex-specific reviewer evidence detection (preserves registry-based lane check)
        if deep_independent_reviewer_evidence(recognized.as_deref(), &tool_input, event) {
            state.review_gate.independent_reviewer_seen = true;
            state.subagent_start_count = state.subagent_start_count.saturating_add(1);
            state.phase = state.phase.max(2);
            let post_facts = ReviewGateFacts::from_prompt(&prompt_for_profile);
            let should_arm_review = state.review_gate.review_required || post_facts.review_required;
            if should_arm_review
                && !core_policy::hook_common::is_interactive_profile(
                    Some(repo_root),
                    &prompt_for_profile,
                )
            {
                state.review_gate.review_required = true;
            }
        }
        Ok((Some(state), ()))
    }) {
        Ok(()) => None,
        Err(err) => {
            eprintln!("[router-rs] codex subagent evidence persist failed (fail-closed): {err}");
            Some(hook_state_persist_block_payload())
        }
    }
}

pub(super) fn handle_codex_stop(repo_root: &Path, event: &Value) -> Option<Value> {
    if stop_hook_active_replay(event) && stop_hook_active_bypass_enabled() {
        return None;
    }

    let stop_signal = stop_signal_text(event);
    let prompt_text = extract_prompt_text(event);
    let response_full = hook_dispatch::extract_response_text(event);

    // my-light / disable suppress: user Stop prompt only (not assistant tail in `stop_signal`).
    if hook_dispatch::is_review_gate_suppressed("codex", Some(repo_root), &prompt_text) {
        if let Some(msg) = hooks::closeout_stop_followup_for_completion_text(
            repo_root,
            &closeout_completion_text(event),
        ) {
            return Some(json!({
                "decision": "block",
                "followup_message": msg
            }));
        }
        super::reset_hook_state(repo_root, event);
        return None;
    }

    if let Some(msg) = hooks::closeout_stop_followup_for_completion_text(
        repo_root,
        &closeout_completion_text(event),
    ) {
        return Some(json!({
            "decision": "block",
            "followup_message": msg
        }));
    }

    match load_state(repo_root, event) {
        Err(reason) => {
            eprintln!("[router-rs] codex hook-state unreadable: {reason}");
            return Some(json!({
                "decision": "block",
                "followup_message": format!(
                    "router-rs {} need=repair_hook_state_json_or_permissions",
                    lifecycle_host().hook_state_unreadable_tag()
                )
            }));
        }
        Ok(Some(mut state)) => {
            let persist = with_codex_state_lock(repo_root, event, |_loaded| {
                if has_override(&prompt_text) {
                    state.review_gate.review_override = true;
                }
                if core_policy::hook_common::saw_reject_reason(&stop_signal, &prompt_text) {
                    state.review_gate.reject_reason_seen = true;
                }
                // Unified goal gate (shared across all 4 hosts)
                let goal_entry = core_policy::hook_common::is_framework_goal_entry_prompt(&prompt_text);
                hook_dispatch::update_goal_gate(
                    &mut state.review_gate,
                    &prompt_text,
                    &response_full,
                    goal_entry,
                );
                let assistant_tail = core_policy::hook_common::hook_assistant_tail_window(
                    &response_full,
                    core_policy::hook_common::HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
                );
                if let Some(phase) = maybe_bump_codex_review_phase_for_compact_findings(
                    state.review_gate.review_required,
                    state.review_gate.review_override,
                    state.phase,
                    state.subagent_start_count,
                    state.review_gate.independent_reviewer_seen,
                    &assistant_tail,
                ) {
                    state.phase = phase;
                }
                let fields = state.review_gate_fields();
                Ok((Some(state), fields))
            });
            match persist {
                Err(_) => return Some(hook_state_persist_block_payload()),
                Ok(fields) => {
                    if let Some(payload) = review_stop_advisory_payload(&fields) {
                        return Some(payload);
                    }
                }
            }
        }
        Ok(None) => {
            let stop_facts = ReviewGateFacts::from_prompt(&prompt_text);
            let reject = core_policy::hook_common::saw_reject_reason(&stop_signal, &prompt_text);
            let fields = core_policy::hook_review_gate_fields_from_facts(&stop_facts, reject);
            if let Some(payload) = review_stop_advisory_payload(&fields) {
                return Some(payload);
            }
        }
    }

    // Unified goal gate followup check (shared across all 4 hosts)
    // Re-load state to check goal gate after update_goal_gate wrote fields
    if let Ok(Some(state)) = load_state(repo_root, event) {
        if hook_dispatch::goal_gate_satisfied(&state.review_gate) {
            super::reset_hook_state(repo_root, event);
            return None;
        }
        // Goal gate not satisfied — inject followup
        let followup = hook_dispatch::shared_goal_stop_followup_line(
            state.review_gate.goal_contract_seen,
            state.review_gate.goal_progress_seen,
            state.review_gate.goal_verify_or_block_seen,
            state.review_gate.goal_followup_count,
        );
        return Some(json!({
            "decision": "block",
            "followup_message": followup
        }));
    }

    super::reset_hook_state(repo_root, event);
    None
}

pub(super) fn handle_codex_subagent_start(repo_root: &Path, event: &Value) -> Option<Value> {
    let tool_name = extract_tool_name(event);
    let tool_input = extract_tool_input(event);
    let prompt = extract_prompt_text(event);
    let facts = ReviewGateFacts::from_prompt(&prompt);
    let recognized = recognized_subagent_kind(&tool_input);

    match with_codex_state_lock(repo_root, event, |loaded| {
        let mut state = match loaded {
            Some(value) => value,
            None => CodexLifecycleContextState {
                seq: 1,
                review_gate: HookReviewDiskCore {
                    review_required: facts.review_required,
                    review_override: facts.review_override,
                    ..HookReviewDiskCore::default()
                },
                ..CodexLifecycleContextState::default()
            },
        };
        state.generic_subagent_seen = true;
        let tool_label = recognized
            .as_ref()
            .map(|kind| format!("{tool_name}#{kind}"))
            .unwrap_or_else(|| format!("{tool_name}#untyped"));
        state.review_subagent_tool = Some(tool_label);
        let (review_lane, parallel_lane) =
            subagent_lane_bits_from_kind(recognized.as_deref());
        if review_lane {
            state.review_lane_seen = true;
        }
        if parallel_lane {
            state.parallel_lane_seen = true;
        }
        state.review_subagent_seen = true;
        state.subagent_start_count = state.subagent_start_count.saturating_add(1);
        if review_lane {
            state.phase = state.phase.max(2);
            if facts.review_required
                && !core_policy::hook_common::is_interactive_profile(Some(repo_root), &prompt)
            {
                state.review_gate.review_required = true;
            }
        }
        Ok((Some(state), ()))
    }) {
        Ok(()) => None,
        Err(err) => {
            eprintln!("[router-rs] codex subagent start persist failed: {err}");
            Some(hook_state_persist_block_payload())
        }
    }
}

pub(super) fn handle_codex_subagent_stop(_repo_root: &Path, _event: &Value) -> Option<Value> {
    // SubagentStop is informational; PostToolUse handles the review gate logic.
    // Return None to allow the agent to continue.
    None
}

pub(crate) fn handle_codex_session_start(repo_root: &Path, payload: &Value) -> Option<Value> {
    if !router_rs_operator_inject_globally_enabled() {
        return None;
    }
    let source = payload
        .get("source")
        .or(payload.get("matcher"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut contexts = Vec::new();
    contexts.push(format!("Repo: {}", repo_root.display()));
    if !source.trim().is_empty() {
        contexts.push(format!("SessionStart source: {source}."));
    }
    let additional_context = compact_contexts_shared(contexts)?;
    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }))
}

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

pub(super) fn lifecycle_input_error(message: &str) -> Value {
    json!({
        "decision": "block",
        "message": message,
        "reason": message,
        "hookSpecificOutput": {
            "hookEventName": "CodexLifecycleContext",
            "permissionDecision": "deny",
            "permissionDecisionReason": message,
        },
    })
}

pub fn run_codex_lifecycle_context_hook_for_state_dir(
    repo_root: &Path,
    payload: &Value,
    state_dir_leaf: &str,
) -> Result<Option<Value>, String> {
    let host = match state_dir_leaf {
        ".codex" => CodexLifecycleHostKind::CODEX,
        other => {
            return Err(format!(
                "unsupported lifecycle state_dir_leaf `{other}` (expected `.codex`)"
            ));
        }
    };
    // Use a thread-local RAII guard to set/restore LIFECYCLE_HOST
    super::LIFECYCLE_HOST.with(|cell| {
        struct Restore(CodexLifecycleHostKind);
        impl Drop for Restore {
            fn drop(&mut self) {
                super::LIFECYCLE_HOST.with(|c| c.set(self.0));
            }
        }
        let prev = cell.get();
        cell.set(host);
        let _restore = Restore(prev);
        run_codex_lifecycle_context_hook_inner(repo_root, payload, host)
    })
}

fn run_codex_lifecycle_context_hook_inner(
    repo_root: &Path,
    payload: &Value,
    host: CodexLifecycleHostKind,
) -> Result<Option<Value>, String> {
    if !payload.is_object() {
        return Ok(Some(lifecycle_input_error(&format!(
            "{} lifecycle hook input schema invalid: expected a JSON object payload.",
            host.lifecycle_label()
        ))));
    }
    let event_name = payload
        .get("hook_event_name")
        .or(payload.get("event"))
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if super::state::require_stable_session_key_enabled() {
        match event_name.as_str() {
            "userpromptsubmit" | "posttooluse" | "stop"
                if super::state::stable_session_raw(payload).is_none() => {
                    return Ok(Some(lifecycle_input_error(&format!(
                        "{} lifecycle hook blocked: stable session key required ({} defaults on). Add session_id / conversation_id / thread_id (snake_case or camelCase) to hook JSON, or set session env fallbacks. Review gate ({}) cannot run without per-session hook-state.",
                        host.lifecycle_label(),
                        host.require_stable_session_key_env(),
                        host.review_gate_tag()
                    ))));
                }
            _ => {}
        }
    }
    let mut result: Option<Value> = match event_name.as_str() {
        "sessionstart" => handle_codex_session_start(repo_root, payload),
        "userpromptsubmit" => handle_codex_userpromptsubmit(repo_root, payload),
        "posttooluse" => handle_codex_posttooluse(repo_root, payload),
        "stop" => handle_codex_stop(repo_root, payload),
        "subagentstart" => handle_codex_subagent_start(repo_root, payload),
        "subagentstop" => handle_codex_subagent_stop(repo_root, payload),
        "" => Some(lifecycle_input_error(&format!(
            "{} lifecycle hook input schema invalid: missing hook_event_name/event.",
            host.lifecycle_label()
        ))),
        other => Some(lifecycle_input_error(&format!(
            "{} lifecycle hook input schema invalid: unsupported hook_event_name/event `{other}`.",
            host.lifecycle_label()
        ))),
    };
    if let Some(ref mut out) = result {
        core_state::state_manager::scrub_followup_fields_in_hook_output(out);
    }
    Ok(result)
}

#[cfg(test)]
pub(super) fn run_codex_review_subagent_gate(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    run_codex_lifecycle_context_hook(repo_root, payload)
}

pub(crate) fn run_codex_lifecycle_context_hook(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    run_codex_lifecycle_context_hook_for_state_dir(repo_root, payload, ".codex")
}

// ---------------------------------------------------------------------------
// Audit hook entry
// ---------------------------------------------------------------------------

pub(super) fn read_stdin_payload() -> Result<Value, String> {
    let mut stdin = std::io::stdin().lock();
    let input = read_codex_stdin_limited(&mut stdin)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).map_err(|err| format!("stdin_json_invalid: {err}"))
}

/// 4 MiB stdin reader — delegates to shared `hooks::read_stdin_limited`.
pub(crate) fn read_codex_stdin_limited<R: std::io::Read>(reader: &mut R) -> Result<String, String> {
    crate::hooks::read_stdin_limited(reader)
}

pub(super) fn canonical_codex_audit_command(command: &str) -> Result<&'static str, String> {
    if let Some(event_name) = lifecycle_event_name(command) {
        if event_name == "PreToolUse" {
            return Ok("pre-tool-use");
        }
        return Ok("lifecycle-context");
    }
    match command {
        "pre-tool-use" => Ok("pre-tool-use"),
        "contract-guard" => Ok("contract-guard"),
        "lifecycle-context" | "review-subagent-gate" => Ok("lifecycle-context"),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

pub(super) fn lifecycle_event_name(command: &str) -> Option<&'static str> {
    match command.trim().to_ascii_lowercase().as_str() {
        "sessionstart" => Some("SessionStart"),
        "pretooluse" => Some("PreToolUse"),
        "userpromptsubmit" => Some("UserPromptSubmit"),
        "posttooluse" => Some("PostToolUse"),
        "stop" => Some("Stop"),
        "subagentstart" => Some("SubagentStart"),
        "subagentstop" => Some("SubagentStop"),
        _ => None,
    }
}

fn attach_codex_hook_observation(mut value: Option<Value>) -> Option<Value> {
    if let Some(ref mut v) = value {
        hooks::attach_router_rs_observation(v, hooks::HookObservationHost::Codex);
    }
    value
}

pub fn run_codex_audit_hook(command: &str, repo_root: &Path) -> Result<Option<Value>, String> {
    hooks::ensure_kernel_bootstrap();
    let _registry_guard = core_policy::registry_review_gate::HookRegistryRepoGuard::new(repo_root);
    let canonical = canonical_codex_audit_command(command)?;
    let telemetry_event = lifecycle_event_name(command)
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_else(|| canonical.to_string());
    hooks::mark_hook_start();
    let mut payload = match read_stdin_payload() {
        Ok(payload) => payload,
        Err(err) if canonical == "lifecycle-context" => {
            let out = Ok(attach_codex_hook_observation(Some(
                lifecycle_input_error(&format!(
                    "Codex lifecycle hook input JSON invalid: {err}"
                )),
            )));
            hooks::emit_hook_fired(
                &telemetry_event,
                hooks::hook_action_from_optional_output(out.as_ref().ok().and_then(|v| v.as_ref())),
            );
            hooks::emit_hook_timing_line(&telemetry_event);
            return out;
        }
        Err(err) => {
            hooks::emit_hook_fired(&telemetry_event, "error");
            hooks::emit_hook_timing_line(&telemetry_event);
            return Err(err);
        }
    };
    if let Some(event_name) = lifecycle_event_name(command)
        && payload.is_object()
            && payload.get("hook_event_name").is_none()
            && payload.get("event").is_none()
        {
            payload["hook_event_name"] = json!(event_name);
        }
    let result = match canonical {
        "pre-tool-use" => Ok(attach_codex_hook_observation(
            super::pretool::run_codex_pre_tool_use(repo_root, &payload)?,
        )),
        "contract-guard" => Ok(attach_codex_hook_observation(
            super::contract_guard::run_codex_contract_guard(repo_root, &payload)?,
        )),
        "lifecycle-context" => Ok(attach_codex_hook_observation(
            run_codex_lifecycle_context_hook(repo_root, &payload)?,
        )),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    };
    match &result {
        Ok(output) => hooks::emit_hook_fired(
            &telemetry_event,
            hooks::hook_action_from_optional_output(output.as_ref()),
        ),
        Err(_) => hooks::emit_hook_fired(&telemetry_event, "error"),
    }
    hooks::emit_hook_timing_line(&telemetry_event);
    result
}
