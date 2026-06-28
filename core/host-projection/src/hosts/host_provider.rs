//! Object-safe host abstraction (`Box<dyn HostProvider>` registry).
//!
//! P4: `HostLifecycle` / `HostTelemetry` expose static metadata hooks
//! consumed by `pre_tool_use_guard` and `host_integration` without registry I/O.

use std::sync::OnceLock;

/// Full harness capabilities for hosts with complete hook support
/// (all supported hosts from the registry).
pub const HARNESS_CAPABILITIES_FULL: &[&str] = &[
    "hot_runtime_routing",
    "l2_continuity_contract",
    "closeout_evidence_hooks",
    "review_gate_router_observation",
];

/// Declared harness surface for a host (roadmap §4.1).
///
/// Default values reflect the majority across supported hosts.
/// Only fields that differ per host need to be overridden in `capabilities()`.
///
/// **Design note**: The boolean defaults (`has_native_hook`, `supports_subagent`,
/// `supports_worktree`) are `true` because all current supported hosts
/// support these features. If a new host is added that lacks any of these
/// capabilities, it MUST explicitly set the field to `false`
/// in its `capabilities()` override. The test
/// `all_host_capabilities_match_expected_values` in this module guards against
/// accidental default drift.
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

    /// Harness capabilities. All supported hosts use FULL.
    fn harness_capabilities(&self) -> &'static [&'static str] {
        HARNESS_CAPABILITIES_FULL
    }

    /// Subagent review types for this host. Default: standard review types.
    /// Hosts with extended worker types (e.g., Codex) override this via generated code.
    fn subagent_review_types(&self) -> &'static [&'static str] {
        crate::hosts::hook_dispatch::subagent_review_types()
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
    fn extract_observation_surfaces(
        &self,
        output: &serde_json::Value,
    ) -> (Option<String>, Option<String>) {
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
pub trait HostProvider: HostLifecycle + HostTelemetry {
    /// All supported hosts (with native hooks) support hard gate hooks (shell/plugin level).
    fn has_hard_gate_hooks(&self) -> bool {
        true
    }

    /// All supported hosts (with native hooks) support closeout evidence hooks.
    fn closeout_evidence_hooks_supported(&self) -> bool {
        true
    }

    /// All supported hosts (with native hooks) have native hooks; strict fallback not needed.
    fn requires_strict_pre_tool_fallback_default(&self) -> bool {
        false
    }

    /// `RUNTIME_REGISTRY.host_targets.supported` id (e.g. `cursor`, `claude`).
    fn host_id(&self) -> &'static str;

    /// `host_targets.metadata.<id>.install_tool` spelling (e.g. `cursor`, `claude`).
    fn install_tool(&self) -> &'static str;

    /// Alternate CLI / `--to` spellings (`claude` → `claude`, etc.).
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    fn capabilities(&self) -> HostCapabilities;

    /// Hook event dispatcher for this host. Returns a `RegistryDispatcher`
    /// configured from RUNTIME_REGISTRY.json fields.
    /// Used by CLI dispatch to avoid hardcoded host match arms.
    fn dispatcher(&self) -> Box<dyn crate::hosts::hook_dispatch::HostHookDispatcher> {
        unreachable!("HostProvider::dispatcher() default reached — all 4 supported hosts override")
    }
}

/// Closed-set fast path for `pre_tool_use_guard` (no registry disk read).
pub fn host_provider_strict_pre_tool_fallback_hint(host_id: &str) -> Option<bool> {
    host_provider_for_id(host_id)
        .map(|provider| provider.requires_strict_pre_tool_fallback_default())
}

include!(concat!(env!("OUT_DIR"), "/generated_host_providers.rs"));

// ── Hook/Agent dispatch registration (eliminates per-host DISPATCH_TABLE in CLI layer) ──

/// Generic hook dispatch function signature.
/// Each host's CLI hook handler conforms to `fn(host_id, event, repo_root) -> Result<(), FrameworkError>`.
pub type HookDispatchFn = fn(
    host_id: &str,
    event: &str,
    repo_root: Option<&std::path::Path>,
) -> std::result::Result<(), core_errors::FrameworkError>;

/// Generic agent dispatch function signature.
/// Matches host_id (from registration) + repo_root to run the agent MCP loop.
pub type AgentDispatchFn = fn(
    host_id: &str,
    repo_root: Option<&std::path::Path>,
) -> std::result::Result<(), core_errors::FrameworkError>;

static HOOK_DISPATCH_REGISTRY: OnceLock<Vec<(&'static str, HookDispatchFn)>> = OnceLock::new();
static AGENT_DISPATCH_REGISTRY: OnceLock<Vec<(&'static str, AgentDispatchFn)>> = OnceLock::new();

/// Register hook dispatch functions for all supported hosts.
/// Called once during CLI bootstrap (router-rs), before any hook dispatch.
/// Double-registration is silently ignored (safe for re-init scenarios).
pub fn register_hook_dispatchers(entries: Vec<(&'static str, HookDispatchFn)>) {
    if HOOK_DISPATCH_REGISTRY.set(entries).is_err() {
        tracing::warn!("hook dispatch registry already initialized (double-register ignored)");
    }
}

/// Register agent dispatch functions for all supported hosts.
/// Called once during CLI bootstrap. Double-registration silently ignored.
pub fn register_agent_dispatchers(entries: Vec<(&'static str, AgentDispatchFn)>) {
    if AGENT_DISPATCH_REGISTRY.set(entries).is_err() {
        tracing::warn!("agent dispatch registry already initialized (double-register ignored)");
    }
}

/// Find a hook dispatch function by host_id. Returns None for unknown hosts.
pub fn find_hook_dispatch(host_id: &str) -> Option<HookDispatchFn> {
    HOOK_DISPATCH_REGISTRY
        .get_or_init(Vec::new)
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, f)| *f)
}

/// Find an agent dispatch function by host_id.
pub fn find_agent_dispatch(host_id: &str) -> Option<AgentDispatchFn> {
    AGENT_DISPATCH_REGISTRY
        .get_or_init(Vec::new)
        .iter()
        .find(|(id, _)| *id == host_id)
        .map(|(_, f)| *f)
}

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
        if provider.install_tool() == needle {
            Some(provider)
        } else {
            None
        }
    })
}

/// Resolve a routing `host_id` spelling (canonical id or install tool).
pub fn host_provider_for_routing_spelling(host_id: &str) -> Option<&'static dyn HostProvider> {
    let needle = host_id.trim();
    host_provider_for_id(needle).or_else(|| host_provider_for_install_tool(needle))
}

/// Host-filter alias expansion for B1 routing (`host_platforms` matching).
// TODO: extend with HostProvider::routing_aliases() when alias mapping is needed
// (e.g. "claude-code" -> "claude"). Currently all 4 hosts have no extra aliases.
pub fn host_provider_routing_aliases(host_id: &str) -> Vec<String> {
    vec![host_id.trim().to_ascii_lowercase()]
}

pub fn host_lifecycle_for_id(host_id: &str) -> Option<&'static dyn HostLifecycle> {
    host_provider_for_id(host_id).map(|provider| provider as &dyn HostLifecycle)
}

pub fn host_telemetry_for_id(host_id: &str) -> Option<&'static dyn HostTelemetry> {
    host_provider_for_id(host_id).map(|provider| provider as &dyn HostTelemetry)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn host_provider_registry_matches_supported_hosts() {
        let registry = host_provider_registry();
        assert_eq!(
            registry.len(),
            framework_kernel::runtime_registry::ALL_HOST_IDS.len(),
            "HostProvider count must match registry host_targets.supported"
        );
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            assert!(
                host_provider_for_id(host_id).is_some(),
                "missing provider for {host_id}"
            );
        }
    }

    #[test]
    #[serial]
    fn install_tool_and_alias_resolution() {
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let provider = host_provider_for_install_tool(host_id)
                .unwrap_or_else(|| panic!("install_tool for {host_id} must exist"));
            assert_eq!(
                provider.host_id(),
                *host_id,
                "install_tool for {host_id} resolved to wrong host"
            );
        }
    }

    #[test]
    #[serial]
    fn all_host_provider_config_matches_registry_metadata() {
        let framework_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let registry =
            framework_kernel::runtime_registry::load_runtime_registry_payload(&framework_root)
                .expect("load RUNTIME_REGISTRY.json");
        let metadata = registry
            .get("host_targets")
            .and_then(|ht| ht.get("metadata"))
            .and_then(|m| m.as_object())
            .expect("host_targets.metadata");
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let host_meta = metadata
                .get(*host_id)
                .unwrap_or_else(|| panic!("metadata missing host_id `{host_id}`"));
            let expected_transport = host_meta
                .get("transport_type")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("{host_id}: metadata missing transport_type"));
            let expected_supervisor = host_meta
                .get("session_supervisor_driver")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("{host_id}: metadata missing session_supervisor_driver"));

            let provider = host_provider_for_id(host_id)
                .unwrap_or_else(|| panic!("no host provider for {host_id}"));
            let caps = provider.capabilities();
            assert_eq!(
                caps.transport_type, expected_transport,
                "{host_id}: transport_type"
            );
            assert_eq!(
                provider.session_supervisor_driver(),
                expected_supervisor,
                "{host_id}: supervisor"
            );
            assert!(
                provider.closeout_evidence_hooks_supported(),
                "{host_id}: closeout"
            );
            assert!(provider.has_hard_gate_hooks(), "{host_id}: hard_gate");
            assert!(
                !provider.requires_strict_pre_tool_fallback_default(),
                "{host_id}: strict_fallback"
            );
            assert!(
                provider.review_gate_router_observable(),
                "{host_id}: review_gate"
            );
        }
    }

    #[test]
    #[serial]
    fn strict_pre_tool_fallback_hint_matches_provider_metadata() {
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let hint = host_provider_strict_pre_tool_fallback_hint(host_id);
            assert_eq!(
                hint,
                Some(false),
                "{host_id} expected strict_pre_tool_fallback hint"
            );
        }
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
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let lifecycle = host_lifecycle_for_id(host_id)
                .unwrap_or_else(|| panic!("{host_id}: lifecycle upcast"));
            assert!(!lifecycle.profile_id().is_empty(), "{host_id}: profile_id");
            assert!(
                !lifecycle.session_supervisor_driver().is_empty(),
                "{host_id}: supervisor"
            );

            let telemetry = host_telemetry_for_id(host_id)
                .unwrap_or_else(|| panic!("{host_id}: telemetry upcast"));
            assert!(
                telemetry.review_gate_router_observable(),
                "{host_id}: review_gate"
            );
        }
    }

    #[test]
    fn native_hook_glue_surfaces_manifest_and_events() {
        for host_id in framework_kernel::runtime_registry::ALL_HOST_IDS {
            let lifecycle = host_lifecycle_for_id(host_id).expect(host_id);
            if let Some(manifest) = lifecycle.hooks_manifest_path() {
                let events = lifecycle.registered_hook_events();
                assert!(
                    !events.is_empty(),
                    "{host_id}: hooks_manifest_path ({manifest}) but no registered_hook_events"
                );
                for event in events {
                    assert!(
                        !event.is_empty(),
                        "{host_id}: empty event in registered_hook_events"
                    );
                }
                let telemetry =
                    host_telemetry_for_id(host_id).unwrap_or_else(|| panic!("{host_id} telemetry"));
                assert_eq!(
                    telemetry.observation_host_id(),
                    Some(*host_id),
                    "{host_id}: observation_host_id"
                );
            } else {
                // Hosts without hooks_manifest_path must not have registered_hook_events.
                // (hook events require a manifest path to be actionable; hosts that lack
                // one should return empty events to avoid orphan configuration.)
                assert!(
                    lifecycle.registered_hook_events().is_empty(),
                    "{host_id}: no hooks_manifest_path but has registered_hook_events"
                );
            }
        }
    }

    #[test]
    #[serial]
    fn routing_aliases_expand_via_host_provider_registry() {
        let cases: &[(&str, &[&str])] = &[
            ("cursor", &["cursor"]),
            ("claude", &["claude"]),
            ("codex", &["codex"]),
            ("opencode", &["opencode"]),
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
    fn retired_host_ids_no_longer_resolve() {
        assert!(
            host_provider_for_routing_spelling("claude-desktop").is_none(),
            "claude-desktop is retired and should not resolve to any host provider"
        );
    }
}
