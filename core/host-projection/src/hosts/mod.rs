pub mod codex_hooks;
pub mod codex_provider;
pub mod cursor_hooks;
pub mod cursor_provider;
pub mod host_provider;
pub mod hook_state_common;

/// Unified hook dispatch trait + shared utilities for all 4 hosts.
pub mod hook_dispatch;

/// Cross-host file state lock abstraction.
pub mod file_state_lock;

// ── Migrated host providers ──
pub mod claude_code_hooks;
pub mod claude_provider;
pub mod mcp_stdio_harness_dir; pub use mcp_stdio_harness_dir as mcp_stdio_harness;
pub mod opencode_agent;
pub mod opencode_hooks;
pub mod opencode_provider;

// ── Test shims ──
#[cfg(any(test, feature = "test-support"))]
pub mod test_shim;

pub use host_provider::{
    default_host_id, host_lifecycle_for_id, host_provider_for_id, host_provider_for_install_tool,
    host_provider_for_routing_spelling, host_provider_registry,
    host_provider_routing_aliases, host_provider_strict_pre_tool_fallback_hint,
    host_telemetry_for_id, host_tool_executor_for_id, HostCapabilities, HostLifecycle,
    HostProvider, HostTelemetry, HostToolExecutor,
};
