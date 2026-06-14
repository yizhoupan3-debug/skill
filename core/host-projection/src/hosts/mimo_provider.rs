use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MimoHostProvider;

impl HostLifecycle for MimoHostProvider {
    fn profile_id(&self) -> &'static str {
        "mimo_profile"
    }

    fn session_supervisor_driver(&self) -> &'static str {
        "unsupported"
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_MIMO.md"
    }

    fn driver_binary(&self) -> &'static str {
        "mimo"
    }
}

impl HostToolExecutor for MimoHostProvider {}

impl HostTelemetry for MimoHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str {
        "native-mimo"
    }

    fn observation_host_id(&self) -> Option<&'static str> {
        Some("mimo")
    }
}

impl HostProvider for MimoHostProvider {
    fn host_id(&self) -> &'static str {
        "mimo"
    }

    fn install_tool(&self) -> &'static str {
        "mimo"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["mimo"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            mcp_config_key: "mcpServers",
            transport_type: "native-mimo",
            config_path: ".mimo/settings.json",
            ..Default::default()
        }
    }
}
