//! Claude Code host: `HostProvider` skeleton (stdio hook metadata).

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
pub struct ClaudeHostProvider;

impl HostLifecycle for ClaudeHostProvider {
    fn profile_id(&self) -> &'static str {
        "claude_code_profile"
    }

    fn session_supervisor_driver(&self) -> &'static str {
        "unsupported"
    }

    fn harness_capabilities(&self) -> &'static [&'static str] {
        HARNESS_CAPABILITIES
    }

    fn context_file(&self) -> &'static str {
        "AGENTS_CLAUDE.md"
    }
}

impl HostToolExecutor for ClaudeHostProvider {
    fn has_hard_gate_hooks(&self) -> bool {
        true
    }

    fn closeout_evidence_hooks_supported(&self) -> bool {
        true
    }

    fn requires_strict_pre_tool_fallback_default(&self) -> bool {
        false
    }
}

impl HostTelemetry for ClaudeHostProvider {
    fn review_gate_router_observable(&self) -> bool {
        true
    }

    fn hook_telemetry_surface(&self) -> &'static str {
        "anthropic-claude-code"
    }

    fn observation_host_id(&self) -> Option<&'static str> {
        Some("claude-code")
    }
}

impl HostProvider for ClaudeHostProvider {
    fn host_id(&self) -> &'static str {
        "claude-code"
    }

    fn install_tool(&self) -> &'static str {
        "claude"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["claude-code", "claude-desktop"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            has_native_hook: true,
            supports_subagent: true,
            supports_worktree: true,
            mcp_config_key: "",
            transport_type: "anthropic-claude-code",
            config_path: ".claude/settings.json",
        }
    }
}
