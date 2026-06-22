pub mod codex_provider;
pub mod cursor_provider;
pub mod hook_state_common;
pub mod host_provider;

/// Unified hook dispatch trait + shared utilities for all 4 hosts.
pub mod hook_dispatch;

/// Cross-host file state lock abstraction.
pub mod file_state_lock;

/// Cross-host worktree auto-save and audit utilities (all 4 hosts).
pub mod worktree_auto_save;

// ── ADR §2.1 unified hook dispatch ──
pub mod event_handlers;
pub mod host_extensions;
pub mod mcp_pre_guard;
pub mod stop_dispatch;

// ── Migrated host providers ──
pub mod claude_agent;
pub mod claude_provider;
pub mod mcp_stdio_harness;
pub mod opencode_agent;
pub mod opencode_provider;

// ── Test shims ──
#[cfg(any(test, feature = "test-support"))]
pub mod test_shim;

// ── Unified hook contract tests (all 4 hosts) ──
#[cfg(test)]
mod unified_hook_tests;

pub use host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
    default_host_id, host_lifecycle_for_id, host_provider_for_id, host_provider_for_install_tool,
    host_provider_for_routing_spelling, host_provider_registry, host_provider_routing_aliases,
    host_provider_strict_pre_tool_fallback_hint, host_telemetry_for_id, host_tool_executor_for_id,
};

// ── Shared MCP agent loop (3 hosts: claude, codex, opencode) ──

use std::io;
use std::path::Path;

pub fn run_agent_mcp_loop(repo_root_arg: Option<&Path>, host_id: &str) -> Result<(), String> {
    let repo_root = crate::hooks::resolve_repo_root_arg(repo_root_arg)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    mcp_stdio_harness::run_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root, host_id)
}
