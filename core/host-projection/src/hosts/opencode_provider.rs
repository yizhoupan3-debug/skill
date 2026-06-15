//! OpenCode host: `HostProvider` with native Rust hook support.
//!
//! OpenCode uses `router-rs opencode hook --event=...` for all hook events,
//! unified with cursor/claude/codex via the shared `HostHookDispatcher` trait.

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};
use serde_json::Value;

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

    fn registered_hook_events(&self) -> &'static [&'static str] {
        super::opencode_hooks::OPENCODE_HOOKS_REGISTERED_EVENTS
    }

    fn driver_binary(&self) -> &'static str {
        "opencode"
    }
}

impl HostToolExecutor for OpencodeHostProvider {}

impl HostTelemetry for OpencodeHostProvider {
    fn hook_telemetry_surface(&self) -> &'static str {
        "native-opencode"
    }

    fn observation_host_id(&self) -> Option<&'static str> {
        Some("opencode")
    }

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

impl HostProvider for OpencodeHostProvider {
    fn host_id(&self) -> &'static str {
        "opencode"
    }

    fn install_tool(&self) -> &'static str {
        "opencode"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["opencode"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            mcp_config_key: "mcpServers",
            transport_type: "native-opencode",
            config_path: ".opencode/opencode.json",
            ..Default::default()
        }
    }
}
