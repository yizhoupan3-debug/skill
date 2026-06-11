//! Roadmap v5 §4.1: object-safe host abstraction for B4 (`Box<dyn HostProvider>` registry).
//!
//! P4: `HostLifecycle` / `HostToolExecutor` / `HostTelemetry` expose static metadata hooks
//! consumed by `pre_tool_use_guard` and `host_integration` without registry I/O.

use std::sync::OnceLock;

/// Declared harness surface for a closed-set host (roadmap §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCapabilities {
    pub has_native_hook: bool,
    pub supports_subagent: bool,
    pub supports_worktree: bool,
    pub mcp_config_key: &'static str,
    pub transport_type: &'static str,
    pub config_path: &'static str,
    // New fields from v6 roadmap I8
    pub batch_execution: bool,
    pub cron_execution: bool,
    pub ci_runner: bool,
    pub non_interactive_entrypoint: bool,
    pub external_session_supervisor: bool,
    pub rate_limit_auto_resume: bool,
}

/// Session / lifecycle metadata from `RUNTIME_REGISTRY.host_projections`.
pub trait HostLifecycle: Send + Sync {
    fn profile_id(&self) -> &'static str;
    fn session_supervisor_driver(&self) -> &'static str;
    fn harness_capabilities(&self) -> &'static [&'static str];
    fn context_file(&self) -> &'static str;

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
    fn has_hard_gate_hooks(&self) -> bool;
    fn closeout_evidence_hooks_supported(&self) -> bool;

    /// Static strict-fallback hint when `has_native_hook` override is absent.
    fn requires_strict_pre_tool_fallback_default(&self) -> bool;
}

/// Telemetry / observation metadata for hook journal routing.
pub trait HostTelemetry: Send + Sync {
    fn review_gate_router_observable(&self) -> bool;
    /// Transport family label for telemetry journal routing.
    fn hook_telemetry_surface(&self) -> &'static str;

    /// `router_rs_observation` host id when outbound hook JSON attaches telemetry.
    fn observation_host_id(&self) -> Option<&'static str> {
        None
    }

    /// Extract followup and additional_context surfaces from hook output JSON.
    /// Returns `(followup, additional)` strings. Default returns `(None, None)`.
    fn extract_observation_surfaces(&self, _output: &serde_json::Value) -> (Option<String>, Option<String>) {
        (None, None)
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
            host_provider_for_install_tool("antigravity")
                .expect("antigravity")
                .host_id(),
            "antigravity"
        );
        assert_eq!(
            host_provider_for_install_tool("antigravity-app")
                .expect("antigravity-app alias")
                .host_id(),
            "antigravity"
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
    fn cursor_capabilities_match_registry_projection() {
        let provider = host_provider_for_id("cursor").expect("cursor provider");
        let caps = provider.capabilities();
        assert!(caps.has_native_hook);
        assert!(caps.supports_subagent);
        assert_eq!(caps.mcp_config_key, "mcpServers");
        assert_eq!(caps.transport_type, "cursor-agent");
        assert!(caps.config_path.contains("mcp.json"));
        assert_eq!(provider.profile_id(), "cursor_profile");
        assert_eq!(provider.session_supervisor_driver(), "unsupported");
        assert_eq!(provider.context_file(), "AGENTS_CURSOR.md");
        assert!(provider.closeout_evidence_hooks_supported());
        assert!(!provider.has_hard_gate_hooks());
        assert!(!provider.requires_strict_pre_tool_fallback_default());
        assert!(provider.review_gate_router_observable());
        assert_eq!(provider.hook_telemetry_surface(), "cursor-agent");
    }

    #[test]
    #[serial]
    fn claude_capabilities_declare_native_hooks_without_mcp_config_key() {
        let provider = host_provider_for_id("claude-code").expect("claude provider");
        let caps = provider.capabilities();
        assert!(caps.has_native_hook);
        assert!(caps.supports_subagent);
        assert_eq!(caps.transport_type, "anthropic-claude-code");
        assert_eq!(caps.config_path, ".claude/settings.json");
        assert!(caps.mcp_config_key.is_empty());
        assert!(provider.has_hard_gate_hooks());
        assert!(!provider.requires_strict_pre_tool_fallback_default());
        assert!(provider.review_gate_router_observable());
    }

    #[test]
    #[serial]
    fn antigravity_capabilities_declare_mcp_stdio_without_native_hooks() {
        let provider = host_provider_for_id("antigravity").expect("antigravity provider");
        let caps = provider.capabilities();
        assert!(!caps.has_native_hook);
        assert_eq!(caps.transport_type, "mcp-stdio");
        assert!(!provider.closeout_evidence_hooks_supported());
        assert!(provider.requires_strict_pre_tool_fallback_default());
        assert!(!provider.review_gate_router_observable());
        assert_eq!(provider.hook_telemetry_surface(), "mcp-stdio");
    }

    #[test]
    #[serial]
    fn opencode_requires_strict_pre_tool_fallback() {
        let provider = host_provider_for_id("opencode").expect("opencode provider");
        assert!(!provider.capabilities().has_native_hook);
        assert!(provider.requires_strict_pre_tool_fallback_default());
        assert_eq!(provider.hook_telemetry_surface(), "opencode-cli");
        assert_eq!(
            provider.capabilities().config_path,
            ".opencode/opencode.json"
        );
    }

    #[test]
    fn codex_capabilities_declare_native_hooks_and_supervisor() {
        let provider = host_provider_for_id("codex").expect("codex provider");
        let caps = provider.capabilities();
        assert!(caps.has_native_hook);
        assert!(caps.supports_subagent);
        assert!(caps.supports_worktree);
        assert_eq!(caps.mcp_config_key, "mcp_servers");
        assert_eq!(caps.transport_type, "native-codex");
        assert!(caps.config_path.contains("config.toml"));
        assert_eq!(provider.session_supervisor_driver(), "codex_driver");
        assert!(!provider.requires_strict_pre_tool_fallback_default());
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
            Some(true)
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
        let telemetry = host_telemetry_for_id("antigravity").expect("antigravity telemetry");
        assert!(!telemetry.review_gate_router_observable());
    }

    #[test]
    fn cursor_native_hook_glue_surfaces_manifest_and_events() {
        let lifecycle = host_lifecycle_for_id("cursor").expect("cursor");
        assert_eq!(lifecycle.hooks_manifest_path(), Some(".cursor/hooks.json"));
        assert!(lifecycle
            .registered_hook_events()
            .contains(&"beforeSubmitPrompt"));
        assert!(lifecycle.registered_hook_events().contains(&"stop"));

        let telemetry = host_telemetry_for_id("cursor").expect("cursor telemetry");
        assert_eq!(telemetry.observation_host_id(), Some("cursor"));
    }

    #[test]
    fn codex_native_hook_glue_surfaces_manifest_and_events() {
        let lifecycle = host_lifecycle_for_id("codex").expect("codex");
        assert_eq!(lifecycle.hooks_manifest_path(), Some(".codex/hooks.json"));
        assert!(lifecycle.registered_hook_events().contains(&"PreToolUse"));
        assert!(lifecycle.registered_hook_events().contains(&"Stop"));

        let telemetry = host_telemetry_for_id("codex").expect("codex telemetry");
        assert_eq!(telemetry.observation_host_id(), Some("codex"));
    }

    #[test]
    #[serial]
    fn anemic_hosts_skip_native_hook_glue_defaults() {
        for host_id in ["antigravity", "opencode"] {
            let lifecycle = host_lifecycle_for_id(host_id).expect(host_id);
            assert_eq!(lifecycle.hooks_manifest_path(), None);
            assert!(lifecycle.registered_hook_events().is_empty());
            assert_eq!(
                host_telemetry_for_id(host_id).and_then(|t| t.observation_host_id()),
                None
            );
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
            (
                "antigravity-cli",
                &["antigravity-cli", "antigravity", "antigravity-app"],
            ),
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
        assert_eq!(
            host_provider_for_routing_spelling("antigravity-cli")
                .expect("antigravity-cli")
                .host_id(),
            "antigravity"
        );
    }
}
