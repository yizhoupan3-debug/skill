//! Roadmap v5 §4.1: object-safe host abstraction for B4 (`Box<dyn HostProvider>` registry).
//!
//! P4: `HostLifecycle` / `HostToolExecutor` / `HostTelemetry` expose static metadata hooks
//! consumed by `pre_tool_use_guard` and `host_integration` without registry I/O.

use std::sync::OnceLock;

/// Full harness capabilities for hosts with complete hook support
/// (claude, cursor, codex, opencode — all 4 closed-set hosts).
pub const HARNESS_CAPABILITIES_FULL: &[&str] = &[
    "hot_runtime_routing",
    "l2_continuity_contract",
    "closeout_evidence_hooks",
    "review_gate_router_observation",
];

/// Declared harness surface for a closed-set host (roadmap §4.1).
///
/// Default values reflect the majority across the 4 closed-set hosts.
/// Only fields that differ per host need to be overridden in `capabilities()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCapabilities {
    pub has_native_hook: bool,
    pub supports_subagent: bool,
    pub supports_worktree: bool,
    pub mcp_config_key: &'static str,
    pub transport_type: &'static str,
    pub config_path: &'static str,
    pub batch_execution: bool,
    pub cron_execution: bool,
    pub ci_runner: bool,
    pub non_interactive_entrypoint: bool,
    pub external_session_supervisor: bool,
    pub rate_limit_auto_resume: bool,
}

impl Default for HostCapabilities {
    fn default() -> Self {
        Self {
            has_native_hook: true,
            supports_subagent: true,
            supports_worktree: true,
            mcp_config_key: "",
            transport_type: "",
            config_path: "",
            batch_execution: false,
            cron_execution: false,
            ci_runner: false,
            non_interactive_entrypoint: false,
            external_session_supervisor: false,
            rate_limit_auto_resume: false,
        }
    }
}

/// Session / lifecycle metadata from `RUNTIME_REGISTRY.host_projections`.
pub trait HostLifecycle: Send + Sync {
    fn profile_id(&self) -> &'static str;
    fn session_supervisor_driver(&self) -> &'static str;
    fn context_file(&self) -> &'static str;

    /// Harness capabilities. All 4 closed-set hosts use FULL.
    fn harness_capabilities(&self) -> &'static [&'static str] {
        HARNESS_CAPABILITIES_FULL
    }

    /// Project-local hook manifest when the host installs a native hook bundle.
    fn hooks_manifest_path(&self) -> Option<&'static str> {
        None
    }

    /// Events registered in the native hook manifest (rich CLI hosts only).
    fn registered_hook_events(&self) -> &'static [&'static str] {
        &[]
    }

    /// Binary name for the session supervisor driver (e.g. "codex", "claude").
    /// Default: returns the install_tool name.
    fn driver_binary(&self) -> &'static str {
        ""
    }

    /// Whether this host's driver supports resume semantics.
    fn driver_supports_resume(&self) -> bool {
        false
    }

    /// Build CLI args for the driver command.
    /// Returns `(args, shell_command)` where shell_command is the full shell-escaped string.
    /// Default: returns `None` (caller falls back to legacy match).
    fn build_driver_args(
        &self,
        _cwd: &str,
        _prompt: Option<&str>,
        _resume_target: Option<&str>,
        _resume_mode: &str,
        _resume_only: bool,
    ) -> Option<(Vec<String>, String)> {
        None
    }
}

/// Tool-guard metadata aligned with `pre_tool_use_guard` registry signals.
pub trait HostToolExecutor: Send + Sync {
    /// All 4 closed-set hosts support hard gate hooks (shell/plugin level).
    fn has_hard_gate_hooks(&self) -> bool {
        true
    }

    /// All 4 closed-set hosts support closeout evidence hooks.
    fn closeout_evidence_hooks_supported(&self) -> bool {
        true
    }

    /// All 4 closed-set hosts have native hooks; strict fallback not needed.
    fn requires_strict_pre_tool_fallback_default(&self) -> bool {
        false
    }
}

/// Telemetry / observation metadata for hook journal routing.
pub trait HostTelemetry: Send + Sync {
    /// All 4 closed-set hosts are review gate observable.
    fn review_gate_router_observable(&self) -> bool {
        true
    }
    /// Transport family label for telemetry journal routing.
    fn hook_telemetry_surface(&self) -> &'static str;

    /// `router_rs_observation` host id when outbound hook JSON attaches telemetry.
    fn observation_host_id(&self) -> Option<&'static str> {
        None
    }

    /// Extract followup and additional_context surfaces from hook output JSON.
    /// Default: followup from `followup_message`, additional from
    /// `/hookSpecificOutput/additionalContext` then `additional_context`.
    fn extract_observation_surfaces(&self, output: &serde_json::Value) -> (Option<String>, Option<String>) {
        let followup = output
            .get("followup_message")
            .and_then(serde_json::Value::as_str)
            .map(|s| s.to_string());
        let additional = output
            .pointer("/hookSpecificOutput/additionalContext")
            .or_else(|| output.get("additional_context"))
            .and_then(serde_json::Value::as_str)
            .map(|s| s.to_string());
        (followup, additional)
    }
}

/// Object-safe provider contract; implementors register via [`host_provider_registry`].
pub trait HostProvider: HostLifecycle + HostToolExecutor + HostTelemetry {
    /// `RUNTIME_REGISTRY.host_targets.supported` id (e.g. `cursor`, `claude-code`).
    fn host_id(&self) -> &'static str;

    /// `host_targets.metadata.<id>.install_tool` spelling (e.g. `cursor`, `claude`).
    fn install_tool(&self) -> &'static str;

    /// Alternate CLI / `--to` spellings (`claude-code` → `claude`, etc.).
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    fn capabilities(&self) -> HostCapabilities;
}

/// Closed-set fast path for `pre_tool_use_guard` (no registry disk read).
pub fn host_provider_strict_pre_tool_fallback_hint(host_id: &str) -> Option<bool> {
    host_provider_for_id(host_id).map(|provider| provider.requires_strict_pre_tool_fallback_default())
}

include!(concat!(env!("OUT_DIR"), "/generated_host_providers.rs"));

static HOST_PROVIDER_REGISTRY: OnceLock<Vec<Box<dyn HostProvider>>> = OnceLock::new();

fn build_host_provider_registry() -> Vec<Box<dyn HostProvider>> {
    let mut providers: Vec<Box<dyn HostProvider>> = Vec::new();
    push_registered_host_providers(&mut providers);
    providers
}

/// Lazily initialized closed-set registry (`Box<dyn HostProvider>`).
pub fn host_provider_registry() -> &'static [Box<dyn HostProvider>] {
    HOST_PROVIDER_REGISTRY
        .get_or_init(build_host_provider_registry)
        .as_slice()
}

/// Returns the host_id of the first registered provider.
/// Used as a data-driven default instead of hardcoding a specific host name.
pub fn default_host_id() -> &'static str {
    host_provider_registry()
        .first()
        .map(|p| p.host_id())
        .unwrap_or("codex")
}

pub fn host_provider_for_id(host_id: &str) -> Option<&'static dyn HostProvider> {
    let needle = host_id.trim();
    host_provider_registry()
        .iter()
        .find(|provider| provider.host_id() == needle)
        .map(|boxed| boxed.as_ref())
}

pub fn host_provider_for_install_tool(tool: &str) -> Option<&'static dyn HostProvider> {
    let needle = tool.trim().to_lowercase();
    host_provider_registry().iter().find_map(|boxed| {
        let provider = boxed.as_ref();
        if provider.install_tool() == needle
            || provider.aliases().iter().any(|alias| *alias == needle)
        {
            Some(provider)
        } else {
            None
        }
    })
}

/// Resolve a routing `host_id` spelling (canonical id, install tool, or retired alias).
pub fn host_provider_for_routing_spelling(host_id: &str) -> Option<&'static dyn HostProvider> {
    let needle = host_id.trim();
    host_provider_for_id(needle)
        .or_else(|| host_provider_for_install_tool(needle))
        .or_else(|| {
            let needle_lower = needle.to_ascii_lowercase();
            host_provider_registry().iter().find_map(|boxed| {
                let provider = boxed.as_ref();
                if provider
                    .aliases()
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(&needle_lower))
                {
                    Some(provider)
                } else {
                    None
                }
            })
        })
}

/// Host-filter alias expansion for B1 routing (`host_platforms` matching).
pub fn host_provider_routing_aliases(host_id: &str) -> Vec<String> {
    let needle = host_id.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    let mut push_unique = |value: &str| {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() || out.iter().any(|existing| existing == &normalized) {
            return;
        }
        out.push(normalized);
    };
    push_unique(&needle);
    if let Some(provider) = host_provider_for_routing_spelling(&needle) {
        push_unique(provider.host_id());
        push_unique(provider.install_tool());
        for alias in provider.aliases() {
            push_unique(alias);
        }
    }
    out
}

pub fn host_lifecycle_for_id(host_id: &str) -> Option<&'static dyn HostLifecycle> {
    host_provider_for_id(host_id).map(|provider| provider as &dyn HostLifecycle)
}

pub fn host_tool_executor_for_id(host_id: &str) -> Option<&'static dyn HostToolExecutor> {
    host_provider_for_id(host_id).map(|provider| provider as &dyn HostToolExecutor)
}

pub fn host_telemetry_for_id(host_id: &str) -> Option<&'static dyn HostTelemetry> {
    host_provider_for_id(host_id).map(|provider| provider as &dyn HostTelemetry)
}

/// Find a provider whose `observation_host_id()` matches, then extract surfaces.
/// Returns `None` if no provider matches the observation host id.
pub fn extract_observation_surfaces_for_host(
    host_id: &str,
    output: &serde_json::Value,
) -> Option<(Option<String>, Option<String>)> {
    let provider = host_provider_registry()
        .iter()
        .find(|p| p.observation_host_id() == Some(host_id))?;
    Some(provider.extract_observation_surfaces(output))
}

pub fn validate_host_providers_against_registry(
    supported_host_ids: &[String],
) -> Result<(), String> {
    for provider in host_provider_registry() {
        let host_id = provider.host_id();
        if !supported_host_ids.iter().any(|id| id == host_id) {
            return Err(format!(
                "HostProvider `{host_id}` is not listed in RUNTIME_REGISTRY.host_targets.supported"
            ));
        }
    }
    for host_id in supported_host_ids {
        if host_provider_for_id(host_id).is_none() {
            return Err(format!(
                "RUNTIME_REGISTRY supported host `{host_id}` has no HostProvider registration"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn registry_exposes_closed_set_host_skeletons() {
        let registry = host_provider_registry();
        assert_eq!(
            registry.len(),
            REGISTRY_SUPPORTED_HOST_IDS.len(),
            "HostProvider count must match registry host_targets.supported"
        );
        for host_id in REGISTRY_SUPPORTED_HOST_IDS {
            assert!(
                host_provider_for_id(host_id).is_some(),
                "missing provider for {host_id}"
            );
        }
    }

    #[test]
    #[serial]
    fn supported_hosts_each_have_provider() {
        for host_id in REGISTRY_SUPPORTED_HOST_IDS {
            assert!(
                host_provider_for_id(host_id).is_some(),
                "RUNTIME_REGISTRY supported host `{host_id}` has no HostProvider entry"
            );
        }
    }

    #[test]
    #[serial]
    fn install_tool_and_alias_resolution() {
        assert_eq!(
            host_provider_for_install_tool("cursor")
                .expect("cursor")
                .host_id(),
            "cursor"
        );
        assert_eq!(
            host_provider_for_install_tool("claude")
                .expect("claude install tool")
                .host_id(),
            "claude-code"
        );
        assert_eq!(
            host_provider_for_install_tool("claude-code")
                .expect("claude-code alias")
                .host_id(),
            "claude-code"
        );
        assert_eq!(
            host_provider_for_install_tool("opencode")
                .expect("opencode")
                .host_id(),
            "opencode"
        );
        assert_eq!(
            host_provider_for_install_tool("codex")
                .expect("codex")
                .host_id(),
            "codex"
        );
        assert_eq!(
            host_provider_for_install_tool("codex-cli")
                .expect("codex-cli alias")
                .host_id(),
            "codex"
        );
    }

    #[test]
    #[serial]
    fn all_host_capabilities_match_expected_values() {
        let cases: &[(&str, &str, &str, &str, bool)] = &[
            // (host_id, transport_type, config_path_contains, session_supervisor, has_worktree)
            ("cursor", "cursor-agent", "mcp.json", "unsupported", true),
            ("claude-code", "anthropic-claude-code", ".claude/settings.json", "mcp_bridge", true),
            ("opencode", "opencode-plugin", ".opencode/opencode.json", "unsupported", true),
            ("codex", "native-codex", "config.toml", "codex_driver", true),
        ];
        for &(host_id, transport, config_contains, supervisor, worktree) in cases {
            let provider = host_provider_for_id(host_id).expect(host_id);
            let caps = provider.capabilities();
            assert!(caps.has_native_hook, "{host_id}: has_native_hook");
            assert!(caps.supports_subagent, "{host_id}: supports_subagent");
            assert_eq!(caps.supports_worktree, worktree, "{host_id}: supports_worktree");
            assert_eq!(caps.transport_type, transport, "{host_id}: transport_type");
            assert!(caps.config_path.contains(config_contains), "{host_id}: config_path");
            assert_eq!(provider.session_supervisor_driver(), supervisor, "{host_id}: supervisor");
            // All 4 hosts share these via trait defaults
            assert!(provider.closeout_evidence_hooks_supported(), "{host_id}: closeout");
            assert!(provider.has_hard_gate_hooks(), "{host_id}: hard_gate");
            assert!(!provider.requires_strict_pre_tool_fallback_default(), "{host_id}: strict_fallback");
            assert!(provider.review_gate_router_observable(), "{host_id}: review_gate");
        }
    }

    #[test]
    #[serial]
    fn strict_pre_tool_fallback_hint_matches_provider_metadata() {
        assert_eq!(
            host_provider_strict_pre_tool_fallback_hint("cursor"),
            Some(false)
        );
        assert_eq!(
            host_provider_strict_pre_tool_fallback_hint("opencode"),
            Some(false)
        );
        assert!(host_provider_strict_pre_tool_fallback_hint("unknown-host").is_none());
    }

    #[test]
    fn unknown_host_and_tool_fail_closed() {
        assert!(host_provider_for_id("unknown-host").is_none());
        assert!(host_provider_for_install_tool("unknown-tool").is_none());
    }

    #[test]
    #[serial]
    fn p4_sub_trait_accessors_upcast_from_host_provider() {
        let lifecycle = host_lifecycle_for_id("cursor").expect("cursor lifecycle");
        assert_eq!(lifecycle.profile_id(), "cursor_profile");
        let tool_exec = host_tool_executor_for_id("codex").expect("codex tool executor");
        assert!(!tool_exec.requires_strict_pre_tool_fallback_default());
        let telemetry = host_telemetry_for_id("opencode").expect("opencode telemetry");
        assert!(telemetry.review_gate_router_observable());
    }

    #[test]
    fn native_hook_glue_surfaces_manifest_and_events() {
        let cases: &[(&str, &str, &[&str])] = &[
            // (host_id, manifest_path, sample_events)
            ("cursor", ".cursor/hooks.json", &["beforeSubmitPrompt", "stop"]),
            ("codex", ".codex/hooks.json", &["PreToolUse", "Stop"]),
            ("opencode", ".opencode/plugins/", &["tool.execute.before"]),
        ];
        for &(host_id, manifest, events) in cases {
            let lifecycle = host_lifecycle_for_id(host_id).expect(host_id);
            assert_eq!(lifecycle.hooks_manifest_path(), Some(manifest), "{host_id}: manifest");
            for event in events {
                assert!(lifecycle.registered_hook_events().contains(event), "{host_id}: event {event}");
            }
            let telemetry = host_telemetry_for_id(host_id).expect(&format!("{host_id} telemetry"));
            assert_eq!(telemetry.observation_host_id(), Some(host_id), "{host_id}: observation_host_id");
        }
    }

    #[test]
    #[serial]
    fn routing_aliases_expand_via_host_provider_registry() {
        let cases: &[(&str, &[&str])] = &[
            ("cursor", &["cursor"]),
            (
                "claude-desktop",
                &["claude-desktop", "claude-code", "claude"],
            ),
            ("claude", &["claude", "claude-code", "claude-desktop"]),
            ("codex-cli", &["codex-cli", "codex", "codex-app"]),
            ("codex", &["codex", "codex-cli", "codex-app"]),
        ];
        for (input, expected) in cases {
            let aliases = host_provider_routing_aliases(input);
            for item in *expected {
                assert!(
                    aliases.iter().any(|alias| alias == item),
                    "input={input} missing alias {item} in {aliases:?}"
                );
            }
        }
    }

    #[test]
    #[serial]
    fn routing_spelling_resolves_retired_host_ids() {
        assert_eq!(
            host_provider_for_routing_spelling("claude-desktop")
                .expect("claude-desktop")
                .host_id(),
            "claude-code"
        );
    }
}
