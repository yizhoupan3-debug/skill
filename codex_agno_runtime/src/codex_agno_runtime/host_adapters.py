from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, Mapping, Sequence

from .framework_profile import (
    CORE_CAPABILITIES,
    FrameworkProfile,
    build_framework_session_contract,
    extract_framework_workspace_bridges,
    ensure_capabilities,
    resolve_host_capability_requirements,
)
from .runtime_registry import default_host_peer_set, framework_native_aliases, host_adapter_records
from .trace import (
    TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
    TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
)
from .runtime_registry import default_host_peer_set

UPSTREAM_SAFE_ZONE = "upstream-safe-zone"
THIN_PATCH_ZONE = "thin-patch-zone"
FORK_DANGER_ZONE = "fork-danger-zone"
_HOST_PRIVATE_OVERRIDE_KEY = "host_private"

COMMON_FORK_DANGER_SURFACES = (
    "aionrs_session_protocol",
    "aionrs_event_grammar",
    "aionrs_resume_semantics",
    "aionrs_tool_approval_semantics",
    "aionrs_provider_plumbing",
)

COMMON_PARITY_FIELDS = (
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "tool_policy",
    "approval_policy",
    "loadout_policy",
    "framework_surface_policy",
    "workspace_bootstrap",
    "session_contract",
    "execution_controller_contract",
    "delegation_contract",
    "supervisor_state_contract",
)

_CANONICAL_HOST_ADAPTER_PAYLOAD_FIELDS = frozenset(
    {
        "profile_id",
        "display_name",
        "framework_profile_version",
        "host_family",
        "runtime_family",
        "capabilities",
        "rules_bundle",
        "skill_bundle",
        "session_policy",
        "tool_policy",
        "approval_policy",
        "loadout_policy",
        "framework_surface_policy",
        "artifact_contract",
        "model_policy",
        "memory_mounts",
        "mcp_servers",
        "workspace_bootstrap",
        "host_capability_requirements",
        "metadata",
    }
)

CODEX_DESKTOP_ADAPTER_ID = "codex_desktop_adapter"
CLI_COMMON_ADAPTER_ID = "cli_common_adapter"
CODEX_COMMON_ADAPTER_ID = "codex_common_adapter"
CODEX_CLI_ADAPTER_ID = "codex_cli_adapter"
CLAUDE_CODE_ADAPTER_ID = "claude_code_adapter"
GEMINI_CLI_ADAPTER_ID = "gemini_cli_adapter"
LEGACY_CODEX_DESKTOP_ADAPTER_ID = "codex_desktop_host_adapter"
CLI_FAMILY_PARITY_ARTIFACT_ID = "cli_family_parity_snapshot"
PARITY_BASELINE_ARTIFACT_ID = CLI_FAMILY_PARITY_ARTIFACT_ID
COMPATIBILITY_INVENTORY_ARTIFACT_ID = "upgrade_compatibility_matrix"
EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID = "execution_controller_contract"
DELEGATION_CONTRACT_ARTIFACT_ID = "delegation_contract"
SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID = "supervisor_state_contract"
EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID = (
    "execution_kernel_live_fallback_retirement_status"
)
EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID = (
    "execution_kernel_live_response_serialization_contract"
)
CLI_FAMILY_ENTRYPOINT_IDS = (
    CODEX_CLI_ADAPTER_ID,
    CLAUDE_CODE_ADAPTER_ID,
    GEMINI_CLI_ADAPTER_ID,
)
DEFAULT_HOST_PEER_SET = (
    CODEX_DESKTOP_ADAPTER_ID,
    *CLI_FAMILY_ENTRYPOINT_IDS,
)


def _clone_json_like(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {str(key): _clone_json_like(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_clone_json_like(item) for item in value]
    return value


def _framework_alias_entrypoints_for(host_id: str) -> Dict[str, str]:
    entrypoints: Dict[str, str] = {}
    for alias_name, payload in framework_native_aliases().items():
        if not isinstance(payload, Mapping):
            continue
        host_entrypoints = payload.get("host_entrypoints")
        if not isinstance(host_entrypoints, Mapping):
            continue
        entrypoint = host_entrypoints.get(host_id)
        if isinstance(entrypoint, str) and entrypoint:
            entrypoints[str(alias_name)] = entrypoint
    return entrypoints


def _split_host_overrides(
    host_overrides: Mapping[str, Any],
) -> tuple[Dict[str, Any], Dict[str, Any] | None]:
    normalized = _clone_json_like(host_overrides)
    host_private = normalized.pop(_HOST_PRIVATE_OVERRIDE_KEY, None)
    public_keys = [key for key in normalized if key not in _CANONICAL_HOST_ADAPTER_PAYLOAD_FIELDS]
    if public_keys:
        raise ValueError(
            "host_private field updates require explicit opt-in via "
            f"{_HOST_PRIVATE_OVERRIDE_KEY}: <mapping>."
        )
    if host_private is None:
        return normalized, None
    if not isinstance(host_private, Mapping):
        raise ValueError(
            f"{_HOST_PRIVATE_OVERRIDE_KEY} must be a mapping when provided in host_overrides."
        )
    return normalized, _clone_json_like(host_private)


def _merge_mapping(base: Mapping[str, Any], override: Mapping[str, Any]) -> Dict[str, Any]:
    merged = _clone_json_like(base)
    for key, value in override.items():
        existing = merged.get(key)
        if isinstance(existing, Mapping) and isinstance(value, Mapping):
            merged[str(key)] = _merge_mapping(existing, value)
            continue
        merged[str(key)] = _clone_json_like(value)
    return merged


def _normalize_bundle_items(
    bundle: Any,
    *,
    list_keys: Sequence[str],
    fallback_field: str,
) -> list[Dict[str, Any]]:
    if isinstance(bundle, Mapping):
        for list_key in list_keys:
            items = bundle.get(list_key)
            if isinstance(items, Sequence) and not isinstance(items, (str, bytes, bytearray)):
                normalized: list[Dict[str, Any]] = []
                for item in items:
                    if isinstance(item, Mapping):
                        normalized.append(dict(_clone_json_like(item)))
                    else:
                        normalized.append({fallback_field: item})
                return normalized
        return [dict(_clone_json_like(bundle))]
    if isinstance(bundle, Sequence) and not isinstance(bundle, (str, bytes, bytearray)):
        normalized = []
        for item in bundle:
            if isinstance(item, Mapping):
                normalized.append(dict(_clone_json_like(item)))
            else:
                normalized.append({fallback_field: item})
        return normalized
    return [{"bundle_id": str(bundle)}]


def _resolve_adapter_host_capability_requirements(
    profile: FrameworkProfile,
    adapter_spec: HostAdapterSpec,
) -> Dict[str, Any]:
    return resolve_host_capability_requirements(
        profile,
        host_id=adapter_spec.host_id,
        adapter_id=adapter_spec.adapter_id,
    )


def _compile_session_mode(profile: FrameworkProfile) -> Dict[str, Any]:
    return build_framework_session_contract(profile.session_policy)


def _compile_aionrs_config(profile: FrameworkProfile) -> Dict[str, Any]:
    model_policy = dict(_clone_json_like(profile.model_policy))
    config_keys = {
        "provider",
        "model",
        "profile",
        "base_url",
        "endpoint",
        "temperature",
        "max_tokens",
        "max_output_tokens",
        "headers",
        "compat_mode",
    }
    config = {key: value for key, value in model_policy.items() if key in config_keys}
    requested_provider = str(model_policy.get("provider", "")).lower()
    builtin_provider_path = requested_provider in {"", "anthropic", "openai", "aws-bedrock", "bedrock"}
    boundary = {
        "provider_managed_by": "aionrs-provider-layer",
        "supports_builtin_provider_path": builtin_provider_path,
        "compatible_entry_required": bool(requested_provider) and not builtin_provider_path,
        "framework_core_provider_pinned": False,
    }
    extras = {key: value for key, value in model_policy.items() if key not in config_keys}
    if extras:
        boundary["framework_model_extras"] = extras
    return {
        "config": config,
        "provider_boundary": boundary,
    }

def _compile_tool_approval_mapping(profile: FrameworkProfile) -> Dict[str, Any]:
    return {
        "tool_policy": dict(_clone_json_like(profile.tool_policy)),
        "approval_policy": dict(_clone_json_like(profile.approval_policy)),
        "loadout_policy": dict(_clone_json_like(profile.loadout_policy)),
        "event_map": {
            "request": "tool.approval.request",
            "approved": "tool.approval.approved",
            "denied": "tool.approval.denied",
        },
    }


def _default_event_translation() -> Dict[str, str]:
    return {
        "session.started": "runtime.session.started",
        "session.resumed": "runtime.session.resumed",
        "tool.requested": "tool.approval.request",
        "tool.completed": "tool.execution.completed",
        "message.delta": "runtime.output.delta",
        "message.completed": "runtime.output.completed",
        "session.completed": "runtime.session.completed",
    }


def _default_event_transport() -> Dict[str, Any]:
    return {
        "schema_version": TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
        "bridge_kind": "runtime_event_bridge",
        "transport_family": "host-facing-bridge",
        "transport_kind": "poll",
        "endpoint_kind": "runtime_method",
        "remote_capable": True,
        "handoff_supported": True,
        "handoff_method": "describe_runtime_event_handoff",
        "subscribe_method": "subscribe_runtime_events",
        "cleanup_method": "cleanup_runtime_events",
        "describe_method": "describe_runtime_event_transport",
        "handoff_kind": "artifact_handoff",
        "binding_refresh_mode": "describe_or_checkpoint",
        "binding_artifact_format": "json",
        "resume_mode": "after_event_id",
        "heartbeat_supported": True,
        "cleanup_semantics": "bridge_cache_only",
        "cleanup_preserves_replay": True,
        "replay_reseed_supported": True,
        "chunk_schema_version": TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
        "cursor_schema_version": TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
        "replay_supported": True,
    }


def _default_event_stream_binding() -> Dict[str, Any]:
    return _default_event_transport()


@dataclass(frozen=True)
class HostAdapterSpec:
    adapter_id: str
    host_id: str
    transport: str
    required_capabilities: tuple[str, ...] = ()
    optional_capabilities: tuple[str, ...] = ()
    host_capabilities: tuple[str, ...] = ()
    emits_artifacts: bool = True
    supports_memory_mounts: bool = True
    supports_orchestration: bool = True
    upgrade_zone: str = UPSTREAM_SAFE_ZONE
    thin_patch_surfaces: tuple[str, ...] = ()
    fork_danger_surfaces: tuple[str, ...] = COMMON_FORK_DANGER_SURFACES
    protocol_hints: Dict[str, Any] = field(default_factory=dict)
    notes: str = ""

    def to_dict(self) -> Dict[str, Any]:
        return {
            "adapter_id": self.adapter_id,
            "host_id": self.host_id,
            "transport": self.transport,
            "required_capabilities": list(self.required_capabilities),
            "optional_capabilities": list(self.optional_capabilities),
            "host_capabilities": list(self.host_capabilities),
            "emits_artifacts": self.emits_artifacts,
            "supports_memory_mounts": self.supports_memory_mounts,
            "supports_orchestration": self.supports_orchestration,
            "upgrade_zone": self.upgrade_zone,
            "thin_patch_surfaces": list(self.thin_patch_surfaces),
            "fork_danger_surfaces": list(self.fork_danger_surfaces),
            "protocol_hints": dict(self.protocol_hints),
            "notes": self.notes,
        }


@dataclass(frozen=True)
class AdaptedHostProfile:
    framework_profile: FrameworkProfile
    adapter: HostAdapterSpec
    host_payload: Dict[str, Any]


AIONRS_COMPANION_ADAPTER = HostAdapterSpec(
    adapter_id="aionrs_companion_adapter",
    host_id="aionrs-companion",
    transport="stdio-jsonl",
    required_capabilities=("runtime", "artifact"),
    optional_capabilities=("memory", "orchestration"),
    host_capabilities=(
        "streaming_events",
        "tool_approval",
        "session_mode",
        "dynamic_config",
        "mcp_config",
        "workspace_bootstrap",
        "skill_bridge",
        "memory_bridge",
    ),
    thin_patch_surfaces=(
        "startup_wrapper",
        "default_config_injection",
        "bridge_cleanup_strategy",
    ),
    protocol_hints={
        "mode": "companion",
        "host_boundary": "outer-framework-owned",
        "deep_adaptation_not_fork": True,
        "legacy_surface": True,
        "legacy_lane": "fallback",
        "default_host_peer_set_member": False,
        "works_without_aionrs": False,
    },
    notes=(
        "Legacy compatibility companion surface only; does not modify aionrs internals "
        "and may not re-enter the default host peer set."
    ),
)

AIONUI_HOST_ADAPTER = HostAdapterSpec(
    adapter_id="aionui_host_adapter",
    host_id="aionui",
    transport="bridge-contract",
    required_capabilities=("runtime", "artifact", "memory"),
    optional_capabilities=("orchestration",),
    host_capabilities=(
        "conversation_bootstrap",
        "tool_approval_ui",
        "event_stream_binding",
        "workspace_sync",
        "team_mode_sync",
    ),
    thin_patch_surfaces=("host_metadata_injection", "bridge_path_cleanup"),
    protocol_hints={
        "ui_binding": "host-shell",
        "state_source": "framework_profile",
        "deep_adaptation_not_fork": True,
        "legacy_surface": True,
        "legacy_lane": "fallback",
        "default_host_peer_set_member": False,
        "preferred_backend": "aionrs_companion_adapter",
    },
    notes=(
        "Legacy compatibility host shell only; maps the framework contract into AionUI "
        "without re-entering the default host peer set."
    ),
)

CLI_COMMON_ADAPTER = HostAdapterSpec(
    adapter_id=CLI_COMMON_ADAPTER_ID,
    host_id="cli-family-shared",
    transport="host-neutral-contract",
    required_capabilities=("runtime", "memory", "artifact", "orchestration"),
    host_capabilities=(
        "artifact_contract",
        "memory_mounts",
        "mcp_servers",
        "tool_policy",
        "approval_policy",
        "loadout_policy",
        "framework_surface_policy",
        "workspace_bootstrap",
        "session_contract",
    ),
    protocol_hints={
        "single_framework_truth": True,
        "shared_between_hosts": (CODEX_DESKTOP_ADAPTER_ID, *CLI_FAMILY_ENTRYPOINT_IDS),
        "codexcli_is_controller": False,
        "cli_family_projection": True,
    },
    notes=(
        "Shared outer contract projection reused by Codex Desktop and the multi-host "
        "CLI family without forking framework truth."
    ),
)

CODEX_COMMON_ADAPTER = HostAdapterSpec(
    adapter_id=CODEX_COMMON_ADAPTER_ID,
    host_id="codex-shared",
    transport=CLI_COMMON_ADAPTER.transport,
    required_capabilities=CLI_COMMON_ADAPTER.required_capabilities,
    optional_capabilities=CLI_COMMON_ADAPTER.optional_capabilities,
    host_capabilities=CLI_COMMON_ADAPTER.host_capabilities,
    emits_artifacts=CLI_COMMON_ADAPTER.emits_artifacts,
    supports_memory_mounts=CLI_COMMON_ADAPTER.supports_memory_mounts,
    supports_orchestration=CLI_COMMON_ADAPTER.supports_orchestration,
    upgrade_zone=CLI_COMMON_ADAPTER.upgrade_zone,
    thin_patch_surfaces=CLI_COMMON_ADAPTER.thin_patch_surfaces,
    fork_danger_surfaces=CLI_COMMON_ADAPTER.fork_danger_surfaces,
    protocol_hints={
        **CLI_COMMON_ADAPTER.protocol_hints,
        "canonical_adapter_id": CLI_COMMON_ADAPTER_ID,
        "compatibility_projection": True,
    },
    notes=(
        "Legacy Codex-flavored view of the shared CLI-family contract; keep for "
        "continuity while cli_common_adapter becomes canonical."
    ),
)

CODEX_DESKTOP_ADAPTER = HostAdapterSpec(
    adapter_id=CODEX_DESKTOP_ADAPTER_ID,
    host_id="codex-desktop",
    transport="local-bridge",
    required_capabilities=("runtime", "memory", "artifact", "orchestration"),
    host_capabilities=(
        "local_runtime",
        "artifact_contract",
        "memory_mounts",
        "mcp_servers",
        "automation_bridge",
        "orchestration_control",
    ),
    thin_patch_surfaces=("desktop_metadata_injection",),
    protocol_hints={
        "desktop_mode": True,
        "state_source": "framework_profile",
        "works_without_aionrs": True,
    },
    notes="Primary non-aionrs host path for preserving portable framework core.",
)

CODEX_DESKTOP_HOST_ADAPTER = HostAdapterSpec(
    adapter_id=LEGACY_CODEX_DESKTOP_ADAPTER_ID,
    host_id=CODEX_DESKTOP_ADAPTER.host_id,
    transport=CODEX_DESKTOP_ADAPTER.transport,
    required_capabilities=CODEX_DESKTOP_ADAPTER.required_capabilities,
    optional_capabilities=CODEX_DESKTOP_ADAPTER.optional_capabilities,
    host_capabilities=CODEX_DESKTOP_ADAPTER.host_capabilities,
    emits_artifacts=CODEX_DESKTOP_ADAPTER.emits_artifacts,
    supports_memory_mounts=CODEX_DESKTOP_ADAPTER.supports_memory_mounts,
    supports_orchestration=CODEX_DESKTOP_ADAPTER.supports_orchestration,
    upgrade_zone=CODEX_DESKTOP_ADAPTER.upgrade_zone,
    thin_patch_surfaces=CODEX_DESKTOP_ADAPTER.thin_patch_surfaces,
    fork_danger_surfaces=CODEX_DESKTOP_ADAPTER.fork_danger_surfaces,
    protocol_hints={
        **CODEX_DESKTOP_ADAPTER.protocol_hints,
        "canonical_adapter_id": CODEX_DESKTOP_ADAPTER_ID,
        "legacy_alias": True,
        "legacy_surface": True,
        "legacy_lane": "compatibility",
        "default_host_peer_set_member": False,
    },
    notes="Compatibility alias for codex_desktop_adapter; preserves the legacy host-specific name.",
)

CODEX_CLI_ADAPTER = HostAdapterSpec(
    adapter_id=CODEX_CLI_ADAPTER_ID,
    host_id="codex-cli",
    transport="headless-exec",
    required_capabilities=("runtime", "memory", "artifact", "orchestration"),
    host_capabilities=(
        "artifact_contract",
        "memory_mounts",
        "mcp_servers",
        "workspace_bootstrap",
        "batch_execution",
        "cron_execution",
        "ci_runner",
        "non_interactive_entrypoint",
        "external_session_supervisor",
        "rate_limit_auto_resume",
        "host_resume_entrypoint",
        "host_tmux_worker_management",
        "framework_alias_entrypoints",
    ),
    thin_patch_surfaces=("cli_metadata_injection",),
    protocol_hints={
        "headless_mode": True,
        "state_source": "framework_profile",
        "works_without_aionrs": True,
        "codexcli_is_controller": False,
        "context_files": ("AGENTS.md",),
        "settings_paths": ("~/.codex/config.toml", ".codex/config.toml"),
        "mcp_config_paths": (".codex/config.toml",),
        "session_supervisor_driver": "codex_driver",
        "resume_command_examples": ("codex resume --last", "codex resume <session_id>"),
        "framework_alias_entrypoints": _framework_alias_entrypoints_for("codex-cli"),
    },
    notes="Formal headless Codex entrypoint that consumes the shared framework contract.",
)

CLAUDE_CODE_ADAPTER = HostAdapterSpec(
    adapter_id=CLAUDE_CODE_ADAPTER_ID,
    host_id="claude-code",
    transport="headless-exec",
    required_capabilities=("runtime", "memory", "artifact", "orchestration"),
    host_capabilities=(
        "artifact_contract",
        "memory_mounts",
        "mcp_servers",
        "workspace_bootstrap",
        "batch_execution",
        "ci_runner",
        "non_interactive_entrypoint",
        "context_file",
        "settings_json",
        "settings_scope_hierarchy",
        "subagent_registry",
        "managed_policy",
        "hook_registry",
        "hook_policy",
        "hook_browser",
        "checkpoint_restore",
        "external_session_supervisor",
        "rate_limit_auto_resume",
        "host_resume_entrypoint",
        "host_tmux_worker_management",
        "framework_alias_entrypoints",
    ),
    thin_patch_surfaces=("cli_metadata_injection", "settings_bridge_projection"),
    protocol_hints={
        "headless_mode": True,
        "state_source": "framework_profile",
        "works_without_aionrs": True,
        "config_root_env_var": "CLAUDE_CONFIG_DIR",
        "context_files": ("CLAUDE.md", "CLAUDE.local.md"),
        "settings_paths": (
            "~/.claude/settings.json",
            ".claude/settings.json",
            ".claude/settings.local.json",
        ),
        "mcp_config_paths": ("~/.claude.json",),
        "settings_scope_order": ("managed", "command_line", "local", "project", "user"),
        "settings_scopes": (
            {
                "scope": "managed",
                "locations": (
                    "server-managed",
                    "managed-settings.json",
                    "managed-settings.d/*.json",
                    "managed-mcp.json",
                ),
                "shared_with_team": True,
            },
            {
                "scope": "user",
                "locations": ("~/.claude/settings.json", "~/.claude/CLAUDE.md", "~/.claude/agents/"),
                "shared_with_team": False,
            },
            {
                "scope": "project",
                "locations": (
                    ".claude/settings.json",
                    "CLAUDE.md",
                    ".claude/agents/",
                ),
                "shared_with_team": True,
            },
            {
                "scope": "local",
                "locations": (".claude/settings.local.json", "CLAUDE.local.md"),
                "shared_with_team": False,
            },
        ),
        "subagent_paths": ("~/.claude/agents/", ".claude/agents/"),
        "claude_directory_features": (
            ".claude/settings.json",
            ".claude/settings.local.json",
            ".claude/hooks/",
            ".claude/agents/",
            ".claude/commands/",
            ".claude/rules/",
            ".claude/output-styles/",
        ),
        "hook_event_names": (
            "PreToolUse",
            "PostToolUse",
            "Notification",
            "Stop",
            "SubagentStart",
            "SubagentStop",
            "PreCompact",
            "PostCompact",
            "SessionStart",
            "SessionEnd",
            "UserPromptSubmit",
            "PostToolUseFailure",
            "StopFailure",
            "PermissionRequest",
            "PermissionDenied",
            "InstructionsLoaded",
            "ConfigChange",
            "CwdChanged",
            "FileChanged",
            "TaskCreated",
            "TaskCompleted",
            "WorktreeCreate",
            "WorktreeRemove",
            "TeammateIdle",
            "Elicitation",
            "ElicitationResult",
        ),
        "hook_handler_types": ("command", "prompt", "agent", "http"),
        "hook_control_settings": (
            "disableAllHooks",
            "allowManagedHooksOnly",
            "allowedHttpHookUrls",
            "httpHookAllowedEnvVars",
        ),
        "hook_definition_sources": (
            {
                "source": "managed_settings",
                "locations": (
                    "/Library/Application Support/ClaudeCode/managed-settings.json",
                    "/etc/claude-code/managed-settings.json",
                    "C:/Program Files/ClaudeCode/managed-settings.json",
                ),
            },
            {
                "source": "user_settings",
                "locations": ("~/.claude/settings.json",),
            },
            {
                "source": "project_settings",
                "locations": (".claude/settings.json",),
            },
            {
                "source": "local_settings",
                "locations": (".claude/settings.local.json",),
            },
            {
                "source": "plugin_manifest",
                "locations": ("hooks/hooks.json",),
            },
            {
                "source": "agent_frontmatter",
                "locations": ("~/.claude/agents/*.md", ".claude/agents/*.md"),
            },
            {
                "source": "skill_frontmatter",
                "locations": (".claude/skills/*.md",),
            },
            {
                "source": "session",
                "locations": ("/hooks",),
            },
            {
                "source": "built_in",
                "locations": ("/hooks",),
            },
            {
                "source": "sdk",
                "locations": ("sdk_message_stream",),
            },
        ),
        "hook_inspection_commands": ("/hooks",),
        "plugin_hook_manifest_paths": ("hooks/hooks.json",),
        "hook_environment_markers": (
            "CLAUDE_ENV_FILE",
            "CLAUDE_PROJECT_DIR",
            "CLAUDE_PLUGIN_ROOT",
            "CLAUDE_PLUGIN_DATA",
            "CLAUDE_CODE_REMOTE",
        ),
        "checkpointing_supported": True,
        "managed_settings_paths": (
            "/Library/Application Support/ClaudeCode/managed-settings.json",
            "/etc/claude-code/managed-settings.json",
            "C:/Program Files/ClaudeCode/managed-settings.json",
        ),
        "managed_mcp_paths": (
            "/Library/Application Support/ClaudeCode/managed-mcp.json",
            "/etc/claude-code/managed-mcp.json",
            "C:/Program Files/ClaudeCode/managed-mcp.json",
        ),
        "session_supervisor_driver": "claude_driver",
        "resume_command_examples": ("claude --continue", "claude --resume <session_id>"),
        "framework_alias_entrypoints": _framework_alias_entrypoints_for("claude-code"),
    },
    notes=(
        "Claude Code projection over the shared framework truth; keep host-specific "
        "settings/context discovery thin."
    ),
)

GEMINI_CLI_ADAPTER = HostAdapterSpec(
    adapter_id=GEMINI_CLI_ADAPTER_ID,
    host_id="gemini-cli",
    transport="headless-exec",
    required_capabilities=("runtime", "memory", "artifact", "orchestration"),
    host_capabilities=(
        "artifact_contract",
        "memory_mounts",
        "mcp_servers",
        "workspace_bootstrap",
        "batch_execution",
        "ci_runner",
        "non_interactive_entrypoint",
        "context_file",
        "settings_json",
    ),
    thin_patch_surfaces=("cli_metadata_injection", "settings_bridge_projection"),
    protocol_hints={
        "headless_mode": True,
        "state_source": "framework_profile",
        "works_without_aionrs": True,
        "context_files": ("GEMINI.md",),
        "settings_paths": ("~/.gemini/settings.json",),
        "mcp_config_paths": ("~/.gemini/settings.json",),
        "structured_output_modes": ("json", "stream-json"),
        "checkpointing_supported": True,
    },
    notes=(
        "Gemini CLI projection over the shared framework truth with headless JSON "
        "and stream-json scripting affordances."
    ),
)

GENERIC_HOST_ADAPTER = HostAdapterSpec(
    adapter_id="generic_host_adapter",
    host_id="generic",
    transport="inproc",
    required_capabilities=("runtime", "memory", "artifact", "orchestration"),
    host_capabilities=("local_runtime", "artifact_contract", "memory_mounts"),
    protocol_hints={"works_without_aionrs": True},
    notes="Fallback adapter for any host that only needs the outer framework contract.",
)


HOST_ADAPTERS: Dict[str, HostAdapterSpec] = {
    CLI_COMMON_ADAPTER.adapter_id: CLI_COMMON_ADAPTER,
    CODEX_COMMON_ADAPTER.adapter_id: CODEX_COMMON_ADAPTER,
    CODEX_DESKTOP_ADAPTER.adapter_id: CODEX_DESKTOP_ADAPTER,
    CODEX_CLI_ADAPTER.adapter_id: CODEX_CLI_ADAPTER,
    CLAUDE_CODE_ADAPTER.adapter_id: CLAUDE_CODE_ADAPTER,
    GEMINI_CLI_ADAPTER.adapter_id: GEMINI_CLI_ADAPTER,
    GENERIC_HOST_ADAPTER.adapter_id: GENERIC_HOST_ADAPTER,
}

COMPATIBILITY_HOST_ADAPTERS: Dict[str, HostAdapterSpec] = {
    AIONRS_COMPANION_ADAPTER.adapter_id: AIONRS_COMPANION_ADAPTER,
    AIONUI_HOST_ADAPTER.adapter_id: AIONUI_HOST_ADAPTER,
    CODEX_DESKTOP_HOST_ADAPTER.adapter_id: CODEX_DESKTOP_HOST_ADAPTER,
}

ALL_HOST_ADAPTERS: Dict[str, HostAdapterSpec] = {
    **HOST_ADAPTERS,
    **COMPATIBILITY_HOST_ADAPTERS,
}


def _select_host_adapter_registry(*, include_legacy_aliases: bool) -> Dict[str, HostAdapterSpec]:
    return ALL_HOST_ADAPTERS if include_legacy_aliases else HOST_ADAPTERS


def get_host_adapter(
    adapter_id: str,
    *,
    include_legacy_aliases: bool = False,
) -> HostAdapterSpec:
    registry = _select_host_adapter_registry(include_legacy_aliases=include_legacy_aliases)
    try:
        return registry[adapter_id]
    except KeyError as exc:
        if not include_legacy_aliases and adapter_id in COMPATIBILITY_HOST_ADAPTERS:
            raise KeyError(
                f"unknown host adapter: {adapter_id}; legacy compatibility surfaces require "
                "include_legacy_aliases=True"
            ) from exc
        raise KeyError(f"unknown host adapter: {adapter_id}") from exc


def list_host_adapters(*, include_legacy_aliases: bool = False) -> tuple[HostAdapterSpec, ...]:
    return tuple(_select_host_adapter_registry(include_legacy_aliases=include_legacy_aliases).values())


def adapt_framework_profile(
    profile: FrameworkProfile,
    adapter: HostAdapterSpec | str,
    *,
    host_overrides: Mapping[str, Any] | None = None,
    include_legacy_aliases: bool = False,
) -> AdaptedHostProfile:
    adapter_spec = (
        get_host_adapter(adapter, include_legacy_aliases=include_legacy_aliases)
        if isinstance(adapter, str)
        else adapter
    )
    ensure_capabilities(profile, adapter_spec.required_capabilities)
    shared_contract_surface = profile.shared_contract_surface()

    payload = {
        "profile_id": profile.profile_id,
        "display_name": profile.display_name,
        "framework_profile_version": profile.framework_profile_version,
        "host_family": profile.host_family,
        "runtime_family": profile.runtime_family,
        "capabilities": {
            "core": list(profile.core_capabilities),
            "optional": list(profile.optional_capabilities),
            "host": list(adapter_spec.host_capabilities),
        },
        "rules_bundle": _clone_json_like(profile.rules_bundle),
        "skill_bundle": _clone_json_like(profile.skill_bundle),
        "session_policy": dict(_clone_json_like(profile.session_policy)),
        "tool_policy": _clone_json_like(shared_contract_surface["tool_policy"]),
        "approval_policy": _clone_json_like(shared_contract_surface["approval_policy"]),
        "loadout_policy": _clone_json_like(shared_contract_surface["loadout_policy"]),
        "framework_surface_policy": _clone_json_like(
            shared_contract_surface["framework_surface_policy"]
        ),
        "artifact_contract": _clone_json_like(shared_contract_surface["artifact_contract"]),
        "model_policy": dict(_clone_json_like(profile.model_policy)),
        "memory_mounts": _clone_json_like(shared_contract_surface["memory_mounts"]),
        "mcp_servers": _clone_json_like(shared_contract_surface["mcp_servers"]),
        "workspace_bootstrap": _clone_json_like(shared_contract_surface["workspace_bootstrap"]),
        "host_capability_requirements": _resolve_adapter_host_capability_requirements(
            profile,
            adapter_spec,
        ),
        "metadata": {
            **dict(_clone_json_like(profile.metadata)),
            "adapter_id": adapter_spec.adapter_id,
            "host_id": adapter_spec.host_id,
            "transport": adapter_spec.transport,
            "deep_adaptation_not_fork": adapter_spec.protocol_hints.get("deep_adaptation_not_fork", False),
            "upgrade_zone": adapter_spec.upgrade_zone,
        },
    }
    if host_overrides:
        public_overrides, host_private_overrides = _split_host_overrides(host_overrides)
        if public_overrides:
            payload = _merge_mapping(payload, public_overrides)
        if host_private_overrides:
            payload = _merge_mapping(payload, host_private_overrides)
    return AdaptedHostProfile(
        framework_profile=profile,
        adapter=adapter_spec,
        host_payload=payload,
    )


def compile_aionrs_companion_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    from codex_agno_runtime.host_adapter_compatibility import compile_aionrs_companion_adapter as impl

    return impl(profile, host_overrides=host_overrides)


def compile_aionui_host_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    from codex_agno_runtime.host_adapter_compatibility import compile_aionui_host_adapter as impl

    return impl(profile, host_overrides=host_overrides)


def _build_codex_shared_contract(profile: FrameworkProfile) -> Dict[str, Any]:
    shared_contract = profile.shared_contract_surface()
    shared_contract["execution_controller_contract"] = build_execution_controller_contract()
    shared_contract["delegation_contract"] = build_delegation_contract()
    shared_contract["supervisor_state_contract"] = build_supervisor_state_contract()
    return shared_contract


def _build_cli_controller_boundary() -> Dict[str, Any]:
    return {
        "framework_truth": "framework_core",
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "host_entrypoints": [
            CODEX_DESKTOP_ADAPTER.adapter_id,
            *CLI_FAMILY_ENTRYPOINT_IDS,
        ],
        "cli_family_entrypoints": list(CLI_FAMILY_ENTRYPOINT_IDS),
        "single_source_of_truth": True,
        "codexcli_is_controller": False,
    }


def _build_cli_parity_contract() -> Dict[str, Any]:
    return {
        "shared_fields": list(COMMON_PARITY_FIELDS),
        "desktop_adapter": CODEX_DESKTOP_ADAPTER_ID,
        "cli_common_adapter": CLI_COMMON_ADAPTER_ID,
        "cli_adapters": list(CLI_FAMILY_ENTRYPOINT_IDS),
        "legacy_codex_common_adapter": CODEX_COMMON_ADAPTER_ID,
    }


def _build_cli_runtime_surface(common_contract: Mapping[str, Any]) -> Dict[str, Any]:
    return {
        field: _clone_json_like(common_contract["shared_contract"][field])
        for field in COMMON_PARITY_FIELDS
    }


def _build_adapter_source_contract(
    *,
    canonical_adapter_id: str,
    shared_contract_field: str,
    runtime_surface_field: str | None = None,
    bridge_contract_field: str | None = "bridge_contract",
    entrypoint_surface_field: str | None = None,
    execution_surface_field: str | None = None,
    adapter_alias_of: str | None = None,
    alias_mode: str | None = None,
) -> Dict[str, Any]:
    contract_source_fields: Dict[str, Any] = {
        "shared_contract": shared_contract_field,
    }
    if runtime_surface_field is not None:
        contract_source_fields["runtime_surface"] = runtime_surface_field
    if bridge_contract_field is not None:
        contract_source_fields["bridge_contract"] = bridge_contract_field
    if entrypoint_surface_field is not None:
        contract_source_fields["entrypoint_surface"] = entrypoint_surface_field
    if execution_surface_field is not None:
        contract_source_fields["execution_surface"] = execution_surface_field

    payload: Dict[str, Any] = {
        "framework_truth": "framework_core",
        "state_source": "framework_profile",
        "single_source_of_truth": True,
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "canonical_adapter_id": canonical_adapter_id,
        "contract_source_fields": contract_source_fields,
    }
    if bridge_contract_field is not None:
        payload["bridge_contract_source"] = (
            f"{shared_contract_field}.workspace_bootstrap.bridges"
        )
    if adapter_alias_of is not None:
        payload["adapter_alias_of"] = adapter_alias_of
    if alias_mode is not None:
        payload["alias_mode"] = alias_mode
    return payload


def _compile_cli_host_surface(adapter_spec: HostAdapterSpec) -> Dict[str, Any]:
    return {
        "host_cli": adapter_spec.host_id,
        "context_files": list(adapter_spec.protocol_hints.get("context_files", ())),
        "settings_paths": list(adapter_spec.protocol_hints.get("settings_paths", ())),
        "mcp_config_paths": list(adapter_spec.protocol_hints.get("mcp_config_paths", ())),
        "config_root_env_var": adapter_spec.protocol_hints.get("config_root_env_var"),
        "settings_scope_order": list(adapter_spec.protocol_hints.get("settings_scope_order", ())),
        "settings_scopes": _clone_json_like(adapter_spec.protocol_hints.get("settings_scopes", ())),
        "subagent_paths": list(adapter_spec.protocol_hints.get("subagent_paths", ())),
        "claude_directory_features": list(
            adapter_spec.protocol_hints.get("claude_directory_features", ())
        ),
        "hook_event_names": list(adapter_spec.protocol_hints.get("hook_event_names", ())),
        "hook_handler_types": list(adapter_spec.protocol_hints.get("hook_handler_types", ())),
        "hook_control_settings": list(
            adapter_spec.protocol_hints.get("hook_control_settings", ())
        ),
        "hook_definition_sources": _clone_json_like(
            adapter_spec.protocol_hints.get("hook_definition_sources", ())
        ),
        "hook_inspection_commands": list(
            adapter_spec.protocol_hints.get("hook_inspection_commands", ())
        ),
        "plugin_hook_manifest_paths": list(
            adapter_spec.protocol_hints.get("plugin_hook_manifest_paths", ())
        ),
        "hook_environment_markers": list(
            adapter_spec.protocol_hints.get("hook_environment_markers", ())
        ),
        "managed_settings_paths": list(adapter_spec.protocol_hints.get("managed_settings_paths", ())),
        "managed_mcp_paths": list(adapter_spec.protocol_hints.get("managed_mcp_paths", ())),
        "structured_output_modes": list(
            adapter_spec.protocol_hints.get("structured_output_modes", ())
        ),
        "checkpointing_supported": bool(
            adapter_spec.protocol_hints.get("checkpointing_supported", False)
        ),
    }


def _compile_cli_entrypoint_payload(
    profile: FrameworkProfile,
    adapter_spec: HostAdapterSpec,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    adapted = adapt_framework_profile(profile, adapter_spec, host_overrides=host_overrides)
    common_contract = compile_cli_common_adapter(profile, host_overrides=host_overrides).host_payload
    payload = dict(adapted.host_payload)
    payload["common_contract"] = _clone_json_like(common_contract["shared_contract"])
    payload["controller_boundary"] = _clone_json_like(common_contract["controller_boundary"])
    payload["runtime_surface"] = _build_cli_runtime_surface(common_contract)
    payload["bridge_contract"] = extract_framework_workspace_bridges(
        payload["common_contract"]["workspace_bootstrap"]
    )
    payload["source_contract"] = _build_adapter_source_contract(
        canonical_adapter_id=adapter_spec.adapter_id,
        shared_contract_field="common_contract",
        runtime_surface_field="runtime_surface",
        execution_surface_field="execution_surface",
    )
    payload["host_projection"] = _compile_cli_host_surface(adapter_spec)
    payload["execution_surface"] = {
        "entrypoint_kind": "headless",
        "non_interactive": True,
        "supports_batch": "batch_execution" in adapter_spec.host_capabilities,
        "supports_cron": "cron_execution" in adapter_spec.host_capabilities,
        "supports_ci": "ci_runner" in adapter_spec.host_capabilities,
        "framework_truth": "framework_core",
        "controller_is_cli": False,
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "host_cli": adapter_spec.host_id,
    }
    payload["fallback_semantics"] = {
        "requires_aionrs": False,
        "preserves_core_capabilities": list(CORE_CAPABILITIES),
        "degrade_to": "generic_host_adapter",
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "desktop_peer": CODEX_DESKTOP_ADAPTER_ID,
        "legacy_desktop_peer_aliases": [LEGACY_CODEX_DESKTOP_ADAPTER_ID],
        "cli_family_peers": [
            adapter_id for adapter_id in CLI_FAMILY_ENTRYPOINT_IDS if adapter_id != adapter_spec.adapter_id
        ],
    }
    return AdaptedHostProfile(
        framework_profile=adapted.framework_profile,
        adapter=adapted.adapter,
        host_payload=payload,
    )


def _compile_shared_adapter_alias(
    profile: FrameworkProfile,
    alias_spec: HostAdapterSpec,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    canonical = compile_cli_common_adapter(profile, host_overrides=host_overrides)
    payload = _clone_json_like(canonical.host_payload)
    payload["metadata"]["adapter_id"] = alias_spec.adapter_id
    payload["metadata"]["host_id"] = alias_spec.host_id
    payload["metadata"]["adapter_alias_of"] = CLI_COMMON_ADAPTER_ID
    payload["metadata"]["canonical_adapter_id"] = CLI_COMMON_ADAPTER_ID
    payload["parity_contract"]["compatibility_aliases"] = [alias_spec.adapter_id]
    payload["source_contract"] = _build_adapter_source_contract(
        canonical_adapter_id=CLI_COMMON_ADAPTER_ID,
        shared_contract_field="shared_contract",
        adapter_alias_of=CLI_COMMON_ADAPTER_ID,
        alias_mode="mirror-only",
    )
    return AdaptedHostProfile(
        framework_profile=canonical.framework_profile,
        adapter=alias_spec,
        host_payload=payload,
    )


def compile_cli_common_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    adapted = adapt_framework_profile(profile, CLI_COMMON_ADAPTER, host_overrides=host_overrides)
    payload = dict(adapted.host_payload)
    payload["shared_contract"] = _build_codex_shared_contract(profile)
    payload["bridge_contract"] = extract_framework_workspace_bridges(
        payload["shared_contract"]["workspace_bootstrap"]
    )
    payload["controller_boundary"] = _build_cli_controller_boundary()
    payload["parity_contract"] = _build_cli_parity_contract()
    payload["source_contract"] = _build_adapter_source_contract(
        canonical_adapter_id=CLI_COMMON_ADAPTER_ID,
        shared_contract_field="shared_contract",
    )
    return AdaptedHostProfile(
        framework_profile=adapted.framework_profile,
        adapter=adapted.adapter,
        host_payload=payload,
    )


def compile_codex_common_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_shared_adapter_alias(profile, CODEX_COMMON_ADAPTER, host_overrides=host_overrides)


def compile_codex_desktop_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    adapted = adapt_framework_profile(profile, CODEX_DESKTOP_ADAPTER, host_overrides=host_overrides)
    common_contract = compile_cli_common_adapter(profile, host_overrides=host_overrides).host_payload
    payload = dict(adapted.host_payload)
    payload["common_contract"] = _clone_json_like(common_contract["shared_contract"])
    payload["controller_boundary"] = _clone_json_like(common_contract["controller_boundary"])
    payload["runtime_surface"] = _build_cli_runtime_surface(common_contract)
    payload["bridge_contract"] = extract_framework_workspace_bridges(
        payload["common_contract"]["workspace_bootstrap"]
    )
    payload["source_contract"] = _build_adapter_source_contract(
        canonical_adapter_id=CODEX_DESKTOP_ADAPTER_ID,
        shared_contract_field="common_contract",
        runtime_surface_field="runtime_surface",
        entrypoint_surface_field="entrypoint_contract",
    )
    payload["entrypoint_contract"] = {
        "entrypoint_kind": "interactive",
        "thread_binding": "desktop-thread",
        "automation_bridge_required": True,
        "framework_truth": "framework_core",
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
    }
    payload["fallback_semantics"] = {
        "requires_aionrs": False,
        "preserves_core_capabilities": list(CORE_CAPABILITIES),
        "degrade_to": "generic_host_adapter",
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "cli_peer": CODEX_CLI_ADAPTER.adapter_id,
    }
    return AdaptedHostProfile(
        framework_profile=adapted.framework_profile,
        adapter=adapted.adapter,
        host_payload=payload,
    )


def compile_codex_cli_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_cli_entrypoint_payload(profile, CODEX_CLI_ADAPTER, host_overrides=host_overrides)


def compile_claude_code_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_cli_entrypoint_payload(
        profile,
        CLAUDE_CODE_ADAPTER,
        host_overrides=host_overrides,
    )


def compile_claude_code_cli_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return compile_claude_code_adapter(profile, host_overrides=host_overrides)


def compile_gemini_cli_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_cli_entrypoint_payload(
        profile,
        GEMINI_CLI_ADAPTER,
        host_overrides=host_overrides,
    )


def build_cli_family_parity_snapshot(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    from codex_agno_runtime.cli_family_contracts import (
        build_cli_family_parity_snapshot as impl,
    )

    return impl(profile, host_overrides=host_overrides)


def build_cli_family_capability_discovery(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    from codex_agno_runtime.cli_family_contracts import (
        build_cli_family_capability_discovery as impl,
    )

    return impl(profile, host_overrides=host_overrides)


def build_codex_dual_entry_parity_snapshot(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    from codex_agno_runtime.cli_family_contracts import (
        build_codex_dual_entry_parity_snapshot as impl,
    )

    return impl(profile, host_overrides=host_overrides)


def build_codex_desktop_alias_retirement_status(
    *,
    alias_inventory_summary: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    from codex_agno_runtime.host_adapter_compatibility import (
        build_codex_desktop_alias_retirement_status as impl,
    )

    return impl(alias_inventory_summary=alias_inventory_summary)


def build_execution_controller_contract() -> Dict[str, Any]:
    from codex_agno_runtime.control_plane_contracts import (
        build_execution_controller_contract as impl,
    )

    return impl()


def build_delegation_contract() -> Dict[str, Any]:
    from codex_agno_runtime.control_plane_contracts import build_delegation_contract as impl

    return impl()


def build_supervisor_state_contract() -> Dict[str, Any]:
    from codex_agno_runtime.control_plane_contracts import (
        build_supervisor_state_contract as impl,
    )

    return impl()


def build_execution_kernel_live_fallback_retirement_status() -> Dict[str, Any]:
    from codex_agno_runtime.control_plane_contracts import (
        build_execution_kernel_live_fallback_retirement_status as impl,
    )

    return impl()


def build_execution_kernel_live_response_serialization_contract() -> Dict[str, Any]:
    from codex_agno_runtime.control_plane_contracts import (
        build_execution_kernel_live_response_serialization_contract as impl,
    )

    return impl()


def build_control_plane_contract_descriptors() -> Dict[str, Any]:
    from codex_agno_runtime.control_plane_contracts import (
        build_control_plane_contract_descriptors as impl,
    )

    return impl()


def should_emit_codex_desktop_alias_artifact(
    alias_inventory_summary: Mapping[str, Any] | None,
) -> bool:
    if alias_inventory_summary is None:
        return True
    if not bool(alias_inventory_summary.get("inventory_complete", False)):
        return True
    if alias_inventory_summary.get("primary_identity_risk_occurrences") != 0:
        return True
    if alias_inventory_summary.get("translation_shim_required") is not False:
        return True
    return False


def compatibility_snapshot(*, include_legacy_aliases: bool = False) -> Dict[str, Dict[str, Any]]:
    from codex_agno_runtime.host_adapter_compatibility import compatibility_snapshot as impl

    return impl(include_legacy_aliases=include_legacy_aliases)


def validate_adapter_compatibility(
    profile: FrameworkProfile,
    adapters: Sequence[HostAdapterSpec | str],
    *,
    include_legacy_aliases: bool = False,
) -> Dict[str, bool]:
    from codex_agno_runtime.host_adapter_compatibility import (
        validate_adapter_compatibility as impl,
    )

    return impl(
        profile,
        adapters,
        include_legacy_aliases=include_legacy_aliases,
    )


def build_upgrade_compatibility_matrix(
    profile: FrameworkProfile | None = None,
    *,
    include_legacy_aliases: bool = False,
) -> Dict[str, Dict[str, Any]]:
    from codex_agno_runtime.host_adapter_compatibility import (
        build_upgrade_compatibility_matrix as impl,
    )

    return impl(profile, include_legacy_aliases=include_legacy_aliases)
