//! Unified hook dispatcher for all 4 closed-set hosts.
//!
//! All hosts use the shared `HostHookDispatcher` trait defaults from hook_dispatch.rs.
//! Codex overrides `handle_pre_tool_use` for custom path protection (pretool.rs).
//! No other host overrides any event handler — they all use the same code path.

use crate::hosts::hook_dispatch::{HookEvent, HookOutput, HostHookConfig, HostHookDispatcher};
use super::config::{ClaudeConfig, CodexConfig, CursorConfig, OpenCodeConfig};
use super::pretool;

// ── Claude dispatcher ──
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeDispatcher;

impl HostHookConfig for ClaudeDispatcher {
    crate::impl_host_config!("claude", "Claude");
}

impl HostHookDispatcher for ClaudeDispatcher {}

// ── OpenCode dispatcher ──
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenCodeDispatcher;

impl HostHookConfig for OpenCodeDispatcher {
    crate::impl_host_config!("opencode", "OpenCode");
    fn supports_session_start(&self) -> bool { true }
    fn supports_subagent_start(&self) -> bool { true }
    fn supports_subagent_stop(&self) -> bool { true }
}

impl HostHookDispatcher for OpenCodeDispatcher {}

// ── Codex dispatcher ──
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexDispatcher;

impl HostHookConfig for CodexDispatcher {
    crate::impl_host_config!("codex", "Codex");
    fn additional_context_max_bytes(&self) -> usize { 640 }
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

impl HostHookDispatcher for CursorDispatcher {}
