//! OpenCode host: `HostProvider` with plugin-based hook support.

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
    HARNESS_CAPABILITIES_FULL,
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
        HARNESS_CAPABILITIES_FULL
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

impl HostToolExecutor for OpencodeHostProvider {
    fn has_hard_gate_hooks(&self) -> bool {
        false
    }

    fn closeout_evidence_hooks_supported(&self) -> bool {
        true
    }

    fn requires_strict_pre_tool_fallback_default(&self) -> bool {
        false
    }
}

impl HostTelemetry for OpencodeHostProvider {
    fn review_gate_router_observable(&self) -> bool {
        true
    }

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
            has_native_hook: true,
            supports_subagent: true,
            supports_worktree: true,
            mcp_config_key: "mcp",
            transport_type: "opencode-plugin",
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
