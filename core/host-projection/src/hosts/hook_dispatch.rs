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

use serde_json::Value;
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
}

/// Core trait: unified hook dispatch for all 4 closed-set hosts.
///
/// Methods are categorized:
/// - **(A) Pure shared** — default implementations, no override needed
/// - **(B) Shared + extension** — default skeleton, hosts can layer on top
/// - **(C) Must implement** — each host provides its own logic
pub trait HostHookDispatcher: HostHookConfig {
    // ── (C) Must implement ─────────────────────────────────────

    /// PreToolUse: path protection interception.
    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput>;

    /// UserPromptSubmit: review gate init + context injection.
    fn handle_user_prompt_submit(&self, event: &HookEvent) -> Option<HookOutput>;

    /// PostToolUse: evidence collection + subagent tracking.
    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput>;

    // ── (B) Shared + extension ─────────────────────────────────

    /// Stop: closeout gate + review gate check.
    /// Default: closeout_followup check (reference only — all hosts override).
    /// Claude adds review gate + TouchState; Codex adds phase bump + reject reason;
    /// Cursor adds goal signals + review gate; OpenCode adds review gate + reject reason.
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        let completion_text = extract_completion_text(event);
        if let Some(msg) = crate::hooks::closeout_stop_followup_for_completion_text(
            event.repo_root,
            &completion_text,
        ) {
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

    // ── (A) Pure shared ────────────────────────────────────────

    /// SubagentStart: default no-op. Cursor/Codex override.
    fn handle_subagent_start(&self, _event: &HookEvent) -> Option<HookOutput> {
        None
    }

    /// SubagentStop: default no-op. Cursor overrides.
    fn handle_subagent_stop(&self, _event: &HookEvent) -> Option<HookOutput> {
        None
    }

    /// Unified dispatch entry. All hosts use the same routing logic.
    fn dispatch(&self, event: &HookEvent) -> Option<HookOutput> {
        let normalized = normalize_event_name(event.event_name);
        debug!(event = %normalized, host = %self.host_id(), "hook dispatch");
        match normalized.as_ref() {
            "sessionstart" if self.supports_session_start() => self.handle_session_start(event),
            "userpromptsubmit" | "beforesubmitprompt" => self.handle_user_prompt_submit(event),
            "pretooluse" => self.handle_pre_tool_use(event),
            "posttooluse" => self.handle_post_tool_use(event),
            "stop" => self.handle_stop(event),
            "subagentstart" if self.supports_subagent_start() => self.handle_subagent_start(event),
            "subagentstop" if self.supports_subagent_stop() => self.handle_subagent_stop(event),
            _ => None,
        }
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
pub fn extract_prompt_text(event: &Value) -> String {
    const KEYS: &[&str] = &[
        "prompt",
        "user_prompt",
        "message",
        "input",
        "text",
        "userPrompt",
        "userMessage",
        "command",
        "content",
        "userContent",
        "query",
        "composerText",
        "editorText",
    ];
    for key in KEYS {
        if let Some(value) = event.get(*key).and_then(Value::as_str)
            && !value.trim().is_empty() {
                return value.to_string();
            }
    }
    // Fallback: scan nested messages arrays for last user message
    extract_prompt_from_nested_messages(event)
}

/// Scan nested messages arrays for the last user-role message.
fn extract_prompt_from_nested_messages(event: &Value) -> String {
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
fn is_user_message_role(obj: &serde_json::Map<String, Value>) -> bool {
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
fn message_body_text(msg: &serde_json::Map<String, Value>) -> Option<String> {
    for key in &["content", "text", "body", "message"] {
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
    event
        .get("tool_name")
        .or(event.get("tool"))
        .or(event.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Extract tool input from event payload.
pub fn extract_tool_input(event: &Value) -> Value {
    event
        .get("tool_input")
        .or(event.get("input"))
        .or(event.get("arguments"))
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}))
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
pub const SESSION_KEY_CWD_FIELDS: &[&str] = &[
    "cwd",
    "workspaceFolder",
    "workspace_folder",
    "workspaceRoot",
    "workspace_root",
    "root",
];

/// Extract session key using core_policy's shared `session_key_core`.
///
/// This is the canonical session key extraction used by all hosts.
/// Each host provides `env_var` (via `HostHookConfig::session_namespace_env`)
/// and `repo_fallback_token` (derived from repo_root).
pub fn extract_session_key(event: &Value, env_var: &'static str, repo_fallback: &str) -> String {
    core_policy::session_key::session_key_core(
        &core_policy::session_key::SessionKeyConfig { env_var },
        || extract_session_id_from_payload(event),
        || extract_cwd_from_payload(event),
        repo_fallback,
    )
}

/// Extract explicit session id from payload (tries multiple field names).
fn extract_session_id_from_payload(event: &Value) -> Option<String> {
    for key in &[
        "session_id",
        "sessionId",
        "conversation_id",
        "conversationId",
    ] {
        if let Some(val) = event.get(*key).and_then(Value::as_str)
            && !val.is_empty() {
                return Some(val.to_string());
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
        || cmd_lower.contains("pytest")
        || cmd_lower.contains("make test")
        || cmd_lower.contains("make check")
        || cmd_lower.contains("go test")
        || cmd_lower.contains("git diff")
        || cmd_lower.contains("git log")
}
