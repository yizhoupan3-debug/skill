pub mod host_provider;
pub mod hook_state_common;

// ── Migrated host providers ──
pub mod antigravity_provider;
pub mod claude_code_hooks;
pub mod claude_provider;
pub mod mcp_stdio_harness;
pub mod opencode_agent;
pub mod opencode_provider;

// ── Test shims ──
#[cfg(any(test, feature = "test-support"))]
pub mod test_shim;

pub use host_provider::{
    host_lifecycle_for_id, host_provider_for_id, host_provider_for_install_tool,
    host_provider_for_routing_spelling, host_provider_registry,
    host_provider_routing_aliases, host_provider_strict_pre_tool_fallback_hint,
    host_telemetry_for_id, host_tool_executor_for_id, HostCapabilities, HostLifecycle,
    HostProvider, HostTelemetry, HostToolExecutor,
};
