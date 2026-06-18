//! OpenCode host: `HostProvider` with native Rust hook support.

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};
use serde_json::Value;

crate::impl_host_provider! {
    OpencodeHostProvider for "opencode";
    capabilities { mcp_key: "mcpServers"; transport: "native-opencode"; config: ".opencode/opencode.json"; }
    aliases: ["opencode"];
}

impl HostLifecycle for OpencodeHostProvider {
    fn profile_id(&self) -> &'static str { "opencode_profile" }
    fn session_supervisor_driver(&self) -> &'static str { "unsupported" }
    fn context_file(&self) -> &'static str { "AGENTS_OPENCODE.md" }
    fn driver_binary(&self) -> &'static str { "opencode" }
    fn registered_hook_events(&self) -> &'static [&'static str] { super::opencode_hooks::OPENCODE_HOOKS_REGISTERED_EVENTS }
}

impl HostTelemetry for OpencodeHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str { "native-opencode" }
    fn observation_host_id(&self) -> Option<&'static str> { Some("opencode") }

    fn extract_observation_surfaces(&self, output: &Value) -> (Option<String>, Option<String>) {
        let followup = output
            .get("followup_message")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let additional = output
            .get("additional_context")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        (followup, additional)
    }
}
