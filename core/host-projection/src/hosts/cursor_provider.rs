//! Cursor host: `HostProvider` skeleton (hook + MCP projection metadata).

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};
use serde_json::Value;

#[derive(Debug, Default, Clone, Copy)]
pub struct CursorHostProvider;

impl HostLifecycle for CursorHostProvider {
    fn profile_id(&self) -> &'static str {
        "cursor_profile"
    }

    fn session_supervisor_driver(&self) -> &'static str {
        "unsupported"
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_CURSOR.md"
    }

    fn hooks_manifest_path(&self) -> Option<&'static str> {
        Some(".cursor/hooks.json")
    }

    fn registered_hook_events(&self) -> &'static [&'static str] {
        crate::hosts::cursor_hooks::CURSOR_HOOKS_REGISTERED_EVENTS
    }
}

impl HostToolExecutor for CursorHostProvider {}

impl HostTelemetry for CursorHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str {
        "cursor-agent"
    }

    fn observation_host_id(&self) -> Option<&'static str> {
        Some("cursor")
    }
}

impl HostProvider for CursorHostProvider {
    fn host_id(&self) -> &'static str {
        "cursor"
    }

    fn install_tool(&self) -> &'static str {
        "cursor"
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            mcp_config_key: "mcpServers",
            transport_type: "cursor-agent",
            config_path: "mcp.json",
            ..Default::default()
        }
    }
}
