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
    is_review_gate_suppressed, is_subagent_tool, recognize_subagent_type, subagent_lane_bits,
};
use crate::hooks;
use core_policy::HookReviewDiskCore;
use core_policy::hook_common::{
    has_override, normalize_tool_name, saw_reject_reason, should_inject_spawn_first_review_nudge,
};
use core_policy::registry_review_gate::review_spawn_first_nudge_line;
use core_policy::review_gate_engine::{
    ReviewGateFacts, fork_context_from_values, review_independent_reviewer_evidence,
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
fn opencode_state_config() -> HookStateConfig {
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
        let suppressed = is_review_gate_suppressed(self.host_id(), Some(event.repo_root), &prompt);

        if suppressed {
            return None;
        }

        // Load or create review gate state
        // state_dir managed by HookStateConfig
        let _state_path = opencode_state_config().state_path(event.repo_root);
        let mut state: OpencodeHookState = opencode_state_config().load_state(event.repo_root);

        // Check for reject reason
        let signal_text = event
            .payload
            .get("signal_text")
            .and_then(Value::as_str)
            .unwrap_or("");
        if saw_reject_reason(signal_text, &prompt) {
            state.core.reject_reason_seen = true;
            state.review_phase = state.review_phase.saturating_add(1);
        }

        // Check for override
        if has_override(&prompt) {
            state.core.review_override = true;
        }

        // Build review gate facts
        let facts = ReviewGateFacts::from_prompt(&prompt);
        if facts.review_required && !state.core.review_override {
            state.core.review_required = true;
        }

        // Save state
        opencode_state_config().save_state(event.repo_root, &state);

        // Build additional context
        let mut contexts = Vec::new();

        // Spawn-first nudge (cross-host contract: inject skill pointer when review arms)
        if state.core.review_required && !state.core.review_override {
            let repo_root_opt = Some(event.repo_root);
            if should_inject_spawn_first_review_nudge(repo_root_opt, &prompt) {
                contexts.push(review_spawn_first_nudge_line(repo_root_opt, "opencode"));
            }
        }

        // Goal drive context
        let session_key = extract_session_key(
            event.payload,
            self.session_namespace_env(),
            &format!("opencode-{}", event.repo_root.display()),
        );
        if let Some(goal_ctx) = build_goal_context(event.repo_root, &session_key) {
            contexts.push(goal_ctx);
        }

        // Paper context injection (parity with Claude/Cursor/Codex)
        crate::hooks::maybe_append_paper_adversarial_context(
            event.repo_root,
            &prompt,
            &mut contexts,
            crate::hooks::PaperProseHookHostType::OpenCode,
        );
        crate::hooks::maybe_append_paper_prose_context(
            event.repo_root,
            &prompt,
            &mut contexts,
            crate::hooks::PaperProseHookHostType::OpenCode,
        );

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

        // Record tool call
        hooks::emit_tool_call(&normalized, duration_ms, succeeded);

        // Check if review gate is suppressed
        let prompt = extract_prompt_text(event.payload);
        let suppressed = is_review_gate_suppressed(self.host_id(), Some(event.repo_root), &prompt);

        if suppressed {
            return None;
        }

        // Load state
        // state_dir managed by HookStateConfig
        let _state_path = opencode_state_config().state_path(event.repo_root);
        let mut state: OpencodeHookState = opencode_state_config().load_state(event.repo_root);

        // Shell evidence for verification commands
        if is_verification_command(&tool_name, &tool_input) {
            append_shell_evidence(event.repo_root, &tool_name, &tool_input, succeeded);
        }

        // Subagent tracking
        if is_subagent_tool(&normalized) {
            let kind = recognize_subagent_type(&tool_input);
            let (review_lane, parallel_lane) = subagent_lane_bits(kind.as_deref());

            if review_lane {
                state.review_subagent_seen = true;
            }
            if parallel_lane {
                state.parallel_lane_seen = true;
            }

            let fork = fork_context_from_values(&tool_input, Some(event.payload));

            state.subagent_start_count += 1;
            state.review_phase = state.review_phase.saturating_add(1);

            if review_lane {
                let is_independent = review_independent_reviewer_evidence(review_lane, fork);
                if is_independent {
                    state.core.independent_reviewer_seen = true;
                }
            }
        }

        // Independent reviewer evidence for non-subagent tools
        let fork = fork_context_from_values(&tool_input, Some(event.payload));
        let is_reviewer_tool = is_reviewer_tool_name(&normalized);
        if review_independent_reviewer_evidence(is_reviewer_tool, fork) {
            state.core.independent_reviewer_seen = true;
        }

        // Save state
        opencode_state_config().save_state(event.repo_root, &state);

        None
    }

    // ── Stop: closeout gate + review gate check ──

    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        // my-light suppression: if stop prompt is a lifecycle entry, skip review gate
        let stop_prompt = extract_prompt_text(event.payload);
        if is_review_gate_suppressed(self.host_id(), Some(event.repo_root), &stop_prompt) {
            opencode_state_config().remove_state(event.repo_root);
            return None;
        }

        // Default closeout check
        let completion_text = hook_dispatch::extract_completion_text(event);
        if let Some(msg) =
            hooks::closeout_stop_followup_for_completion_text(event.repo_root, &completion_text)
        {
            return Some(HookOutput::Block { reason: msg });
        }

        // Load review gate state
        let mut state: OpencodeHookState = opencode_state_config().load_state(event.repo_root);

        // Check override in stop prompt (cross-host contract: Cursor/Codex/Opencode check Stop-time override)
        if has_override(&stop_prompt) {
            state.core.review_override = true;
        }

        // Check reject / rg_clear tokens in stop prompt, signal_text, or response text
        // (cross-host contract: reject tokens in user prompt or assistant response clear the gate)
        let signal_text = event
            .payload
            .get("signal_text")
            .and_then(Value::as_str)
            .unwrap_or("");
        if saw_reject_reason(signal_text, &stop_prompt)
            || saw_reject_reason(&stop_prompt, &completion_text)
            || saw_reject_reason(&completion_text, &stop_prompt)
        {
            state.core.reject_reason_seen = true;
            state.core.review_required = false;
            opencode_state_config().save_state(event.repo_root, &state);
            return None;
        }

        // Check reject from previous arming
        if state.core.reject_reason_seen {
            return Some(HookOutput::Block {
                reason: format!(
                    "[{OPENCODE_REVIEW_GATE_TAG}] Reject reason seen. \
                     Review before ending this turn."
                ),
            });
        }

        // Check review gate using shared core_policy function
        if let Some(nudge) = core_policy::hook_review_disk_state::hook_review_stop_advisory_needed(
            &state.core.gate_fields(),
            OPENCODE_REVIEW_GATE_TAG,
        ) {
            return Some(HookOutput::Advisory { message: nudge });
        }

        // Cleanup state
        opencode_state_config().remove_state(event.repo_root);

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

        let kind = recognize_subagent_type(&tool_input);
        let (review_lane, parallel_lane) = subagent_lane_bits(kind.as_deref());

        // state_dir managed by HookStateConfig
        let _state_path = opencode_state_config().state_path(event.repo_root);
        let mut state: OpencodeHookState = opencode_state_config().load_state(event.repo_root);

        if review_lane {
            state.review_subagent_seen = true;
        }
        if parallel_lane {
            state.parallel_lane_seen = true;
        }

        state.subagent_start_count += 1;
        state.review_phase = state.review_phase.saturating_add(1);

        opencode_state_config().save_state(event.repo_root, &state);

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
        if let Some(val) = tool_input.get(*key).and_then(Value::as_str) {
            if !val.is_empty() {
                paths.push(val.to_string());
            }
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
        || path_str.contains("AGENTS_OPENCODE.md")
        || path_str.contains("AGENTS_CLAUDE.md")
        || path_str.contains("AGENTS_CURSOR.md")
        || path_str.contains("AGENTS_CODEX.md")
    {
        return Some("Generated entrypoint. Edit via framework tools.");
    }

    None
}

/// Check if a tool call represents a verification command.
fn is_verification_command(tool_name: &str, tool_input: &Value) -> bool {
    let command = tool_input
        .get("command")
        .or(tool_input.get("cmd"))
        .and_then(Value::as_str)
        .unwrap_or("");
    hook_dispatch::is_verification_command(tool_name, command)
}

/// Append shell evidence to the EVIDENCE_INDEX for verification commands.
fn append_shell_evidence(repo_root: &Path, tool_name: &str, tool_input: &Value, succeeded: bool) {
    if !is_verification_command(tool_name, tool_input) {
        return;
    }
    let command = tool_input
        .get("command")
        .or(tool_input.get("cmd"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let cmd_trimmed: String = command.chars().take(200).collect();
    let mut entry = serde_json::Map::new();
    entry.insert("kind".to_string(), json!("auto_evidence"));
    entry.insert("source".to_string(), json!("post_tool_use_auto"));
    entry.insert("tool_name".to_string(), json!(tool_name));
    entry.insert("command_preview".to_string(), json!(cmd_trimmed));
    entry.insert("success".to_string(), json!(succeeded));
    entry.insert(
        "recorded_at".to_string(),
        json!(crate::hooks::current_local_timestamp()),
    );
    if let Err(err) = crate::hooks::append_evidence_index(repo_root, None, entry) {
        eprintln!("[router-rs] opencode auto-evidence record failed: {err}");
    }
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
