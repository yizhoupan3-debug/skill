//! Codex host: `HostProvider` skeleton (native hook + TOML projection metadata).

use super::host_provider::{
    HostCapabilities, HostLifecycle, HostProvider, HostTelemetry, HostToolExecutor,
};

const HARNESS_CAPABILITIES: &[&str] = &[
    "hot_runtime_routing",
    "l2_continuity_contract",
    "closeout_evidence_hooks",
    "review_gate_router_observation",
];

#[derive(Debug, Default, Clone, Copy)]
pub struct CodexHostProvider;

impl HostLifecycle for CodexHostProvider {
    fn profile_id(&self) -> &'static str {
        "codex_profile"
    }

    fn session_supervisor_driver(&self) -> &'static str {
        "codex_driver"
    }

    fn harness_capabilities(&self) -> &'static [&'static str] {
        HARNESS_CAPABILITIES
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_CODEX.md"
    }

    fn hooks_manifest_path(&self) -> Option<&'static str> {
        Some(crate::hosts::codex_hooks::CODEX_HOOKS_PATH)
    }

    fn registered_hook_events(&self) -> &'static [&'static str] {
        &crate::hosts::codex_hooks::INSTALL_LIFECYCLE_EVENTS
    }
}

impl HostToolExecutor for CodexHostProvider {
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

impl HostTelemetry for CodexHostProvider {
    fn review_gate_router_observable(&self) -> bool {
        true
    }

    fn hook_telemetry_surface(&self) -> &'static str {
        "native-codex"
    }

    fn observation_host_id(&self) -> Option<&'static str> {
        Some("codex")
    }
}

impl HostProvider for CodexHostProvider {
    fn host_id(&self) -> &'static str {
        "codex"
    }

    fn install_tool(&self) -> &'static str {
        "codex"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["codex-cli", "codex-app"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            has_native_hook: true,
            supports_subagent: true,
            supports_worktree: true,
            mcp_config_key: "mcp_servers",
            transport_type: "native-codex",
            config_path: ".codex/config.toml",
            batch_execution: true,
            cron_execution: true,
            ci_runner: true,
            non_interactive_entrypoint: true,
            external_session_supervisor: true,
            rate_limit_auto_resume: true,
        }
    }
}
