//! Antigravity host: `HostProvider` skeleton (MCP stdio projection metadata).

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};

const HARNESS_CAPABILITIES: &[&str] = &["hot_runtime_routing", "l2_continuity_contract"];

#[derive(Debug, Default, Clone, Copy)]
pub struct AntigravityHostProvider;

impl HostLifecycle for AntigravityHostProvider {
    fn profile_id(&self) -> &'static str {
        "antigravity_profile"
    }

    fn session_supervisor_driver(&self) -> &'static str {
        "unsupported"
    }

    fn harness_capabilities(&self) -> &'static [&'static str] {
        HARNESS_CAPABILITIES
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_ANTIGRAVITY.md"
    }
}

impl HostToolExecutor for AntigravityHostProvider {
    fn has_hard_gate_hooks(&self) -> bool {
        false
    }

    fn closeout_evidence_hooks_supported(&self) -> bool {
        false
    }

    fn requires_strict_pre_tool_fallback_default(&self) -> bool {
        true
    }
}

impl HostTelemetry for AntigravityHostProvider {
    fn review_gate_router_observable(&self) -> bool {
        false
    }

    fn hook_telemetry_surface(&self) -> &'static str {
        "mcp-stdio"
    }
}

impl HostProvider for AntigravityHostProvider {
    fn host_id(&self) -> &'static str {
        "antigravity"
    }

    fn install_tool(&self) -> &'static str {
        "antigravity"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["antigravity-app", "antigravity-cli"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            has_native_hook: false,
            supports_subagent: false,
            supports_worktree: false,
            mcp_config_key: "mcpServers",
            transport_type: "mcp-stdio",
            config_path: "mcp.json",
            batch_execution: false,
            cron_execution: false,
            ci_runner: false,
            non_interactive_entrypoint: false,
            external_session_supervisor: false,
            rate_limit_auto_resume: false,
        }
    }
}
