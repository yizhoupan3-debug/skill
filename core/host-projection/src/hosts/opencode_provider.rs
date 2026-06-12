//! OpenCode host: `HostProvider` with plugin-based hook support.

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct OpencodeHostProvider;

impl HostLifecycle for OpencodeHostProvider {
    fn profile_id(&self) -> &'static str {
        "opencode_profile"
    }

    fn session_supervisor_driver(&self) -> &'static str {
        "unsupported"
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_OPENCODE.md"
    }

    fn hooks_manifest_path(&self) -> Option<&'static str> {
        Some(super::opencode_hooks::OPENCODE_HOOKS_PATH)
    }

    fn registered_hook_events(&self) -> &'static [&'static str] {
        super::opencode_hooks::OPENCODE_HOOKS_REGISTERED_EVENTS
    }
}

impl HostToolExecutor for OpencodeHostProvider {}

impl HostTelemetry for OpencodeHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str {
        "opencode-plugin"
    }

    fn observation_host_id(&self) -> Option<&'static str> {
        Some("opencode")
    }
}

impl HostProvider for OpencodeHostProvider {
    fn host_id(&self) -> &'static str {
        "opencode"
    }

    fn install_tool(&self) -> &'static str {
        "opencode"
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            mcp_config_key: "mcp",
            transport_type: "opencode-plugin",
            config_path: ".opencode/opencode.json",
            ..Default::default()
        }
    }
}
