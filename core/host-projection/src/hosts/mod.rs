/// Registry-generated host provider structs and trait impls.
/// Source: configs/framework/RUNTIME_REGISTRY.json -> host_targets.metadata
pub mod host_provider;

/// Host-specific logic overrides organized by capability, not by host.
pub mod capability_overrides;

/// Unified hook dispatch trait + shared utilities for all 4 hosts.
pub mod hook_dispatch;

/// Shared state management (AgentDiskState, TouchState, review state).
pub mod host_state;

/// Generic host configuration (data-driven, no hardcoded host names).
pub mod generic_config;

/// Cross-host file state lock abstraction.
pub mod file_state_lock;

/// Cross-host worktree auto-save and audit utilities (all 4 hosts).
pub mod worktree_auto_save;

// ── ADR §2.1 unified hook dispatch ──
pub mod host_extensions;
pub mod stop_dispatch;

// ── MCP agent loop (registry-driven: all hosts via run_agent_mcp_loop) ──
pub mod mcp_stdio_harness;

// ── Test shims ──
#[cfg(any(test, feature = "test-support"))]
pub mod test_shim;

// ── Unified hook contract tests (all 4 hosts) ──

pub use host_provider::{
    AgentDispatchFn, HookDispatchFn, HostCapabilities, HostLifecycle, HostProvider,
    HostTelemetry, HostToolExecutor, default_host_id, find_agent_dispatch, find_hook_dispatch,
    host_lifecycle_for_id, host_provider_for_id, host_provider_for_install_tool,
    host_provider_for_routing_spelling, host_provider_registry, host_provider_routing_aliases,
    host_provider_strict_pre_tool_fallback_hint, host_telemetry_for_id, host_tool_executor_for_id,
    register_agent_dispatchers, register_hook_dispatchers,
};

// ── Shared MCP agent loop (registry-driven: host_id from RUNTIME_REGISTRY) ──

use std::io;
use std::path::Path;

pub fn run_agent_mcp_loop(repo_root_arg: Option<&Path>, host_id: &str) -> Result<(), String> {
    let repo_root = crate::hooks::resolve_repo_root_arg(repo_root_arg)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    mcp_stdio_harness::run_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root, host_id)
}
