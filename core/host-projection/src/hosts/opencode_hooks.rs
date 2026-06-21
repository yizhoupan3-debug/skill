//! OpenCode host: full `HostHookDispatcher` implementation.
//!
//! OpenCode uses `router-rs opencode hook --event=...` for all hook events,
//! unified with cursor/claude/codex via the shared `HostHookDispatcher` trait.
//! Hook launcher: `configs/framework/opencode-router-rs-hook.sh`.
//!
//! Hook events: tool.execute.before, tool.execute.after, session.idle, etc.

use super::file_state_lock::HookStateConfig;
use super::hook_dispatch::{
    self, HookEvent, HookOutput, HostHookConfig, HostHookDispatcher, compact_contexts,
    extract_prompt_text, extract_session_key, extract_tool_input, extract_tool_name,
    is_review_gate_suppressed, is_subagent_tool,
};
use crate::hooks;
use core_policy::HookReviewDiskCore;
use core_policy::hook_common::{
    has_override, normalize_tool_name, saw_reject_reason,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

// ────────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────────

pub const OPENCODE_HOOKS_PATH: &str = ".opencode/hooks.json";

/// Hook events registered by the OpenCode plugin system.
pub const OPENCODE_HOOKS_REGISTERED_EVENTS: &[&str] = &[
    "tool.execute.before",
    "tool.execute.after",
    "session.idle",
    "session.created",
    "session.deleted",
    "permission.asked",
    "permission.replied",
    "file.edited",
    "shell.env",
];

const OPENCODE_HOOK_STATE_UNREADABLE: &str =
    "router-rs OPENCODE_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions";

const OPENCODE_REVIEW_GATE_TAG: &str = "opencode-review-gate";

/// Shared state configuration for opencode hook state.
fn state_config() -> HookStateConfig {
    HookStateConfig {
        host_id: "opencode",
        state_dir_leaf: ".opencode",
        state_filename: "review-subagent-state.json",
        unreadable_tag: OPENCODE_HOOK_STATE_UNREADABLE,
    }
}

// ────────────────────────────────────────────────────────────────
// State structure
// ────────────────────────────────────────────────────────────────

/// OpenCode on-disk hook state (extends shared `HookReviewDiskCore`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpencodeHookState {
    /// Shared review gate fields (cross-host compatible).
    #[serde(flatten)]
    pub core: HookReviewDiskCore,

    /// Subagent start count (for review phase tracking).
    #[serde(default)]
    pub subagent_start_count: u32,

    /// Review phase counter.
    #[serde(default)]
    pub review_phase: u32,

    /// Whether a review-type subagent has been seen.
    #[serde(default)]
    pub review_subagent_seen: bool,

    /// Whether a parallel-type subagent has been seen.
    #[serde(default)]
    pub parallel_lane_seen: bool,
}

// ────────────────────────────────────────────────────────────────
// Dispatcher
// ────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Copy)]
pub struct OpencodeHookDispatcher;

impl HostHookConfig for OpencodeHookDispatcher {
    fn host_id(&self) -> &'static str {
        "opencode"
    }

    fn state_dir_leaf(&self) -> &'static str {
        ".opencode"
    }

    fn hook_state_unreadable_tag(&self) -> &'static str {
        OPENCODE_HOOK_STATE_UNREADABLE
    }

    fn session_namespace_env(&self) -> &'static str {
        "ROUTER_RS_OPENCODE_SESSION_NAMESPACE"
    }

    fn log_label(&self) -> &'static str {
        "opencode"
    }

    fn additional_context_max_bytes(&self) -> usize {
        640
    }

    fn supports_session_start(&self) -> bool {
        true
    }

    fn supports_subagent_start(&self) -> bool {
        true
    }

    fn supports_subagent_stop(&self) -> bool {
        true
    }
}

impl HostHookDispatcher for OpencodeHookDispatcher {
    // ── PreToolUse: path protection ──

    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        let tool_name = extract_tool_name(event.payload);
        let normalized = normalize_tool_name(Some(&tool_name));
        let tool_input = extract_tool_input(event.payload);

        // Extract paths from tool input
        let paths = extract_paths_from_tool_input(&tool_input, &normalized);
        if paths.is_empty() {
            return None;
        }

        // Check each path against protected patterns
        for path_str in &paths {
            let path = Path::new(path_str);
            if let Some(reason) = classify_protected_path(path, event.repo_root) {
                return Some(HookOutput::Deny {
                    reason: format!("Protected framework path: {path_str}. {reason}"),
                });
            }
        }

        None
    }

    // ── UserPromptSubmit: review gate + context injection ──

    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput> {
        let prompt = extract_prompt_text(event.payload);
        let mut state: OpencodeHookState = state_config().load_state(event.repo_root);

        // Shared review gate merge (4-host unified)
        let merge = hook_dispatch::merge_review_gate_on_user_prompt(
            &state.core,
            &prompt,
            event.repo_root,
            self.host_id(),
        );
        if merge.suppressed {
            return None;
        }
        state.core = merge.core;

        // OpenCode-specific: reject reason on submit
        let signal_text = event.payload.get("signal_text").and_then(Value::as_str).unwrap_or("");
        if saw_reject_reason(signal_text, &prompt) {
            state.core.reject_reason_seen = true;
        }
        if has_override(&prompt) {
            state.core.review_override = true;
        }

        state_config().save_state(event.repo_root, &state);

        // Shared context injection (4-host unified)
        let mut contexts = hook_dispatch::build_user_prompt_context_injection(
            event.repo_root,
            &prompt,
            "opencode",
            crate::hooks::PaperProseHookHostType::OpenCode,
            state.core.review_required,
            state.core.review_override,
        );

        // Goal drive context (OpenCode-specific)
        let session_key = extract_session_key(
            event.payload,
            self.session_namespace_env(),
            &format!("opencode-{}", event.repo_root.display()),
        );
        if let Some(goal_ctx) = build_goal_context(event.repo_root, &session_key) {
            contexts.push(goal_ctx);
        }

        compact_contexts(contexts, self.additional_context_max_bytes())
            .map(HookOutput::AdditionalContext)
    }

    // ── PostToolUse: evidence + subagent tracking ──

    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        let tool_name = extract_tool_name(event.payload);
        let normalized = normalize_tool_name(Some(&tool_name));
        let tool_input = extract_tool_input(event.payload);
        let succeeded = event
            .payload
            .get("succeeded")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let duration_ms = event
            .payload
            .get("duration_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);

        // Shared tool call telemetry (4-host unified)
        hook_dispatch::record_tool_call_emission(event.repo_root, &normalized, duration_ms, succeeded);

        // Check if review gate is suppressed
        let prompt = extract_prompt_text(event.payload);
        if is_review_gate_suppressed(self.host_id(), Some(event.repo_root), &prompt) {
            return None;
        }

        // Load state
        let mut state: OpencodeHookState = state_config().load_state(event.repo_root);

        // Shell evidence for verification commands (shared with Claude/Cursor/Codex)
        let command = tool_input
            .get("command")
            .or(tool_input.get("cmd"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if hook_dispatch::is_verification_command(&tool_name, command) {
            if let Err(err) = crate::hooks::try_append_post_tool_shell_evidence(
                event.repo_root,
                event.payload,
                "opencode_post_tool_verification",
            ) {
                eprintln!("[router-rs] opencode auto-evidence record failed: {err}");
            }
        }

        // Subagent tracking (using shared lane bits)
        if is_subagent_tool(&normalized) {
            let kind = hook_dispatch::recognize_subagent_type(&tool_input);
            let (review_lane, parallel_lane) = hook_dispatch::subagent_lane_bits(kind.as_deref());

            if review_lane {
                state.review_subagent_seen = true;
            }
            if parallel_lane {
                state.parallel_lane_seen = true;
            }

            state.subagent_start_count += 1;
            state.review_phase = state.review_phase.saturating_add(1);

            // Shared reviewer evidence detection (4-host unified)
            if hook_dispatch::detect_reviewer_evidence(&tool_input, review_lane) {
                state.core.independent_reviewer_seen = true;
            }
        }

        // Independent reviewer evidence for non-subagent tools
        let is_reviewer_tool = is_reviewer_tool_name(&normalized);
        if hook_dispatch::detect_reviewer_evidence(&tool_input, is_reviewer_tool) {
            state.core.independent_reviewer_seen = true;
        }

        // Save state
        state_config().save_state(event.repo_root, &state);

        None
    }

    // ── Stop: closeout gate + review gate check ──

    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        let stop_prompt = extract_prompt_text(event.payload);
        if is_review_gate_suppressed(self.host_id(), Some(event.repo_root), &stop_prompt) {
            state_config().remove_state(event.repo_root);
            return None;
        }

        let mut state: OpencodeHookState = state_config().load_state(event.repo_root);

        // OpenCode-specific: reject reason seen on Submit clears gate (parity with Cursor early reject)
        let signal_text = event.payload.get("signal_text").and_then(Value::as_str).unwrap_or("");
        if saw_reject_reason(signal_text, &stop_prompt) {
            state.core.reject_reason_seen = true;
            state.core.review_required = false;
            state_config().save_state(event.repo_root, &state);
            return None;
        }

        // Shared stop decision pipeline (4-host unified)
        let response_text = hook_dispatch::extract_response_text(event.payload);
        let completion_text = hook_dispatch::extract_completion_text(event);

        match hook_dispatch::evaluate_stop_decision(
            &mut state.core,
            &stop_prompt,
            &response_text,
            &format!("{stop_prompt}\n{response_text}"),
            &completion_text,
            event.repo_root,
            self.host_id(),
        ) {
            hook_dispatch::StopDecision::Closeout { message } => {
                return Some(HookOutput::Advisory { message });
            }
            hook_dispatch::StopDecision::ReviewGateNudge { message } => {
                return Some(HookOutput::Advisory { message });
            }
            hook_dispatch::StopDecision::GoalFollowup { message } => {
                return Some(HookOutput::Advisory { message });
            }
            hook_dispatch::StopDecision::Clean => {}
        }

        state_config().remove_state(event.repo_root);
        None
    }

    // ── SessionStart: context injection ──

    fn handle_session_start(&self, event: &HookEvent) -> Option<HookOutput> {
        if !hooks::router_rs_operator_inject_globally_enabled() {
            return None;
        }

        let session_key = extract_session_key(
            event.payload,
            self.session_namespace_env(),
            &format!("opencode-{}", event.repo_root.display()),
        );

        let ctx = format!(
            "Repo: {}\nSession: {}\nHost: opencode",
            event.repo_root.display(),
            session_key,
        );

        Some(HookOutput::AdditionalContext(
            hook_dispatch::truncate_bytes(&ctx, self.additional_context_max_bytes(), "..."),
        ))
    }

    // ── SubagentStart: review lane tracking ──

    fn handle_subagent_start(&self, event: &HookEvent) -> Option<HookOutput> {
        let tool_name = extract_tool_name(event.payload);
        let normalized = normalize_tool_name(Some(&tool_name));
        let tool_input = extract_tool_input(event.payload);

        if !is_subagent_tool(&normalized) {
            return None;
        }

        let kind = hook_dispatch::recognize_subagent_type(&tool_input);
        let (review_lane, parallel_lane) = hook_dispatch::subagent_lane_bits(kind.as_deref());

        // state_dir managed by HookStateConfig
        let _state_path = state_config().state_path(event.repo_root);
        let mut state: OpencodeHookState = state_config().load_state(event.repo_root);

        if review_lane {
            state.review_subagent_seen = true;
        }
        if parallel_lane {
            state.parallel_lane_seen = true;
        }

        state.subagent_start_count += 1;
        state.review_phase = state.review_phase.saturating_add(1);

        state_config().save_state(event.repo_root, &state);

        None
    }

    // ── SubagentStop: informational (default no-op is fine, but we track) ──

    fn handle_subagent_stop(&self, _event: &HookEvent) -> Option<HookOutput> {
        // SubagentStop is informational; PostToolUse handles review gate logic.
        None
    }
}

// ────────────────────────────────────────────────────────────────
// Helper functions (opencode-specific)
// ────────────────────────────────────────────────────────────────

/// Extract file paths from tool input for PreToolUse path protection.
fn extract_paths_from_tool_input(tool_input: &Value, _tool_name: &str) -> Vec<String> {
    let mut paths = Vec::new();

    // Common path field names
    for key in &["path", "file_path", "filePath", "filename", "target"] {
        if let Some(val) = tool_input.get(*key).and_then(Value::as_str)
            && !val.is_empty() {
                paths.push(val.to_string());
            }
    }

    paths
}

/// Check if a path is protected and return the reason if so.
fn classify_protected_path(path: &Path, repo_root: &Path) -> Option<&'static str> {
    let path_str = path.to_string_lossy();

    // Framework-guarded paths (cross-host)
    if path_str.contains(".claude/settings.json")
        || path_str.contains(".claude/rules/")
        || path_str.contains(".claude/CLAUDE.md")
    {
        return Some("Framework configuration path. Use framework tools instead.");
    }

    // Host-private paths: protect own host state and other hosts
    if path_str.contains(".opencode/") {
        return Some("OpenCode host-private directory. Use framework host-integration tools.");
    }
    if path_str.contains(".codex/") {
        return Some("Codex host-private directory. Use framework host-integration tools.");
    }
    if path_str.contains(".cursor/")
        && !path_str.starts_with(&repo_root.to_string_lossy().to_string())
    {
        return Some("Other host's private directory.");
    }

    // Retired host paths
    if path_str.contains(".antigravity/") || path_str.contains(".gemini/") {
        return Some("Retired host directory.");
    }

    // Generated entrypoints
    if path_str.contains("AGENTS.md")
    {
        return Some("Generated entrypoint. Edit via framework tools.");
    }

    None
}

/// Check if a tool name indicates a reviewer-type tool.
fn is_reviewer_tool_name(normalized: &str) -> bool {
    normalized.contains("review") || normalized.contains("agent") || normalized.contains("task")
}

/// Build goal drive context from disk state.
fn build_goal_context(repo_root: &Path, session_key: &str) -> Option<String> {
    let goal_path = repo_root
        .join("artifacts/current")
        .join(session_key)
        .join("GOAL_STATE.json");
    if !goal_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&goal_path).ok()?;
    let goal_state: Value = serde_json::from_str(&content).ok()?;
    let goal = goal_state.get("goal").and_then(Value::as_str)?;
    let status = goal_state
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("active");
    Some(format!("[goal:{status}] {goal}"))
}

// ────────────────────────────────────────────────────────────────
// Test harness (shared with hook_contract matrix)
// ────────────────────────────────────────────────────────────────

#[cfg(any(test, feature = "test-support"))]
pub fn dispatch_opencode_hook_event(repo_root: &Path, event_name: &str, payload: &Value) -> Value {
    crate::hooks::ensure_kernel_bootstrap();
    let event = HookEvent {
        repo_root,
        event_name,
        payload,
    };
    match OpencodeHookDispatcher.dispatch(&event) {
        Some(HookOutput::AdditionalContext(ctx)) => {
            json!({ "continue": true, "additional_context": ctx })
        }
        Some(HookOutput::Deny { reason }) => {
            json!({ "continue": false, "followup_message": reason })
        }
        Some(HookOutput::Warn { message }) => {
            json!({ "continue": true, "followup_message": message })
        }
        Some(HookOutput::Block { reason }) => {
            json!({ "continue": false, "followup_message": reason })
        }
        Some(HookOutput::Advisory { message }) => {
            json!({ "continue": true, "followup_message": message })
        }
        Some(HookOutput::Raw(val)) => val,
        None | Some(HookOutput::None) => json!({}),
    }
}
