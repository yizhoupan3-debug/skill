//! Unified hook dispatch trait for all 4 closed-set hosts.
//!
//! All hosts (cursor, claude, codex, opencode) implement `HostHookDispatcher`,
//! sharing common logic through trait defaults. Host-specific overrides are minimal.
//!
//! Architecture:
//! ```text
//!   HostHookDispatcher (trait with defaults)
//!     ├── handle_pre_tool_use       (C: must implement — path protection)
//!     ├── handle_user_prompt_submit (C: must implement — review gate init)
//!     ├── handle_post_tool_use      (C: must implement — evidence + tracking)
//!     ├── handle_stop               (B: default + override — closeout gate)
//!     ├── handle_session_start      (B: default + override — context injection)
//!     ├── handle_subagent_start     (A: default no-op — cursor/codex override)
//!     ├── handle_subagent_stop      (A: default no-op — cursor override)
//!     └── dispatch                  (A: shared routing — never override)
//! ```

use serde_json::{Map, Value};
use std::path::Path;
use tracing::debug;

// ────────────────────────────────────────────────────────────────
// Standard types
// ────────────────────────────────────────────────────────────────

/// Standardized hook event input.
pub struct HookEvent<'a> {
    pub repo_root: &'a Path,
    pub event_name: &'a str,
    pub payload: &'a Value,
}

/// Standardized hook output.
pub enum HookOutput {
    /// Additional context for the host (UserPromptSubmit / SessionStart).
    AdditionalContext(String),
    /// Deny tool execution (PreToolUse).
    Deny { reason: String },
    /// Warn but allow (PreToolUse soft guard).
    Warn { message: String },
    /// Block stop (Stop — closeout/review gate).
    Block { reason: String },
    /// Advisory payload without blocking (Stop).
    Advisory { message: String },
    /// No output — silent pass-through.
    None,
    /// Full JSON override for host-specific formatting.
    Raw(Value),
}

/// Unified input for Stop event orchestration (`evaluate_stop_decision_full`).
///
/// Aggregates all context needed by the 8-step Stop pipeline:
/// closeout → override → goal gate → verification gate → quality gate
/// → review gate → goal followup → cleanup.
pub struct StopOrchestrationInput<'a> {
    /// Repository root path.
    pub repo_root: &'a Path,
    /// Host identifier (claude / cursor / codex / opencode).
    pub host_id: &'a str,
    /// Raw event payload from the host.
    pub payload: &'a Value,
    /// Extracted prompt text (user prompt / last user message).
    pub prompt: String,
    /// Extracted response/assistant text from the event.
    pub response_text: String,
    /// Extracted completion text (tail of assistant message for closeout matching).
    pub completion_text: String,
    /// Optional stop signal reason from the host.
    pub stop_signal: Option<String>,
}

/// Unified result from Stop event orchestration.
///
/// Each host consumes these fields to produce its own JSON response.
pub struct StopOrchestrationResult {
    /// Advisory review nudge message (if review gate produced one).
    pub review_nudge: Option<String>,
    /// Goal followup injection (if active Goal with uncovered done_when conditions).
    pub goal_followup: Option<String>,
    /// Verification gate advisory (if gate failed or produced warnings).
    pub verification_advisory: Option<String>,
    /// Quality gate advisory (if gate not closed).
    pub quality_advisory: Option<String>,
    /// Updated state to persist (JSON value).
    pub updated_state: Option<Value>,
    /// Whether the stop handler should clear runtime state.
    pub should_clear_state: bool,
}

/// Host-specific configuration parameters.
pub trait HostHookConfig: Send + Sync {
    /// Host identifier for env flag resolution.
    fn host_id(&self) -> &'static str;

    /// State directory leaf name (e.g. ".cursor", ".claude", ".codex", ".opencode").
    fn state_dir_leaf(&self) -> &'static str;

    /// Hook state unreadable error tag.
    fn hook_state_unreadable_tag(&self) -> &'static str;

    /// Session namespace env var name.
    fn session_namespace_env(&self) -> &'static str;

    /// Log label for diagnostics.
    fn log_label(&self) -> &'static str;

    /// Maximum bytes for additional context output.
    fn additional_context_max_bytes(&self) -> usize {
        640
    }

    /// Whether this host supports SessionStart events.
    fn supports_session_start(&self) -> bool {
        false
    }

    /// Whether this host supports SubagentStart events.
    fn supports_subagent_start(&self) -> bool {
        false
    }

    /// Whether this host supports SubagentStop events.
    fn supports_subagent_stop(&self) -> bool {
        false
    }

    /// Optional worktree auto-save audit result injection.
    /// Default delegates to `worktree_auto_save::take_audit_result`.
    fn take_audit_result(&self, repo_root: &Path) -> Option<String> {
        crate::hosts::worktree_auto_save::take_audit_result(repo_root, self.host_id())
    }

    /// Called at the start of `dispatch()` to ensure kernel bootstrap.
    /// Default delegates to `crate::hooks::ensure_kernel_bootstrap`.
    fn ensure_dispatch_bootstrap(&self) {
        crate::hooks::ensure_kernel_bootstrap();
    }

    /// Closeout stop-followup check for completion text patterns.
    /// Default delegates to `crate::hooks::closeout_stop_followup_for_completion_text`.
    fn closeout_check(&self, repo_root: &Path, text: &str) -> Option<String> {
        crate::hooks::closeout_stop_followup_for_completion_text(repo_root, text)
    }
}

/// Core trait: unified hook dispatch for all 4 closed-set hosts.
///
/// All methods have default implementations. Hosts override only when
/// they have truly unique protocol requirements — which should be rare.
/// The goal is that all 4 hosts use the same code path.
pub trait HostHookDispatcher: HostHookConfig {
    // ── (A) Pure shared — default implementations, never override ──

    /// PreToolUse: path protection interception. Default: no-op.
    /// Codex overrides with custom path protection logic.
    fn handle_pre_tool_use(&self, _event: &HookEvent) -> Option<HookOutput> {
        None
    }

    /// UserPromptSubmit: review gate init + context injection.
    /// Default: extract prompt, inject review/advisory context.
    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput> {
        let prompt = extract_prompt_text(event.payload);
        if is_review_gate_suppressed(self.host_id(), Some(event.repo_root), &prompt) {
            return None;
        }
        let contexts = build_user_prompt_context_injection(
            event.repo_root,
            &prompt,
            self.host_id(),
            self.host_id(),
            false,
            core_policy::hook_common::has_override(&prompt),
        );
        if contexts.is_empty() { None } else {
            Some(HookOutput::AdditionalContext(contexts.join("\n")))
        }
    }

    /// PostToolUse: evidence collection + subagent tracking.
    /// Default: record telemetry + auto evidence.
    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        crate::hooks::ensure_kernel_bootstrap();
        let tool_name = crate::hosts::hook_dispatch::extract_tool_name(event.payload);
        let normalized = core_policy::hook_common::normalize_tool_name(Some(&tool_name));
        crate::hosts::hook_dispatch::record_tool_call_emission(
            event.repo_root,
            &normalized,
            crate::hooks::extract_post_tool_duration_ms(event.payload).unwrap_or(0),
            crate::hooks::post_tool_call_succeeded(event.payload),
        );
        crate::hosts::hook_dispatch::auto_record_verification_evidence(event.repo_root, event.payload);
        crate::hosts::hook_dispatch::auto_record_research_activity(event.repo_root, event.payload);
        None
    }

    // ── (B) Shared + extension — default skeleton, host CAN override ──

    /// Stop: closeout gate + review gate check.
    /// Default: closeout_followup check. Hosts with review gates override.
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        let completion_text = extract_completion_text(event);
        if let Some(msg) = self.closeout_check(event.repo_root, &completion_text) {
            return Some(HookOutput::Block { reason: msg });
        }
        None
    }

    /// SessionStart: context injection.
    /// Default: operator_inject check + repo context.
    fn handle_session_start(&self, event: &HookEvent) -> Option<HookOutput> {
        if !crate::hooks::router_rs_operator_inject_globally_enabled() {
            return None;
        }
        let ctx = format!("Repo: {}", event.repo_root.display());
        Some(HookOutput::AdditionalContext(truncate_bytes(
            &ctx,
            self.additional_context_max_bytes(),
            "...",
        )))
    }

    /// SubagentStart: default no-op.
    fn handle_subagent_start(&self, _event: &HookEvent) -> Option<HookOutput> {
        None
    }

    /// SubagentStop: default no-op.
    fn handle_subagent_stop(&self, _event: &HookEvent) -> Option<HookOutput> {
        None
    }

    /// Unified dispatch entry. All hosts use the same routing logic.
    /// Calls `self.ensure_dispatch_bootstrap()` before dispatching to host handlers.
    fn dispatch(&self, event: &HookEvent) -> Option<HookOutput> {
        self.ensure_dispatch_bootstrap();
        let normalized = normalize_event_name(event.event_name);
        debug!(event = %normalized, host = %self.host_id(), "hook dispatch");
        let output = match normalized.as_ref() {
            "sessionstart" if self.supports_session_start() => self.handle_session_start(event),
            "userpromptsubmit" | "beforesubmitprompt" => self.handle_user_prompt_submit(event),
            "pretooluse" => self.handle_pre_tool_use(event),
            "posttooluse" => self.handle_post_tool_use(event),
            "stop" => self.handle_stop(event),
            "subagentstart" if self.supports_subagent_start() => self.handle_subagent_start(event),
            "subagentstop" if self.supports_subagent_stop() => self.handle_subagent_stop(event),
            _ => None,
        };
        // Inject pending session-start audit into the first event after
        // SessionStart — not into SessionStart itself. Written by
        // dispatch_hook_command(), consumed once here.
        if output.is_none() && normalized.as_ref() != "sessionstart"
            && let Some(audit) = self.take_audit_result(event.repo_root) {
                return Some(HookOutput::AdditionalContext(audit));
            }
        output
    }
}

// ────────────────────────────────────────────────────────────────
// Shared utilities (used by all host implementations)
// ────────────────────────────────────────────────────────────────

/// Normalize event name from various host formats (PascalCase, camelCase, kebab-case).
/// Returns Cow<str> to avoid allocation for already-canonical names.
pub fn normalize_event_name(name: &str) -> std::borrow::Cow<'_, str> {
    // Fast path: check if already lowercase with no separators (most common case)
    if name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        return std::borrow::Cow::Borrowed(name);
    }
    let lower = name.to_lowercase();
    // Map common variants to canonical names
    match lower.as_str() {
        "sessionstart" | "session-start" | "session.start" => std::borrow::Cow::Borrowed("sessionstart"),
        "userpromptsubmit"
        | "user-prompt-submit"
        | "user.prompt.submit"
        | "beforesubmitprompt"
        | "before-submit-prompt" => std::borrow::Cow::Borrowed("userpromptsubmit"),
        "pretooluse" | "pre-tool-use" | "pre.tool.use" | "tool.execute.before" => {
            std::borrow::Cow::Borrowed("pretooluse")
        }
        "posttooluse" | "post-tool-use" | "post.tool.use" | "tool.execute.after" => {
            std::borrow::Cow::Borrowed("posttooluse")
        }
        "stop" | "session.idle" => std::borrow::Cow::Borrowed("stop"),
        "subagentstart" | "subagent-start" | "subagent.start" => std::borrow::Cow::Borrowed("subagentstart"),
        "subagentstop" | "subagent-stop" | "subagent.end" => std::borrow::Cow::Borrowed("subagentstop"),
        other => std::borrow::Cow::Owned(other.to_string()),
    }
}

/// Extract prompt text from event payload, trying all common field names.
/// This is the superset of all host field names (13 direct keys).
///
/// Field ordering is by observed frequency across closed-set hosts:
///   - `prompt`, `user_prompt`: Claude + OpenCode (highest frequency)
///   - `message`, `content`: multi-host generic payloads
///   - `input`, `text`: OpenCode + generic
///   - `userPrompt`, `userMessage`: Cursor (camelCase)
///   - remaining: rare / host-specific fallbacks
pub fn extract_prompt_text(event: &Value) -> String {
    const KEYS: &[&str] = &[
        "prompt",
        "user_prompt",
        "message",
        "content",
        "input",
        "text",
        "userPrompt",
        "userMessage",
        "query",
        "userContent",
        "command",
        "composerText",
        "editorText",
    ];
    for key in KEYS {
        if let Some(value) = event.get(*key).and_then(Value::as_str)
            && !value.trim().is_empty() {
                return value.to_string();
            }
    }
    // Fallback: check prompt-like keys inside nested payload objects
    if let Some(obj) = event.as_object() {
        const NESTED: &[&str] = &["payload", "hookPayload", "data", "body", "hook_input"];
        for nest_key in NESTED {
            if let Some(nested) = obj.get(*nest_key).and_then(Value::as_object) {
                for key in KEYS {
                    if let Some(value) = nested.get(*key).and_then(Value::as_str)
                        && !value.trim().is_empty() {
                            return value.to_string();
                        }
                }
            }
        }
        // Also check tool_input for prompt-like keys (Cursor nested payload convention)
        for ti_key in &["tool_input", "input", "arguments"] {
            if let Some(ti) = obj.get(*ti_key).and_then(Value::as_object) {
                for key in KEYS {
                    if let Some(value) = ti.get(*key).and_then(Value::as_str)
                        && !value.trim().is_empty() {
                            return value.to_string();
                        }
                }
            }
        }
    }
    // Fallback: scan nested messages arrays for last user message
    extract_prompt_from_nested_messages(event)
}

/// Scan nested messages arrays for the last user-role message.
pub fn extract_prompt_from_nested_messages(event: &Value) -> String {
    extract_prompt_from_nested_messages_inner(event, 0)
}

const MAX_NESTING_DEPTH: usize = 4;

fn extract_prompt_from_nested_messages_inner(event: &Value, depth: usize) -> String {
    if depth > MAX_NESTING_DEPTH {
        return String::new();
    }
    const MESSAGE_KEYS: &[&str] = &[
        "messages",
        "conversationMessages",
        "chatMessages",
        "history",
    ];
    const NESTED_KEYS: &[&str] = &["hookPayload", "data", "body", "payload", "event"];

    if let Some(obj) = event.as_object() {
        // Try message arrays
        for key in MESSAGE_KEYS {
            if let Some(Value::Array(arr)) = obj.get(*key) {
                for item in arr.iter().rev() {
                    if let Some(msg) = item.as_object()
                        && is_user_message_role(msg)
                            && let Some(text) = message_body_text(msg) {
                                return text;
                            }
                }
            }
        }
        // Recurse into nested containers (with depth limit)
        for key in NESTED_KEYS {
            if let Some(nested) = obj.get(*key) {
                let s = extract_prompt_from_nested_messages_inner(nested, depth + 1);
                if !s.trim().is_empty() {
                    return s;
                }
            }
        }
    }
    String::new()
}

/// Check if a message object has a user/human role.
pub fn is_user_message_role(obj: &serde_json::Map<String, Value>) -> bool {
    let role = obj
        .get("role")
        .or_else(|| obj.get("type"))
        .or_else(|| obj.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(role.as_str(), "user" | "human")
}

/// Extract body text from a message object.
pub fn message_body_text(msg: &serde_json::Map<String, Value>) -> Option<String> {
    for key in &["content", "text", "body", "message", "value"] {
        if let Some(val) = msg.get(*key) {
            if let Some(s) = val.as_str()
                && !s.trim().is_empty() {
                    return Some(s.to_string());
                }
            // Handle content as array of parts (Claude/OpenAI format)
            if let Some(arr) = val.as_array() {
                let text: String = arr
                    .iter()
                    .filter_map(|part| {
                        part.get("text")
                            .or_else(|| part.get("content"))
                            .and_then(Value::as_str)
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// Extract tool name from event payload.
pub fn extract_tool_name(event: &Value) -> String {
    let from_obj = |obj: &Map<String, Value>| {
        obj.get("tool_name")
            .or(obj.get("tool"))
            .or(obj.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    if let Some(name) = event.as_object().and_then(from_obj) {
        return name;
    }
    // Fallback: check inside nested payload objects
    if let Some(obj) = event.as_object() {
        for nest_key in &["payload", "hookPayload", "data", "body"] {
            if let Some(nested) = obj.get(*nest_key).and_then(Value::as_object) {
                if let Some(name) = from_obj(nested) {
                    return name;
                }
            }
        }
    }
    String::new()
}

/// Extract tool input from event payload.
pub fn extract_tool_input(event: &Value) -> Value {
    let from_obj = |obj: &Map<String, Value>| {
        obj.get("tool_input")
            .or(obj.get("input"))
            .or(obj.get("arguments"))
            .cloned()
            .filter(Value::is_object)
    };
    if let Some(input) = event.as_object().and_then(from_obj) {
        return input;
    }
    // Fallback: check inside nested payload objects
    if let Some(obj) = event.as_object() {
        for nest_key in &["payload", "hookPayload", "data", "body"] {
            if let Some(nested) = obj.get(*nest_key).and_then(Value::as_object) {
                if let Some(input) = from_obj(nested) {
                    return input;
                }
            }
        }
    }
    serde_json::json!({})
}

/// Extract completion text from Stop event payload.
pub fn extract_completion_text(event: &HookEvent) -> String {
    event
        .payload
        .get("completion_text")
        .or(event.payload.get("response"))
        .or(event.payload.get("result"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Shared stop signal text: combines prompt + assistant response for gate detection.
/// Used by Claude, Codex, OpenCode Stop handlers. Cursor uses `hook_event_signal_text` with scrape.
pub fn stop_signal_text_from_payload(payload: &Value) -> String {
    let prompt = extract_prompt_text(payload);
    let response = extract_response_text(payload);
    if prompt.trim().is_empty() {
        response
    } else if response.trim().is_empty() {
        prompt
    } else {
        format!("{prompt}\n{response}")
    }
}

/// Extract assistant response text from payload, covering all host response key variants.
/// Keys checked: response, agent_response, agentResponse, assistant_response,
/// last_assistant_message, content, text, output (first non-empty wins).
pub fn extract_response_text(payload: &Value) -> String {
    const RESPONSE_KEYS: &[&str] = &[
        "response",
        "agent_response",
        "agentResponse",
        "assistant_response",
        "last_assistant_message",
        "content",
        "text",
        "output",
    ];
    for key in RESPONSE_KEYS {
        if let Some(value) = payload.get(*key).and_then(Value::as_str)
            && !value.trim().is_empty() {
                return value.to_string();
            }
    }
    String::new()
}

/// Borrow response text directly from payload (zero-alloc when possible).
/// Returns `None` if no response key is found or value is not a string.
pub fn borrow_response_text<'a>(payload: &'a Value) -> Option<&'a str> {
    const RESPONSE_KEYS: &[&str] = &[
        "response",
        "agent_response",
        "agentResponse",
        "assistant_response",
        "last_assistant_message",
        "content",
        "text",
        "output",
    ];
    for key in RESPONSE_KEYS {
        if let Some(value) = payload.get(*key).and_then(Value::as_str)
            && !value.trim().is_empty() {
                return Some(value);
            }
    }
    None
}

/// Re-export from core-policy (single source of truth).
pub use core_policy::subagent::{SUBAGENT_TOOL_NAMES, is_subagent_tool};

/// Recognized subagent type names for review gate tracking.
pub const SUBAGENT_REVIEW_TYPES: &[&str] = &[
    "explore",
    "explorer",
    "general-purpose",
    "deep-review-agent",
    "review",
    "verifyx-agent",
    "plan",
    "claude",
];

/// Extract and normalize subagent type from tool input fields.
pub fn recognize_subagent_type(tool_input: &Value) -> Option<String> {
    use core_policy::hook_common::normalize_subagent_type;
    let typed_fields = [
        tool_input.get("subagent_type").and_then(Value::as_str),
        tool_input.get("agent_type").and_then(Value::as_str),
        tool_input.get("agentType").and_then(Value::as_str),
        tool_input.get("type").and_then(Value::as_str),
    ];
    typed_fields
        .into_iter()
        .map(|field| normalize_subagent_type(field))
        .find(|normalized| SUBAGENT_REVIEW_TYPES.contains(&normalized.as_str()))
}

/// Compute review lane and parallel lane bits from subagent kind.
pub fn subagent_lane_bits(kind: Option<&str>) -> (bool, bool) {
    let Some(k) = kind else {
        return (false, false);
    };
    let review_lane = SUBAGENT_REVIEW_TYPES.contains(&k);
    let parallel_lane = matches!(k, "general-purpose" | "deep-review-agent" | "claude");
    (review_lane, parallel_lane)
}

/// Default review types (used by most hosts).
pub const DEFAULT_REVIEW_TYPES: &[&str] = SUBAGENT_REVIEW_TYPES;

/// Extended review types (includes worker/shell variants for hosts that use them).
pub const EXTENDED_REVIEW_TYPES: &[&str] = &[
    "explore", "explorer", "general-purpose", "generalpurpose",
    "default", "shell", "worker", "browser-use", "browseruse",
    "ci-investigator", "ciinvestigator", "best-of-n-runner", "bestofnrunner",
    "cursor-guide", "cursorguide",
];

/// Compute review lane and parallel lane bits using a provided review type set.
pub fn subagent_lane_bits_with_types(kind: Option<&str>, review_types: &[&str]) -> (bool, bool) {
    let Some(k) = kind else { return (false, false); };
    let review_lane = review_types.contains(&k);
    let parallel_lane = SUBAGENT_REVIEW_TYPES.contains(&k);
    (review_lane, parallel_lane)
}

/// Host-aware subagent lane bits. Uses host-specific review type set.
pub fn subagent_lane_bits_for_host(kind: Option<&str>, host_id: &str) -> (bool, bool) {
    let review_types = crate::hosts::host_provider_for_id(host_id)
        .map(|p| p.subagent_review_types())
        .unwrap_or(DEFAULT_REVIEW_TYPES);
    subagent_lane_bits_with_types(kind, review_types)
}

/// Truncate string preserving UTF-8 character boundaries, with optional suffix.
pub fn truncate_bytes(s: &str, max_bytes: usize, suffix: &str) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let suffix_len = suffix.len();
    let target = max_bytes.saturating_sub(suffix_len);
    let mut end = target;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{suffix}", &s[..end])
}

/// Compact multiple context parts: dedup + join + truncate with suffix.
pub fn compact_contexts(parts: Vec<String>, max_bytes: usize) -> Option<String> {
    compact_contexts_with_suffix(parts, max_bytes, "...")
}

/// Compact multiple context parts with configurable truncation suffix.
pub fn compact_contexts_with_suffix(
    parts: Vec<String>,
    max_bytes: usize,
    suffix: &str,
) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<&str> = parts
        .iter()
        .filter(|p| !p.is_empty() && seen.insert(p.as_str()))
        .map(|s| s.as_str())
        .collect();
    if deduped.is_empty() {
        return None;
    }
    let combined = deduped.join("\n");
    Some(truncate_bytes(&combined, max_bytes, suffix))
}

/// Check if review gate is suppressed for this host/prompt combination.
pub fn is_review_gate_suppressed(host_id: &str, repo_root: Option<&Path>, prompt: &str) -> bool {
    core_policy::env_flags::router_rs_review_gate_disabled_for_host(host_id)
        || core_policy::hook_common::review_gate_hard_block_disabled(repo_root, prompt)
}

/// Standard cwd field names used across all hosts for session key fallback.
///
/// Re-exports `core_policy::session_key::SESSION_KEY_CWD_FIELDS` for host-projection consumers.
pub use core_policy::session_key::SESSION_KEY_CWD_FIELDS;

/// Standard session id field names (4-host superset).
pub use core_policy::session_key::SESSION_ID_FIELDS;

/// Tool-input parent session id field names (Cursor scan_tool_input).
pub use core_policy::session_key::TOOL_INPUT_SESSION_ID_FIELDS;

/// Tool-input metadata session id field names.
pub use core_policy::session_key::TOOL_INPUT_METADATA_SESSION_ID_FIELDS;

/// Extract session key using core_policy's shared `session_key_core`.
///
/// This is the canonical session key extraction used by all hosts.
/// Each host provides `env_var` (via `HostHookConfig::session_namespace_env`),
/// `repo_fallback_token` (derived from repo_root), and `scan_tool_input`
/// (Cursor sets this to `true` to scan `tool_input` for parent session ids).
pub fn extract_session_key(
    event: &Value,
    env_var: &'static str,
    repo_fallback: &str,
    scan_tool_input: bool,
) -> String {
    core_policy::session_key::session_key_core(
        &core_policy::session_key::SessionKeyConfig { env_var, scan_tool_input },
        || {
            // When scan_tool_input, first check root-level fields,
            // then tool_input, then nested objects.
            if scan_tool_input {
                if let Some(s) = extract_session_id_from_payload(event) {
                    return Some(s);
                }
                let tool_input = extract_tool_input(event);
                if let Some(s) = extract_session_id_from_tool_input(&tool_input) {
                    return Some(s);
                }
                return extract_session_id_from_nested(event);
            }
            extract_session_id_from_payload(event)
        },
        || extract_cwd_from_payload(event),
        repo_fallback,
    )
}

/// Extract explicit session id from payload (tries shared SESSION_ID_FIELDS).
fn extract_session_id_from_payload(event: &Value) -> Option<String> {
    for key in core_policy::session_key::SESSION_ID_FIELDS {
        if let Some(val) = event.get(*key).and_then(Value::as_str)
            && !val.is_empty() {
                return Some(val.to_string());
            }
    }
    None
}

/// Extract parent session id from `tool_input` object (Cursor scan_tool_input path).
fn extract_session_id_from_tool_input(tool_input: &Value) -> Option<String> {
    let obj = tool_input.as_object()?;
    for key in core_policy::session_key::TOOL_INPUT_SESSION_ID_FIELDS {
        if let Some(value) = obj.get(*key).and_then(Value::as_str) {
            let t = value.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    // Check nested metadata
    if let Some(meta) = obj.get("metadata").and_then(Value::as_object) {
        for key in core_policy::session_key::TOOL_INPUT_METADATA_SESSION_ID_FIELDS {
            if let Some(value) = meta.get(*key).and_then(Value::as_str) {
                let t = value.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// Extract session id from nested objects (e.g. `hookPayload`).
fn extract_session_id_from_nested(event: &Value) -> Option<String> {
    for nest in &["hookPayload", "metadata", "context"] {
        if let Some(nobj) = event.get(*nest).and_then(Value::as_object) {
            for key in core_policy::session_key::SESSION_ID_FIELDS {
                if let Some(val) = nobj.get(*key).and_then(Value::as_str)
                    && !val.is_empty() {
                        return Some(val.to_string());
                    }
            }
        }
    }
    None
}

/// Extract cwd from payload using standard field names.
fn extract_cwd_from_payload(event: &Value) -> Option<String> {
    for key in SESSION_KEY_CWD_FIELDS {
        if let Some(val) = event.get(*key).and_then(Value::as_str)
            && !val.is_empty() {
                return Some(val.to_string());
            }
    }
    None
}

/// Session key hash helper (delegates to core-policy crypto_util).
pub fn short_hash_for_session(input: &str) -> String {
    core_policy::crypto_util::short_hash(input)
}

/// Check if a shell command is a verification/test command.
/// Shared across all hosts for PostToolUse evidence collection.
pub fn is_verification_command(tool_name: &str, command: &str) -> bool {
    let name_lower = tool_name.to_ascii_lowercase();
    if !name_lower.contains("bash")
        && !name_lower.contains("shell")
        && !name_lower.contains("exec")
        && !name_lower.contains("terminal")
    {
        return false;
    }
    let cmd_lower = command.to_ascii_lowercase();
    cmd_lower.contains("cargo test")
        || cmd_lower.contains("cargo check")
        || cmd_lower.contains("cargo build")
        || cmd_lower.contains("cargo clippy")
        || cmd_lower.contains("cargo fmt")
        || cmd_lower.contains("npm test")
        || cmd_lower.contains("npm run test")
        || cmd_lower.contains("pytest")
        || cmd_lower.contains("make test")
        || cmd_lower.contains("make check")
        || cmd_lower.contains("go test")
        || cmd_lower.contains("git diff")
        || cmd_lower.contains("git log")
}

// ────────────────────────────────────────────────────────────────
// Shared Stop decision logic (used by all hosts)
// ────────────────────────────────────────────────────────────────

/// `need=` segment for REVIEW_GATE incomplete stop lines.
/// Shared across all hosts for consistent observation classification.
pub const REVIEW_GATE_FOLLOWUP_NEED_SEGMENT: &str =
    "need=deep_reviewer_cycle general-purpose|best-of-n|deep-reviewer fork_context=false";

/// Stable hint suffix for REVIEW_GATE incomplete lines.
pub const REVIEW_GATE_FOLLOWUP_HINT_SEGMENT: &str =
    "hint=fork_context_json_false_not_omitted";

/// `merge_hook_nudge_paragraph` dedup prefix for REVIEW_GATE detail.
pub const REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX: &str = "router-rs REVIEW_GATE detail";

/// Check if goal tracking is active for this state.
/// Shared: tracks whether `goal_required` or `goal_drive_entry_active` is set.
pub fn shared_tracks_goal(goal_required: bool, goal_drive_entry_active: bool) -> bool {
    goal_required || goal_drive_entry_active
}

/// Check if the goal gate is satisfied.
/// Shared decision logic: goal is satisfied when:
/// 1. Goal tracking is not active, OR
/// 2. Override is in effect, OR
/// 3. All three signals (contract, progress, verify) are seen.
pub fn shared_goal_is_satisfied(
    goal_required: bool,
    goal_drive_entry_active: bool,
    goal_contract_seen: bool,
    goal_progress_seen: bool,
    goal_verify_or_block_seen: bool,
    review_override: bool,
    delegation_override: bool,
) -> bool {
    if !shared_tracks_goal(goal_required, goal_drive_entry_active) {
        return true;
    }
    if review_override || delegation_override {
        return true;
    }
    goal_contract_seen && goal_progress_seen && goal_verify_or_block_seen
}

/// Check if review output lint should be suppressed during Stop.
/// Shared: skip lint when review gate or goal followup is active.
pub fn shared_stop_review_output_lint_suppressed(
    review_advisory_needed: bool,
    goal_required: bool,
    goal_drive_entry_active: bool,
    goal_contract_seen: bool,
    goal_progress_seen: bool,
    goal_verify_or_block_seen: bool,
    review_override: bool,
    delegation_override: bool,
) -> bool {
    if review_advisory_needed {
        return true;
    }
    if shared_tracks_goal(goal_required, goal_drive_entry_active)
        && !shared_goal_is_satisfied(
            goal_required,
            goal_drive_entry_active,
            goal_contract_seen,
            goal_progress_seen,
            goal_verify_or_block_seen,
            review_override,
            delegation_override,
        )
    {
        return true;
    }
    false
}

/// Unified goal gate update — **single implementation for all 4 hosts**.
///
/// Call this from each host's Stop/PostTool handler. It:
/// 1. Detects goal drive entry from prompt (via `is_framework_goal_entry_prompt`)
/// 2. Detects goal signals from response text (contract / progress / verify)
/// 3. Optionally reads disk state via `hooks::evaluate_goal_readiness_from_disk` (more precise)
/// 4. Updates `HookReviewDiskCore` fields in-place
///
/// Hosts should pass their `review_state.core` (or equivalent `HookReviewDiskCore`).
pub fn update_goal_gate(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    goal_drive_entrypoint: bool,
) {
    update_goal_gate_with_disk(core, prompt, response_text, goal_drive_entrypoint, None, None)
}

/// Extended goal gate update with optional disk-based readiness evaluation.
///
/// When `repo_root` and `task_id` are provided, reads `GOAL_STATE.json` via
/// `hooks::evaluate_goal_readiness_from_disk` for more precise signal detection.
/// Disk signals are merged with regex-based signals (union: either can arm a field).
pub fn update_goal_gate_with_disk(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    goal_drive_entrypoint: bool,
    repo_root: Option<&std::path::Path>,
    task_id: Option<&str>,
) {
    // Arm goal drive on entry
    if goal_drive_entrypoint {
        core.goal_drive_entry_active = true;
    }
    // Only scan for signals if goal tracking is active
    if !core.goal_drive_entry_active {
        return;
    }
    // Scan combined signal text for goal signals (regex-based, all hosts)
    let signal = if prompt.is_empty() {
        response_text.to_string()
    } else if response_text.is_empty() {
        prompt.to_string()
    } else {
        format!("{prompt}\n{response_text}")
    };
    if core_policy::hook_common::has_structured_goal_contract(&signal) {
        core.goal_contract_seen = true;
    }
    if core_policy::hook_common::has_goal_progress_signal(&signal) {
        core.goal_progress_seen = true;
    }
    if core_policy::hook_common::has_goal_verify_or_block_signal(&signal) {
        core.goal_verify_or_block_seen = true;
    }
    // Disk-based readiness (more precise: reads GOAL_STATE.json + EVIDENCE_INDEX.json)
    if let (Some(root), Some(tid)) = (repo_root, task_id) {
        let goal_val = serde_json::Value::Null; // placeholder; real evaluator reads disk
        let readiness = crate::hooks::evaluate_goal_readiness_from_disk(root, &goal_val, tid);
        if readiness.contract {
            core.goal_contract_seen = true;
        }
        if readiness.progress {
            core.goal_progress_seen = true;
        }
        if readiness.verification {
            core.goal_verify_or_block_seen = true;
        }
    }
}

/// Check if goal gate is satisfied using shared `HookReviewDiskCore` fields.
pub fn goal_gate_satisfied(core: &core_policy::HookReviewDiskCore) -> bool {
    shared_goal_is_satisfied(
        false, // goal_required is Cursor-specific; shared uses goal_drive_entry_active
        core.goal_drive_entry_active,
        core.goal_contract_seen,
        core.goal_progress_seen,
        core.goal_verify_or_block_seen,
        core.review_override,
        core.delegation_override,
    )
}

/// Generate the goal stop followup line using shared logic.
/// Phase-aware: includes short code for goal drive continuation.
pub fn shared_goal_stop_followup_line(
    goal_contract_seen: bool,
    goal_progress_seen: bool,
    goal_verify_or_block_seen: bool,
    goal_followup_count: u32,
) -> String {
    let missing = {
        let mut m = Vec::new();
        if !goal_contract_seen {
            m.push("contract");
        }
        if !goal_progress_seen {
            m.push("progress");
        }
        if !goal_verify_or_block_seen {
            m.push("verify");
        }
        m.join(",")
    };
    format!(
        "router-rs GOAL_FOLLOWUP missing={} nudge={}",
        missing, goal_followup_count
    )
}

/// Shared advisory for settings changed but not validated.
pub fn shared_settings_validation_advisory() -> String {
    "Validate Claude hook/settings JSON before ending this turn.".to_string()
}

/// Shared advisory for framework source changed but not tested.
pub fn shared_framework_test_advisory() -> String {
    "Framework source files were modified. Consider running tests.".to_string()
}

// ════════════════════════════════════════════════════════════════════
// Shared handler logic (4-host unification)
// ════════════════════════════════════════════════════════════════════

/// Record tool call telemetry + session tracking (PostToolUse).
/// All 4 hosts emit the same 2-line sequence; call once after extracting tool_name + duration.
pub fn record_tool_call_emission(repo_root: &Path, tool_name: &str, duration_ms: u64, succeeded: bool) {
    crate::hooks::emit_tool_call(tool_name, duration_ms, succeeded);
    if let Err(e) = crate::hooks::record_tool_call(repo_root, tool_name, None) {
        eprintln!("[router-rs] session tracker record_tool_call failed (non-fatal): {e}");
    }
}

/// Merge review gate state on UserPromptSubmit (pure logic, no I/O).
///
/// All 4 hosts share this core sequence:
/// 1. my_light / goal_drive / narrow → suppress review (clear `review_required` + `independent_reviewer_seen`)
/// 2. review_arms && !override_now → clear `independent_reviewer_seen` (fresh cycle)
/// 3. Accumulate: `review_required = prev || review_arms`, `review_override = prev || override_now`
///
/// Returns the updated `HookReviewDiskCore` and flags for the caller:
/// - `review_required` (post-merge): whether review gate is armed
/// - `review_override` (post-merge): whether override is active
/// - `fresh_cycle`: whether a new review cycle was armed this call
pub struct ReviewGateMergeResult {
    pub core: core_policy::HookReviewDiskCore,
    pub review_arms: bool,
    pub override_now: bool,
    pub fresh_cycle: bool,
    pub suppressed: bool,
}

pub fn merge_review_gate_on_user_prompt(
    prev: &core_policy::HookReviewDiskCore,
    prompt: &str,
    repo_root: &Path,
    host_id: &str,
) -> ReviewGateMergeResult {
    let suppressed = is_review_gate_suppressed(host_id, Some(repo_root), prompt);
    if suppressed {
        return ReviewGateMergeResult {
            core: prev.clone(),
            review_arms: false,
            override_now: false,
            fresh_cycle: false,
            suppressed: true,
        };
    }

    let interactive = core_policy::hook_common::is_interactive_profile(Some(repo_root), prompt);
    let goal_drive = core_policy::hook_common::is_framework_goal_entry_prompt(prompt);
    let narrow = core_policy::hook_common::is_narrow_review_prompt(prompt);
    let review_arms = core_policy::hook_common::is_review_prompt(prompt) && !goal_drive;
    let override_now = core_policy::hook_common::has_override(prompt);

    let mut core = prev.clone();

    if interactive || goal_drive || narrow {
        core.review_required = false;
        core.independent_reviewer_seen = false;
    } else {
        if review_arms && !override_now {
            core.independent_reviewer_seen = false;
        }
        core.review_required = core.review_required || review_arms;
    }
    core.review_override = core.review_override || override_now;

    let fresh_cycle = review_arms && !override_now && !interactive && !goal_drive && !narrow;

    ReviewGateMergeResult {
        core,
        review_arms,
        override_now,
        fresh_cycle,
        suppressed: false,
    }
}

/// Apply override + reject detection to review gate state (Stop event).
/// All 4 hosts run this sequence. Call before gate evaluation.
pub fn apply_override_and_reject(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    stop_signal: &str,
) {
    if core_policy::hook_common::has_override(prompt) {
        core.review_override = true;
        core.delegation_override = true;
    }
    if core_policy::hook_common::saw_reject_reason(stop_signal, prompt)
        || core_policy::hook_common::saw_reject_reason(prompt, stop_signal)
    {
        core.reject_reason_seen = true;
        core.followup_count = 0;
        core.review_followup_count = 0;
    }
}

/// Stop decision enum — returned by `evaluate_stop_decision`.
pub enum StopDecision {
    /// Closeout advisory (unmet closeout evidence).
    Closeout { message: String },
    /// Review gate nudge (unmet review requirements).
    ReviewGateNudge { message: String },
    /// Goal followup nudge (unmet goal contract/progress/verify).
    GoalFollowup { message: String },
    /// All gates satisfied — safe to stop.
    Clean,
}

/// Evaluate the full stop decision sequence (pure logic, no I/O).
///
/// All 4 hosts run the same pipeline:
/// 1. Closeout check → advisory
/// 2. Override + reject detection
/// 3. Goal gate update (via `update_goal_gate`)
/// 4. Review gate check → advisory nudge
/// 5. Goal followup check → advisory nudge
/// 6. Clean
///
/// Callers must pass mutable `HookReviewDiskCore` (override/reject mutations applied in-place).
pub fn evaluate_stop_decision(
    core: &mut core_policy::HookReviewDiskCore,
    prompt: &str,
    response_text: &str,
    stop_signal: &str,
    completion_text: &str,
    repo_root: &Path,
    host_id: &str,
) -> StopDecision {
    // 1. Closeout
    if let Some(msg) = crate::hooks::closeout_stop_followup_for_completion_text(repo_root, completion_text) {
        return StopDecision::Closeout { message: msg };
    }

    // 2. Override + reject
    apply_override_and_reject(core, prompt, stop_signal);

    // 3. Goal gate update
    let goal_entry = core_policy::hook_common::is_framework_goal_entry_prompt(prompt);
    update_goal_gate(core, prompt, response_text, goal_entry);

    // 4. Review gate
    let gate_fields = core_policy::hook_review_gate_fields_from_parts(
        core.review_required,
        core.review_override,
        core.independent_reviewer_seen,
        core.reject_reason_seen,
    );
    if let Some(nudge) = core_policy::hook_review_stop_advisory_needed(
        &gate_fields,
        &format!("{}_REVIEW_GATE", host_id.to_ascii_uppercase()),
    ) {
        return StopDecision::ReviewGateNudge { message: nudge };
    }

    // 5. Goal followup
    if !goal_gate_satisfied(core) {
        let followup = shared_goal_stop_followup_line(
            core.goal_contract_seen,
            core.goal_progress_seen,
            core.goal_verify_or_block_seen,
            core.goal_followup_count,
        );
        return StopDecision::GoalFollowup { message: followup };
    }

    StopDecision::Clean
}

/// Build UserPromptSubmit additional context (spawn-first nudge + paper context).
/// All 4 hosts inject the same context sequence. Returns empty vec if nothing to inject.
pub fn build_user_prompt_context_injection(
    repo_root: &Path,
    prompt: &str,
    host_id: &str,
    paper_host: crate::hooks::PaperProseHookHostType,
    review_required: bool,
    review_override: bool,
) -> Vec<String> {
    let mut contexts = Vec::new();

    // Spawn-first review nudge
    if review_required && !review_override {
        if core_policy::hook_common::should_inject_spawn_first_review_nudge(Some(repo_root), prompt) {
            contexts.push(core_policy::registry_review_gate::review_spawn_first_nudge_line(
                Some(repo_root),
                host_id,
            ));
        }
    }

    // Paper context injection
    crate::hooks::maybe_append_paper_adversarial_context(repo_root, prompt, &mut contexts, paper_host);
    crate::hooks::maybe_append_paper_prose_context(repo_root, prompt, &mut contexts, paper_host);

    contexts
}

/// Detect reviewer evidence from PostToolUse (fork_context + review lane).
/// Returns true if independent_reviewer_seen should be armed.
/// All 4 hosts run this same detection after subagent type recognition.
pub fn detect_reviewer_evidence(
    tool_input: &Value,
    reviewer_lane: bool,
) -> bool {
    if !reviewer_lane {
        return false;
    }
    let fork = extract_fork_context(tool_input);
    core_policy::review_gate_engine::review_independent_reviewer_evidence(fork, reviewer_lane)
}

/// Extract fork_context from tool input (tries multiple field names).
/// Returns `None` if field is absent or unparseable (Claude semantics: absent ≠ false).
fn extract_fork_context(tool_input: &Value) -> Option<bool> {
    tool_input
        .get("fork_context")
        .or_else(|| tool_input.get("forkContext"))
        .and_then(|v| {
            if let Some(b) = v.as_bool() {
                Some(b)
            } else if let Some(s) = v.as_str() {
                match s {
                    "true" | "1" | "yes" => Some(true),
                    "false" | "0" | "no" => Some(false),
                    _ => None,
                }
            } else {
                None
            }
        })
}

// ════════════════════════════════════════════════════════════════
// Shared payload/state helpers (migrated from host_extensions::claude)
// These are host-agnostic utilities used by all host implementations.
// ════════════════════════════════════════════════════════════════

use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, PathBuf};

/// Lexically normalize `.` / `..` segments (no filesystem access).
pub fn normalize_path_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => { out.push(comp); }
            Component::CurDir => {}
            Component::ParentDir => {
                if out.file_name().is_some() { out.pop(); } else { out.push(".."); }
            }
            Component::Normal(c) => out.push(c),
        }
    }
    out
}

/// Collapse `.` / `..` in a relative path string. Extra `..` at virtual root are ignored.
pub fn compact_repo_relative_segments(rel_raw: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in Path::new(rel_raw).components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(s) => out.push(s),
            Component::ParentDir => {
                if out.file_name().is_some() { out.pop(); }
            }
            _ => {}
        }
    }
    if out.as_os_str().is_empty() { None } else { Some(out) }
}

/// Build a `context_append` JSON output for a hook event.
pub fn add_context(event: &str, context: &str) -> Option<Value> {
    Some(serde_json::json!({ "context_append": format!("[{event}] {context}") }))
}

/// Silent success response (empty JSON object).
pub fn silent_success() -> Value { serde_json::json!({}) }

/// Extract completion/closeout text from a Stop payload.
pub fn closeout_completion_text(payload: &Value) -> String {
    payload.get("response")
        .or_else(|| payload.get("assistant_response"))
        .or_else(|| payload.get("last_assistant_message"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Extract the first non-empty string from a set of payload keys.
pub fn first_nonempty_payload_str(payload: &Value, keys: &[&str]) -> String {
    for key in keys {
        let val = payload.get(key).and_then(Value::as_str).unwrap_or("").trim();
        if !val.is_empty() { return val.to_string(); }
    }
    String::new()
}

/// Recursively collect path-like string values from JSON.
pub fn collect_payload_paths(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if is_path_key(k) {
                    collect_path_value(v, paths);
                } else if v.is_object() || v.is_array() {
                    collect_payload_paths(v, paths);
                }
            }
        }
        Value::Array(arr) => { for v in arr { collect_payload_paths(v, paths); } }
        _ => {}
    }
}

/// Collect path strings from a single value.
pub fn collect_path_value(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::String(s) => { if !s.is_empty() { paths.insert(s.clone()); } }
        Value::Array(arr) => { for v in arr { collect_path_value(v, paths); } }
        _ => {}
    }
}

/// Check if a JSON key name suggests it contains a file path.
pub fn is_path_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("path") || lower.contains("file") || lower.contains("dir")
        || lower.contains("directory") || lower == "cwd" || lower == "command"
}

/// Extract a numeric value from JSON by trying multiple keys.
pub fn find_numeric_key(value: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        if let Some(n) = value.get(key).and_then(Value::as_i64) { return Some(n); }
        if let Some(n) = value.get(key).and_then(Value::as_u64) { return Some(n as i64); }
        if let Some(s) = value.get(key).and_then(Value::as_str) {
            if let Ok(n) = s.trim().parse::<i64>() { return Some(n); }
        }
    }
    None
}

/// Extract bash command from a tool payload.
pub fn bash_command(payload: &Value) -> Option<&str> {
    payload.get("command").or_else(|| payload.get("cmd"))
        .and_then(Value::as_str).filter(|s| !s.trim().is_empty())
}

/// Check if a path looks like framework source code.
pub fn is_framework_source_path(path: &str) -> bool {
    path.contains("/core/") || path.contains("/src/") || path.ends_with(".rs")
}

/// Check if a path is a generated entrypoint file.
pub fn is_generated_entrypoint(path: &str) -> bool {
    path.contains("hook") && (path.ends_with(".sh") || path.ends_with(".json"))
}

/// Check if a path is host-private state (data-driven via generated `host_home_dirs`).
pub fn is_host_private_path(path: &str) -> bool {
    if path.contains("/hook-state/") { return true; }
    for dir in framework_kernel::runtime_registry::host_home_dirs() {
        if path.contains(&format!("/{dir}/")) { return true; }
    }
    false
}

/// Check if a path is a settings/config file.
pub fn is_settings_path(path: &str) -> bool {
    path.ends_with("settings.json") || path.ends_with("settings.local.json")
        || path.ends_with("hooks.json") || path.ends_with("config.json")
}

/// Check if a path should be guarded from modification.
pub fn is_framework_guarded_path(path: &str) -> bool {
    is_framework_source_path(path) || is_generated_entrypoint(path) || is_settings_path(path)
}

/// Extract exit code from a tool payload.
pub fn payload_exit_code(payload: &Value) -> Option<i64> {
    find_numeric_key(payload, &["exit_code", "exitCode", "exit_status"])
}

/// Check if a bash tool call succeeded.
pub fn payload_is_successful_bash(payload: &Value) -> bool {
    payload_exit_code(payload).map_or(true, |ec| ec == 0)
        && !payload.get("is_error").and_then(Value::as_bool).unwrap_or(false)
}

/// Check if a generic tool call succeeded.
pub fn payload_is_successful_tool(payload: &Value) -> bool {
    !payload.get("is_error").and_then(Value::as_bool).unwrap_or(false)
        && payload.get("error").map_or(true, |e| e.is_null())
}

/// Check if a tool name suggests a subagent/subprocess.
pub fn subagent_tool(payload: &Value) -> bool {
    let name = payload.get("tool_name").or_else(|| payload.get("tool"))
        .and_then(Value::as_str).unwrap_or("");
    tool_name_implies_subagent(name)
}

/// Check if a tool name implies subagent execution.
pub fn tool_name_implies_subagent(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("subagent") || lower.contains("spawn") || lower.contains("dispatch")
        || lower == "task" || lower.contains("sub_process")
}

/// Extract a summary of tool output for evidence tracking.
pub fn extract_output_summary(payload: &Value, max_chars: usize) -> Option<String> {
    let text = payload.get("output").or_else(|| payload.get("result"))
        .and_then(Value::as_str)?;
    if text.is_empty() { return None; }
    let trimmed = if text.len() > max_chars { &text[..max_chars] } else { text };
    Some(trimmed.to_string())
}

/// Convert a JSON value to a HookOutput.
pub fn value_to_hook_output(val: &Value) -> Option<HookOutput> {
    if val.is_null() { return None; }
    if let Some(s) = val.get("context_append").and_then(Value::as_str) {
        return Some(HookOutput::AdditionalContext(s.to_string()));
    }
    if let Some(s) = val.get("followup_message").and_then(Value::as_str) {
        return Some(HookOutput::Advisory { message: s.to_string() });
    }
    Some(HookOutput::Raw(val.clone()))
}

/// Convert a HookOutput to a JSON value (host-agnostic).
pub fn hook_output_to_json_value(event_name: &str, output: Option<HookOutput>) -> Value {
    match output {
        None => serde_json::json!({}),
        Some(HookOutput::None) => serde_json::json!({}),
        Some(HookOutput::AdditionalContext(ctx)) => serde_json::json!({ "context_append": format!("[{event_name}] {ctx}") }),
        Some(HookOutput::Advisory { message }) => serde_json::json!({ "followup_message": message }),
        Some(HookOutput::Block { reason }) => serde_json::json!({ "decision": "block", "reason": reason }),
        Some(HookOutput::Deny { reason }) => serde_json::json!({ "decision": "block", "reason": reason }),
        Some(HookOutput::Warn { message }) => serde_json::json!({ "warning": message }),
        Some(HookOutput::Raw(v)) => v,
    }
}

/// Read stdin with a 4 MiB limit and UTF-8 validation.
pub fn read_stdio_agent_stdin_limited<R: Read>(reader: &mut R) -> Result<String, String> {
    const MAX_BYTES: usize = 4 * 1024 * 1024;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 8192];
    loop {
        let n = reader.read(&mut tmp).map_err(|e| format!("stdin read error: {e}"))?;
        if n == 0 { break; }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > MAX_BYTES {
            return Err(format!("stdin exceeded {} byte limit", MAX_BYTES));
        }
    }
    String::from_utf8(buf).map_err(|e| format!("stdin is not valid UTF-8: {e}"))
}

/// Parse and trim stdin JSON, handling both raw JSON and wrapped envelopes.
pub fn parse_stdio_agent_hook_stdin_trimmed(trimmed: &str) -> Result<Value, String> {
    serde_json::from_str(trimmed).map_err(|e| format!("parse stdin JSON failed: {e}"))
}

/// Check if a payload looks like a foreign host's hook stdin envelope (not the active host).
/// Detects Cursor-style envelopes by checking for `cursor_version` + `workspace_roots`.
pub fn payload_looks_like_foreign_hook_stdin(payload: &Value) -> bool {
    payload.get("cursor_version").and_then(Value::as_str).is_some_and(|s| !s.is_empty())
        && payload.get("workspace_roots").and_then(Value::as_array).is_some_and(|a| !a.is_empty())
}


// ════════════════════════════════════════════════════════════════
// Shared state management & evidence recording (migrated from host_extensions)
// ════════════════════════════════════════════════════════════════

/// Generic disk state indicator for hook state files.
pub enum AgentDiskState<T> {
    Absent,
    Ok(T),
    Unreadable,
}

/// Shared touch state tracking (settings/framework file modifications).
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct TouchState {
    pub settings: bool,
    pub framework: bool,
    pub settings_validated: bool,
    pub framework_tested: bool,
}

/// Build a deny response for PreToolUse.
pub fn deny_pre_tool_use(reason: String) -> Option<Value> {
    Some(serde_json::json!({
        "suppressOutput": true,
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

/// Write HookReviewDiskCore to disk with version bump.
pub fn write_review_state_unlocked(path: &Path, state: &core_policy::HookReviewDiskCore) -> Result<(), String> {
    let mut to_write = state.clone();
    to_write.bump_version_for_save();
    let mut body = serde_json::to_string(&to_write).map_err(|e| e.to_string())?;
    body.push('\n');
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, &body).map_err(|e| e.to_string())
}

/// Read HookReviewDiskCore from disk with migration support.
pub fn read_review_gate_file(path: &Path) -> AgentDiskState<core_policy::HookReviewDiskCore> {
    if !path.is_file() {
        return AgentDiskState::Absent;
    }
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return AgentDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return AgentDiskState::Unreadable;
    }
    let value: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return AgentDiskState::Unreadable,
    };
    AgentDiskState::Ok(core_policy::migrate_hook_review_disk_core(&value))
}

/// Read TouchState from disk.
pub fn load_touch_state_from_path(touch_path: &Path) -> AgentDiskState<TouchState> {
    if !touch_path.is_file() {
        return AgentDiskState::Absent;
    }
    match std::fs::read_to_string(touch_path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(state) => AgentDiskState::Ok(state),
            Err(_) => AgentDiskState::Unreadable,
        },
        Err(_) => AgentDiskState::Unreadable,
    }
}

/// Determine if review gate should sync on UserPromptSubmit.
pub fn should_sync_review_gate_on_user_prompt(repo_root: &Path, prompt: &str) -> bool {
    core_policy::hook_common::is_interactive_profile(Some(repo_root), prompt)
        || core_policy::hook_common::is_framework_goal_entry_prompt(prompt)
        || core_policy::hook_common::is_my_pre_execution_entry_prompt(prompt)
        || core_policy::hook_common::is_narrow_review_prompt(prompt)
        || core_policy::hook_common::is_review_prompt(prompt)
        || core_policy::hook_common::has_override(prompt)
}

/// Auto-record verification evidence from a Bash tool call.
pub fn auto_record_verification_evidence(repo_root: &Path, payload: &Value) {
    let tool_name = payload.get("tool_name").and_then(Value::as_str).unwrap_or_default();
    let Some(command) = bash_command(payload) else { return; };
    let cmd_trimmed = command.trim();
    if !is_verification_command(tool_name, cmd_trimmed) { return; }
    let exit_code = payload_exit_code(payload);
    let output_summary = extract_output_summary(payload, 500);
    let mut entry = serde_json::Map::new();
    entry.insert("kind".to_string(), serde_json::json!("auto_evidence"));
    entry.insert("source".to_string(), serde_json::json!("post_tool_use_auto"));
    entry.insert("tool_name".to_string(), serde_json::json!("Bash"));
    entry.insert("command_preview".to_string(), serde_json::json!(cmd_trimmed));
    entry.insert("recorded_at".to_string(), serde_json::json!(crate::hooks::current_local_timestamp()));
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), serde_json::json!(ec));
        entry.insert("success".to_string(), serde_json::json!(ec == 0));
    }
    if let Some(ref text) = output_summary {
        entry.insert("output".to_string(), serde_json::json!(text));
    }
    let _ = crate::hooks::append_evidence_index(repo_root, None, entry);
}

/// Auto-record research activity from tool calls.
pub fn auto_record_research_activity(repo_root: &Path, payload: &Value) {
    let tool_name = payload.get("tool_name").or_else(|| payload.get("tool"))
        .and_then(Value::as_str).unwrap_or_default();
    let summary = match tool_name {
        "Bash" => bash_command(payload).unwrap_or_default().to_string(),
        "WebFetch" | "web_fetch" => payload.get("tool_input")
            .and_then(|ti| ti.get("url")).and_then(Value::as_str)
            .unwrap_or_default().to_string(),
        other => other.to_string(),
    };
    if summary.is_empty() || summary == tool_name { return; }
    crate::hooks::maybe_record_research_activity(repo_root, tool_name, &summary);
}

/// Standard closeout check (delegates to hooks proxy).
pub fn closeout_check_shared(repo_root: &Path, text: &str) -> Option<String> {
    crate::hooks::closeout_stop_followup_for_completion_text(repo_root, text)
}

/// Standard dispatch bootstrap.
pub fn ensure_dispatch_bootstrap_shared() {
    crate::hooks::ensure_kernel_bootstrap();
}

// ════════════════════════════════════════════════════════════════
// Generic host configuration (data-driven, no hardcoded host names)
// ════════════════════════════════════════════════════════════════

/// Generic host configuration derived from host_id.
/// All values are computed from the host_id string — no hardcoded host names.
pub struct GenericHostConfig {
    pub id: &'static str,
    pub label: &'static str,
    pub state_dir: &'static str,
    pub namespace_env: &'static str,
    pub unreadable_tag: &'static str,
    pub max_context_bytes: usize,
    pub session_start: bool,
    pub subagent_start: bool,
    pub subagent_stop: bool,
}

impl GenericHostConfig {
    pub const fn new(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            state_dir: "",       // computed at runtime from id
            namespace_env: "",   // computed at runtime from id
            unreadable_tag: "",  // computed at runtime from id
            max_context_bytes: 640,
            session_start: false,
            subagent_start: false,
            subagent_stop: false,
        }
    }

    /// Derive state_dir from host_id: ".{host_id}"
    pub fn derived_state_dir(&self) -> String {
        format!(".{}", self.id)
    }

    /// Derive session namespace env from host_id: "ROUTER_RS_{HOST_UPPER}_SESSION_NAMESPACE"
    pub fn derived_namespace_env(&self) -> String {
        format!("ROUTER_RS_{}_SESSION_NAMESPACE", self.id.to_uppercase())
    }

    /// Derive unreadable tag from host_id: "ROUTER_RS_{HOST_UPPER}_HOOK_STATE_UNREADABLE"
    pub fn derived_unreadable_tag(&self) -> String {
        format!("ROUTER_RS_{}_HOOK_STATE_UNREADABLE", self.id.to_uppercase())
    }
}

impl HostHookConfig for GenericHostConfig {
    fn host_id(&self) -> &'static str { self.id }
    fn state_dir_leaf(&self) -> &'static str { self.state_dir }
    fn hook_state_unreadable_tag(&self) -> &'static str { self.unreadable_tag }
    fn session_namespace_env(&self) -> &'static str { self.namespace_env }
    fn log_label(&self) -> &'static str { self.label }
    fn additional_context_max_bytes(&self) -> usize { self.max_context_bytes }
    fn supports_session_start(&self) -> bool { self.session_start }
    fn supports_subagent_start(&self) -> bool { self.subagent_start }
    fn supports_subagent_stop(&self) -> bool { self.subagent_stop }
}

/// Macro to generate a complete `HostHookConfig` implementation from just a host_id.
/// All config values are derived from the host_id string — no hardcoded host names in code.
///
/// Usage: `impl_host_config!("claude", "Claude");`
#[macro_export]
macro_rules! impl_host_config {
    ($id:expr, $label:expr) => {
        fn host_id(&self) -> &'static str { $id }
        fn state_dir_leaf(&self) -> &'static str { concat!(".", $id) }
        fn hook_state_unreadable_tag(&self) -> &'static str {
            framework_kernel::runtime_registry::hook_state_unreadable_tag($id)
        }
        fn session_namespace_env(&self) -> &'static str {
            framework_kernel::runtime_registry::session_namespace_env($id)
        }
        fn log_label(&self) -> &'static str { $label }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn test_silent_success_returns_empty_json() {
        assert_eq!(silent_success(), json!({}));
    }

    #[test]
    fn test_add_context_formats_correctly() {
        let result = add_context("TestEvent", "hello world");
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["context_append"], "[TestEvent] hello world");
    }

    #[test]
    fn test_normalize_path_lexical_removes_dot_segments() {
        let path = Path::new("a/./b/../c");
        let result = normalize_path_lexical(path);
        assert_eq!(result, Path::new("a/c"));
    }

    #[test]
    fn test_compact_repo_relative_segments_removes_extra_parent() {
        let result = compact_repo_relative_segments("a/../../b");
        assert_eq!(result, Some(PathBuf::from("b")));
    }

    #[test]
    fn test_is_host_private_path_matches_dot_claude() {
        assert!(is_host_private_path("repo/.claude/hooks.json"));
        assert!(!is_host_private_path("repo/src/main.rs"));
    }

    #[test]
    fn test_is_framework_source_path() {
        assert!(is_framework_source_path("src/main.rs"));
        assert!(!is_framework_source_path("README.md"));
    }

    #[test]
    fn test_bash_command_extracts_from_payload() {
        let payload = json!({"command": "cargo build"});
        assert_eq!(bash_command(&payload), Some("cargo build"));
    }

    #[test]
    fn test_payload_looks_like_foreign_hook_stdin() {
        let cursor_payload = json!({
            "cursor_version": "1.0",
            "workspace_roots": ["/path"],
            "hook_event_name": "stop"
        });
        assert!(payload_looks_like_foreign_hook_stdin(&cursor_payload));
        let claude_payload = json!({"event": "stop"});
        assert!(!payload_looks_like_foreign_hook_stdin(&claude_payload));
    }

    #[test]
    fn test_hook_output_to_json_value_covers_all_variants() {
        let ctx = HookOutput::AdditionalContext("test".to_string());
        let result = hook_output_to_json_value("E", Some(ctx));
        assert_eq!(result["context_append"], "[E] test");

        let block = HookOutput::Block { reason: "blocked".to_string() };
        let result = hook_output_to_json_value("E", Some(block));
        assert_eq!(result["decision"], "block");

        let deny = HookOutput::Deny { reason: "denied".to_string() };
        let result = hook_output_to_json_value("E", Some(deny));
        assert_eq!(result["decision"], "block");
    }

    #[test]
    fn test_warn_returns_warning_field() {
        let warn = HookOutput::Warn { message: "caution".to_string() };
        let result = hook_output_to_json_value("E", Some(warn));
        assert_eq!(result["warning"], "caution");
    }

    #[test]
    fn test_should_sync_review_gate_detects_review_prompt() {
        let tmp = std::env::temp_dir();
        assert!(should_sync_review_gate_on_user_prompt(&tmp, "review this code"));
        assert!(!should_sync_review_gate_on_user_prompt(&tmp, "hello world"));
    }
}
