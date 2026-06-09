pub mod mcp_stdio_harness;
pub mod claude_code_hooks;
#[cfg(feature = "host-antigravity")]
mod antigravity_provider;
#[cfg(feature = "host-claude-code")]
mod claude_provider;
pub mod codex_hooks;
#[cfg(feature = "host-codex")]
mod codex_provider;
pub mod cursor_hooks;
#[cfg(feature = "host-cursor")]
mod cursor_provider;
pub mod host_provider;
pub mod opencode_agent;
#[cfg(feature = "host-opencode")]
mod opencode_provider;
pub mod hook_state_common;

pub use host_provider::{
    host_lifecycle_for_id, host_provider_for_id, host_provider_for_install_tool,
    host_provider_for_routing_spelling, host_provider_registry,
    host_provider_routing_aliases, host_provider_strict_pre_tool_fallback_hint,
    host_telemetry_for_id, host_tool_executor_for_id, HostCapabilities, HostLifecycle,
    HostProvider, HostTelemetry, HostToolExecutor,
};
