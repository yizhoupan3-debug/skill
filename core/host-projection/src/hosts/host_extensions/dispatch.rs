//! Unified hook dispatcher for all 4 closed-set hosts.
//!
//! All hosts use the shared `HostHookDispatcher` trait defaults from hook_dispatch.rs.
//! Codex overrides `handle_pre_tool_use` for custom path protection (pretool.rs).
//! All hosts now override `handle_stop` to use the unified 13-step Stop pipeline.

use crate::hosts::hook_dispatch::{extract_session_key, stop_signal_text_from_payload, value_to_hook_output, HookEvent, HookOutput, HostHookConfig, HostHookDispatcher};
use crate::hosts::stop_dispatch::{StopHostOps, run_unified_stop};
use super::pretool;
use std::path::PathBuf;
use serde_json::Value;

// ── Claude dispatcher ──
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeDispatcher;

impl HostHookConfig for ClaudeDispatcher {
    crate::impl_host_config!("claude", "Claude");
}

impl StopHostOps for ClaudeDispatcher {
    fn host_id(&self) -> &'static str { "claude" }
    fn log_label(&self) -> &'static str { "Claude" }
    fn hook_state_base(&self, repo_root: &std::path::Path) -> PathBuf { repo_root.join(".claude").join("hook-state") }
    fn session_key(&self, repo_root: &std::path::Path, payload: &Value) -> String {
        let fallback = repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
        extract_session_key(payload, "", fallback, false)
    }
    fn stop_signal_text(&self, payload: &Value) -> String { stop_signal_text_from_payload(payload) }
}

impl HostHookDispatcher for ClaudeDispatcher {
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        value_to_hook_output(
            &run_unified_stop(event.repo_root, event.payload, self).unwrap_or_default()
        )
    }
}

// ── OpenCode dispatcher ──
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenCodeDispatcher;

impl HostHookConfig for OpenCodeDispatcher {
    crate::impl_host_config!("opencode", "OpenCode");
    fn supports_session_start(&self) -> bool { true }
    fn supports_subagent_start(&self) -> bool { true }
    fn supports_subagent_stop(&self) -> bool { true }
}

impl StopHostOps for OpenCodeDispatcher {
    fn host_id(&self) -> &'static str { "opencode" }
    fn log_label(&self) -> &'static str { "OpenCode" }
    fn hook_state_base(&self, repo_root: &std::path::Path) -> PathBuf { repo_root.join(".claude").join("hook-state") }
    fn session_key(&self, repo_root: &std::path::Path, payload: &Value) -> String {
        let fallback = repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
        extract_session_key(payload, "", fallback, false)
    }
    fn stop_signal_text(&self, payload: &Value) -> String { stop_signal_text_from_payload(payload) }
}

impl HostHookDispatcher for OpenCodeDispatcher {
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        value_to_hook_output(
            &run_unified_stop(event.repo_root, event.payload, self).unwrap_or_default()
        )
    }
}

// ── Codex dispatcher ──
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexDispatcher;

impl HostHookConfig for CodexDispatcher {
    crate::impl_host_config!("codex", "Codex");
    fn additional_context_max_bytes(&self) -> usize { 640 }
}

impl StopHostOps for CodexDispatcher {
    fn host_id(&self) -> &'static str { "codex" }
    fn log_label(&self) -> &'static str { "Codex" }
    fn hook_state_base(&self, repo_root: &std::path::Path) -> PathBuf { repo_root.join(".claude").join("hook-state") }
    fn session_key(&self, repo_root: &std::path::Path, payload: &Value) -> String {
        let fallback = repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
        extract_session_key(payload, "", fallback, false)
    }
    fn stop_signal_text(&self, payload: &Value) -> String { stop_signal_text_from_payload(payload) }
}

impl HostHookDispatcher for CodexDispatcher {
    /// Codex-specific PreToolUse path protection (shared pretool module).
    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        match pretool::run_pre_tool_use(
            event.repo_root,
            event.payload,
            &std::collections::HashSet::<String>::new(),
            &["configs/framework/", "core/", "AGENTS.md"],
            "codex install",
        ) {
            Ok(Some(val)) => Some(HookOutput::Raw(val)),
            Ok(None) => None,
            Err(err) => Some(HookOutput::Deny { reason: err }),
        }
    }
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        value_to_hook_output(
            &run_unified_stop(event.repo_root, event.payload, self).unwrap_or_default()
        )
    }
}

// ── Cursor dispatcher ──
#[derive(Debug, Default, Clone, Copy)]
pub struct CursorDispatcher;

impl HostHookConfig for CursorDispatcher {
    crate::impl_host_config!("cursor", "Cursor");
    fn supports_session_start(&self) -> bool { true }
    fn supports_subagent_start(&self) -> bool { true }
    fn supports_subagent_stop(&self) -> bool { true }
}

impl StopHostOps for CursorDispatcher {
    fn host_id(&self) -> &'static str { "cursor" }
    fn log_label(&self) -> &'static str { "Cursor" }
    fn hook_state_base(&self, repo_root: &std::path::Path) -> PathBuf { repo_root.join(".claude").join("hook-state") }
    fn session_key(&self, repo_root: &std::path::Path, payload: &Value) -> String {
        let fallback = repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
        // Cursor may have session_id in tool_input (subagent chaining).
        extract_session_key(payload, "", fallback, true)
    }
    fn stop_signal_text(&self, payload: &Value) -> String { extract_completion_text(payload) }
}

impl HostHookDispatcher for CursorDispatcher {
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        value_to_hook_output(
            &run_unified_stop(event.repo_root, event.payload, self).unwrap_or_default()
        )
    }
}
