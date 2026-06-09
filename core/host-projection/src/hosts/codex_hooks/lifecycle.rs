use router_rs::hook_common::{
    is_deep_review_gate_lane_normalized, normalize_subagent_type, normalize_tool_name,
};
use router_rs::review_gate_engine::{
    codex_review_independent_fork, fork_context_from_values, ReviewGateFacts,
};
use router_rs::router_env_flags::{
    router_rs_env_enabled_default_false, router_rs_operator_inject_globally_enabled,
};
use super::install::codex_projection_drift_warning;
use router_rs::framework_error::FrameworkError;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

use super::audit::{codex_lifecycle_input_error, run_pre_tool_use};
use super::state::{
    codex_require_stable_session_key_enabled, codex_stable_session_raw, with_codex_state_lock,
};
use super::{
    lifecycle_host, CodexLifecycleHostKind, CODEX_ADDITIONAL_CONTEXT_MAX_BYTES,
    CODEX_REVIEW_SUBAGENT_TOOL_NAMES, CODEX_REVIEW_SUBAGENT_TYPES, LIFECYCLE_HOST,
};

pub fn codex_hook_command_timeout_secs(host: CodexLifecycleHostKind, event: &str) -> u64 {
    match event {
        "SessionStart" => 3,
        "PostToolUse" => 5,
        "SubagentStart" | "SubagentStop" => {
            if host.state_dir_leaf == ".antigravitycli" { 10 } else { 5 }
        }
        _ => 8,
    }
}

/// Upper bound for merged `additionalContext` UTF-8 **bytes** (not Unicode scalar count).
///
/// Reads `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` first when set; otherwise
/// `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` (legacy name; still interpreted as bytes).
/// Value is clamped to \[256, 8192].
pub fn codex_additional_context_max_bytes() -> usize {
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

pub fn truncate_codex_additional_context_bytes(combined: &str, max_bytes: usize) -> String {
    router_rs::hook_outbound_protect::truncate_hook_outbound_lines_preserving(
        combined,
        max_bytes,
        "...",
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    Apply,
    Check,
}

#[derive(Debug, Clone)]
pub struct HooksMergeStat {
    pub status: &'static str,
    pub preserved_existing_entries: usize,
    pub added_entries: usize,
    pub removed_legacy_entries: usize,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodexLifecycleContextState {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    pub seq: i64,
    #[serde(default)]
    pub review_subagent_seen: bool,
    #[serde(default)]
    pub generic_subagent_seen: bool,
    #[serde(default)]
    pub review_lane_seen: bool,
    #[serde(default)]
    pub parallel_lane_seen: bool,
    #[serde(default)]
    pub review_required: bool,
    #[serde(default)]
    pub review_override: bool,
    #[serde(default)]
    pub independent_review_subagent_seen: bool,
    #[serde(default)]
    pub phase: u32,
    #[serde(default)]
    pub subagent_start_count: u32,
    #[serde(default)]
    pub reject_reason_seen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_subagent_tool: Option<String>,
}

impl router_rs::hosts::hook_state_common::HookStateVersion for CodexLifecycleContextState {
    const STATE_VERSION: u32 = 1;
    fn version(&self) -> u32 { self.version }
}

pub fn codex_prompt_text(event: &Value) -> String {
    for key in ["prompt", "user_prompt", "message", "input"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

#[cfg(test)]
#[cfg(test)]
pub fn codex_first_nonempty_prompt_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

pub fn codex_tool_name(event: &Value) -> String {
    event
        .get("tool_name")
        .or(event.get("tool"))
        .or(event.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn codex_tool_input(event: &Value) -> Value {
    event
        .get("tool_input")
        .or(event.get("input"))
        .or(event.get("arguments"))
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

pub fn saw_subagent_codex(tool_name: &str, _tool_input: &Value) -> bool {
    let name = normalize_tool_name(Some(tool_name));
    CODEX_REVIEW_SUBAGENT_TOOL_NAMES.contains(&name.as_str())
}

pub fn codex_recognized_subagent_kind(tool_input: &Value) -> Option<String> {
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

pub fn codex_subagent_lane_bits_from_kind(kind: Option<&str>) -> (bool, bool) {
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

pub fn codex_tool_fork_context(tool_input: &Value, event: &Value) -> Option<bool> {
    fork_context_from_values(tool_input, Some(event))
}

/// 与 Cursor `REVIEW_GATE` 深度 lane 对齐：`general-purpose` / `best-of-n-runner`（已 normalize）；缺字段推断见 [`codex_review_independent_fork`].
pub fn codex_deep_independent_reviewer_evidence(
    recognized_kind: Option<&str>,
    tool_input: &Value,
    event: &Value,
) -> bool {
    let deep_lane = recognized_kind.is_some_and(is_deep_review_gate_lane_normalized);
    codex_review_independent_fork(codex_tool_fork_context(tool_input, event), deep_lane)
}

pub fn codex_hook_state_persist_block_payload() -> Value {
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

pub fn codex_stop_hook_active_replay(event: &Value) -> bool {
    event
        .get("stop_hook_active")
        .or(event.get("stopHookActive"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Codex-internal Stop replays (`stop_hook_active`): skip gate enforcement only when explicitly opted in.
pub fn codex_stop_hook_active_bypass_enabled() -> bool {
    router_rs_env_enabled_default_false(lifecycle_host().stop_hook_active_bypass_env())
}

/// **仅当** host-specific `*_REVIEW_GATE_DISABLE=1|true|yes|on` 时关闭 review gate（unset 保持启用）。
pub fn codex_review_gate_disabled_by_env() -> bool {
    router_rs_env_enabled_default_false(lifecycle_host().review_gate_disable_env())
}

/// Env disable (my-light profile only) **or** `my-light` profile (advisory-only mode).
/// In non-my-light lifecycle profile, env-var bypass is **ignored** — review gate stays hard-enabled.
pub fn codex_review_gate_suppressed(repo_root: &Path, text: &str) -> bool {
    if router_rs::hook_common::review_gate_hard_block_disabled(Some(repo_root), text) {
        return true;
    }
    // Codex: env bypass only valid in my-light profile (non-my-light ignores env)
    router_rs::hook_common::my_light_profile_active(Some(repo_root), text)
        && codex_review_gate_disabled_by_env()
}

pub fn clear_codex_review_gate_hook_state(repo_root: &Path, event: &Value) {
    codex_reset_hook_state(repo_root, event);
}

pub fn codex_agent_response_text(event: &Value) -> String {
    const KEYS: &[&str] = &[
        "response",
        "agent_response",
        "agentResponse",
        "content",
        "text",
        "output",
    ];
    for key in KEYS {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            if !value.trim().is_empty() {
                return value.to_string();
            }
        }
    }
    String::new()
}

pub fn codex_stop_signal_text(event: &Value) -> String {
    let prompt = codex_prompt_text(event);
    let response = codex_agent_response_text(event);
    if prompt.trim().is_empty() {
        response
    } else if response.trim().is_empty() {
        prompt
    } else {
        format!("{prompt}\n{response}")
    }
}

pub fn codex_closeout_completion_text(event: &Value) -> String {
    codex_stop_signal_text(event)
}

pub fn codex_review_stop_followup_line(phase: u32) -> String {
    let host = lifecycle_host();
    format!(
        "router-rs {} incomplete phase={phase} need=deep_reviewer_posttool_and_compact_findings_or_rg_clear hint=fork_context_json_false_or_codex_fork_infer_off; Stop response needs [P0]/[P1]/[P2]/Caveat: substantive line see=skills/code-review-deep/SKILL.md see={}=1|{}=1",
        host.review_gate_tag(),
        host.review_gate_disable_env(),
        host.stop_hook_active_bypass_env()
    )
}

pub fn codex_reset_hook_state(repo_root: &Path, event: &Value) {
    let _ = with_codex_state_lock(repo_root, event, |_loaded| {
        let reset = CodexLifecycleContextState {
            seq: 0,
            ..CodexLifecycleContextState::default()
        };
        Ok((Some(reset), ()))
    });
}

pub fn codex_compact_contexts(parts: Vec<String>) -> Option<String> {
    let mut dedup = HashSet::new();
    let mut unique = Vec::new();
    for part in parts {
        let normalized = part.trim();
        if normalized.is_empty() {
            continue;
        }
        // Deduplicate on exact trimmed text only. Prior ASCII-lowercase keys incorrectly merged
        // distinct lines that differed only by case or subtle spelling.
        let key = normalized.to_string();
        if dedup.insert(key.clone()) {
            unique.push(key);
        }
    }
    if unique.is_empty() {
        return None;
    }
    let combined = unique.join("\n");
    let max_bytes = codex_additional_context_max_bytes();
    if combined.len() <= max_bytes {
        return Some(combined);
    }
    Some(truncate_codex_additional_context_bytes(
        &combined, max_bytes,
    ))
}

pub fn handle_codex_userpromptsubmit(repo_root: &Path, event: &Value) -> Option<Value> {
    let prompt = codex_prompt_text(event);
    if codex_review_gate_suppressed(repo_root, &prompt) {
        clear_codex_review_gate_hook_state(repo_root, event);
        return None;
    }
    let my_light = router_rs::hook_common::my_light_profile_active(Some(repo_root), &prompt);
    let mut facts = ReviewGateFacts::from_prompt(&prompt);
    if my_light {
        facts.review_required = false;
    }
    let state = CodexLifecycleContextState {
        seq: 0,
        review_required: facts.review_required,
        review_override: facts.review_override,
        ..CodexLifecycleContextState::default()
    };

    let narrow = router_rs::hook_common::is_narrow_review_prompt(&prompt);
    let review_arms = facts.review_required;
    let override_now = facts.review_override;
    let write_result = with_codex_state_lock(repo_root, event, |loaded| {
        let mut next = state.clone();
        if let Some(prev) = loaded {
            next.seq = prev.seq.saturating_add(1);
            if my_light || narrow {
                next.review_required = false;
                next.independent_review_subagent_seen = false;
                next.phase = 0;
                next.subagent_start_count = 0;
            } else {
                if review_arms && !override_now {
                    next.independent_review_subagent_seen = false;
                    next.phase = 0;
                    next.subagent_start_count = 0;
                    next.review_subagent_seen = false;
                    next.generic_subagent_seen = false;
                } else {
                    next.independent_review_subagent_seen =
                        prev.independent_review_subagent_seen;
                    next.phase = prev.phase;
                    next.subagent_start_count = prev.subagent_start_count;
                }
                next.review_required = prev.review_required || review_arms;
            }
            next.review_override = prev.review_override || override_now;
            next.reject_reason_seen = prev.reject_reason_seen;
        } else {
            next.seq = 1;
        }
        Ok((Some(next), ()))
    });
    if write_result.is_err() {
        return Some(codex_hook_state_persist_block_payload());
    }

    if !router_rs_operator_inject_globally_enabled() {
        return None;
    }

    let mut contexts: Vec<String> = Vec::new();
    if let Some(warning) = codex_projection_drift_warning(repo_root) {
        contexts.push(warning);
    }
    if facts.review_required
        && !facts.review_override
        && router_rs::hook_common::should_inject_spawn_first_review_nudge(Some(repo_root), &prompt)
    {
        contexts.push(router_rs::runtime_registry::review_spawn_first_nudge_line(
            Some(repo_root),
            lifecycle_host().spawn_first_host_id(),
        ));
    }
    // I7: inject heterogeneous adversarial review hint for broad/deep review prompts.
    if facts.review_required
        && router_rs::review::heterogeneous::should_plan_heterogeneous_adversarial_lane(&prompt, true)
    {
        if let Some(hint) = router_rs::review::heterogeneous::heterogeneous_review_hint_for_lane() {
            contexts.push(hint);
        }
    }
    let paper_host = lifecycle_host().paper_prose_hook_host();
    router_rs::paper_adversarial_hook::maybe_append_paper_adversarial_context(
        repo_root,
        &prompt,
        &mut contexts,
        paper_host,
    );
    router_rs::paper_prose_hook::maybe_append_paper_prose_context(
        repo_root,
        &prompt,
        &mut contexts,
        paper_host,
    );
    let additional_context = codex_compact_contexts(contexts);
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

pub fn codex_fresh_lifecycle_context_from_event(event: &Value) -> CodexLifecycleContextState {
    let prompt = codex_prompt_text(event);
    let facts = ReviewGateFacts::from_prompt(&prompt);
    CodexLifecycleContextState {
        seq: 1,
        review_required: facts.review_required,
        review_override: facts.review_override,
        ..CodexLifecycleContextState::default()
    }
}

pub fn handle_codex_subagent_start(repo_root: &Path, event: &Value) -> Option<Value> {
    let tool_name = codex_tool_name(event);
    let tool_input = codex_tool_input(event);
    let prompt = codex_prompt_text(event);
    let facts = ReviewGateFacts::from_prompt(&prompt);
    let recognized = codex_recognized_subagent_kind(&tool_input);

    match with_codex_state_lock(repo_root, event, |loaded| {
        let mut state = match loaded {
            Some(value) => value,
            None => CodexLifecycleContextState {
                seq: 1,
                review_required: facts.review_required,
                review_override: facts.review_override,
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
            codex_subagent_lane_bits_from_kind(recognized.as_deref());
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
                && !router_rs::hook_common::my_light_profile_active(Some(repo_root), &prompt)
            {
                state.review_required = true;
            }
        }
        Ok((Some(state), ()))
    }) {
        Ok(()) => None,
        Err(err) => {
            eprintln!("[router-rs] codex subagent start persist failed: {err}");
            Some(codex_hook_state_persist_block_payload())
        }
    }
}

pub fn handle_codex_subagent_stop(_repo_root: &Path, _event: &Value) -> Option<Value> {
    // SubagentStop is informational; PostToolUse handles the review gate logic.
    // Return None to allow the agent to continue.
    None
}

pub fn handle_codex_session_start(repo_root: &Path, payload: &Value) -> Option<Value> {
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
    let additional_context = codex_compact_contexts(contexts)?;
    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }))
}

pub fn run_codex_lifecycle_context_hook(
    repo_root: &Path,
    payload: &Value,
) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    run_codex_lifecycle_context_hook_for_state_dir(repo_root, payload, ".codex")
}

pub fn run_codex_lifecycle_context_hook_for_state_dir(
    repo_root: &Path,
    payload: &Value,
    state_dir_leaf: &str,
) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    let host = match state_dir_leaf {
        ".codex" => CodexLifecycleHostKind::CODEX,
        ".antigravitycli" => CodexLifecycleHostKind::ANTIGRAVITY_CLI,
        other => {
            return Err(FrameworkError::unsupported(format!(
                "unsupported lifecycle state_dir_leaf `{other}` (expected `.codex` or `.antigravitycli`)"
            )));
        }
    };
    LIFECYCLE_HOST.with(|cell| {
        struct Restore(CodexLifecycleHostKind);
        impl Drop for Restore {
            fn drop(&mut self) {
                LIFECYCLE_HOST.with(|c| c.set(self.0));
            }
        }
        let prev = cell.get();
        cell.set(host);
        let _restore = Restore(prev);
        run_codex_lifecycle_context_hook_inner(repo_root, payload, host)
    })
}

fn codex_lifecycle_canonical_from_payload_event(event_name: &str) -> Option<&'static str> {
    match event_name {
        "userpromptsubmit" => Some("user-prompt-submit"),
        "posttooluse" => Some("post-tool-use"),
        "stop" => Some("stop"),
        _ => None,
    }
}

/// Stable session key gate shared by lifecycle-context router and [`CodexHookHost`].
pub fn codex_maybe_block_missing_stable_session_key(
    payload: &Value,
    canonical_event: &str,
) -> Option<Value> {
    if !codex_require_stable_session_key_enabled() {
        return None;
    }
    match canonical_event {
        "user-prompt-submit" | "post-tool-use" | "stop"
            if codex_stable_session_raw(payload).is_none() => {
                let host = LIFECYCLE_HOST.with(|c| c.get());
                return Some(codex_lifecycle_input_error(&format!(
                    "{} lifecycle hook blocked: stable session key required ({} defaults on). Add session_id / conversation_id / thread_id (snake_case or camelCase) to hook JSON, or set session env fallbacks. Review gate ({}) cannot run without per-session hook-state.",
                    host.lifecycle_label(),
                    host.require_stable_session_key_env(),
                    host.review_gate_tag()
                )));
            }
        _ => {}
    }
    None
}

pub fn run_codex_lifecycle_context_hook_inner(
    repo_root: &Path,
    payload: &Value,
    host: CodexLifecycleHostKind,
) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    if !payload.is_object() {
        return Ok(Some(codex_lifecycle_input_error(&format!(
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
    if let Some(canonical) = codex_lifecycle_canonical_from_payload_event(&event_name) {
        if let Some(block) = codex_maybe_block_missing_stable_session_key(payload, canonical) {
            return Ok(Some(block));
        }
    }
    if event_name == "pretooluse" && host == CodexLifecycleHostKind::ANTIGRAVITY_CLI {
        return run_pre_tool_use(repo_root, payload);
    }
    let mut result: Option<Value> = match event_name.as_str() {
        "sessionstart" => handle_codex_session_start(repo_root, payload),
        "userpromptsubmit" => handle_codex_userpromptsubmit(repo_root, payload),
        "posttooluse" => super::evaluate_codex_post_tool_use(repo_root, payload),
        "stop" => super::evaluate_codex_stop(repo_root, payload),
        "subagentstart" => handle_codex_subagent_start(repo_root, payload),
        "subagentstop" => handle_codex_subagent_stop(repo_root, payload),
        "" => Some(codex_lifecycle_input_error(&format!(
            "{} lifecycle hook input schema invalid: missing hook_event_name/event.",
            host.lifecycle_label()
        ))),
        other => Some(codex_lifecycle_input_error(&format!(
            "{} lifecycle hook input schema invalid: unsupported hook_event_name/event `{other}`.",
            host.lifecycle_label()
        ))),
    };
    if let Some(ref mut out) = result {
        router_rs::goal_state::scrub_followup_fields_in_hook_output(out);
    }
    Ok(result)
}







#[cfg(test)]
pub fn run_codex_review_subagent_gate(
    repo_root: &Path,
    payload: &Value,
) -> router_rs::framework_error::FrameworkResult<Option<Value>> {
    run_codex_lifecycle_context_hook(repo_root, payload)
}
