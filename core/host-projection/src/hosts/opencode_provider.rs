//! OpenCode host: `HostProvider` skeleton (MCP stdio projection metadata).

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
    HARNESS_CAPABILITIES_MINIMAL,
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

    fn harness_capabilities(&self) -> &'static [&'static str] {
        HARNESS_CAPABILITIES_MINIMAL
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_OPENCODE.md"
    }
}

impl HostToolExecutor for OpencodeHostProvider {
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

impl HostTelemetry for OpencodeHostProvider {
    fn review_gate_router_observable(&self) -> bool {
        false
    }

    fn hook_telemetry_surface(&self) -> &'static str {
        "opencode-cli"
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
            has_native_hook: false,
            supports_subagent: false,
            supports_worktree: false,
            mcp_config_key: "mcp",
            transport_type: "opencode-cli",
            config_path: ".opencode/opencode.json",
            batch_execution: false,
            cron_execution: false,
            ci_runner: false,
            non_interactive_entrypoint: false,
            external_session_supervisor: false,
            rate_limit_auto_resume: false,
        }
    }
}
