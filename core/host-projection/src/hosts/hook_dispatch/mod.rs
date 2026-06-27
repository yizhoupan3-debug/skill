//! Unified hook dispatch trait for all supported hosts (registry-driven).
//!
//! All hosts implement `HostHookDispatcher`, sharing common logic through trait
//! defaults. Host-specific overrides are minimal.
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
use std::fs;
use std::path::Path;
use tracing::debug;

// ── Sub-modules ──

mod event_extract;
mod gate_eval;
mod helpers;
mod path_utils;

pub use event_extract::*;
pub use gate_eval::*;
pub use helpers::*;
pub use path_utils::*;

// Internal uses: private functions from sub-modules needed by trait code
use self::helpers::{
    extract_subagent_error_from_payload, extract_subagent_id_from_payload,
    payload_signal_contains_failure,
};

#[cfg(test)]
mod tests;

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

    /// State directory leaf name (from RUNTIME_REGISTRY.json host_private_config_dir).
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
        // Compute review_required from the prompt (was previously hardcoded false,
        // making spawn-first review nudge permanently disabled — P1.6).
        let review_required = core_policy::hook_common::is_review_prompt(&prompt);
        let contexts = build_user_prompt_context_injection(
            event.repo_root,
            &prompt,
            self.host_id(),
            self.host_id(),
            review_required,
            core_policy::hook_common::has_override(&prompt),
        );
        if contexts.is_empty() { None } else {
            Some(HookOutput::AdditionalContext(contexts.join("\n")))
        }
    }

    /// PostToolUse: evidence collection + subagent tracking + auto-checkpoint.
    /// Default: record telemetry + auto evidence + progress checkpoint.
    fn handle_post_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        crate::hooks::ensure_kernel_bootstrap();
        let tool_name = crate::hosts::hook_dispatch::extract_tool_name(event.payload);
        let normalized = core_policy::hook_common::normalize_tool_name(Some(&tool_name));
        crate::hosts::host_state::auto_record_verification_evidence(event.repo_root, event.payload);
        crate::hosts::host_state::auto_record_research_activity(event.repo_root, event.payload);

        // Auto-register subagent in health registry when a subagent tool is called.
        // This covers hosts (like Claude) that don't fire SubagentStart/SubagentStop events.
        if core_policy::subagent::is_subagent_tool(&normalized) {
            let agent_id = extract_subagent_id_from_payload(event.payload)
                .unwrap_or_else(|| format!("agent-{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
            let now = framework_kernel::time::now_iso();
            let _payload = serde_json::json!({
                "operation": "agent_register",
                "agent_id": agent_id,
                "host_id": self.host_id(),
                "tool_type": format!("post_tool_use:{normalized}"),
                "now": now,
            });
        }

        // Auto-checkpoint: if there's an active goal and the tool call succeeded,
        // record a basic progress checkpoint (advisory, best-effort).
        if crate::hooks::post_tool_call_succeeded(event.payload)
            && let Ok(Some(goal)) = core_state::state_manager::read_goal_state(event.repo_root, None)
            && core_state::state_manager::goal_state_requests_continuation(&goal)
        {
            let note = format!("auto-checkpoint: tool={normalized}");
            let payload = serde_json::json!({
                "repo_root": event.repo_root.to_string_lossy().to_string(),
                "operation": "checkpoint",
                "note": note,
            });
            // Best-effort: failure to checkpoint should not disrupt PostToolUse flow.
            let _ = core_state::state_manager::framework_goal_drive(payload);
        }

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
    /// Default: operator_inject check + repo context + task list summary.
    fn handle_session_start(&self, event: &HookEvent) -> Option<HookOutput> {
        let mut contexts = Vec::new();

        if core_policy::env_flags::router_rs_operator_inject_globally_enabled() {
            contexts.push(format!("Repo: {}", event.repo_root.display()));
        }

        // Task list summary for session continuity.
        if let Some(task_ctx) = build_task_list_summary_context(event.repo_root) {
            contexts.push(task_ctx);
        }

        // Detect stale goals from a previous session and warn the user
        if let Ok(Some(goal)) = core_state::state_manager::read_goal_state(event.repo_root, None)
            && goal.get("stale").and_then(Value::as_bool).unwrap_or(false) {
                let stale_reason = goal
                    .get("stale_reason")
                    .and_then(Value::as_str)
                    .unwrap_or("previous session");
                let goal_text = goal
                    .get("goal")
                    .and_then(Value::as_str)
                    .unwrap_or("(unnamed)");
                contexts.push(format!(
                    "[Continuity] 检测到前一个 session 的未完成 Goal: 「{goal_text}」({stale_reason})。\
                     如需清除请调用 goal_state_manage(operation=clear)。"
                ));
            }

        if contexts.is_empty() {
            None
        } else {
            let ctx = contexts.join("\n");
            Some(HookOutput::AdditionalContext(truncate_bytes(
                &ctx,
                self.additional_context_max_bytes(),
                "...",
            )))
        }
    }

    /// SubagentStart: register agent in agent-orchestrator health registry.
    fn handle_subagent_start(&self, event: &HookEvent) -> Option<HookOutput> {
        let agent_id = extract_subagent_id_from_payload(event.payload)
            .unwrap_or_else(|| format!("agent-{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
        let host_id = self.host_id();
        let now = framework_kernel::time::now_iso();
        let _payload = serde_json::json!({
            "operation": "agent_register",
            "agent_id": agent_id,
            "host_id": host_id,
            "tool_type": "subagent_start_hook",
            "now": now,
        });
        None
    }

    /// SubagentStop: unregister agent in agent-orchestrator health registry.
    fn handle_subagent_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        let Some(agent_id) = extract_subagent_id_from_payload(event.payload) else {
            debug!("SubagentStop: no agent_id in payload, skipping unregister");
            return None;
        };
        let now = framework_kernel::time::now_iso();
        let terminal_status = if payload_signal_contains_failure(event.payload) {
            "failed"
        } else {
            "completed"
        };
        let error = extract_subagent_error_from_payload(event.payload);
        let _payload = serde_json::json!({
            "operation": "agent_unregister",
            "agent_id": agent_id,
            "terminal_status": terminal_status,
            "error": error,
            "now": now,
        });
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

/// Build a compact task list summary for SessionStart continuity digest.
/// Returns `None` if no tasks exist (avoids injecting noise for new repos).
fn build_task_list_summary_context(repo_root: &Path) -> Option<String> {
    let current = repo_root.join("artifacts/current");
    let entries = fs::read_dir(&current).ok()?;
    let task_dirs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_dir()
                && e.file_name().to_string_lossy() != "review-lanes"
                && !e.file_name().to_string_lossy().starts_with('.')
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    if task_dirs.is_empty() {
        return None;
    }

    let current_task_id = core_state::state_manager::read_primary_task_id(repo_root);
    let mut in_progress = 0u32;
    let mut completed = 0u32;
    let mut other = 0u32;

    for task_id in &task_dirs {
        let goal_path = current.join(task_id).join("GOAL_STATE.json");
        let status = if goal_path.is_file() {
            fs::read_to_string(&goal_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .and_then(|v| v.get("status").and_then(|s| s.as_str()).map(str::to_string))
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "created".to_string()
        };
        match status.as_str() {
            "in_progress" | "active" | "running" => in_progress += 1,
            "completed" | "done" | "closed" => completed += 1,
            _ => other += 1,
        }
    }

    let total = task_dirs.len();
    let current_display = current_task_id.as_deref().unwrap_or("none");
    let mut out = format!(
        "[Task state] {total} tasks ({in_progress} in-progress, {completed} completed, {other} other). \
         Current: {current_display}."
    );
    if in_progress == 0 && total > 0 {
        out.push_str(
            "\nHint: No active task. For multi-step requests, use task_create to define a todo list, \
             then task_complete as you finish each step."
        );
    }
    Some(out)
}

// ── Re-exports from sub-modules (backward compat) ──

pub use core_policy::subagent::{SUBAGENT_TOOL_NAMES, is_subagent_tool};

pub use core_policy::session_key::SESSION_KEY_CWD_FIELDS;
pub use core_policy::session_key::SESSION_ID_FIELDS;
pub use core_policy::session_key::TOOL_INPUT_SESSION_ID_FIELDS;
pub use core_policy::session_key::TOOL_INPUT_METADATA_SESSION_ID_FIELDS;

pub use crate::hosts::generic_config::GenericHostConfig;
pub use crate::hosts::host_state::{
    auto_record_research_activity, auto_record_verification_evidence,
};
