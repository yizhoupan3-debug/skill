//! MIMO host: `HostProvider` with native Rust hook support.

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};

crate::impl_host_provider! {
    MimoHostProvider for "mimo";
    capabilities { mcp_key: "mcpServers"; transport: "native-mimo"; config: ".mimo/settings.json"; }
    aliases: ["mimo"];
}

impl HostLifecycle for MimoHostProvider {
    fn profile_id(&self) -> &'static str { "mimo_profile" }
    fn session_supervisor_driver(&self) -> &'static str { "unsupported" }
    fn context_file(&self) -> &'static str { "AGENTS_MIMO.md" }
    fn driver_binary(&self) -> &'static str { "mimo" }
}

impl HostTelemetry for MimoHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str { "native-mimo" }
    fn observation_host_id(&self) -> Option<&'static str> { Some("mimo") }
}
