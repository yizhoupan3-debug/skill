mod state;
pub(crate) use state::CodexHookError;
use state::*;

use crate::framework_runtime::{
    build_framework_contract_summary_envelope, try_append_post_tool_shell_evidence,
};
use crate::hook_common::{
    has_override, is_reviewer_lane_normalized, normalize_subagent_type, normalize_tool_name,
};
use crate::host_entrypoint_sync::HostEntrypointPayloadProvider;
use crate::host_integration::ensure_codex_skill_surface;
use crate::review_gate_engine::{
    fork_context_from_values, maybe_bump_codex_review_phase_for_compact_findings,
    review_independent_fork, review_independent_reviewer_evidence,
    ReviewGateFacts,
};
use core_policy::HookReviewDiskCore;
use crate::router_env_flags::{
    router_rs_env_enabled_default_false,
    router_rs_operator_inject_globally_enabled,
};
use chrono::Utc;
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Read};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod policy_embed;
pub(crate) use policy_embed::build_codex_agent_policy;

const CODEX_HOOK_AUTHORITY: &str = "rust-codex-audit";
pub(crate) const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str =
    ".codex/host_entrypoints_sync_manifest.json";
const HOST_ENTRYPOINT_SYNC_HINT: &str =
    "cargo run --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root \"$PWD\"";
pub(crate) const CODEX_AGENT_POLICY_PATH: &str = "AGENTS_CODEX.md";
pub(crate) const CODEX_HOOKS_PATH: &str = ".codex/hooks.json";
pub(crate) const CODEX_HOOKS_README_PATH: &str = ".codex/README.md";
pub(crate) const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 1] = [CODEX_HOOKS_PATH];
const PROTECTED_GENERATED_PATHS: [&str; 9] = [
    CODEX_AGENT_POLICY_PATH,
    "AGENTS.md",
    "AGENTS_ANTIGRAVITY.md",
    "AGENTS_CURSOR.md",
    CODEX_HOOKS_PATH,
    CODEX_HOOKS_README_PATH,
    HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
    ".antigravitycli/hooks.json",
    ".antigravitycli/.router-rs-install.manifest.json",
];
const PROTECTED_GENERATED_PREFIXES: [&str; 3] = [
    "skills/SKILL_",
    "configs/framework/RUNTIME_REGISTRY.json",
    "core/router-rs/",
];
const CODEX_REVIEW_SUBAGENT_TOOL_NAMES: [&str; 6] = [
    "task",
    "functions.task",
    "functions.subagent",
    "functions.spawn_agent",
    "subagent",
    "spawn_agent",
];
const CODEX_REVIEW_SUBAGENT_TYPES: &[&str] = &[
    "default",
    "explore",
    "explorer",
    "general-purpose",
    "generalpurpose",
    "shell",
    "worker",
    "browser-use",
    "browseruse",
    "ci-investigator",
    "ciinvestigator",
    "best-of-n-runner",
    "bestofnrunner",
    "cursor-guide",
    "cursorguide",
];
pub(crate) const INSTALL_LIFECYCLE_EVENTS: [&str; 7] = [
    "SessionStart",
    "PreToolUse",
    "UserPromptSubmit",
    "PostToolUse",
    "Stop",
    "SubagentStart",
    "SubagentStop",
];
const INSTALL_EVENTS: [&str; 7] = INSTALL_LIFECYCLE_EVENTS;

struct HostStrings {
    review_gate_tag: &'static str,
    review_gate_disable_env: &'static str,
    stop_hook_active_bypass_env: &'static str,
    require_stable_session_key_env: &'static str,
    hook_state_salt_env: &'static str,
    hook_state_unreadable_tag: &'static str,
    lifecycle_label: &'static str,
    spawn_first_host_id: &'static str,
    paper_prose_hook_env: &'static str,
    paper_adversarial_hook_env: &'static str,
}

const CODEX_STRINGS: HostStrings = HostStrings {
    review_gate_tag: "CODEX_REVIEW_GATE",
    review_gate_disable_env: "ROUTER_RS_CODEX_REVIEW_GATE_DISABLE",
    stop_hook_active_bypass_env: "ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS",
    require_stable_session_key_env: "ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY",
    hook_state_salt_env: "ROUTER_RS_CODEX_HOOK_STATE_SALT",
    hook_state_unreadable_tag: "CODEX_HOOK_STATE_UNREADABLE",
    lifecycle_label: "Codex",
    spawn_first_host_id: "codex-cli",
    paper_prose_hook_env: "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
    paper_adversarial_hook_env: "ROUTER_RS_CODEX_PAPER_ADVERSARIAL_HOOK",
};

const ANTIGRAVITY_CLI_STRINGS: HostStrings = HostStrings {
    review_gate_tag: "ANTIGRAVITY_CLI_REVIEW_GATE",
    review_gate_disable_env: "ROUTER_RS_ANTIGRAVITY_CLI_REVIEW_GATE_DISABLE",
    stop_hook_active_bypass_env: "ROUTER_RS_ANTIGRAVITY_CLI_STOP_HOOK_ACTIVE_BYPASS",
    require_stable_session_key_env: "ROUTER_RS_ANTIGRAVITY_CLI_REQUIRE_STABLE_SESSION_KEY",
    hook_state_salt_env: "ROUTER_RS_ANTIGRAVITY_CLI_HOOK_STATE_SALT",
    hook_state_unreadable_tag: "ANTIGRAVITY_CLI_HOOK_STATE_UNREADABLE",
    lifecycle_label: "Antigravity CLI",
    spawn_first_host_id: "antigravity-cli",
    paper_prose_hook_env: "ROUTER_RS_ANTIGRAVITY_CLI_PAPER_PROSE_HOOK",
    paper_adversarial_hook_env: "ROUTER_RS_ANTIGRAVITY_CLI_PAPER_ADVERSARIAL_HOOK",
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CodexLifecycleHostKind {
    state_dir_leaf: &'static str,
}

impl CodexLifecycleHostKind {
    pub(crate) const CODEX: Self = Self {
        state_dir_leaf: ".codex",
    };
    pub(crate) const ANTIGRAVITY_CLI: Self = Self {
        state_dir_leaf: ".antigravitycli",
    };

    fn strings(self) -> &'static HostStrings {
        match self.state_dir_leaf {
            ".antigravitycli" => &ANTIGRAVITY_CLI_STRINGS,
            _ => &CODEX_STRINGS,
        }
    }

    fn review_gate_tag(self) -> &'static str {
        self.strings().review_gate_tag
    }

    fn review_gate_disable_env(self) -> &'static str {
        self.strings().review_gate_disable_env
    }

    fn stop_hook_active_bypass_env(self) -> &'static str {
        self.strings().stop_hook_active_bypass_env
    }

    fn require_stable_session_key_env(self) -> &'static str {
        self.strings().require_stable_session_key_env
    }

    fn hook_state_salt_env(self) -> &'static str {
        self.strings().hook_state_salt_env
    }

    fn hook_state_unreadable_tag(self) -> &'static str {
        self.strings().hook_state_unreadable_tag
    }

    fn lifecycle_label(self) -> &'static str {
        self.strings().lifecycle_label
    }

    fn spawn_first_host_id(self) -> &'static str {
        self.strings().spawn_first_host_id
    }

    // TODO: integrate
    #[allow(dead_code)]
    fn paper_prose_hook_env(self) -> &'static str {
        self.strings().paper_prose_hook_env
    }

    // TODO: integrate
    #[allow(dead_code)]
    fn paper_adversarial_hook_env(self) -> &'static str {
        self.strings().paper_adversarial_hook_env
    }

    fn paper_prose_hook_host(self) -> crate::paper_prose_hook::PaperProseHookHost {
        crate::paper_prose_hook::PaperProseHookHost::from_codex_lifecycle_state_dir(
            self.state_dir_leaf,
        )
    }
}


thread_local! {
    static LIFECYCLE_HOST: Cell<CodexLifecycleHostKind> =
        const { Cell::new(CodexLifecycleHostKind::CODEX) };
}


pub(super) fn lifecycle_host() -> CodexLifecycleHostKind {
    LIFECYCLE_HOST.with(|cell| cell.get())
}
pub const ROUTER_RS_HOOK_PROJECTION_VERSION: &str = "v1.0.0";
const INSTALL_STATUS_USER_PROMPT: &str = "Loading Codex turn context";
const INSTALL_STATUS_SESSION_START: &str = "Loading Codex live state";
const INSTALL_STATUS_PRE_TOOL: &str = "Checking generated-surface guard";
const INSTALL_STATUS_POST_TOOL: &str = "Recording Codex tool evidence";
const INSTALL_STATUS_STOP: &str = "Enforcing Codex review gate";
const INSTALL_STATUS_SUBAGENT_START: &str = "Recording Codex subagent start";
const INSTALL_STATUS_SUBAGENT_STOP: &str = "Recording Codex subagent stop";

const INSTALL_STATUS_ANTIGRAVITY_SESSION_START: &str = "Loading Antigravity CLI session state";
const INSTALL_STATUS_ANTIGRAVITY_PRE_TOOL: &str = "Checking Antigravity CLI generated-surface guard";
const INSTALL_STATUS_ANTIGRAVITY_USER_PROMPT: &str = "Loading Antigravity CLI turn context";
const INSTALL_STATUS_ANTIGRAVITY_POST_TOOL: &str = "Recording Antigravity CLI tool evidence";
const INSTALL_STATUS_ANTIGRAVITY_STOP: &str = "Enforcing Antigravity CLI review gate";
const INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_START: &str = "Recording Antigravity CLI subagent start";
const INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_STOP: &str = "Recording Antigravity CLI subagent stop";
/// Default UTF-8 **byte** budget for merged Codex `additionalContext` (SessionStart / UserPromptSubmit).
const CODEX_ADDITIONAL_CONTEXT_MAX_BYTES: usize = 640;
// TODO: integrate
#[allow(dead_code)]
static CODEX_SESSION_KEY_FALLBACK_WARN: Once = Once::new();
static ATOMIC_WRITE_NONCE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
thread_local! {
    static FORCE_ATOMIC_WRITE_FAIL: Cell<bool> = const { Cell::new(false) };
}

fn codex_hook_command_timeout_secs(host: CodexLifecycleHostKind, event: &str) -> u64 {
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
fn codex_additional_context_max_bytes() -> usize {
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

fn truncate_codex_additional_context_bytes(combined: &str, max_bytes: usize) -> String {
    crate::hook_outbound_protect::truncate_hook_outbound_lines_preserving(
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
pub(crate) struct HooksMergeStat {
    pub(crate) status: &'static str,
    pub(crate) preserved_existing_entries: usize,
    pub(crate) added_entries: usize,
    pub(crate) removed_legacy_entries: usize,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct CodexLifecycleContextState {
    #[serde(flatten)]
    review_gate: HookReviewDiskCore,
    #[serde(default)]
    seq: i64,
    #[serde(default)]
    review_subagent_seen: bool,
    #[serde(default)]
    generic_subagent_seen: bool,
    #[serde(default)]
    review_lane_seen: bool,
    #[serde(default)]
    parallel_lane_seen: bool,
    #[serde(default)]
    phase: u32,
    #[serde(default)]
    subagent_start_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_subagent_tool: Option<String>,
}

impl crate::hosts::hook_state_common::HookStateVersion for CodexLifecycleContextState {
    const STATE_VERSION: u32 = core_policy::HOOK_REVIEW_DISK_VERSION;
    fn disk_version(&self) -> u32 {
        self.review_gate.disk_version()
    }
}

impl CodexLifecycleContextState {
    fn review_gate_fields(&self) -> core_policy::HookReviewGateFields {
        self.review_gate.gate_fields()
    }
}


fn codex_prompt_text(event: &Value) -> String {
    for key in ["prompt", "user_prompt", "message", "input"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

#[cfg(test)]
#[cfg(test)]
fn codex_first_nonempty_prompt_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

fn codex_tool_name(event: &Value) -> String {
    event
        .get("tool_name")
        .or(event.get("tool"))
        .or(event.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn codex_tool_input(event: &Value) -> Value {
    event
        .get("tool_input")
        .or(event.get("input"))
        .or(event.get("arguments"))
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn saw_subagent_codex(tool_name: &str, _tool_input: &Value) -> bool {
    let name = normalize_tool_name(Some(tool_name));
    CODEX_REVIEW_SUBAGENT_TOOL_NAMES.contains(&name.as_str())
}

fn codex_recognized_subagent_kind(tool_input: &Value) -> Option<String> {
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

fn codex_subagent_lane_bits_from_kind(kind: Option<&str>) -> (bool, bool) {
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

fn codex_tool_fork_context(tool_input: &Value, event: &Value) -> Option<bool> {
    fork_context_from_values(tool_input, Some(event))
}

/// 与 Cursor `REVIEW_GATE` 深度 lane 对齐：`general-purpose` / `best-of-n-runner`（已 normalize）；缺字段推断见 [`review_independent_fork`].
fn codex_deep_independent_reviewer_evidence(
    recognized_kind: Option<&str>,
    tool_input: &Value,
    event: &Value,
) -> bool {
    let reviewer_lane = recognized_kind.is_some_and(is_reviewer_lane_normalized);
    review_independent_reviewer_evidence(reviewer_lane, codex_tool_fork_context(tool_input, event))
}

fn codex_hook_state_persist_block_payload() -> Value {
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

fn codex_stop_hook_active_replay(event: &Value) -> bool {
    event
        .get("stop_hook_active")
        .or(event.get("stopHookActive"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Codex-internal Stop replays (`stop_hook_active`): skip gate enforcement only when explicitly opted in.
fn codex_stop_hook_active_bypass_enabled() -> bool {
    router_rs_env_enabled_default_false(lifecycle_host().stop_hook_active_bypass_env())
}

/// Canonical `ROUTER_RS_REVIEW_GATE_DISABLE` or legacy `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE`.
fn codex_review_gate_disabled_by_env() -> bool {
    core_policy::env_flags::router_rs_review_gate_disabled_for_host("codex")
}

/// Env disable **or** `my-light` profile (advisory-only mode; Claude parity).
fn codex_review_gate_suppressed(repo_root: &Path, text: &str) -> bool {
    if codex_review_gate_disabled_by_env() {
        return true;
    }
    crate::hook_common::review_gate_hard_block_disabled(Some(repo_root), text)
}

fn clear_codex_review_gate_hook_state(repo_root: &Path, event: &Value) {
    codex_reset_hook_state(repo_root, event);
}

fn codex_agent_response_text(event: &Value) -> String {
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

fn codex_stop_signal_text(event: &Value) -> String {
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

fn codex_closeout_completion_text(event: &Value) -> String {
    codex_stop_signal_text(event)
}

fn codex_review_stop_advisory_payload(fields: &core_policy::HookReviewGateFields) -> Option<Value> {
    core_policy::hook_review_stop_advisory_needed(fields, lifecycle_host().review_gate_tag())
        .map(|followup_message| json!({ "followup_message": followup_message }))
}

fn codex_reset_hook_state(repo_root: &Path, event: &Value) {
    let _ = with_codex_state_lock(repo_root, event, |_loaded| {
        let reset = CodexLifecycleContextState {
            seq: 0,
            ..CodexLifecycleContextState::default()
        };
        Ok((Some(reset), ()))
    });
}

fn codex_compact_contexts(parts: Vec<String>) -> Option<String> {
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

fn handle_codex_userpromptsubmit(repo_root: &Path, event: &Value) -> Option<Value> {
    let prompt = codex_prompt_text(event);
    if codex_review_gate_suppressed(repo_root, &prompt) {
        clear_codex_review_gate_hook_state(repo_root, event);
        return None;
    }
    let my_light = crate::hook_common::my_light_profile_active(Some(repo_root), &prompt);
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

    let narrow = crate::hook_common::is_narrow_review_prompt(&prompt);
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
                next.review_gate.review_required =
                    prev.review_gate.review_required || review_arms;
            }
            next.review_gate.review_override =
                prev.review_gate.review_override || override_now;
            next.review_gate.reject_reason_seen = prev.review_gate.reject_reason_seen;
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
        && crate::hook_common::should_inject_spawn_first_review_nudge(Some(repo_root), &prompt)
    {
        contexts.push(crate::runtime_registry::review_spawn_first_nudge_line(
            Some(repo_root),
            lifecycle_host().spawn_first_host_id(),
        ));
    }
    let paper_host = lifecycle_host().paper_prose_hook_host();
    crate::paper_adversarial_hook::maybe_append_paper_adversarial_context(
        repo_root,
        &prompt,
        &mut contexts,
        paper_host,
    );
    crate::paper_prose_hook::maybe_append_paper_prose_context(
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

fn handle_codex_posttooluse(repo_root: &Path, event: &Value) -> Option<Value> {
    let tool_name = codex_tool_name(event);
    crate::telemetry_emit::emit_tool_call(
        &tool_name,
        crate::framework_runtime::extract_post_tool_duration_ms(event).unwrap_or(0),
        crate::framework_runtime::post_tool_call_succeeded(event),
    );
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
    let tool_input = codex_tool_input(event);
    if let Err(e) = crate::session_call_tracker::record_tool_call(repo_root, &tool_name, None) {
        eprintln!("[router-rs] session tracker record_tool_call failed (non-fatal): {e}");
    }
    if !saw_subagent_codex(&tool_name, &tool_input) {
        return None;
    }
    match with_codex_state_lock(repo_root, event, |loaded| {
        let mut state = match loaded {
            Some(value) => value,
            None => {
                let prompt = codex_prompt_text(event);
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
            state.review_gate.independent_reviewer_seen = true;
            state.subagent_start_count = state.subagent_start_count.saturating_add(1);
            state.phase = state.phase.max(2);
            let post_facts = ReviewGateFacts::from_prompt(&prompt_for_profile);
            let should_arm_review = state.review_gate.review_required || post_facts.review_required;
            if should_arm_review
                && !crate::hook_common::my_light_profile_active(Some(repo_root), &prompt_for_profile)
            {
                state.review_gate.review_required = true;
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

fn handle_codex_stop(repo_root: &Path, event: &Value) -> Option<Value> {
    if codex_stop_hook_active_replay(event) && codex_stop_hook_active_bypass_enabled() {
        return None;
    }

    let stop_signal = codex_stop_signal_text(event);
    let prompt_text = codex_prompt_text(event);
    let response_full = codex_agent_response_text(event);

    // my-light / disable suppress: user Stop prompt only (not assistant tail in `stop_signal`).
    if codex_review_gate_suppressed(repo_root, &prompt_text) {
        if let Some(msg) = crate::framework_runtime::closeout_stop_followup_for_completion_text(
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

    if let Some(msg) = crate::framework_runtime::closeout_stop_followup_for_completion_text(
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
                if crate::hook_common::saw_reject_reason(&stop_signal, &prompt_text) {
                    state.review_gate.reject_reason_seen = true;
                }
                let assistant_tail = crate::hook_common::hook_assistant_tail_window(
                    &response_full,
                    crate::hook_common::CURSOR_HOOK_SIGNAL_ASSISTANT_TAIL_CHARS,
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
                Err(_) => return Some(codex_hook_state_persist_block_payload()),
                Ok(fields) => {
                    if let Some(payload) = codex_review_stop_advisory_payload(&fields) {
                        return Some(payload);
                    }
                }
            }
        }
        Ok(None) => {
            let stop_facts = ReviewGateFacts::from_prompt(&prompt_text);
            let reject = crate::hook_common::saw_reject_reason(&stop_signal, &prompt_text);
            let fields = core_policy::hook_review_gate_fields_from_facts(&stop_facts, reject);
            if let Some(payload) = codex_review_stop_advisory_payload(&fields) {
                return Some(payload);
            }
        }
    }

    codex_reset_hook_state(repo_root, event);
    None
}

fn handle_codex_subagent_start(repo_root: &Path, event: &Value) -> Option<Value> {
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
                && !crate::hook_common::my_light_profile_active(Some(repo_root), &prompt)
            {
                state.review_gate.review_required = true;
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

fn handle_codex_subagent_stop(_repo_root: &Path, _event: &Value) -> Option<Value> {
    // SubagentStop is informational; PostToolUse handles the review gate logic.
    // Return None to allow the agent to continue.
    None
}

fn handle_codex_session_start(repo_root: &Path, payload: &Value) -> Option<Value> {
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

fn run_codex_lifecycle_context_hook(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    run_codex_lifecycle_context_hook_for_state_dir(repo_root, payload, ".codex")
}

pub(crate) fn run_codex_lifecycle_context_hook_for_state_dir(
    repo_root: &Path,
    payload: &Value,
    state_dir_leaf: &str,
) -> Result<Option<Value>, String> {
    let host = match state_dir_leaf {
        ".codex" => CodexLifecycleHostKind::CODEX,
        ".antigravitycli" => CodexLifecycleHostKind::ANTIGRAVITY_CLI,
        other => {
            return Err(format!(
                "unsupported lifecycle state_dir_leaf `{other}` (expected `.codex` or `.antigravitycli`)"
            ));
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

fn run_codex_lifecycle_context_hook_inner(
    repo_root: &Path,
    payload: &Value,
    host: CodexLifecycleHostKind,
) -> Result<Option<Value>, String> {
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
    if codex_require_stable_session_key_enabled() {
        match event_name.as_str() {
            "userpromptsubmit" | "posttooluse" | "stop" => {
                if codex_stable_session_raw(payload).is_none() {
                    return Ok(Some(codex_lifecycle_input_error(&format!(
                        "{} lifecycle hook blocked: stable session key required ({} defaults on). Add session_id / conversation_id / thread_id (snake_case or camelCase) to hook JSON, or set session env fallbacks. Review gate ({}) cannot run without per-session hook-state.",
                        host.lifecycle_label(),
                        host.require_stable_session_key_env(),
                        host.review_gate_tag()
                    ))));
                }
            }
            _ => {}
        }
    }
    if event_name == "pretooluse" && host == CodexLifecycleHostKind::ANTIGRAVITY_CLI {
        return run_pre_tool_use(repo_root, payload);
    }
    let mut result: Option<Value> = match event_name.as_str() {
        "sessionstart" => handle_codex_session_start(repo_root, payload),
        "userpromptsubmit" => handle_codex_userpromptsubmit(repo_root, payload),
        "posttooluse" => handle_codex_posttooluse(repo_root, payload),
        "stop" => handle_codex_stop(repo_root, payload),
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
        crate::autopilot_goal::scrub_followup_fields_in_hook_output(out);
    }
    Ok(result)
}

pub(crate) fn read_hook_stdin_payload() -> Result<Value, String> {
    read_stdin_payload()
}

pub(crate) fn merge_lifecycle_install_hooks_json(
    host: CodexLifecycleHostKind,
    existing: Option<Value>,
    hook_commands: &BTreeMap<String, String>,
    events: &[&str],
) -> Result<(Value, HooksMergeStat), String> {
    merge_hooks_json_for_events(host, existing, hook_commands, events)
}

pub(crate) fn hooks_install_serialize_pretty(value: &Value) -> Result<String, String> {
    serialize_ascii_json_pretty(value)
}

pub(crate) fn hooks_install_write_atomic(path: &Path, text: &str) -> Result<(), String> {
    write_atomic_text(path, text)
}

pub(crate) fn hooks_install_sha256_hex(text: &str) -> String {
    sha256_hex(text)
}

pub(crate) fn hooks_install_acquire_lock(home: &Path) -> Result<HooksInstallLock, String> {
    acquire_install_lock(home)
}

// TODO: integrate
#[allow(dead_code)]
pub(crate) fn lifecycle_hook_command_timeout_secs(host: CodexLifecycleHostKind, event: &str) -> u64 {
    codex_hook_command_timeout_secs(host, event)
}

// TODO: integrate
#[allow(dead_code)]
pub(crate) fn lifecycle_hook_event_status_message(host: CodexLifecycleHostKind, event_name: &str) -> &'static str {
    hook_event_status_message(host, event_name)
}

pub(crate) fn run_codex_pre_tool_use_hook(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    run_pre_tool_use(repo_root, payload)
}

#[cfg(test)]
fn run_codex_review_subagent_gate(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    run_codex_lifecycle_context_hook(repo_root, payload)
}
pub fn build_codex_hook_manifest() -> Value {
    let mut hooks = serde_json::Map::new();
    for event in INSTALL_EVENTS {
        let timeout = codex_hook_command_timeout_secs(CodexLifecycleHostKind::CODEX, event);
        let hook = json!({
            "type": "command",
            "command": build_project_hook_command(event),
            "timeout": timeout,
            "statusMessage": hook_event_status_message(CodexLifecycleHostKind::CODEX, event),
        });
        let mut entry = json!({
            "hooks": [hook],
        });
        if event == "SessionStart" {
            entry["matcher"] = json!("startup|resume|clear");
        }
        hooks.insert(event.to_string(), json!([entry]));
    }
    json!({
        "version": 1,
        "_comment": "Managed by router-rs. Regenerate with `cargo run --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root \"$PWD\"`.",
        "hooks": hooks,
    })
}

fn protected_generated_paths() -> Vec<&'static str> {
    PROTECTED_GENERATED_PATHS.to_vec()
}

/// Codex hook install projection. `codex_audit_commands.legacy_review_subagent_gate` is a
/// stable JSON key alias only: the CLI subcommand is `review-subagent-gate`, which maps to the
/// same `lifecycle-context` handler as `codex_lifecycle_context` (not a separate gate).
pub fn build_codex_hook_projection() -> Value {
    json!({
        "schema_version": "router-rs-codex-hook-projection-v1",
        "authority": CODEX_HOOK_AUTHORITY,
        "codex_agent_policy": build_codex_agent_policy(),
        "codex_hooks_readme": build_codex_hooks_readme(),
        "codex_hooks": build_codex_hook_manifest(),
        "codex_audit_commands": {
            "pre_tool_use": build_codex_hook_command("--event=PreToolUse"),
            "contract_guard": build_codex_hook_command("contract-guard"),
            "codex_lifecycle_context": build_codex_hook_command("lifecycle-context"),
            "legacy_review_subagent_gate": build_codex_hook_command("review-subagent-gate"),
        },
    })
}

pub(crate) fn build_codex_hooks_readme() -> String {
    "# Codex Hooks Projection\n\n\
Codex hooks are enabled for this repo and are managed by the Rust `router-rs` control plane.\n\n\
<!-- managed_by: router-rs codex sync -->\n\n\
**Policy snapshot:** the `codex_agent_policy` payload embeds repository `AGENTS.md` + `AGENTS_CODEX.md` at **router-rs compile time** (`include_str!`), not from disk on each hook run. `codex sync` / `framework sync-entrypoints` materialize **`AGENTS_CODEX.md`** and **`.codex/README.md`** (see `.codex/host_entrypoints_sync_manifest.json`); an existing `AGENTS_CODEX.md` on disk is preserved. When the delta file is missing, sync bootstraps **delta-only** content (not a merged kernel+delta blob). Rebuild before sync when hook payloads must carry policy edits (see `AGENTS_CODEX.md` → **Codex 构建快照与同步逻辑**).\n\n\
Project-local `.codex/hooks.json` uses the official Codex lifecycle surface: `SessionStart`, `PreToolUse`, `UserPromptSubmit`, `PostToolUse`, and `Stop`.\n\n\
Feature enablement uses `[features] hooks = true`; older public examples may still show `codex_hooks`, which this repository treats as a deprecated compatibility key and rewrites to `hooks`.\n\n\
`SessionStart` injects a lightweight workspace pointer (`Repo:` and optional `source`) when operator inject is enabled; it does **not** inject a continuity digest or hook-driven `GOAL_CONTINUE`. `UserPromptSubmit` injects only trigger-specific context. `PreToolUse` blocks direct edits to generated Codex surfaces. `PostToolUse` records subagent/tool telemetry and, when opted in (`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1`, default off), may append verification-like shell commands (for example `cargo test`) to `EVIDENCE_INDEX.json` when continuity is active. `Stop` enforces closeout; `CODEX_REVIEW_GATE` is **advisory-only** (no `decision: block` on review gate — see `docs/host_adapter_contract.md` §0.1). Clear gate (Claude canonical): PostTool countable deep-lane evidence → `independent_reviewer_seen`, or bounded `rg_clear` / reject override tokens; Stop may inject a one-line nudge until satisfied. Set **`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1`** to suppress advisory nudge (unset keeps enabled). `my-light` lifecycle (`/discussx|planx|implementx|verifyx` or `GOAL_STATE.lifecycle_profile`) suppresses review Stop nudge and spawn-first. It does **not** write an automatic continuity checkpoint (`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT` is a no-op). Resume work via `/implementx`, `framework_goal_drive` stdio, and manual boards under `artifacts/current/<task_id>/`. Durable cleanup should use explicit session-artifact or snapshot commands rather than an extra end-of-session hook.\n\n\
Hook state is transient and lives under `.codex/hook-state/` in the current repository while the session is active. Stable keys require `session_id` / `conversation_id` / `thread_id` in hook payloads (snake_case **or** camelCase, e.g. `sessionId`) or `CODEX_SESSION_ID` / `CODEX_CONVERSATION_ID` in the environment; otherwise hook-state may not persist across invocations (router-rs logs a one-time stderr warning per process).\n\n\
**`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY`** defaults **on** (`unset` = require stable keys). Set `0`/`false`/`off`/`no` for legacy payloads without `session_id` / env fallbacks (`SessionStart` is unaffected). Without a stable id and with strict mode off, hook-state uses a deterministic fallback keyed by **repo + cwd** (optional `ROUTER_RS_CODEX_HOOK_STATE_SALT`), not a single global file per machine.\n\n\
**`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`** (default on): deep lane + omitted `fork_context` counts as independent reviewer evidence on PostTool. Set `0`/`false`/`off`/`no` to require explicit JSON `fork_context: false`.\n\n\
Generated hook commands resolve `router-rs` in order: **`ROUTER_RS_BIN`** when set to an executable path, then `core/router-rs/target/{release,debug}/router-rs`, then repo `target/{release,debug}/router-rs`, finally `command -v router-rs` (last resort — prefer pinning `ROUTER_RS_BIN` or building into the repo). If the binary is missing, **all** lifecycle hooks fail closed with a JSON `decision:block` line.\n\n\
Merged `additionalContext` for SessionStart/UserPromptSubmit is capped by UTF-8 **byte** length (not Unicode character count). Tune with `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` or legacy `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` (same semantics; clamped 256–8192; default 640 bytes).\n\n\
Successful Codex hook processes always print one JSON object line on stdout (including `{}` when there is no hook-specific output).\n\n\
Stop hook blocks when `.codex/hook-state` cannot be read or parsed (non-recoverable JSON/IO): fix permissions or delete corrupted state files before continuing.\n\n\
Use `cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint install-codex-user-hooks` when you want to install the same Codex hook projection into a user-level `~/.codex/hooks.json`. The installer keeps existing hooks and idempotently appends the managed command hook without replacing unrelated handlers.\n\n\
Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.\n\n\
Regenerate with:\n\n\
```sh\n\
cargo run --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root \"$PWD\"\n\
```\n\n\
Steady-state documentation map: `docs/README.md`.\n"
        .to_string()
}

pub(crate) fn codex_host_entrypoint_provider(
    repo_root: &Path,
) -> Result<HostEntrypointPayloadProvider, String> {
    let mut files = BTreeMap::new();
    let policy_path = repo_root.join(CODEX_AGENT_POLICY_PATH);
    let policy = match fs::read(&policy_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            include_str!("../../../../../AGENTS_CODEX.md").as_bytes().to_vec()
        }
        Err(err) => {
            return Err(format!(
                "failed to read {}: {err}",
                policy_path.to_string_lossy()
            ));
        }
    };
    files.insert(CODEX_AGENT_POLICY_PATH.to_string(), policy);
    files.insert(
        CODEX_HOOKS_PATH.to_string(),
        serialize_pretty_json_bytes(&build_codex_hook_manifest())?,
    );
    files.insert(
        CODEX_HOOKS_README_PATH.to_string(),
        build_codex_hooks_readme().into_bytes(),
    );
    Ok(HostEntrypointPayloadProvider {
        files,
        json_relative_paths: HOST_ENTRYPOINT_JSON_RELATIVE_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
        manifest_relative_path: HOST_ENTRYPOINT_SYNC_MANIFEST_PATH.to_string(),
        agent_policy_entrypoint: CODEX_AGENT_POLICY_PATH.to_string(),
        after_apply: Some(codex_host_entrypoint_after_apply),
    })
}

fn codex_host_entrypoint_after_apply(repo_root: &Path) -> Result<Value, String> {
    ensure_codex_skill_surface(repo_root)
}

fn serialize_pretty_json_bytes(payload: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(payload).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn build_hook_binary_preamble(
    project_var: &str,
    env_var: &str,
    missing_binary_fallback: &str,
) -> String {
    let mut command = String::new();
    command.push_str(&format!(
        "{project_var}=\"${{{env_var}:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}\"; "
    ));
    command.push_str(&format!(
        "RS_BIN=\"\"; \
if [ -n \"${{ROUTER_RS_BIN:-}}\" ] && [ -x \"${{ROUTER_RS_BIN}}\" ]; then \
_CMDV=\"$(command -v router-rs 2>/dev/null || true)\"; \
if [ \"$ROUTER_RS_BIN\" = \"$_CMDV\" ] || [[ \"$ROUTER_RS_BIN\" == \"${project_var}/\"* ]]; then RS_BIN=\"${{ROUTER_RS_BIN}}\"; \
else echo \"[router-rs] ROUTER_RS_BIN rejected (not in repo or PATH): $ROUTER_RS_BIN\" >&2; fi; \
elif [ -x \"${project_var}/core/router-rs/target/release/router-rs\" ]; then RS_BIN=\"${project_var}/core/router-rs/target/release/router-rs\"; \
elif [ -x \"${project_var}/core/router-rs/target/debug/router-rs\" ]; then RS_BIN=\"${project_var}/core/router-rs/target/debug/router-rs\"; \
elif [ -x \"${project_var}/target/release/router-rs\" ]; then RS_BIN=\"${project_var}/target/release/router-rs\"; \
elif [ -x \"${project_var}/target/debug/router-rs\" ]; then RS_BIN=\"${project_var}/target/debug/router-rs\"; \
else RS_BIN=\"$(command -v router-rs 2>/dev/null || true)\"; fi; "
    ));
    command.push_str("if [ ! -x \"$RS_BIN\" ]; then ");
    command.push_str(missing_binary_fallback);
    command.push_str("; fi; ");
    command
}

fn build_codex_hook_command(event: &str) -> String {
    let mut command =
        build_hook_binary_preamble("CODEX_PROJECT_ROOT", "CODEX_PROJECT_ROOT", "printf '%s\\n' '{\"decision\":\"block\",\"message\":\"router-rs binary unavailable for Codex hook\",\"reason\":\"router-rs binary unavailable; fail-closed instead of silently bypassing critical hook enforcement\"}'; exit 1");
    command.push_str(&format!(
        "\"$RS_BIN\" codex hook {event} --repo-root \"$CODEX_PROJECT_ROOT\""
    ));
    command
}

fn build_project_hook_command(event: &str) -> String {
    build_install_hook_command(Path::new("."), event)
}

pub(crate) struct HooksInstallLock {
    path: PathBuf,
}

impl Drop for HooksInstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_install_lock(codex_home: &Path) -> Result<HooksInstallLock, String> {
    let lock_path = codex_home.join(".install.lock");
    for _ in 0..30 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let stamp = format!("pid={} ts={now_ms}\n", std::process::id());
                use std::io::Write as _;
                file.write_all(stamp.as_bytes())
                    .map_err(CodexHookError::StateLockWrite)?;
                file.sync_all()
                    .map_err(CodexHookError::StateLockSync)?;
                return Ok(HooksInstallLock { path: lock_path });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(format!("install_lock_acquire_failed: {err}")),
        }
    }
    Err("install_lock_timeout".to_string())
}

fn projection_version_older(manifest_version: &str, current: &str) -> bool {
    fn parse(value: &str) -> Option<(u64, u64, u64)> {
        let cleaned = value.trim().trim_start_matches('v');
        let mut parts = cleaned.split('.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ))
    }
    match (parse(manifest_version), parse(current)) {
        (Some(found), Some(expected)) => found < expected,
        _ => true,
    }
}

static DRIFT_CACHE: LazyLock<std::sync::Mutex<(std::time::Instant, Option<String>)>> =
    LazyLock::new(|| std::sync::Mutex::new((std::time::Instant::now() - std::time::Duration::from_secs(600), None)));

fn codex_projection_drift_warning(repo_root: &Path) -> Option<String> {
    const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);
    {
        let guard = DRIFT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if guard.0.elapsed() < CACHE_TTL {
            return guard.1.clone();
        }
    }
    let warning = "[router-rs] hook projection drift detected; consider re-running `router-rs framework maint install-codex-user-hooks`.".to_string();
    let local_codex_home = repo_root.join("codex-home");
    let manifest_path = if local_codex_home.is_dir() {
        local_codex_home.join(".router-rs-install.manifest.json")
    } else {
        let codex_home = env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))?;
        if !codex_home.is_dir() {
            return None;
        }
        codex_home.join(".router-rs-install.manifest.json")
    };
    let text = match fs::read_to_string(manifest_path) {
        Ok(v) => v,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => return Some(warning),
    };
    let manifest: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Some(warning),
    };
    let projection = manifest
        .get("projection_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let result = if projection_version_older(projection, ROUTER_RS_HOOK_PROJECTION_VERSION) {
        Some(warning)
    } else {
        None
    };
    if let Ok(mut guard) = DRIFT_CACHE.lock() {
        *guard = (std::time::Instant::now(), result.clone());
    }
    result
}

pub fn resolve_codex_home(arg: Option<&Path>) -> Result<PathBuf, String> {
    let candidate = if let Some(path) = arg {
        path.to_path_buf()
    } else if let Some(path) = env::var_os("CODEX_HOME") {
        PathBuf::from(path)
    } else if let Some(home) = env::var_os("HOME") {
        PathBuf::from(home).join(".codex")
    } else {
        return Err(
            "Could not resolve codex home: missing --codex-home, CODEX_HOME, and HOME".to_string(),
        );
    };
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        env::current_dir()
            .map_err(|err| format!("Could not resolve current directory: {err}"))?
            .join(candidate)
    };
    fs::create_dir_all(&absolute)
        .map_err(|err| format!("Failed to create codex home {}: {err}", absolute.display()))?;
    absolute.canonicalize().map_err(|err| {
        format!(
            "Failed to canonicalize codex home {}: {err}",
            absolute.display()
        )
    })
}

pub fn install_codex_cli_hooks(
    codex_home: &Path,
    repo_root: &Path,
    mode: InstallMode,
) -> Result<Value, String> {
    let apply = matches!(mode, InstallMode::Apply);
    let resolved_codex_home = resolve_codex_home(Some(codex_home))?;
    let resolved_repo_root = if repo_root.is_absolute() {
        repo_root.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|err| format!("Could not resolve current directory: {err}"))?
            .join(repo_root)
    };
    let resolved_repo_root = resolved_repo_root.canonicalize().map_err(|err| {
        format!(
            "Failed to canonicalize repo root {}: {err}",
            resolved_repo_root.display()
        )
    })?;
    if !resolved_repo_root.exists() {
        return Err(format!(
            "Repo root does not exist: {}",
            resolved_repo_root.display()
        ));
    }

    let config_path = resolved_codex_home.join("config.toml");
    let hooks_path = resolved_codex_home.join("hooks.json");
    let hook_commands = INSTALL_EVENTS
        .iter()
        .map(|event| {
            (
                (*event).to_string(),
                build_install_hook_command(&resolved_repo_root, event),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let command_digest = sha256_hex(&serialize_ascii_json_pretty(&json!(hook_commands))?);
    let _install_guard: Option<HooksInstallLock> = if apply {
        Some(acquire_install_lock(&resolved_codex_home)?)
    } else {
        None
    };

    let existing_config = fs::read_to_string(&config_path).ok();
    let (merged_config, config_status) = merge_features_codex_hooks(existing_config.as_deref());
    let config_changed = existing_config.as_deref() != Some(merged_config.as_str());
    if apply && config_changed {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create config parent directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        write_atomic_text(&config_path, &merged_config)?;
    }

    let hooks_existed = hooks_path.exists();
    if apply
        && hooks_existed
        && fs::symlink_metadata(&hooks_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(format!(
            "Refusing to update symlinked hooks.json: {}",
            hooks_path.display()
        ));
    }
    let hooks_text = fs::read_to_string(&hooks_path).ok();
    let hooks_value = if let Some(text) = hooks_text.as_deref() {
        Some(
            serde_json::from_str::<Value>(text)
                .map_err(|err| format!("Failed to parse {}: {err}", hooks_path.display()))?,
        )
    } else {
        None
    };
    let (merged_hooks, hooks_stat) = merge_hooks_json(hooks_value, &hook_commands)?;
    let hooks_serialized = serialize_ascii_json_pretty(&merged_hooks)?;
    let hooks_changed = hooks_text.as_deref() != Some(hooks_serialized.as_str());
    let mut backup_path: Option<PathBuf> = None;

    if apply && hooks_changed {
        if let Some(parent) = hooks_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create hooks parent directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        if hooks_existed {
            let backup = PathBuf::from(format!(
                "{}.bak.{}",
                hooks_path.display(),
                Utc::now().format("%Y%m%d%H%M%S")
            ));
            fs::copy(&hooks_path, &backup).map_err(|err| {
                format!(
                    "Failed to backup hooks {} -> {}: {err}",
                    hooks_path.display(),
                    backup.display()
                )
            })?;
            backup_path = Some(backup);
        }
        let write_result = write_atomic_text(&hooks_path, &hooks_serialized);
        if let Err(err) = write_result {
            if let Some(backup) = backup_path.as_ref() {
                let _ = fs::copy(backup, &hooks_path);
            }
            return Err(err);
        }
        if !hooks_existed {
            #[cfg(unix)]
            {
                let _ = fs::set_permissions(&hooks_path, fs::Permissions::from_mode(0o644));
            }
        }
    }
    if apply {
        let manifest = json!({
            "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
            "command_digest": command_digest,
        });
        let manifest_text = serialize_ascii_json_pretty(&manifest)?;
        write_atomic_text(
            &resolved_codex_home.join(".router-rs-install.manifest.json"),
            &manifest_text,
        )?;
    }

    Ok(json!({
        "schema_version": "router-rs-codex-install-hooks-v1",
        "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
        "command_digest": command_digest,
        "authority": "rust-codex-install-hooks",
        "codex_home": resolved_codex_home.to_string_lossy().into_owned(),
        "repo_root": resolved_repo_root.to_string_lossy().into_owned(),
        "applied": apply,
        "config_toml": {
            "path": config_path.to_string_lossy().into_owned(),
            "status": mode_status(config_status, mode),
        },
        "hooks_json": {
            "path": hooks_path.to_string_lossy().into_owned(),
            "status": mode_status(hooks_stat.status, mode),
            "events": INSTALL_EVENTS,
            "preserved_existing_entries": hooks_stat.preserved_existing_entries,
            "added_entries": hooks_stat.added_entries,
            "removed_legacy_entries": hooks_stat.removed_legacy_entries,
            "backup_path": backup_path.map(|v| v.to_string_lossy().into_owned()),
        },
        "hook_commands": hook_commands,
    }))
}

fn mode_status(status: &'static str, mode: InstallMode) -> &'static str {
    match mode {
        InstallMode::Apply => status,
        InstallMode::Check => match status {
            "created" => "would-create",
            "updated" => "would-update",
            "unchanged" => "would-leave-unchanged",
            _ => "would-update",
        },
    }
}

/// Codex-side atomic write: thin wrapper on top of [`crate::atomic_write::write_atomic_text_to_temp`].
///
/// Two intentional differences vs [`crate::atomic_write::write_atomic_text`]:
///
/// 1. A `#[cfg(test)]` failure-injection seam (`FORCE_ATOMIC_WRITE_FAIL`) so the installer's
///    error-handling paths stay covered without faking filesystem failures.
/// 2. A unique hidden temp filename (`.<stem>.tmp-<pid>-<nanos>-<nonce>`). Codex installs run
///    against shared `~/.codex` paths where concurrent writers (multiple agents / sessions)
///    can collide on a plain `<ext>.tmp` sidecar; the unique nonce + pid scheme avoids that.
///
/// The actual durable-write algorithm (write → fsync → rename → fsync parent) lives in
/// `atomic_write`, so there is **one** source of truth for the IO sequence.
fn write_atomic_text(path: &Path, text: &str) -> Result<(), String> {
    #[cfg(test)]
    if FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.get()) {
        return Err("forced atomic write failure".to_string());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("atomic-write-target");
    let ts_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let nonce = ATOMIC_WRITE_NONCE.fetch_add(1, Ordering::Relaxed);



    let tmp_path = parent.join(format!(
        ".{stem}.tmp-{}-{ts_nanos}-{nonce}",
        std::process::id()
    ));
    crate::atomic_write::write_atomic_text_to_temp(path, text, &tmp_path)
}

fn serialize_ascii_json_pretty(value: &Value) -> Result<String, String> {
    let pretty = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    let mut out = String::with_capacity(pretty.len() + 1);
    for ch in pretty.chars() {
        if ch.is_ascii() {
            out.push(ch);
            continue;
        }
        let mut buf = [0u16; 2];
        for unit in ch.encode_utf16(&mut buf).iter() {
            out.push_str(&format!("\\u{:04x}", unit));
        }
    }
    out.push('\n');
    Ok(out)
}

fn hook_event_status_message(host: CodexLifecycleHostKind, event_name: &str) -> &'static str {
    match host.state_dir_leaf {
        ".antigravitycli" => match event_name {
            "SessionStart" => INSTALL_STATUS_ANTIGRAVITY_SESSION_START,
            "PreToolUse" => INSTALL_STATUS_ANTIGRAVITY_PRE_TOOL,
            "UserPromptSubmit" => INSTALL_STATUS_ANTIGRAVITY_USER_PROMPT,
            "PostToolUse" => INSTALL_STATUS_ANTIGRAVITY_POST_TOOL,
            "Stop" => INSTALL_STATUS_ANTIGRAVITY_STOP,
            "SubagentStart" => INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_START,
            "SubagentStop" => INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_STOP,
            _ => "",
        },
        _ => match event_name {
            "SessionStart" => INSTALL_STATUS_SESSION_START,
            "PreToolUse" => INSTALL_STATUS_PRE_TOOL,
            "UserPromptSubmit" => INSTALL_STATUS_USER_PROMPT,
            "PostToolUse" => INSTALL_STATUS_POST_TOOL,
            "Stop" => INSTALL_STATUS_STOP,
            "SubagentStart" => INSTALL_STATUS_SUBAGENT_START,
            "SubagentStop" => INSTALL_STATUS_SUBAGENT_STOP,
            _ => "",
        },
    }
}

fn build_install_hook_command(_repo_root: &Path, event: &str) -> String {
    let _ = _repo_root;
    format!(
        "/usr/bin/env bash \"${{SKILL_FRAMEWORK_ROOT:-${{CODEX_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}}}/configs/framework/codex-router-rs-hook.sh\" {event}"
    )
}

fn merge_features_codex_hooks(existing: Option<&str>) -> (String, &'static str) {
    match existing {
        None => ("[features]\nhooks = true\n\n".to_string(), "created"),
        Some(text) => {
            let lines = text.lines().collect::<Vec<_>>();
            let mut out = Vec::new();
            let mut in_features = false;
            let mut features_seen = false;
            let mut hooks_set = false;
            for line in lines {
                let stripped = line.trim();
                if stripped.starts_with('[') && stripped.ends_with(']') {
                    if in_features && !hooks_set {
                        out.push("hooks = true".to_string());
                        hooks_set = true;
                    }
                    in_features = stripped == "[features]";
                    if in_features {
                        features_seen = true;
                    }
                    out.push(line.to_string());
                    continue;
                }
                if in_features
                    && (is_named_setting(line, "codex_hooks") || is_named_setting(line, "hooks"))
                {
                    out.push("hooks = true".to_string());
                    hooks_set = true;
                } else {
                    out.push(line.to_string());
                }
            }
            if in_features && !hooks_set {
                out.push("hooks = true".to_string());
            }
            if !features_seen {
                if out.last().is_some_and(|line| !line.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push("[features]".to_string());
                out.push("hooks = true".to_string());
            }
            let merged = format!("{}\n", out.join("\n").trim_end());
            let canonical_existing = format!("{}\n", text.trim_end());
            if (text.ends_with('\n') && merged == canonical_existing) || merged == text {
                (merged, "unchanged")
            } else {
                (merged, "updated")
            }
        }
    }
}

fn is_named_setting(line: &str, key: &str) -> bool {
    line.split_once('=')
        .map(|(name, _)| name.trim() == key)
        .unwrap_or(false)
}

fn merge_hooks_json(
    existing: Option<Value>,
    hook_commands: &BTreeMap<String, String>,
) -> Result<(Value, HooksMergeStat), String> {
    merge_hooks_json_for_events(CodexLifecycleHostKind::CODEX, existing, hook_commands, &INSTALL_EVENTS)
}

fn merge_hooks_json_for_events(
    host: CodexLifecycleHostKind,
    existing: Option<Value>,
    hook_commands: &BTreeMap<String, String>,
    events: &[&str],
) -> Result<(Value, HooksMergeStat), String> {
    let created = existing.is_none();
    let mut data = match existing {
        None => json!({}),
        Some(value) => {
            if !value.is_object() {
                return Err("Invalid hooks.json root type: expected object".to_string());
            }
            value
        }
    };
    let root = data
        .as_object_mut()
        .ok_or_else(|| "Invalid hooks.json root type: expected object".to_string())?;
    if !root.contains_key("hooks") {
        root.insert("hooks".to_string(), json!({}));
    }
    let hooks_root = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Invalid hooks.json: `hooks` must be an object".to_string())?;

    let mut preserved_existing_entries = 0usize;
    let mut added_entries = 0usize;
    let mut removed_legacy_entries = 0usize;

    for event in events {
        let hook_command = hook_commands
            .get(*event)
            .ok_or_else(|| format!("Missing install hook command for event {event}"))?;
        if !hooks_root.contains_key(*event) {
            hooks_root.insert(event.to_string(), Value::Array(Vec::new()));
        }
        let entries = hooks_root
            .get_mut(*event)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| format!("Invalid hooks.json: hooks.{event} must be an array"))?;
        removed_legacy_entries += remove_legacy_python_codex_hooks(entries);
        preserved_existing_entries += entries.len();

        let exists = entries.iter().any(|entry| {
            entry
                .as_object()
                .and_then(|obj| obj.get("hooks"))
                .and_then(Value::as_array)
                .is_some_and(|hooks| {
                    hooks.iter().any(|hook| {
                        hook.as_object().is_some_and(|hook_obj| {
                            hook_obj.get("type").and_then(Value::as_str) == Some("command")
                                && hook_obj.get("command").and_then(Value::as_str)
                                    == Some(hook_command.as_str())
                        })
                    })
                })
        });
        if !exists {
            entries.push(json!({
                "hooks": [{
                    "type": "command",
                    "command": hook_command,
                    "timeout": codex_hook_command_timeout_secs(host, event),
                    "statusMessage": hook_event_status_message(host, event),
                }]
            }));
            added_entries += 1;
        }
    }
    let status = if created {
        "created"
    } else if added_entries > 0 || removed_legacy_entries > 0 {
        "updated"
    } else {
        "unchanged"
    };
    Ok((
        data,
        HooksMergeStat {
            status,
            preserved_existing_entries,
            added_entries,
            removed_legacy_entries,
        },
    ))
}

fn remove_legacy_python_codex_hooks(entries: &mut Vec<Value>) -> usize {
    let mut removed = 0usize;
    for entry in entries.iter_mut() {
        let Some(hooks) = entry
            .as_object_mut()
            .and_then(|obj| obj.get_mut("hooks"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = hooks.len();
        hooks.retain(|hook| {
            !hook
                .as_object()
                .and_then(|obj| obj.get("command"))
                .and_then(Value::as_str)
                .is_some_and(is_legacy_python_codex_hook_command)
        });
        removed += before.saturating_sub(hooks.len());
    }
    entries.retain(|entry| {
        entry
            .as_object()
            .and_then(|obj| obj.get("hooks"))
            .and_then(Value::as_array)
            .is_none_or(|hooks| !hooks.is_empty())
    });
    removed
}

fn is_legacy_python_codex_hook_command(command: &str) -> bool {
    command.contains("review_subagent_gate.py")
        || command.contains(".codex/hooks/review_subagent_gate.py")
}

fn attach_codex_hook_observation(mut value: Option<Value>) -> Option<Value> {
    if let Some(ref mut v) = value {
        crate::router_rs_observation::attach_router_rs_observation(
            v,
            crate::router_rs_observation::HookObservationHost::Codex,
        );
    }
    value
}

pub fn run_codex_audit_hook(command: &str, repo_root: &Path) -> Result<Option<Value>, String> {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
    let _registry_guard = crate::runtime_registry::HookRegistryRepoGuard::new(repo_root);
    let canonical = canonical_codex_audit_command(command)?;
    let telemetry_event = codex_lifecycle_event_name(command)
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_else(|| canonical.to_string());
    crate::hook_timing::mark_hook_start();
    let mut payload = match read_stdin_payload() {
        Ok(payload) => payload,
        Err(err) if canonical == "lifecycle-context" => {
            let out = Ok(attach_codex_hook_observation(Some(
                codex_lifecycle_input_error(&format!(
                    "Codex lifecycle hook input JSON invalid: {err}"
                )),
            )));
            crate::telemetry_emit::emit_hook_fired(
                &telemetry_event,
                crate::telemetry_emit::hook_action_from_optional_output(out.as_ref().ok().and_then(|v| v.as_ref())),
            );
            crate::hook_timing::emit_hook_timing_line(&telemetry_event);
            return out;
        }
        Err(err) => {
            crate::telemetry_emit::emit_hook_fired(&telemetry_event, "error");
            crate::hook_timing::emit_hook_timing_line(&telemetry_event);
            return Err(err);
        }
    };
    if let Some(event_name) = codex_lifecycle_event_name(command) {
        if payload.is_object()
            && payload.get("hook_event_name").is_none()
            && payload.get("event").is_none()
        {
            payload["hook_event_name"] = json!(event_name);
        }
    }
    let result = match canonical {
        "pre-tool-use" => Ok(attach_codex_hook_observation(run_codex_pre_tool_use(
            repo_root, &payload,
        )?)),
        "contract-guard" => Ok(attach_codex_hook_observation(run_codex_contract_guard(
            repo_root, &payload,
        )?)),
        "lifecycle-context" => Ok(attach_codex_hook_observation(
            run_codex_lifecycle_context_hook(repo_root, &payload)?,
        )),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    };
    match &result {
        Ok(output) => crate::telemetry_emit::emit_hook_fired(
            &telemetry_event,
            crate::telemetry_emit::hook_action_from_optional_output(output.as_ref()),
        ),
        Err(_) => crate::telemetry_emit::emit_hook_fired(&telemetry_event, "error"),
    }
    crate::hook_timing::emit_hook_timing_line(&telemetry_event);
    result
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn run_codex_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    run_pre_tool_use(repo_root, payload)
}

fn run_codex_contract_guard(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let envelope = build_framework_contract_summary_envelope(repo_root)?;
    let summary = envelope
        .get("contract_summary")
        .ok_or_else(|| "framework contract summary missing contract_summary".to_string())?;
    let drift_flags = detect_contract_drift(summary, payload);
    let explicit_update = payload_bool(payload, "contract_update_intent")
        || payload_bool(payload, "allow_contract_update")
        || payload_bool(payload, "explicit_contract_update");
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let decision = if !drift_flags.is_empty() && !explicit_update {
        "block"
    } else {
        "approve"
    };
    let reason = if drift_flags.is_empty() {
        "contract guard passed; no drift detected".to_string()
    } else if explicit_update {
        format!(
            "contract guard observed drift but explicit update intent was provided: {}",
            drift_flags.join(", ")
        )
    } else {
        format!(
            "contract guard blocked drift without explicit contract update intent: {}",
            drift_flags.join(", ")
        )
    };
    let mut response = json!({
        "decision": decision,
        "authority": CODEX_HOOK_AUTHORITY,
        "contract_guard": {
            "schema_version": "router-rs-codex-contract-guard-v1",
            "live_contract_digest": live_digest,
            "drift_flags": drift_flags,
            "explicit_contract_update": explicit_update,
            "prompt_lines": summary.get("prompt_lines").cloned().unwrap_or(Value::Array(Vec::new())),
            "reason": reason,
        },
    });
    if decision == "block" {
        response["hookSpecificOutput"] = json!({
            "hookEventName": "ContractGuard",
            "permissionDecision": "deny",
            "permissionDecisionReason": response["contract_guard"]["reason"].clone(),
        });
    }
    Ok(Some(response))
}

fn canonical_codex_audit_command(command: &str) -> Result<&'static str, String> {
    if let Some(event_name) = codex_lifecycle_event_name(command) {
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

fn codex_lifecycle_event_name(command: &str) -> Option<&'static str> {
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

fn detect_contract_drift(summary: &Value, payload: &Value) -> Vec<String> {
    let mut flags = Vec::new();
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(expected) = payload_string(payload, "expected_contract_digest")
        .or_else(|| payload_string(payload, "contract_digest"))
    {
        let expected = expected.strip_prefix("sha256:").unwrap_or(&expected);
        if !expected.is_empty() && expected != live_digest {
            flags.push("contract_digest_drift".to_string());
        }
    }

    let live_owner = summary
        .get("primary_owner")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(proposed_owner) = payload_string(payload, "proposed_primary_owner")
        .or_else(|| payload_string(payload, "primary_owner"))
    {
        if !live_owner.is_empty() && proposed_owner != live_owner {
            flags.push("owner_drift".to_string());
        }
    }

    let contract_active = summary
        .get("contract_guard")
        .and_then(|guard| guard.get("contract_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if contract_active {
        let live_task = summary
            .get("continuity")
            .and_then(|continuity| continuity.get("task"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(proposed_task) =
            payload_string(payload, "proposed_task").or_else(|| payload_string(payload, "task"))
        {
            if !live_task.is_empty() && proposed_task != live_task {
                flags.push("scope_drift".to_string());
            }
        }

        let live_goal = scalar_contract_text(summary.get("goal"));
        if let Some(proposed_goal) =
            payload_string(payload, "proposed_goal").or_else(|| payload_string(payload, "goal"))
        {
            if !live_goal.is_empty() && proposed_goal != live_goal {
                flags.push("scope_drift".to_string());
            }
        }

        let live_evidence = string_array(summary.get("evidence_required"));
        let proposed_evidence_exists = payload.get("proposed_evidence_required").is_some();
        let proposed_evidence = string_array(payload.get("proposed_evidence_required"));
        let drops_evidence = payload_bool(payload, "drops_evidence_required");
        let evidence_changed = proposed_evidence_exists
            && normalized_string_set(&proposed_evidence) != normalized_string_set(&live_evidence);
        if (drops_evidence && !live_evidence.is_empty()) || evidence_changed {
            flags.push("evidence_drift".to_string());
        }
    }

    flags.sort();
    flags.dedup();
    flags
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn scalar_contract_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => String::new(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalized_string_set(values: &[String]) -> Vec<String> {
    let mut deduped = HashSet::new();
    let mut normalized = values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            let lower = item.to_ascii_lowercase();
            deduped.insert(lower.clone()).then_some(lower)
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
}

fn block_codex_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let mut rel_paths = HashSet::new();
    for path in iter_payload_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    for path in rel_paths.iter().cloned().collect::<Vec<_>>() {
        if classify_protected_generated_path(&path).is_some() {
            let message = pre_tool_use_message(&path);
            return Ok(block_codex_pre_tool_use(message));
        }
    }
    if let Some(path) = bash_generated_write_target(payload) {
        let message = pre_tool_use_message(&path);
        return Ok(block_codex_pre_tool_use(message));
    }
    Ok(None)
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut stdin = io::stdin().lock();
    let input = read_codex_stdin_limited(&mut stdin)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).map_err(|err| format!("stdin_json_invalid: {err}"))
}

fn codex_lifecycle_input_error(message: &str) -> Value {
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

fn read_codex_stdin_limited<R: Read>(reader: &mut R) -> Result<String, String> {
    const LIMIT: u64 = 4 * 1024 * 1024;
    let mut input = String::new();
    let mut limited = reader.take(LIMIT);
    limited.read_to_string(&mut input).map_err(|err| {
        let msg = err.to_string();
        let lower = msg.to_ascii_lowercase();
        // `Read::read_to_string` UTF-8 failures vary by Rust/stdlib wording and punctuation
        // (e.g. hyphen vs minus); normalize any obvious decode error to a stable hook token.
        if matches!(err.kind(), std::io::ErrorKind::InvalidData)
            || lower.contains("utf-8")
            || lower.contains("utf8")
            || lower.contains("utf")
        {
            return "stdin_invalid_utf8".to_string();
        }
        msg
    })?;
    if limited.limit() == 0 {
        let inner = limited.into_inner();
        let mut probe = [0u8; 1];
        if inner.read(&mut probe).map_err(|err| err.to_string())? > 0 {
            return Err("stdin payload exceeds 4 MiB limit".to_string());
        }
    }
    Ok(input)
}

fn iter_candidate_paths(payload: &Value) -> Vec<String> {
    let mut candidates = Vec::new();
    for key in [
        "file_path",
        "changed_path",
        "path",
        "config_path",
        "target_path",
    ] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                candidates.push(normalized);
            }
        }
    }
    if let Some(items) = payload.get("changed_files").and_then(Value::as_array) {
        for item in items {
            if let Some(text) = item.as_str() {
                let normalized = text.replace('\\', "/");
                if !normalized.is_empty() {
                    candidates.push(normalized);
                }
            }
        }
    }
    candidates
}

fn iter_payload_paths(payload: &Value) -> Vec<String> {
    let mut candidates = iter_candidate_paths(payload);
    if let Some(tool_input) = payload.get("tool_input") {
        candidates.extend(iter_candidate_paths(tool_input));
    }
    candidates
}

fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        if let Ok(rel) = candidate
            .canonicalize()
            .unwrap_or(candidate.clone())
            .strip_prefix(
                repo_root
                    .canonicalize()
                    .unwrap_or_else(|_| repo_root.to_path_buf()),
            )
        {
            return normalize_repo_relative_path(&rel.to_string_lossy());
        }
    }
    normalize_repo_relative_path(path)
}

fn normalize_repo_relative_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else {
                    parts.push(part);
                }
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn classify_protected_generated_path(path: &str) -> Option<&'static str> {
    let normalized = normalize_repo_relative_path(path);
    if protected_generated_paths().contains(&normalized.as_str()) {
        return Some("generated_file");
    }
    if PROTECTED_GENERATED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return Some("generated_file");
    }
    None
}

fn pre_tool_use_message(path: &str) -> String {
    format!(
        "[codex-pre-tool-use] blocked direct edits to generated Codex agent surface {path}; rerun `{}` instead."
        ,
        HOST_ENTRYPOINT_SYNC_HINT
    )
}

fn bash_generated_write_target(payload: &Value) -> Option<String> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in protected_generated_paths() {
            if bash_segment_mentions_generated_path(&segment, hint)
                && (looks_mutating || bash_segment_redirects_to_hint(&segment, hint))
            {
                return Some(hint.to_string());
            }
        }
    }
    None
}

fn split_bash_segments(command: &str) -> Vec<String> {
    let chars = command.chars().collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut idx = 0usize;

    while idx < chars.len() {
        let current = chars[idx];
        let next = chars.get(idx + 1).copied();
        let prev = if idx > 0 { Some(chars[idx - 1]) } else { None };
        let mut separator_len = 0usize;

        if current == ';' {
            separator_len = 1;
        } else if next == Some(current) && matches!(current, '&' | '|') {
            separator_len = 2;
        } else if current == '|' && prev != Some('>') {
            separator_len = 1;
        }

        if separator_len > 0 {
            let segment = chars[start..idx].iter().collect::<String>();
            let trimmed = segment.trim();
            if !trimmed.is_empty() {
                segments.push(trimmed.to_string());
            }
            idx += separator_len;
            start = idx;
            continue;
        }

        idx += 1;
    }

    let tail = chars[start..].iter().collect::<String>();
    let trimmed = tail.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    if segments.is_empty() {
        vec![command.trim().to_string()]
    } else {
        segments
    }
}

static MUTATING_COMMAND_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate)\b",
        r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b",
        r"^\s*git\s+(checkout\s+--|restore\b)",
        r"\bsed\s+-i\b",
        r"\bperl\s+-pi\b",
        r"\bpython3?\s+-c\b",
        r"\bnode\s+-e\b",
        r"\bruby\s+-e\b",
        r"\btee\b",
        r"\bdd\b",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

fn bash_command_looks_mutating(command: &str) -> bool {
    MUTATING_COMMAND_PATTERNS.iter().any(|re| re.is_match(command))
}

fn bash_segment_mentions_generated_path(segment: &str, hint: &str) -> bool {
    segment
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | ';' | '&' | '|'))
        .map(|token| token.trim_start_matches('>').trim_start_matches("of="))
        .any(|token| normalize_repo_relative_path(token) == hint)
}

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    thread_local! {
        static HINT_RE_CACHE: std::cell::RefCell<std::collections::HashMap<String, [Regex; 3]>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    HINT_RE_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        let regexes = map.entry(hint.to_string()).or_insert_with(|| {
            let escaped = regex::escape(hint);
            let p1 = format!(r#"(>>?|>\|)\s*['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            let p2 = format!(r#"\btee\b(?:\s+-a)?\s+['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            let p3 = format!(r#"\bdd\b[^\n;&|]*\bof=['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#);
            [
                Regex::new(&p1).unwrap(),
                Regex::new(&p2).unwrap(),
                Regex::new(&p3).unwrap(),
            ]
        });
        regexes.iter().any(|re| re.is_match(segment))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;

    #[test]
    fn codex_first_nonempty_prompt_line_skips_leading_blank_lines() {
        assert_eq!(
            super::codex_first_nonempty_prompt_line("\n  \nreal task\nmore"),
            "real task"
        );
    }

    #[test]
    fn protected_generated_paths_match_lexical_variants() {
        assert_eq!(normalize_repo_relative_path("./AGENTS.md"), "AGENTS.md");
        assert_eq!(
            normalize_repo_relative_path(".codex/../.codex/host_entrypoints_sync_manifest.json"),
            ".codex/host_entrypoints_sync_manifest.json"
        );
        assert!(classify_protected_generated_path("./AGENTS.md").is_some());
        assert!(classify_protected_generated_path(
            ".codex/../.codex/host_entrypoints_sync_manifest.json"
        )
        .is_some());
        assert!(classify_protected_generated_path("./.codex/prompts/gitx.md").is_none());
    }

    #[test]
    fn pre_tool_use_blocks_normalized_direct_paths() {
        let payload = json!({"tool_input": {"file_path": "./AGENTS.md"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({"tool_input": {"file_path": ".codex/../.codex/host_entrypoints_sync_manifest.json"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({"tool_input": {"file_path": ".codex/../.codex/prompts/autopilot.md"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());
    }

    #[test]
    fn pre_tool_use_blocks_normalized_bash_write_targets() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x > ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x | tee .codex/../.codex/host_entrypoints_sync_manifest.json"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x | tee .codex/prompts/gitx.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x >| ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
    }

    #[test]
    fn pre_tool_use_allows_read_only_bash_commands_on_protected_paths() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cat ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg contract_digest .codex/host_entrypoints_sync_manifest.json"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());
    }

    mod install_codex_cli_hooks_tests {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::SystemTime;

        static INSTALL_SEQ: AtomicU64 = AtomicU64::new(0);

        fn fresh_path(label: &str) -> PathBuf {
            let base = std::env::temp_dir().join(format!(
                "install-codex-cli-hooks-{}-{}-{}",
                label,
                std::process::id(),
                INSTALL_SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir_all(&base).unwrap();
            base
        }

        fn run_install(codex_home: &Path, repo_root: &Path, mode: InstallMode) -> Value {
            install_codex_cli_hooks(codex_home, repo_root, mode).unwrap()
        }

        fn install_hook_commands(repo_root: &Path) -> BTreeMap<String, String> {
            INSTALL_EVENTS
                .iter()
                .map(|event| {
                    (
                        (*event).to_string(),
                        build_install_hook_command(repo_root, event),
                    )
                })
                .collect()
        }

        #[test]
        fn empty_codex_home_creates_config_and_hooks() {
            let root = fresh_path("empty");
            let codex_home = root.join("new-codex-home");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let config_path = codex_home.join("config.toml");
            let hooks_path = codex_home.join("hooks.json");
            assert!(config_path.exists());
            assert!(hooks_path.exists());
            assert_eq!(payload["config_toml"]["status"].as_str(), Some("created"));
            assert_eq!(payload["hooks_json"]["status"].as_str(), Some("created"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_config_with_features_block_preserves_other_keys() {
            let root = fresh_path("features-preserve");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(
                codex_home.join("config.toml"),
                "[features]\nother_flag = true\n",
            )
            .unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert!(text.contains("other_flag = true"));
            assert!(text.contains("hooks = true"));
            assert!(!text.contains("codex_hooks"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_config_with_codex_hooks_false_under_features_replaces() {
            let root = fresh_path("replace");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(
                codex_home.join("config.toml"),
                "[features]\ncodex_hooks = false\n",
            )
            .unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert_eq!(text, "[features]\nhooks = true\n");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_config_with_codex_hooks_under_other_section_untouched() {
            let root = fresh_path("other-section");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(
                codex_home.join("config.toml"),
                "[custom]\ncodex_hooks = false\n[features]\nother = 1\n",
            )
            .unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert!(text.contains("[custom]\ncodex_hooks = false"));
            assert!(text.contains("[features]\nother = 1\nhooks = true"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn config_without_features_appends_section() {
            let root = fresh_path("append-features");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[custom]\nvalue = 1\n").unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert!(text.ends_with("[features]\nhooks = true\n"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_hooks_json_preserves_existing_entry() {
            let root = fresh_path("preserve-hooks");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(
                codex_home.join("hooks.json"),
                "{\n  \"hooks\": {\n    \"Stop\": [\n      {\"hooks\": [{\"type\": \"command\", \"command\": \"echo keep\"}]}\n    ]\n  }\n}\n",
            )
            .unwrap();
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert!(text.contains("echo keep"));
            assert!(
                payload["hooks_json"]["preserved_existing_entries"]
                    .as_u64()
                    .unwrap()
                    >= 1
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_removes_legacy_python_codex_hooks() {
            let root = fresh_path("remove-legacy-python");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(
                codex_home.join("hooks.json"),
                r#"{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/usr/bin/env python3 \"/Users/joe/Developer/skill/.codex/hooks/review_subagent_gate.py\"",
            "timeout": 10
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {"type": "command", "command": "echo keep"},
          {"type": "command", "command": "python3 review_subagent_gate.py"}
        ]
      }
    ]
  }
}
"#,
            )
            .unwrap();
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert!(!text.contains("review_subagent_gate.py"));
            assert!(text.contains("echo keep"));
            assert!(text.contains("codex-router-rs-hook.sh"));
            assert!(text.contains("UserPromptSubmit"));
            assert_eq!(
                payload["hooks_json"]["removed_legacy_entries"].as_u64(),
                Some(2)
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn idempotent_install() {
            let root = fresh_path("idempotent");
            let codex_home = root.join("codex");
            let first = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let second = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            assert_eq!(first["config_toml"]["status"].as_str(), Some("created"));
            assert_eq!(second["config_toml"]["status"].as_str(), Some("unchanged"));
            assert_eq!(second["hooks_json"]["status"].as_str(), Some("unchanged"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn check_mode_does_not_write() {
            let root = fresh_path("check-mode");
            let codex_home = root.join("codex-check-do-not-write");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Check);
            assert_eq!(
                payload["config_toml"]["status"].as_str(),
                Some("would-create")
            );
            assert_eq!(
                payload["hooks_json"]["status"].as_str(),
                Some("would-create")
            );
            assert!(!codex_home.join("config.toml").exists());
            assert!(!codex_home.join("hooks.json").exists());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn hook_command_format_pure_router_rs_binary() {
            let repo_root = Path::new("/Users/joe/Developer/skill");
            let stop_command = build_install_hook_command(repo_root, "Stop");
            assert!(stop_command.contains("codex-router-rs-hook.sh\" Stop"));
            assert!(!stop_command.contains("codex hook --event=Stop"));
            assert!(!stop_command.contains("/Users/joe/Developer/skill"));
            let pre_tool_command = build_install_hook_command(repo_root, "PreToolUse");
            assert!(pre_tool_command.contains("codex-router-rs-hook.sh\" PreToolUse"));
            assert!(!pre_tool_command.contains("codex hook pre-tool-use"));
        }

        #[test]
        fn hook_command_ignores_repo_root_shell_content() {
            let repo_root = Path::new("/tmp/repo-with-'quote");
            let command = build_install_hook_command(repo_root, "UserPromptSubmit");
            assert!(!command.contains("/tmp/repo-with-"));
            assert!(command.contains("git rev-parse --show-toplevel"));
            assert!(command.contains("codex-router-rs-hook.sh"));
            let status = Command::new("bash")
                .arg("-n")
                .arg("-c")
                .arg(&command)
                .status()
                .unwrap();
            assert!(status.success());
        }

        #[test]
        fn apply_creates_backup_when_hooks_existed() {
            let root = fresh_path("backup");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
            let before = fs::metadata(codex_home.join("hooks.json"))
                .unwrap()
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let backup = payload["hooks_json"]["backup_path"]
                .as_str()
                .map(PathBuf::from)
                .unwrap();
            assert!(backup.exists());
            let after = fs::metadata(codex_home.join("hooks.json"))
                .unwrap()
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            assert!(after >= before);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_payload_contains_projection_version_and_digest() {
            let root = fresh_path("payload-meta");
            let codex_home = root.join("codex");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            assert_eq!(
                payload["projection_version"].as_str(),
                Some(ROUTER_RS_HOOK_PROJECTION_VERSION)
            );
            assert!(payload["command_digest"]
                .as_str()
                .is_some_and(|v| v.len() == 64));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_writes_manifest_file_with_version() {
            let root = fresh_path("manifest");
            let codex_home = root.join("codex");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let manifest_path = codex_home.join(".router-rs-install.manifest.json");
            let manifest_text = fs::read_to_string(manifest_path).unwrap();
            let manifest: Value = serde_json::from_str(&manifest_text).unwrap();
            assert_eq!(
                manifest["projection_version"].as_str(),
                Some(ROUTER_RS_HOOK_PROJECTION_VERSION)
            );
            assert_eq!(manifest["command_digest"], payload["command_digest"]);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_backup_failure_bubbles_error() {
            let root = fresh_path("backup-failure");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o500)).unwrap();
            let before = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(result.is_err());
            let after = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert_eq!(before, after);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_write_failure_restores_backup() {
            let root = fresh_path("write-failure");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
            let before = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.set(true));
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.set(false));
            assert!(result.is_err());
            let after = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert_eq!(before, after);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_permission_denied_fails_cleanly() {
            let root = fresh_path("permission-denied");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o500)).unwrap();
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(result.is_err());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_symlink_target_handled_safely() {
            let root = fresh_path("symlink-hooks");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            let target = root.join("actual-hooks.json");
            fs::write(&target, "{\"hooks\":{}}\n").unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, codex_home.join("hooks.json")).unwrap();
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            assert!(result.is_err());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_invalid_root_returns_error() {
            let result = merge_hooks_json(Some(json!([])), &install_hook_commands(Path::new(".")));
            assert!(result
                .err()
                .unwrap_or_default()
                .contains("root type: expected object"));
        }

        #[test]
        fn install_hooks_invalid_hooks_field_returns_error() {
            let result = merge_hooks_json(
                Some(json!({"hooks":"not-an-object"})),
                &install_hook_commands(Path::new(".")),
            );
            assert!(result
                .err()
                .unwrap_or_default()
                .contains("`hooks` must be an object"));
        }

        #[test]
        fn install_hooks_invalid_event_array_returns_error() {
            let result = merge_hooks_json(
                Some(json!({"hooks":{"Stop":{"x":1}}})),
                &install_hook_commands(Path::new(".")),
            );
            assert!(result
                .err()
                .unwrap_or_default()
                .contains("hooks.Stop must be an array"));
        }

        #[test]
        fn atomic_write_completes_normally_with_fsync() {
            let root = fresh_path("atomic-fsync");
            let output = root.join("file.txt");
            write_atomic_text(&output, "hello").unwrap();
            assert_eq!(fs::read_to_string(output).unwrap(), "hello");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn codex_hook_rejects_oversized_stdin() {
            let large = vec![b'a'; 5 * 1024 * 1024];
            let mut cursor = std::io::Cursor::new(large);
            let err = read_codex_stdin_limited(&mut cursor).unwrap_err();
            assert!(err.contains("exceeds 4 MiB"));
        }

        #[test]
        fn codex_hook_rejects_invalid_utf8_stdin() {
            let bytes = vec![0xff, 0xfe, 0xfd];
            let mut cursor = std::io::Cursor::new(bytes);
            let err = read_codex_stdin_limited(&mut cursor).unwrap_err();
            assert_eq!(err, "stdin_invalid_utf8");
        }

        #[test]
        fn codex_hook_rejects_truncated_utf8_sequence_stdin() {
            let mut buf = vec![b'a'; 64];
            buf.push(0x80);
            let mut cursor = std::io::Cursor::new(buf);
            let err = read_codex_stdin_limited(&mut cursor).unwrap_err();
            assert_eq!(err, "stdin_invalid_utf8");
        }
    }

    mod lifecycle_context_tests {
        use super::*;
        use serde_json::json;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQ: AtomicU64 = AtomicU64::new(0);

        fn env_lock() -> crate::test_env_sync::ProcessEnvLockGuard {
            crate::test_env_sync::process_env_lock()
        }

        fn fresh_repo() -> std::path::PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "codex-lifecycle-context-test-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(dir.join(".codex/hook-state")).unwrap();
            dir
        }

        fn run_gate(repo: &std::path::Path, payload: &Value) -> Result<Option<Value>, String> {
            let _g = env_lock();
            run_codex_review_subagent_gate(repo, payload)
        }

        const TEST_COMPACT_FINDING: &str = "[P1] core/router-rs/src/hosts/codex_hooks/mod.rs:1 — wave-2 compact gate clear evidence line";

        #[test]
        fn operator_inject_off_skips_session_start_additional_context() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_OPERATOR_INJECT");
            std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
            let repo = fresh_repo();
            let out =
                super::super::handle_codex_session_start(&repo, &json!({"source": "startup"}));
            assert!(
                out.is_none(),
                "advisory SessionStart must honor ROUTER_RS_OPERATOR_INJECT kill-switch: {out:?}"
            );
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
                None => std::env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
            }
        }

        #[test]
        fn operator_inject_off_skips_user_prompt_submit_additional_context() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_OPERATOR_INJECT");
            std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
            let repo = fresh_repo();
            let evt = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-inject-off-ups",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let out = super::super::handle_codex_userpromptsubmit(&repo, &evt);
            assert!(
                out.is_none(),
                "advisory UserPromptSubmit must honor ROUTER_RS_OPERATOR_INJECT kill-switch: {out:?}"
            );
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
                None => std::env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
            }
        }

        #[test]
        fn user_prompt_submit_injects_paper_prose_hook_by_default() {
            let _g = env_lock();
            let prior_hook = std::env::var_os("ROUTER_RS_CODEX_PAPER_PROSE_HOOK");
            std::env::remove_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK");
            let repo = fresh_repo();
            let evt = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"prose-ups-default",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"SCI润色 abstract"
            });
            let out = super::super::handle_codex_userpromptsubmit(&repo, &evt);
            let ctx = out
                .as_ref()
                .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
                .unwrap_or_default();
            assert!(
                ctx.contains("PAPER_PROSE_QUALITY_HOOK"),
                "expected prose hook in UPS context: {ctx}"
            );
            match prior_hook {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK", v),
                None => std::env::remove_var("ROUTER_RS_CODEX_PAPER_PROSE_HOOK"),
            }
        }

        #[test]
        fn user_prompt_submit_review_emits_subagent_gate_context() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-1",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review全仓找bug"
            });
            let out = run_gate(&repo, &payload).unwrap();
            let ctx = out
                .as_ref()
                .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
                .unwrap_or_default();
            assert!(
                ctx.contains("配对审稿") || ctx.contains("fork_context"),
                "spawn-first nudge: {ctx}"
            );
            assert!(ctx.contains("fork_context=false"));
            assert!(ctx.contains("general-purpose") || ctx.contains("best-of-n-runner"));
            if !ctx.is_empty() {
                assert!(ctx.len() <= codex_additional_context_max_bytes());
            }
            let state = codex_load_state(&repo, &payload).unwrap().unwrap();
            assert_eq!(state.seq, 1);
            assert!(state.review_gate.review_required);
        }

        #[test]
        fn user_prompt_submit_narrow_path_skips_review_arm() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-narrow",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"review ./README.md"
            });
            let out = run_gate(&repo, &payload).unwrap();
            assert!(
                out.is_none(),
                "narrow single-path review must not arm gate: {out:?}"
            );
            let armed = codex_load_state(&repo, &payload)
                .ok()
                .flatten()
                .map(|s| s.review_gate.review_required)
                .unwrap_or(false);
            assert!(!armed, "narrow prompt should not set review_required");
        }

        #[test]
        fn user_prompt_submit_with_override_does_not_emit() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-ovr",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review全仓找bug，不要用子代理"
            });
            let out = run_gate(&repo, &payload).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn additional_context_is_deduped_and_capped() {
            let duplicate = "Codex live state: one".to_string();
            let long_line = "x".repeat(codex_additional_context_max_bytes());
            let ctx = codex_compact_contexts(vec![
                duplicate.clone(),
                duplicate,
                long_line.clone(),
                long_line,
            ])
            .unwrap();
            assert!(ctx.len() <= codex_additional_context_max_bytes());
            assert_eq!(ctx.matches("Codex live state: one").count(), 1);
        }

        #[test]
        fn session_start_compact_context_under_small_budget_without_digest() {
            let repo = fresh_repo();
            let task_id = "session-priority";
            fs::create_dir_all(repo.join("artifacts/current").join(task_id)).expect("mkdir task");
            fs::write(
                repo.join("artifacts/current/active_task.json"),
                format!(r#"{{"task_id":"{task_id}"}}"#),
            )
            .expect("write active");
            fs::write(
                repo.join("artifacts/current").join(task_id).join("GOAL_STATE.json"),
                r#"{"goal":"keep the active goal visible before any static context","status":"running","drive_until_done":true,"done_when":["done"],"validation_commands":["cargo test -q"]}"#,
            )
            .expect("write goal");
            fs::write(
                repo.join("artifacts/current/SESSION_SUMMARY.md"),
                "very long continuity line ".repeat(80),
            )
            .expect("write summary");

            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
            std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
            let out = handle_codex_session_start(&repo, &json!({"source":"startup"}))
                .expect("session start output");
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
            let ctx = out["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .expect("additionalContext");
            assert!(!ctx.contains("Continuity digest:"), "{ctx}");
            assert!(ctx.contains("Repo:"), "{ctx}");
            assert!(!ctx.contains("Goal: running"), "{ctx}");
            assert!(ctx.len() <= 256, "len={} ctx={ctx:?}", ctx.len());
        }

        #[test]
        fn post_tool_use_with_subagent_marks_seen_without_explore_counting_deep_independent() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore","fork_context":false}
            });
            let out = run_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(state.review_subagent_seen);
            assert!(
                !state.review_gate.independent_reviewer_seen,
                "explore must not satisfy Codex independent deep-review bar"
            );
            assert!(state.generic_subagent_seen);
            assert!(state.review_lane_seen);
            assert!(!state.parallel_lane_seen);
            assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
        }

        #[test]
        fn post_tool_general_purpose_fork_false_counts_deep_independent() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2gp",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2gp",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let out = run_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(state.review_gate.independent_reviewer_seen);
            assert!(state.review_lane_seen);
        }

        #[test]
        fn post_tool_review_lane_fork_false_does_not_count_deep_independent() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2rev",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2rev",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"review","fork_context":false}
            });
            let out = run_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(
                !state.review_gate.independent_reviewer_seen,
                "review subagent_type is Claude-only; must not satisfy Codex reviewer_lanes"
            );
        }

        #[test]
        fn post_tool_use_without_subagent_type_marks_generic_and_untyped_label() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2b",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2b",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"prompt":"no type field"}
            });
            let out = run_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(state.generic_subagent_seen);
            assert!(state.review_subagent_seen);
            assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#untyped"));
            assert!(!state.review_lane_seen);
            assert!(!state.parallel_lane_seen);
        }

        #[test]
        fn saw_subagent_codex_accepts_whitelisted_tool_without_recognized_type() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"prompt":"missing type"})
            ));
        }

        #[test]
        fn delegation_stop_unblocks_after_worker_subagent() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-6c",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"前端后端测试并行推进"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-6c",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"worker"}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-6c",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn stop_blocks_when_hook_state_corrupt() {
            let _guard = env_lock();
            std::env::set_var("ROUTER_RS_HOOK_STATE_FAIL_OPEN", "true");
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"stop-corrupt-1",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"x"
            });
            let path = super::super::codex_state_path(&repo, &payload);
            fs::write(&path, b"{not json").unwrap();
            // B-3: corrupted state auto-recovers (backup .bak + reset to fresh)
            let out = super::super::handle_codex_stop(&repo, &payload);
            // Stop with no review_required proceeds normally (None = allow)
            assert!(out.is_none(), "corrupted state should auto-recover, not block: {out:?}");
            // Verify backup was created
            let bak_path = path.with_extension("json.bak");
            assert!(bak_path.exists(), "corrupt file should be backed up to .bak");
        }

        #[test]
        fn session_key_without_stable_identifier_is_deterministic() {
            let _g = env_lock();
            std::env::remove_var("CODEX_SESSION_ID");
            std::env::remove_var("CODEX_CONVERSATION_ID");
            std::env::remove_var("ROUTER_RS_CODEX_HOOK_STATE_SALT");
            let repo = fresh_repo();
            let event = json!({"cwd": repo.to_string_lossy()});
            let k1 = super::super::codex_session_key(&repo, &event);
            let k2 = super::super::codex_session_key(&repo, &event);
            assert_eq!(k1, k2, "fallback keys must alias the same hook-state file");
            assert_eq!(k1.len(), 32);
        }

        #[test]
        fn codex_session_key_differs_by_payload_session_when_strict_off() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY");
            std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "0");
            std::env::remove_var("CODEX_SESSION_ID");
            std::env::remove_var("CODEX_CONVERSATION_ID");
            let repo = fresh_repo();
            let cwd = repo.to_string_lossy().to_string();
            let k1 = super::super::codex_session_key(
                &repo,
                &json!({"session_id":"sess-a","cwd":cwd}),
            );
            let k2 = super::super::codex_session_key(
                &repo,
                &json!({"session_id":"sess-b","cwd":cwd}),
            );
            assert_ne!(k1, k2, "payload session_id must isolate hook-state when strict off");
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", v),
                None => std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY"),
            }
        }

        #[test]
        fn delegation_stop_does_not_block_when_only_explore_subagent_observed() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-6b",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"前端后端测试并行推进"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-6b",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-6b",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn additional_context_truncates_on_newline_preference_under_small_budget() {
            // codex_additional_context_max_bytes clamps to [256, 8192]; use the
            // floor so the assertions exercise the real budget rather than a
            // value that the clamp silently rewrites.
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
            std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
            let line1 = format!("{}{}", "A".repeat(24), ": L1");
            let line2 = format!("{}{}", "C".repeat(24), ": L2");
            let line3 = "B".repeat(240);
            let ctx = codex_compact_contexts(vec![format!("{line1}\n{line2}\n{line3}")]).unwrap();
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
            assert!(ctx.ends_with("..."));
            assert!(
                ctx.matches('\n').count() >= 1,
                "expected multiple lines before ellipsis when budget allows: {ctx:?}"
            );
            assert!(ctx.len() <= 256);
        }

        #[test]
        fn codex_compact_contexts_dedup_requires_exact_trim_match() {
            let a = "Repo: /path/A";
            let b = "repo: /path/B";
            let ctx = codex_compact_contexts(vec![a.to_string(), b.to_string()]).expect("ctx");
            assert!(
                ctx.contains(a),
                "distinct lines must not merge on ASCII case: {ctx:?}"
            );
            assert!(
                ctx.contains(b),
                "distinct lines must not merge on ASCII case: {ctx:?}"
            );
        }

        /// Multi-segment `codex_compact_contexts` join order is preserved when the
        /// combined string is truncated (SessionStart budget). Complements
        /// `additional_context_truncates_on_newline_preference_under_small_budget`
        /// (single blob + newline preference inside one segment).
        #[test]
        fn codex_compact_contexts_preserves_join_order_under_small_budget() {
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
            std::env::set_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX", "256");
            let part1 = "CODEX_JOIN_ORDER_MARK_FIRST:alpha";
            let part2 = "CODEX_JOIN_ORDER_MARK_SECOND:beta";
            let part3 = format!("CODEX_JOIN_ORDER_MARK_TAIL:{}", "Z".repeat(280));
            let ctx = codex_compact_contexts(vec![part1.to_string(), part2.to_string(), part3])
                .expect("expected combined contexts");
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX");
            std::env::remove_var("ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES");
            assert!(ctx.len() <= 256, "len={}", ctx.len());
            assert!(ctx.ends_with("..."));
            assert!(
                ctx.contains("CODEX_JOIN_ORDER_MARK_FIRST"),
                "first joined segment should survive truncation: {ctx:?}"
            );
            assert!(
                ctx.contains("CODEX_JOIN_ORDER_MARK_SECOND"),
                "second joined segment should appear before tail is cut: {ctx:?}"
            );
            let pos_first = ctx.find("CODEX_JOIN_ORDER_MARK_FIRST").expect("first mark");
            let pos_second = ctx
                .find("CODEX_JOIN_ORDER_MARK_SECOND")
                .expect("second mark");
            assert!(
                pos_first < pos_second,
                "join order should be preserved in truncated output: {ctx:?}"
            );
        }

        #[test]
        fn saw_subagent_codex_accepts_subagent_type_field() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"subagent_type":"explore"})
            ));
        }

        #[test]
        fn saw_subagent_codex_accepts_agent_type_field() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"agent_type":"ci-investigator"})
            ));
        }

        #[test]
        fn saw_subagent_codex_accepts_native_codex_agent_types() {
            for agent_type in ["default", "explorer", "worker"] {
                assert!(
                    saw_subagent_codex("functions.spawn_agent", &json!({"agent_type":agent_type})),
                    "expected native Codex agent_type={agent_type} to count as a subagent"
                );
            }
        }

        #[test]
        fn saw_subagent_codex_accepts_whitelisted_tool_even_when_type_unrecognized() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"subagent_type":"random-thing"})
            ));
        }

        #[test]
        fn post_tool_use_without_state_is_non_fatal() {
            let repo = fresh_repo();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2c",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore","fork_context":false}
            });
            let out = run_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post)
                .unwrap()
                .expect("lazy hook-state");
            assert!(state.generic_subagent_seen);
            assert!(
                !state.review_gate.independent_reviewer_seen,
                "explore must not satisfy deep independent reviewer ledger"
            );
        }

        #[test]
        fn post_tool_use_without_prior_state_persists_independent_deep_reviewer() {
            let _g = env_lock();
            let repo = fresh_repo();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-no-ups-deep",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review",
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let out = run_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().expect("state");
            assert!(state.review_gate.independent_reviewer_seen);
            assert!(
                state.review_gate.review_required,
                "deep PostTool with review prompt must arm review_required (B5 lazy bypass)"
            );
        }

        #[test]
        fn post_tool_deep_reviewer_without_review_prompt_does_not_arm_gate() {
            let repo = fresh_repo();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-no-review-arm",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"前端后端测试并行推进",
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let state = codex_load_state(&repo, &post).unwrap().expect("state");
            assert!(state.review_gate.independent_reviewer_seen);
            assert!(!state.review_gate.review_required, "non-review PostTool must not arm review_required");
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-no-review-arm",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(
                out.is_none(),
                "Stop must not block when review_required was never armed: {out:?}"
            );
        }

        #[test]
        fn lazy_post_tool_deep_reviewer_arms_gate_and_stop_blocks_without_compact() {
            let _g = env_lock();
            let repo = fresh_repo();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-lazy-stop-contract",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review",
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            assert!(run_gate(&repo, &post)
                .unwrap()
                .is_none());
            let loaded = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(loaded.review_gate.independent_reviewer_seen);
            assert!(loaded.review_gate.review_required, "deep PostTool must arm review_required");
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-lazy-stop-contract",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":""
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(
                out.is_none(),
                "independent reviewer evidence must clear Stop advisory: {out:?}"
            );
        }

        #[test]
        fn post_tool_use_observes_fork_context_on_event_root() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-event-fork",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-event-fork",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "fork_context": false,
                "tool_input":{"subagent_type":"general-purpose"}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-event-fork",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续",
                "response": TEST_COMPACT_FINDING
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(
                out.is_none(),
                "event-root fork_context should satisfy independent reviewer; out={out:?}"
            );
        }

        #[test]
        fn post_tool_use_with_invalid_state_blocks_fail_closed() {
            let _guard = env_lock();
            std::env::set_var("ROUTER_RS_HOOK_STATE_FAIL_OPEN", "true");
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2d",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let state_path = codex_state_path(&repo, &start);
            fs::write(&state_path, "{invalid").unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2d",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            // B-3: corrupted state auto-recovers; PostToolUse proceeds with fresh state
            let out = run_gate(&repo, &post).unwrap();
            // Fresh state with subagent_type=explore should trigger review gate
            // but not due to corruption block
            assert!(
                out.is_none() || out.as_ref().and_then(|v| v.get("decision")).and_then(Value::as_str) != Some("block"),
                "invalid hook-state should auto-recover on PostToolUse, not block: {out:?}"
            );
            // Verify backup was created
            let bak_path = state_path.with_extension("json.bak");
            assert!(bak_path.exists(), "corrupt file should be backed up to .bak");
        }

        #[test]
        fn stop_without_state_blocks_when_review_prompt_without_ups_evidence() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-3",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let out = run_gate(&repo, &payload).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
        }

        #[test]
        fn stop_without_state_does_not_block_when_no_text() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-4",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":""
            });
            let out = run_gate(&repo, &payload).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn stop_with_review_prompt_no_subagent_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-5",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-5",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
        }

        #[test]
        fn stop_with_review_prompt_shared_fork_subagent_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-5b",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-5b",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore","fork_context":true}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-5b",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
        }

        #[test]
        fn stop_with_review_prompt_missing_fork_context_subagent_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-5c",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-5c",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-5c",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
        }

        #[test]
        fn stop_with_delegation_prompt_does_not_block() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-6",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"前端后端测试并行推进"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-6",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn stop_with_subagent_seen_resets_state_after_general_purpose_deep_reviewer() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续",
                "response": TEST_COMPACT_FINDING
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &stop).unwrap().unwrap();
            assert_eq!(state.seq, 0);
            assert!(!state.review_subagent_seen);
            assert!(!state.review_gate.independent_reviewer_seen);
        }

        #[test]
        fn stop_blocks_after_posttool_without_compact_findings() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-wave2-post-only",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-wave2-post-only",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-wave2-post-only",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(
                out.is_none(),
                "independent reviewer PostTool must clear Stop advisory: {out:?}"
            );
        }

        #[test]
        fn stop_compact_alone_without_posttool_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-wave2-compact-only",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-wave2-compact-only",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续",
                "response": TEST_COMPACT_FINDING
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(
                msg.contains("CODEX_REVIEW_GATE"),
                "compact alone must not clear without countable posttool: {out:?}"
            );
        }

        #[test]
        fn stop_rg_clear_clears_review_gate() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-rg-clear",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-rg-clear",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"rg_clear"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none(), "rg_clear must clear codex review gate: {out:?}");
        }

        #[test]
        fn my_light_implementx_stop_suppresses_review_gate() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-my-light",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"/implementx run waves"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let armed = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-my-light",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &armed).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-my-light",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"/implementx finish"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(
                out.is_none(),
                "my-light must suppress CODEX_REVIEW_GATE on Stop: {out:?}"
            );
        }

        #[test]
        fn my_light_post_tool_suppress_clears_hook_state() {
            let repo = fresh_repo();
            let sid = "sm-my-light-post";
            let arm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &arm).unwrap();
            assert!(
                codex_load_state(&repo, &arm)
                    .unwrap()
                    .map(|s| s.review_gate.review_required)
                    .unwrap_or(false)
            );
            let my = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"/implementx run waves"
            });
            let _ = run_gate(&repo, &my).unwrap();
            assert!(
                !codex_load_state(&repo, &my)
                    .unwrap()
                    .map(|s| s.review_gate.review_required)
                    .unwrap_or(true),
                "my-light UPS must clear review_required"
            );
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"/implementx",
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            assert!(
                codex_load_state(&repo, &post)
                    .unwrap()
                    .map(|s| s.seq)
                    .unwrap_or(0)
                    == 0,
                "my-light PostTool (suppress) must clear hook-state"
            );
        }

        #[test]
        fn codex_review_gate_disable_env_skips_block() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
            crate::hook_common::set_test_my_light_override(Some(true));
            std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1");
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-disable",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-disable",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none(), "disable env must skip gate: {out:?}");
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v),
                None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE"),
            }
            crate::hook_common::set_test_my_light_override(None);
        }

        #[test]
        fn codex_review_gate_disable_clears_armed_state_on_userpromptsubmit() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
            let repo = fresh_repo();
            let arm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-disable-clear",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &arm).unwrap();
            assert!(
                codex_load_state(&repo, &arm)
                    .unwrap()
                    .map(|s| s.review_gate.review_required)
                    .unwrap_or(false)
            );
            crate::hook_common::set_test_my_light_override(Some(true));
            std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1");
            let ups_disable = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-disable-clear",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let _ = run_gate(&repo, &ups_disable).unwrap();
            let state = codex_load_state(&repo, &ups_disable).unwrap().unwrap();
            assert_eq!(state.seq, 0, "disable UPS must reset hook-state");
            assert!(!state.review_gate.review_required);
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v),
                None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE"),
            }
            crate::hook_common::set_test_my_light_override(None);
        }

        #[test]
        fn codex_review_gate_disable_clears_state_on_posttool() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE");
            let repo = fresh_repo();
            let arm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-disable-post",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &arm).unwrap();
            crate::hook_common::set_test_my_light_override(Some(true));
            std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", "1");
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-disable-post",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review",
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert_eq!(state.seq, 0, "disable PostTool must reset hook-state");
            assert!(!state.review_gate.review_required);
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE", v),
                None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_GATE_DISABLE"),
            }
            crate::hook_common::set_test_my_light_override(None);
        }

        #[test]
        fn post_tool_delegate_tool_does_not_count_deep_evidence() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-delegate",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-delegate",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Delegate",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(!state.review_gate.independent_reviewer_seen);
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-delegate",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(msg.contains("CODEX_REVIEW_GATE"));
        }

        #[test]
        fn post_tool_gp_missing_fork_codex_infer_off_blocks_at_stop() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
            std::env::set_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", "0");
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-infer-off",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-infer-off",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose"}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(!state.review_gate.independent_reviewer_seen);
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-infer-off",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(msg.contains("CODEX_REVIEW_GATE"));
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", v),
                None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE"),
            }
        }

        #[test]
        fn user_prompt_submit_review_and_implementx_suppresses_review_arming() {
            let _g = env_lock();
            let repo = fresh_repo();
            let sid = "sm-dual-review-implementx";
            let arm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review这个仓库"
            });
            let _ = run_gate(&repo, &arm).unwrap();
            let armed = codex_load_state(&repo, &arm).unwrap().unwrap();
            assert!(armed.review_gate.review_required, "review-only UPS should arm; got {armed:?}");
            let dual = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"请全面review这个仓库 /implementx 修复刚发现的问题"
            });
            let _ = run_gate(&repo, &dual).unwrap();
            let cleared = codex_load_state(&repo, &dual).unwrap().unwrap();
            assert!(
                !cleared.review_gate.review_required,
                "my-light goal drive must clear/disarm review on Codex UPS; got {cleared:?}"
            );
        }

        #[test]
        fn rearm_review_resets_codex_independent_evidence() {
            let _g = env_lock();
            let repo = fresh_repo();
            let sid = "sm-rearm-evidence";
            let arm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &arm).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let seeded = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(seeded.review_gate.independent_reviewer_seen);
            assert!(seeded.phase >= 2);
            let rearm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review全仓找bug"
            });
            let _ = run_gate(&repo, &rearm).unwrap();
            let reset = codex_load_state(&repo, &rearm).unwrap().unwrap();
            assert!(
                !reset.review_gate.independent_reviewer_seen,
                "re-arm review must reset PostTool evidence"
            );
            assert_eq!(reset.phase, 0);
            assert_eq!(reset.subagent_start_count, 0);
            assert!(!reset.review_subagent_seen);
            assert!(!reset.generic_subagent_seen);
            assert!(reset.review_gate.review_required);
        }

        #[test]
        fn rearm_review_preserves_evidence_when_override() {
            let repo = fresh_repo();
            let sid = "sm-rearm-override";
            let arm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &arm).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let seeded = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(seeded.review_gate.independent_reviewer_seen);
            let override_ups = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review，不要用子代理"
            });
            let _ = run_gate(&repo, &override_ups).unwrap();
            let kept = codex_load_state(&repo, &override_ups).unwrap().unwrap();
            assert!(
                kept.review_gate.independent_reviewer_seen,
                "override must not reset prior PostTool reviewer evidence"
            );
            assert!(kept.review_gate.review_override);
        }

        #[test]
        fn legacy_phase_two_alone_compact_does_not_clear_codex_review_gate() {
            let _g = env_lock();
            let repo = fresh_repo();
            let sid = "sm-legacy-phase2-compact";
            let arm = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &arm).unwrap();
            let sp = codex_state_path(&repo, &arm);
            let mut state = codex_load_state(&repo, &arm).unwrap().unwrap();
            state.phase = 2;
            state.subagent_start_count = 0;
            state.review_gate.independent_reviewer_seen = false;
            state.review_gate.review_required = true;
            assert!(codex_save_state_to_path(&sp, &mut state));
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id": sid,
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续",
                "response":"[P1] scripts/foo.rs:1 — issue — impact — verify",
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(
                msg.contains("CODEX_REVIEW_GATE"),
                "legacy phase=2 without PostTool start/independent must not clear gate; msg={msg:?}"
            );
            let loaded = codex_load_state(&repo, &stop).unwrap().unwrap();
            assert!(
                loaded.phase < 3,
                "compact must not bump to phase 3 without countable evidence"
            );
        }

        #[test]
        fn stop_reject_reason_in_response_clears_gate() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-reject-resp",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-reject-resp",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"",
                "response":"small_task"
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none(), "reject token in response must clear: {out:?}");
        }

        #[test]
        fn stop_clears_after_best_of_n_runner_posttool_and_compact() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-bon",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-bon",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"best-of-n-runner","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-bon",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续",
                "response": TEST_COMPACT_FINDING
            });
            let out = run_gate(&repo, &stop).unwrap();
            assert!(out.is_none(), "best-of-n + compact must clear: {out:?}");
        }

        #[test]
        fn stop_with_review_explore_fork_false_still_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-7-explore",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-7-explore",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore","fork_context":false}
            });
            let _ = run_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-7-explore",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(msg.contains("CODEX_REVIEW_GATE"), "out={out:?}");
        }

        #[test]
        fn stop_hook_active_bypass_skips_gate_only_when_env_set() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
            std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", "1");
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-8-bypass",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-8-bypass",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续",
                "stop_hook_active": true
            });
            let out = run_gate(&repo, &payload).unwrap();
            assert!(out.is_none(), "bypass env must skip review gate on replay: {out:?}");
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", v),
                None => std::env::remove_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS"),
            }
        }

        #[test]
        fn stop_hook_active_still_blocks_review_gate_by_default() {
            let _g = env_lock();
            let prior = std::env::var_os("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
            std::env::remove_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS");
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-8-default",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-8-default",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续",
                "stop_hook_active": true
            });
            let out = run_gate(&repo, &payload).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert!(
                out.as_ref().and_then(|v| v.get("decision")).and_then(Value::as_str) != Some("block"),
                "review gate Stop must be advisory-only: {out:?}"
            );
            assert!(
                msg.contains("CODEX_REVIEW_GATE"),
                "stop_hook_active without bypass must still nudge review: {out:?}"
            );
            match prior {
                Some(v) => std::env::set_var("ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS", v),
                None => {}
            }
        }

        #[test]
        fn stop_completion_claim_blocks_with_closeout_followup_when_strict() {
            let _g = env_lock();
            let prev = std::env::var_os("ROUTER_RS_CLOSEOUT_ENFORCEMENT");
            std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", "1");
            let repo = fresh_repo();
            let tid = "t-codex-closeout";
            fs::create_dir_all(repo.join("artifacts/current").join(tid)).unwrap();
            fs::write(
                repo.join("artifacts/current/active_task.json"),
                format!(r#"{{"task_id":"{tid}"}}"#),
            )
            .unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-closeout",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"all done, shipped"
            });
            let out = run_gate(&repo, &stop).unwrap();
            let msg = out
                .as_ref()
                .and_then(|v| v["followup_message"].as_str())
                .unwrap_or_default();
            assert_eq!(
                out.as_ref()
                    .and_then(|v| v.get("decision"))
                    .and_then(Value::as_str),
                Some("block")
            );
            assert!(
                msg.contains("CLOSEOUT_FOLLOWUP") && msg.contains("missing_record"),
                "expected closeout block on Stop; got {out:?}"
            );
            match prev {
                Some(v) => std::env::set_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT", v),
                None => std::env::remove_var("ROUTER_RS_CLOSEOUT_ENFORCEMENT"),
            }
        }

        #[test]
        fn post_tool_state_lock_failure_blocks_like_user_prompt_submit() {
            let repo = fresh_repo();
            let event = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"lock-pt-block",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"general-purpose","fork_context":false}
            });
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
            fs::write(&lock_path, "pid=1 ts=1\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o000)).unwrap();
            }
            #[cfg(not(unix))]
            {
                let guard = acquire_codex_state_lock(&state_path).unwrap();
                let _hold = guard;
            }
            let out = run_gate(&repo, &event).unwrap();
            assert_eq!(
                out.as_ref().and_then(|v| v.get("decision")).and_then(Value::as_str),
                Some("block"),
                "PostTool lock failure must fail-closed: {out:?}"
            );
            assert_eq!(
                out.as_ref().and_then(|v| v.get("reason")).and_then(Value::as_str),
                Some("Codex hook state could not be persisted under .codex/hook-state.")
            );
        }

        #[test]
        fn no_drift_warn_when_manifest_missing() {
            let repo = fresh_repo();
            let codex_home = repo.join("codex-home");
            fs::create_dir_all(&codex_home).unwrap();
            std::env::set_var("CODEX_HOME", &codex_home);
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-drift-1",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"普通提问"
            });
            let out = run_gate(&repo, &payload).unwrap();
            // Plain prompts no longer arm a hard subagent gate,
            // so the hook may return None (no context to emit). If context IS
            // emitted for other reasons, it must not contain a drift warning.
            let ctx = out
                .as_ref()
                .and_then(|v| v["hookSpecificOutput"]["additionalContext"].as_str())
                .unwrap_or_default()
                .to_string();
            assert!(!ctx.contains("hook projection drift detected"));
        }

        #[test]
        fn no_drift_warn_when_manifest_matches() {
            let repo = fresh_repo();
            let codex_home = repo.join("codex-home");
            fs::create_dir_all(&codex_home).unwrap();
            std::env::set_var("CODEX_HOME", &codex_home);
            let manifest = json!({
                "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
                "command_digest": "abc",
            });
            fs::write(
                codex_home.join(".router-rs-install.manifest.json"),
                serde_json::to_string(&manifest).unwrap(),
            )
            .unwrap();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-drift-2",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"普通提问"
            });
            let out = run_gate(&repo, &payload).unwrap();
            if let Some(value) = out {
                let ctx = value["hookSpecificOutput"]["additionalContext"]
                    .as_str()
                    .unwrap_or_default();
                assert!(!ctx.contains("hook projection drift detected"));
            }
        }

        #[test]
        fn v1_migration_ignores_removed_override_flag() {
            let repo = fresh_repo();
            let event = json!({"session_id":"v1-override"});
            let state_path = codex_state_path(&repo, &event);
            fs::write(
                state_path,
                r#"{"schema_version":1,"override":true,"subagent_required":true}"#,
            )
            .unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert_eq!(state.seq, 0);
        }

        #[test]
        fn v1_migration_ignores_removed_reject_reason_flag() {
            let repo = fresh_repo();
            let event = json!({"session_id":"v1-reject"});
            let state_path = codex_state_path(&repo, &event);
            fs::write(
                state_path,
                r#"{"schema_version":1,"reject_reason_seen":true}"#,
            )
            .unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert_eq!(state.seq, 0);
        }

        #[test]
        fn v1_delegation_only_maps_to_phase1() {
            let repo = fresh_repo();
            let event = json!({"session_id":"v1-phase"});
            let state_path = codex_state_path(&repo, &event);
            fs::write(
                state_path,
                r#"{"schema_version":1,"delegation_required":true,"review_subagent_seen":false}"#,
            )
            .unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert_eq!(state.seq, 1);
        }

        #[test]
        fn codex_session_key_fallback_is_stable_without_identifiers() {
            let _guard = env_lock();
            std::env::remove_var("CODEX_SESSION_ID");
            std::env::remove_var("CODEX_CONVERSATION_ID");
            std::env::remove_var("ROUTER_RS_CODEX_HOOK_STATE_SALT");
            let repo = fresh_repo();
            let event = json!({"cwd": repo.to_string_lossy()});
            let a = codex_session_key(&repo, &event);
            let b = codex_session_key(&repo, &event);
            assert_eq!(a, b);
            assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
            assert_eq!(a.len(), 32);
        }

        #[test]
        fn codex_session_key_differs_by_cwd_when_unstable() {
            let _guard = env_lock();
            std::env::remove_var("CODEX_SESSION_ID");
            std::env::remove_var("CODEX_CONVERSATION_ID");
            let repo = fresh_repo();
            let a = codex_session_key(&repo, &json!({"cwd":"/tmp/a"}));
            let b = codex_session_key(&repo, &json!({"cwd":"/tmp/b"}));
            assert_ne!(a, b, "unstable fallback must not collapse unlike cwd");
        }

        #[test]
        fn saw_subagent_codex_accepts_agent_type_camel_case_field() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"agentType":"browser-use"})
            ));
        }

        #[test]
        fn post_tool_use_with_agent_type_camel_case_marks_seen_without_deep_independent() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2e",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"please do deep review"
            });
            let _ = run_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2e",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"agentType":"explore","fork_context":false}
            });
            let out = run_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(state.review_subagent_seen);
            assert!(
                !state.review_gate.independent_reviewer_seen,
                "explore must not satisfy Codex independent deep-review bar"
            );
            assert!(state.generic_subagent_seen);
            assert!(state.review_lane_seen);
            assert!(!state.parallel_lane_seen);
            assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
        }

        #[test]
        fn dispatch_unknown_event_blocks_with_message() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Other",
                "session_id":"sm-9",
                "cwd": repo.to_string_lossy().to_string()
            });
            let out = run_gate(&repo, &payload)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
            assert!(out
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("unsupported"));
        }

        #[test]
        fn dispatch_missing_event_blocks_with_message() {
            let repo = fresh_repo();
            let payload = json!({"session_id":"sm-10"});
            let out = run_gate(&repo, &payload)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
            assert!(out
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("missing"));
        }

        #[test]
        fn codex_state_lock_recovers_from_stale_lock() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-stale"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
            fs::write(&lock_path, "pid=999999 ts=1\n").unwrap();
            let lock = acquire_codex_state_lock(&state_path);
            assert!(lock.is_ok());
        }

        #[test]
        fn codex_state_lock_recovers_from_corrupt_lock_metadata() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-corrupt"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
            fs::write(&lock_path, "not-a-lock-metadata-line\n").unwrap();
            let lock = acquire_codex_state_lock(&state_path);
            assert!(lock.is_ok());
        }

        #[test]
        fn codex_state_lock_recovers_from_unparseable_pid_and_ts() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-unparseable"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
            fs::write(&lock_path, "pid=bad ts=bad\n").unwrap();
            let lock = acquire_codex_state_lock(&state_path);
            assert!(lock.is_ok());
        }

        #[cfg(unix)]
        #[test]
        fn codex_state_lock_blocks_until_released() {
            use std::sync::mpsc;

            let repo = fresh_repo();
            let event = json!({"session_id":"lock-held"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let guard = acquire_codex_state_lock(&state_path).unwrap();
            let state_path_clone = state_path.clone();
            let (tx, rx) = mpsc::channel();
            let waiter = std::thread::spawn(move || {
                let second = acquire_codex_state_lock(&state_path_clone).unwrap();
                let _ = tx.send(());
                drop(second);
            });
            std::thread::sleep(Duration::from_millis(50));
            assert!(rx.try_recv().is_err());
            drop(guard);
            rx.recv_timeout(Duration::from_secs(5))
                .expect("second acquirer should proceed after lock release");
            waiter.join().unwrap();
        }

        #[cfg(not(unix))]
        #[test]
        fn codex_state_lock_blocks_when_held() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-held"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let guard = acquire_codex_state_lock(&state_path).unwrap();
            let started = std::time::Instant::now();
            let second = acquire_codex_state_lock(&state_path);
            assert!(second.is_err());
            assert!(started.elapsed() >= Duration::from_millis(1200));
            drop(guard);
        }

        #[test]
        fn codex_state_lock_serializes_concurrent_writes() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-inc"});
            let repo_a = repo.clone();
            let repo_b = repo.clone();
            let event_a = event.clone();
            let event_b = event.clone();
            let worker = move |repo_root: PathBuf, ev: Value| {
                for _ in 0..1000 {
                    with_codex_state_lock(&repo_root, &ev, |loaded| {
                        let mut state = loaded.unwrap_or_default();
                        state.seq += 1;
                        Ok((Some(state), ()))
                    })
                    .unwrap();
                }
            };
            let t1 = std::thread::spawn(move || worker(repo_a, event_a));
            let t2 = std::thread::spawn(move || worker(repo_b, event_b));
            t1.join().unwrap();
            t2.join().unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            // flock on macOS has known edge cases with concurrent threads;
            // accept 1999-2000 to avoid flaky test failures.
            assert!(
                state.seq >= 1999 && state.seq <= 2000,
                "concurrent seq should be 1999 or 2000, got {}",
                state.seq
            );
        }

        #[test]
        fn userpromptsubmit_simple_prompt_records_only_telemetry() {
            let repo = fresh_repo();
            let event = json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": "test-p0a-simple",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt": "just a simple question about coding"
            });
            let _ = run_gate(&repo, &event).unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert_eq!(state.seq, 1);
            assert!(!state.review_subagent_seen);
        }

        #[test]
        fn userpromptsubmit_review_prompt_records_gate_requirement() {
            let repo = fresh_repo();
            let event = json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": "test-p0a-review",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt": "please do a deep code review of this module"
            });
            let _ = run_gate(&repo, &event).unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert_eq!(state.seq, 1);
            assert!(state.review_gate.review_required);
            assert!(!state.review_subagent_seen);
        }

        // P0-B: protected prefix tests
        #[test]
        fn protected_prefixes_cover_skill_files_and_registry() {
            assert!(
                classify_protected_generated_path("skills/SKILL_ROUTING_RUNTIME.json").is_some(),
                "SKILL_ROUTING_RUNTIME.json should be protected"
            );
            assert!(
                classify_protected_generated_path("skills/SKILL_MANIFEST.json").is_some(),
                "SKILL_MANIFEST.json should be protected"
            );
            assert!(
                classify_protected_generated_path("configs/framework/RUNTIME_REGISTRY.json")
                    .is_some(),
                "RUNTIME_REGISTRY.json should be protected"
            );
            assert!(
                classify_protected_generated_path("skills/other_file.json").is_none(),
                "non-SKILL_ prefixed file should not be protected"
            );
        }

        // P1-B: CODEX_SESSION_ID env var fallback test
        #[test]
        fn codex_session_key_uses_codex_session_id_env_when_no_event_fields() {
            let _guard = env_lock();
            // Use a unique env-var value to avoid cross-test pollution.
            let unique_id = format!(
                "test-stable-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            );
            let event = json!({});
            let repo = fresh_repo();
            std::env::set_var("CODEX_SESSION_ID", &unique_id);
            let a = codex_session_key(&repo, &event);
            let b = codex_session_key(&repo, &event);
            std::env::remove_var("CODEX_SESSION_ID");
            assert_eq!(a, b, "env var fallback should produce a stable key");
            assert!(
                a.chars().all(|c| c.is_ascii_hexdigit()),
                "key should be hex"
            );
            assert_eq!(a.len(), 32, "key should be 32 hex chars");
        }

        #[test]
        fn codex_session_key_matches_for_session_id_camel_case() {
            let repo = fresh_repo();
            let sid = "sess-key-camel-01";
            let snake = codex_session_key(&repo, &json!({"session_id": sid}));
            let camel = codex_session_key(&repo, &json!({"sessionId": sid}));
            assert_eq!(snake, camel);
        }

        #[test]
        fn codex_session_key_uses_codex_conversation_id_env_when_no_event_fields() {
            let _guard = env_lock();
            let unique_id = format!(
                "test-conv-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            );
            let event = json!({});
            std::env::remove_var("CODEX_SESSION_ID");
            let repo = fresh_repo();
            std::env::set_var("CODEX_CONVERSATION_ID", &unique_id);
            let a = codex_session_key(&repo, &event);
            let b = codex_session_key(&repo, &event);
            std::env::remove_var("CODEX_CONVERSATION_ID");
            assert_eq!(a, b, "CODEX_CONVERSATION_ID fallback should be stable");
            assert_eq!(a.len(), 32);
        }

        #[test]
        fn strict_stable_session_key_blocks_userpromptsubmit_without_identifier() {
            let _guard = env_lock();
            std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "1");
            std::env::remove_var("CODEX_SESSION_ID");
            std::env::remove_var("CODEX_CONVERSATION_ID");
            let repo = fresh_repo();
            let event = json!({
                "hook_event_name": "UserPromptSubmit",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt": "hello"
            });
            let out = super::run_codex_lifecycle_context_hook(&repo, &event)
                .unwrap()
                .unwrap();
            assert_eq!(out["decision"], json!("block"));
            std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY");
        }

        #[test]
        fn strict_stable_session_key_allows_sessionstart_without_identifier() {
            let _guard = env_lock();
            std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "1");
            std::env::remove_var("CODEX_SESSION_ID");
            std::env::remove_var("CODEX_CONVERSATION_ID");
            let repo = fresh_repo();
            let event = json!({
                "hook_event_name": "SessionStart",
                "source": "startup"
            });
            let out = super::run_codex_lifecycle_context_hook(&repo, &event)
                .unwrap()
                .expect("sessionstart output");
            assert!(out.get("hookSpecificOutput").is_some());
            std::env::remove_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY");
        }

        #[test]
        fn strict_stable_session_key_off_allows_userpromptsubmit_without_identifier() {
            let _guard = env_lock();
            std::env::set_var("ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY", "0");
            std::env::remove_var("CODEX_SESSION_ID");
            std::env::remove_var("CODEX_CONVERSATION_ID");
            let repo = fresh_repo();
            let event = json!({
                "hook_event_name": "UserPromptSubmit",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt": "hello"
            });
            let out = super::run_codex_lifecycle_context_hook(&repo, &event).unwrap();
            assert!(
                !matches!(out, Some(ref v) if v.get("decision") == Some(&json!("block"))),
                "unexpected lifecycle block when strict mode off"
            );
        }

        // P1-C: prune_stale_hook_state_files test
        #[test]
        fn prune_removes_excess_files_over_limit() {
            let repo = fresh_repo();
            let state_dir = repo.join(".codex/hook-state");
            // Create 60 fake review-subagent JSON files
            for i in 0..60u64 {
                let name = format!("review-subagent-{:032x}.json", i);
                fs::write(state_dir.join(&name), "{}").unwrap();
            }
            prune_stale_hook_state_files(&state_dir);
            let count = fs::read_dir(&state_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name();
                    let s = n.to_string_lossy();
                    s.starts_with("review-subagent-") && s.ends_with(".json")
                })
                .count();
            assert!(
                count <= 50,
                "after pruning, at most 50 files should remain, got {count}"
            );
        }
    }
}
