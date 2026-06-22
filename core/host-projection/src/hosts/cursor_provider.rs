//! Cursor host: `HostProvider` skeleton (hook + MCP projection metadata).

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};

crate::impl_host_provider! {
    CursorHostProvider for "cursor";
    capabilities { mcp_key: "mcpServers"; transport: "cursor-agent"; config: "mcp.json"; }
}

impl HostLifecycle for CursorHostProvider {
    fn profile_id(&self) -> &'static str { "cursor_profile" }
    fn session_supervisor_driver(&self) -> &'static str { "unsupported" }
    fn context_file(&self) -> &'static str { "AGENTS.md" }
    fn driver_binary(&self) -> &'static str { "cursor" }
    fn hooks_manifest_path(&self) -> Option<&'static str> { Some(".cursor/hooks.json") }
    fn registered_hook_events(&self) -> &'static [&'static str] { crate::hosts::host_extensions::cursor_impl::CURSOR_HOOKS_REGISTERED_EVENTS }
}

impl HostTelemetry for CursorHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str { "cursor-agent" }
    fn observation_host_id(&self) -> Option<&'static str> { Some("cursor") }
}
