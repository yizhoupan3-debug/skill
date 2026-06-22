//! Unified host configurations for all 4 hosts.
//!
//! Each host has a struct implementing `HostHookConfig` via `impl_host_config!`.
//! This is the only place where per-host config values are defined.
//! All host-specific behavior differences live here as overridable trait methods.

use crate::hosts::hook_dispatch::{HostHookConfig, GenericHostConfig};

/// Claude host configuration.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeConfig;

impl HostHookConfig for ClaudeConfig {
    crate::impl_host_config!("claude", "Claude");
}

/// OpenCode host configuration.
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenCodeConfig;

impl HostHookConfig for OpenCodeConfig {
    crate::impl_host_config!("opencode", "OpenCode");
    fn supports_session_start(&self) -> bool { true }
    fn supports_subagent_start(&self) -> bool { true }
    fn supports_subagent_stop(&self) -> bool { true }
}

/// Codex host configuration.
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexConfig;

impl HostHookConfig for CodexConfig {
    crate::impl_host_config!("codex", "Codex");
    fn additional_context_max_bytes(&self) -> usize { 640 }
}

/// Cursor host configuration.
#[derive(Debug, Default, Clone, Copy)]
pub struct CursorConfig;

impl HostHookConfig for CursorConfig {
    crate::impl_host_config!("cursor", "Cursor");
    fn supports_session_start(&self) -> bool { true }
    fn supports_subagent_start(&self) -> bool { true }
    fn supports_subagent_stop(&self) -> bool { true }
}

// ── Registered hook events per host ──

/// Events registered by Claude hooks.
pub const CLAUDE_HOOKS_REGISTERED_EVENTS: &[&str] = &[
    "pre-tool-use", "user-prompt-submit", "post-tool-use", "stop",
];

/// Events registered by OpenCode hooks.
pub const OPENCODE_HOOKS_REGISTERED_EVENTS: &[&str] = &[
    "tool.execute.before", "tool.execute.after", "session.idle",
    "session.created", "session.deleted", "permission.asked",
    "permission.replied", "file.edited", "shell.env",
];

/// Events registered by Cursor hooks.
pub const CURSOR_HOOKS_REGISTERED_EVENTS: &[&str] = &[
    "beforeSubmitPrompt", "stop", "sessionStart", "sessionEnd",
    "postToolUse", "subagentStart", "subagentStop",
];
