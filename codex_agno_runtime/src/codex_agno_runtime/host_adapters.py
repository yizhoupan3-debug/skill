from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, Iterable, Mapping, Sequence

from .framework_profile import (
    CORE_CAPABILITIES,
    FrameworkProfile,
    build_framework_workspace_bootstrap,
    ensure_capabilities,
    resolve_host_capability_requirements,
)
from .execution_kernel_contracts import (
    EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    build_execution_kernel_live_response_serialization_contract_core,
)
from .trace import (
    TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
    TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
)

UPSTREAM_SAFE_ZONE = "upstream-safe-zone"
THIN_PATCH_ZONE = "thin-patch-zone"
FORK_DANGER_ZONE = "fork-danger-zone"

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


def _normalize_memory_mounts(memory_mounts: Sequence[Any]) -> list[Dict[str, Any]]:
    normalized: list[Dict[str, Any]] = []
    for mount in memory_mounts:
        if isinstance(mount, Mapping):
            payload = dict(_clone_json_like(mount))
            payload.setdefault("mount_id", payload.get("id", "unnamed-memory-mount"))
            normalized.append(payload)
            continue
        normalized.append(
            {
                "mount_id": str(mount),
                "source": str(mount),
                "bridge_kind": "framework-memory-mount",
            }
        )
    return normalized


def _normalize_mcp_servers(mcp_servers: Sequence[Any]) -> list[Dict[str, Any]]:
    normalized: list[Dict[str, Any]] = []
    for server in mcp_servers:
        if isinstance(server, Mapping):
            payload = dict(_clone_json_like(server))
            payload.setdefault("server_id", payload.get("id", "unnamed-mcp-server"))
            normalized.append(payload)
            continue
        normalized.append({"server_id": str(server)})
    return normalized


def _compile_session_mode(profile: FrameworkProfile) -> Dict[str, Any]:
    session_policy = dict(_clone_json_like(profile.session_policy))
    return {
        "mode": session_policy.get("mode", "default"),
        "approval_mode": session_policy.get("approval_mode", "inherit"),
        "history_policy": session_policy.get("history_policy", "host-managed"),
        "takeover": bool(session_policy.get("takeover", False)),
        "extras": {
            key: value
            for key, value in session_policy.items()
            if key not in {"mode", "approval_mode", "history_policy", "takeover"}
        },
    }


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


def _compile_workspace_bootstrap(profile: FrameworkProfile) -> Dict[str, Any]:
    return build_framework_workspace_bootstrap(
        profile.workspace_bootstrap,
        profile.memory_mounts,
    )


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
        "tool_policy": dict(_clone_json_like(profile.tool_policy)),
        "approval_policy": dict(_clone_json_like(profile.approval_policy)),
        "loadout_policy": dict(_clone_json_like(profile.loadout_policy)),
        "framework_surface_policy": dict(_clone_json_like(profile.framework_surface_policy)),
        "artifact_contract": dict(_clone_json_like(profile.artifact_contract)),
        "model_policy": dict(_clone_json_like(profile.model_policy)),
        "memory_mounts": _normalize_memory_mounts(profile.memory_mounts),
        "mcp_servers": _normalize_mcp_servers(profile.mcp_servers),
        "workspace_bootstrap": _compile_workspace_bootstrap(profile),
        "host_capability_requirements": resolve_host_capability_requirements(
            profile,
            host_id=adapter_spec.host_id,
            adapter_id=adapter_spec.adapter_id,
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
        payload = _merge_mapping(payload, host_overrides)
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
    adapted = adapt_framework_profile(profile, AIONRS_COMPANION_ADAPTER, host_overrides=host_overrides)
    payload = dict(adapted.host_payload)
    payload["companion_contract"] = {
        "presetRules": _normalize_bundle_items(
            profile.rules_bundle,
            list_keys=("rules", "items"),
            fallback_field="rule",
        ),
        "enabledSkills": _normalize_bundle_items(
            profile.skill_bundle,
            list_keys=("skills", "items"),
            fallback_field="skill_id",
        ),
        "sessionMode": _compile_session_mode(profile),
        "aionrsConfig": _compile_aionrs_config(profile),
        "mcpConfig": {"servers": _normalize_mcp_servers(profile.mcp_servers)},
        "workspaceBootstrap": payload["workspace_bootstrap"],
        "bridges": dict(payload["workspace_bootstrap"].get("bridges", {})),
        "toolApprovalMapping": _compile_tool_approval_mapping(profile),
        "eventTranslation": _default_event_translation(),
        "fallbackSemantics": {
            "requires_aionrs": True,
            "portable_core_preserved": list(CORE_CAPABILITIES),
            "fallback_adapter": CODEX_DESKTOP_ADAPTER_ID,
            "legacy_fallback_aliases": [LEGACY_CODEX_DESKTOP_ADAPTER_ID],
            "default_host_peer_set": list(DEFAULT_HOST_PEER_SET),
        },
    }
    payload["metadata"]["legacy_surface"] = True
    payload["legacy_boundary"] = {
        "adapter_lifecycle": "legacy-compatibility",
        "exposure_lane": "fallback-only-explicit",
        "default_host_peer_set": list(DEFAULT_HOST_PEER_SET),
        "default_host_peer_set_member": False,
        "may_become_framework_truth": False,
        "may_become_default_host_peer": False,
        "removal_readiness": "blocked-on-upstream-consumer-retirement",
        "migration_guardrails": [
            "do_not_promote_aionrs_back_to_primary_host_path",
            "keep_fallback_contract_mirror_only",
            "preserve_framework_truth_in_shared_contract",
        ],
    }
    return AdaptedHostProfile(
        framework_profile=adapted.framework_profile,
        adapter=adapted.adapter,
        host_payload=payload,
    )


def compile_aionui_host_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    adapted = adapt_framework_profile(profile, AIONUI_HOST_ADAPTER, host_overrides=host_overrides)
    companion_contract = compile_aionrs_companion_adapter(profile).host_payload["companion_contract"]
    payload = dict(adapted.host_payload)
    payload["host_session_create"] = {
        "presetRules": _clone_json_like(companion_contract["presetRules"]),
        "enabledSkills": _clone_json_like(companion_contract["enabledSkills"]),
        "sessionMode": _clone_json_like(companion_contract["sessionMode"]),
        "aionrsConfig": _clone_json_like(companion_contract["aionrsConfig"]),
    }
    payload["host_runtime_contract"] = {
        "preferred_backend": "aionrs_companion_adapter",
        "artifact_contract": _clone_json_like(payload["artifact_contract"]),
        "memory_mounts": _clone_json_like(payload["memory_mounts"]),
        "workspace_bootstrap": _clone_json_like(payload["workspace_bootstrap"]),
        "approval_bridge": _clone_json_like(companion_contract["toolApprovalMapping"]),
        "event_bridge": _clone_json_like(companion_contract["eventTranslation"]),
        "event_transport": _default_event_transport(),
        "event_stream_binding": _default_event_stream_binding(),
        "fallback_semantics": {
            "degrade_to": "generic_host_adapter",
            "deep_adaptation_not_fork": True,
            "default_host_peer_set": list(DEFAULT_HOST_PEER_SET),
        },
    }
    payload["metadata"]["legacy_surface"] = True
    payload["legacy_boundary"] = {
        "adapter_lifecycle": "legacy-compatibility",
        "exposure_lane": "fallback-only-explicit",
        "default_host_peer_set": list(DEFAULT_HOST_PEER_SET),
        "default_host_peer_set_member": False,
        "may_become_framework_truth": False,
        "may_become_default_host_peer": False,
        "removal_readiness": "blocked-on-aionui-shell-consumer-retirement",
        "migration_guardrails": [
            "do_not_promote_aionui_back_to_primary_host_path",
            "keep_aionui_as_outer_contract_shell_only",
            "preserve_aionrs_companion_as_preferred_backend_when_enabled",
        ],
    }
    return AdaptedHostProfile(
        framework_profile=adapted.framework_profile,
        adapter=adapted.adapter,
        host_payload=payload,
    )


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
    payload["bridge_contract"] = dict(payload["shared_contract"]["workspace_bootstrap"].get("bridges", {}))
    payload["controller_boundary"] = _build_cli_controller_boundary()
    payload["parity_contract"] = _build_cli_parity_contract()
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


def compile_codex_desktop_host_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    canonical = compile_codex_desktop_adapter(profile, host_overrides=host_overrides)
    payload = _clone_json_like(canonical.host_payload)
    payload["metadata"]["adapter_id"] = CODEX_DESKTOP_HOST_ADAPTER.adapter_id
    payload["metadata"]["adapter_alias_of"] = CODEX_DESKTOP_ADAPTER.adapter_id
    payload["metadata"]["canonical_adapter_id"] = CODEX_DESKTOP_ADAPTER.adapter_id
    payload["metadata"]["legacy_surface"] = True
    payload["entrypoint_contract"]["canonical_adapter_id"] = CODEX_DESKTOP_ADAPTER.adapter_id
    payload["entrypoint_contract"]["legacy_adapter_id"] = CODEX_DESKTOP_HOST_ADAPTER.adapter_id
    payload["fallback_semantics"]["legacy_adapter_id"] = CODEX_DESKTOP_HOST_ADAPTER.adapter_id
    payload["legacy_boundary"] = {
        "adapter_lifecycle": "legacy-compatibility",
        "exposure_lane": "compatibility-only-explicit",
        "default_host_peer_set": list(DEFAULT_HOST_PEER_SET),
        "default_host_peer_set_member": False,
        "may_become_framework_truth": False,
        "may_become_default_host_peer": False,
        "removal_readiness": "blocked-on-continuity-consumer-retirement",
        "migration_guardrails": [
            "keep_alias_mirror_only_to_codex_desktop_adapter",
            "do_not_add_new_host_semantics_to_legacy_alias",
            "require_explicit_compatibility_lane_opt_in",
        ],
    }
    return AdaptedHostProfile(
        framework_profile=canonical.framework_profile,
        adapter=CODEX_DESKTOP_HOST_ADAPTER,
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
    common = compile_cli_common_adapter(profile, host_overrides=host_overrides).host_payload
    adapters = {
        CODEX_CLI_ADAPTER_ID: compile_codex_cli_adapter(profile, host_overrides=host_overrides).host_payload,
        CLAUDE_CODE_ADAPTER_ID: compile_claude_code_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
        GEMINI_CLI_ADAPTER_ID: compile_gemini_cli_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
    }
    anchor = adapters[CODEX_CLI_ADAPTER_ID]["runtime_surface"]
    parity_checks = {
        field: all(payload["runtime_surface"][field] == anchor[field] for payload in adapters.values())
        for field in COMMON_PARITY_FIELDS
    }
    return {
        "framework_truth": "framework_core",
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "shared_contract_fields": list(COMMON_PARITY_FIELDS),
        "parity_checks": parity_checks,
        "all_shared_contract_checks_pass": all(parity_checks.values()),
        "cli_hosts": {
            adapter_id: {
                "adapter_id": payload["metadata"]["adapter_id"],
                "host_id": payload["metadata"]["host_id"],
                "entrypoint_kind": payload["execution_surface"]["entrypoint_kind"],
                "shared_adapter": payload["execution_surface"]["shared_adapter"],
                "context_files": _clone_json_like(payload["host_projection"]["context_files"]),
                "config_root_env_var": payload["host_projection"]["config_root_env_var"],
                "settings_paths": _clone_json_like(payload["host_projection"]["settings_paths"]),
                "mcp_config_paths": _clone_json_like(payload["host_projection"]["mcp_config_paths"]),
                "settings_scope_order": _clone_json_like(
                    payload["host_projection"]["settings_scope_order"]
                ),
                "subagent_paths": _clone_json_like(payload["host_projection"]["subagent_paths"]),
                "hook_event_names": _clone_json_like(payload["host_projection"]["hook_event_names"]),
                "hook_control_settings": _clone_json_like(
                    payload["host_projection"]["hook_control_settings"]
                ),
                "hook_inspection_commands": _clone_json_like(
                    payload["host_projection"]["hook_inspection_commands"]
                ),
                "plugin_hook_manifest_paths": _clone_json_like(
                    payload["host_projection"]["plugin_hook_manifest_paths"]
                ),
                "hook_environment_markers": _clone_json_like(
                    payload["host_projection"]["hook_environment_markers"]
                ),
                "checkpointing_supported": payload["host_projection"]["checkpointing_supported"],
            }
            for adapter_id, payload in adapters.items()
        },
        "controller_boundary": _clone_json_like(common["controller_boundary"]),
    }


def _build_cli_family_capability_discovery_entry(
    *,
    profile: FrameworkProfile,
    adapter_spec: HostAdapterSpec,
    payload: Mapping[str, Any],
) -> Dict[str, Any]:
    requirements = resolve_host_capability_requirements(
        profile,
        host_id=adapter_spec.host_id,
        adapter_id=adapter_spec.adapter_id,
    )
    required_host_capabilities = list(requirements.get("required_host_capabilities", []))
    available_host_capabilities = list(adapter_spec.host_capabilities)
    available_set = set(available_host_capabilities)
    missing_host_capabilities = [
        capability for capability in required_host_capabilities if capability not in available_set
    ]
    host_projection = payload["host_projection"]
    execution_surface = payload["execution_surface"]
    return {
        "adapter_id": adapter_spec.adapter_id,
        "host_id": adapter_spec.host_id,
        "transport": adapter_spec.transport,
        "entrypoint_kind": execution_surface["entrypoint_kind"],
        "shared_adapter": execution_surface["shared_adapter"],
        "framework_truth": execution_surface["framework_truth"],
        "works_without_aionrs": bool(adapter_spec.protocol_hints.get("works_without_aionrs", False)),
        "available_host_capabilities": available_host_capabilities,
        "resolved_host_requirements": _clone_json_like(requirements),
        "required_host_capabilities": required_host_capabilities,
        "missing_host_capabilities": missing_host_capabilities,
        "supports_batch": execution_surface["supports_batch"],
        "supports_cron": execution_surface["supports_cron"],
        "supports_ci": execution_surface["supports_ci"],
        "context_files": _clone_json_like(host_projection["context_files"]),
        "settings_paths": _clone_json_like(host_projection["settings_paths"]),
        "mcp_config_paths": _clone_json_like(host_projection["mcp_config_paths"]),
        "config_root_env_var": host_projection["config_root_env_var"],
        "settings_scope_order": _clone_json_like(host_projection["settings_scope_order"]),
        "subagent_paths": _clone_json_like(host_projection["subagent_paths"]),
        "hook_event_names": _clone_json_like(host_projection["hook_event_names"]),
        "hook_control_settings": _clone_json_like(host_projection["hook_control_settings"]),
        "hook_inspection_commands": _clone_json_like(host_projection["hook_inspection_commands"]),
        "hook_environment_markers": _clone_json_like(host_projection["hook_environment_markers"]),
        "checkpointing_supported": host_projection["checkpointing_supported"],
        "compatibility_passes": len(missing_host_capabilities) == 0,
    }


def build_cli_family_capability_discovery(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    common = compile_cli_common_adapter(profile, host_overrides=host_overrides).host_payload
    adapters = {
        CODEX_CLI_ADAPTER_ID: (
            CODEX_CLI_ADAPTER,
            compile_codex_cli_adapter(profile, host_overrides=host_overrides).host_payload,
        ),
        CLAUDE_CODE_ADAPTER_ID: (
            CLAUDE_CODE_ADAPTER,
            compile_claude_code_adapter(profile, host_overrides=host_overrides).host_payload,
        ),
        GEMINI_CLI_ADAPTER_ID: (
            GEMINI_CLI_ADAPTER,
            compile_gemini_cli_adapter(profile, host_overrides=host_overrides).host_payload,
        ),
    }
    discovery_entries = {
        adapter_id: _build_cli_family_capability_discovery_entry(
            profile=profile,
            adapter_spec=adapter_spec,
            payload=payload,
        )
        for adapter_id, (adapter_spec, payload) in adapters.items()
    }
    return {
        "framework_truth": "framework_core",
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "discovery_contract": "cli_family_host_capability_contract_v1",
        "required_core_capabilities": list(CORE_CAPABILITIES),
        "required_shared_contract_fields": list(COMMON_PARITY_FIELDS),
        "cli_hosts": discovery_entries,
        "all_cli_hosts_compatible": all(
            entry["compatibility_passes"] for entry in discovery_entries.values()
        ),
        "controller_boundary": _clone_json_like(common["controller_boundary"]),
    }


def build_codex_dual_entry_parity_snapshot(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    common = compile_cli_common_adapter(profile, host_overrides=host_overrides).host_payload
    desktop = compile_codex_desktop_adapter(profile, host_overrides=host_overrides).host_payload
    cli = compile_codex_cli_adapter(profile, host_overrides=host_overrides).host_payload

    parity_checks = {
        field: desktop["runtime_surface"][field] == cli["runtime_surface"][field]
        for field in COMMON_PARITY_FIELDS
    }
    return {
        "framework_truth": "framework_core",
        "shared_adapter": CLI_COMMON_ADAPTER_ID,
        "shared_adapter_aliases": [CODEX_COMMON_ADAPTER_ID],
        "codexcli_is_framework_controller": False,
        "compatibility_view_of": CLI_FAMILY_PARITY_ARTIFACT_ID,
        "shared_contract_fields": list(COMMON_PARITY_FIELDS),
        "parity_checks": parity_checks,
        "all_shared_contract_checks_pass": all(parity_checks.values()),
        "desktop": {
            "adapter_id": desktop["metadata"]["adapter_id"],
            "entrypoint_kind": desktop["entrypoint_contract"]["entrypoint_kind"],
            "shared_adapter": desktop["entrypoint_contract"]["shared_adapter"],
            "legacy_aliases": [LEGACY_CODEX_DESKTOP_ADAPTER_ID],
        },
        "cli": {
            "adapter_id": cli["metadata"]["adapter_id"],
            "entrypoint_kind": cli["execution_surface"]["entrypoint_kind"],
            "shared_adapter": cli["execution_surface"]["shared_adapter"],
        },
        "controller_boundary": _clone_json_like(common["controller_boundary"]),
    }


def build_codex_desktop_alias_retirement_status(
    *,
    alias_inventory_summary: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    inventory_summary = (
        _clone_json_like(alias_inventory_summary)
        if alias_inventory_summary is not None
        else {
            "inventory_complete": False,
            "primary_identity_risk_occurrences": None,
            "translation_shim_required": None,
        }
    )
    inventory_complete = bool(inventory_summary.get("inventory_complete", False))
    primary_identity_risk_occurrences = inventory_summary.get("primary_identity_risk_occurrences")
    translation_shim_required = inventory_summary.get("translation_shim_required")
    runtime_primary_identity_consumers_cleared = (
        primary_identity_risk_occurrences == 0 if inventory_complete else None
    )

    return {
        "canonical_adapter_id": CODEX_DESKTOP_ADAPTER_ID,
        "legacy_alias_id": LEGACY_CODEX_DESKTOP_ADAPTER_ID,
        "alias_lifecycle": "compatibility-only",
        "alias_mode": "mirror-only",
        "framework_truth": "framework_core",
        "primary_regression_artifact": PARITY_BASELINE_ARTIFACT_ID,
        "codex_dual_entry_compatibility_artifact": "codex_dual_entry_parity_snapshot",
        "secondary_inventory_artifact": COMPATIBILITY_INVENTORY_ARTIFACT_ID,
        "emitter_contract": {
            "python_emits_alias_artifact": False,
            "rust_emits_alias_artifact": False,
            "drop_requires_joint_emitter_flip": True,
            "legacy_alias_artifact_opt_in": True,
            "alias_may_not_gain_new_host_semantics": True,
        },
        "retirement_gates": {
            "canonical_desktop_identity_locked": True,
            "parity_snapshot_is_primary_baseline": True,
            "compatibility_matrix_is_secondary_inventory": True,
            "runtime_primary_identity_consumers_cleared": runtime_primary_identity_consumers_cleared,
            "translation_shim_required": translation_shim_required,
            "translation_shim_ready_if_needed": False if translation_shim_required else True,
        },
        "inventory_summary": inventory_summary,
    }


def build_execution_controller_contract() -> Dict[str, Any]:
    return {
        "framework_truth": "framework_core",
        "contract_artifact": EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
        "status_contract": "execution_controller_contract_v1",
        "artifact_role": "shared-contract-evidence",
        "controller": {
            "primary_owner": "execution-controller-coding",
            "role": "kernel-level-execution-controller",
            "framework_phase": "runtime-orchestration",
            "state_artifact": ".supervisor_state.json",
            "user_facing_aliases": ["gsd", "get shit done"],
        },
        "gsd_execution_posture": {
            "label": "get-shit-done",
            "auto_continue_safe_local_work": True,
            "main_thread_stays_decision_heavy": True,
            "verify_before_done": True,
            "runtime_dependency": "none",
        },
        "boundaries": {
            "host_adapters_remain_thin_projections": True,
            "runtime_branching_changes_required": False,
            "business_code_mutation_required": False,
            "single_framework_truth": True,
        },
        "continuity_artifacts": [
            "SESSION_SUMMARY.md",
            "NEXT_ACTIONS.json",
            "EVIDENCE_INDEX.json",
            "TRACE_METADATA.json",
            ".supervisor_state.json",
        ],
        "required_execution_contract_fields": [
            "goal",
            "scope",
            "forbidden_scope",
            "acceptance_criteria",
            "evidence_required",
        ],
        "phase_model": {
            "state_owner": "supervisor_state_contract",
            "phase_field": "active_phase",
            "verification_field": "verification.verification_status",
            "resumable": True,
        },
        "retained_local_authority": {
            "orchestration_decisions": True,
            "final_integration_judgment": True,
            "rollback_decision": True,
        },
    }


def build_delegation_contract() -> Dict[str, Any]:
    return {
        "framework_truth": "framework_core",
        "contract_artifact": DELEGATION_CONTRACT_ARTIFACT_ID,
        "status_contract": "delegation_contract_v1",
        "artifact_role": "shared-contract-evidence",
        "gate": {
            "gate_skill": "subagent-delegation",
            "gate_type": "delegation",
            "decision_before_spawn": True,
            "spawn_is_optional": True,
        },
        "local_supervisor_mode": {
            "preserves_sidecar_boundaries": True,
            "preserves_output_contracts": True,
            "allowed_when_runtime_blocks_spawning": True,
        },
        "delegation_state_fields": [
            "delegation_plan_created",
            "spawn_attempted",
            "spawn_block_reason",
            "fallback_mode",
            "delegated_sidecars",
        ],
        "sidecar_contract": {
            "bounded_parallelism_only": True,
            "main_thread_stays_decision_heavy": True,
            "integration_remains_local": True,
            "worker_traces_sink_to_artifacts": True,
        },
        "non_goals": [
            "runtime_spawn_policy_rewrite",
            "host-specific delegation_branching",
            "overlapping_write_scopes_between_workers",
        ],
    }


def build_supervisor_state_contract() -> Dict[str, Any]:
    return {
        "framework_truth": "framework_core",
        "contract_artifact": SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID,
        "status_contract": "supervisor_state_contract_v2",
        "artifact_role": "shared-contract-evidence",
        "state_artifact_path": ".supervisor_state.json",
        "schema_expectations": {
            "top_level_fields": [
                "schema_version",
                "task_id",
                "task_summary",
                "controller",
                "primary_owner",
                "active_phase",
                "execution_contract",
                "delegation",
                "workers",
                "progress",
                "verification",
                "open_blockers",
                "next_actions",
            ],
            "execution_contract_fields": [
                "goal",
                "scope",
                "forbidden_scope",
                "acceptance_criteria",
                "evidence_required",
            ],
            "delegation_fields": [
                "delegation_plan_created",
                "spawn_attempted",
                "spawn_block_reason",
                "fallback_mode",
                "delegated_sidecars",
            ],
            "workers_fields": [
                "running",
                "completed_unintegrated",
                "integrated",
                "failed",
                "stalled",
            ],
            "verification_fields": [
                "verification_status",
                "last_verification_summary",
            ],
        },
        "cross_artifact_alignment": {
            "continuity_artifacts_must_share_task_story": True,
            "phase_must_be_resumable": True,
            "delegation_structure_must_be_explicit": True,
        },
        "compatibility_rules": {
            "rust_may_validate_or_emit": True,
            "python_may_continue_to_author": True,
            "no_shadow_replacement_artifact": True,
        },
    }


def build_execution_kernel_live_fallback_retirement_status() -> Dict[str, Any]:
    return {
        "framework_truth": "framework_core",
        "status_contract": "execution_kernel_live_fallback_retirement_status_v1",
        "affected_host_projections": [
            CODEX_DESKTOP_ADAPTER_ID,
            CODEX_CLI_ADAPTER_ID,
            CLAUDE_CODE_ADAPTER_ID,
            GEMINI_CLI_ADAPTER_ID,
        ],
        "live_primary": {
            "contract_mode": "rust-live-primary",
            "adapter_kind": "router-rs",
            "authority": "rust-execution-cli",
            "family": "rust-cli",
            "impl": "router-rs",
        },
        "compatibility_fallback": {
            "runtime_path_available": False,
            "retired_mode": "retired",
            "request_behavior": "surface-removed",
            "former_adapter_kind": "python-agno",
            "former_authority": "python-agno-kernel-adapter",
            "former_family": "python",
            "former_impl": "agno",
            "purpose_before_retirement": "compatibility-only-escape-hatch",
        },
        "control_surfaces": {
            "former_settings_field": "rust_execute_fallback_to_python",
            "former_env_var": "CODEX_AGNO_RUST_EXECUTE_FALLBACK_TO_PYTHON",
            "enabled_by_default_before_removal": False,
            "accepted_after_retirement": False,
            "request_behavior": "surface-removed",
            "steady_state_mode": "removed",
            "surface_role": "removed-retired-request-surface",
            "removal_status": "completed",
        },
        "retirement_exit_contract": {
            "surface_status": "removed",
            "current_decision": "completed",
            "removal_owner": "runtime-integrator",
            "remove_when": [],
            "observation_sources": {
                "local_runtime_health": [
                    "ExecutionEnvironmentService.describe_kernel_contract()",
                    "RouterRsExecutionKernel.health().kernel_live_backend_impl",
                ],
                "local_contract_artifacts": [
                    "execution_kernel_live_fallback_retirement_status.control_surfaces",
                    (
                        "execution_kernel_live_fallback_retirement_status."
                        "current_contract_truth.live_fallback_request_behavior"
                    ),
                ],
                "external_confirmation": [
                    (
                        "host or integration owner evidence that no downstream caller "
                        "still probes the retired request surface"
                    )
                ],
            },
            "stop_rule": "request surface already removed from runtime settings and steady-state artifacts",
        },
        "public_runtime_contract_fields": [
            "execution_kernel",
            "execution_kernel_authority",
            "execution_kernel_contract_mode",
            "execution_kernel_in_process_replacement_complete",
            "execution_kernel_delegate",
            "execution_kernel_delegate_authority",
            "execution_kernel_live_primary",
            "execution_kernel_live_primary_authority",
        ],
        "public_runtime_response_metadata_fields": [
            "execution_kernel_delegate_family",
            "execution_kernel_delegate_impl",
        ],
        "retired_runtime_response_metadata_fields": [
            EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
        ],
        "current_contract_truth": {
            "execution_kernel_contract_mode": "rust-live-primary",
            "execution_kernel_in_process_replacement_complete": True,
            "dry_run_delegate_kind": "router-rs",
            "dry_run_delegate_authority": "rust-execution-cli",
            "live_primary_kind": "router-rs",
            "live_primary_authority": "rust-execution-cli",
            "live_fallback_runtime_path_available": False,
            "live_fallback_mode": "retired",
            "live_fallback_request_behavior": "surface-removed",
            "live_fallback_request_surface": "removed",
            "live_prompt_preview_passthrough_disabled": True,
            "compatibility_fallback_reason_metadata_key": EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
        },
        "current_response_metadata_truth": {
            "live_delegate_family": "rust-cli",
            "live_delegate_impl": "router-rs",
            "dry_run_delegate_family": "rust-cli",
            "dry_run_delegate_impl": "router-rs",
            "compatibility_fallback_reason_present_in_steady_state": False,
            "retired_response_metadata_fields": [
                EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
            ],
        },
        "remaining_python_owned_surfaces": [],
        "retirement_readiness": {
            "ready": True,
            "status": "retired",
            "contract_lane_complete": True,
            "runtime_control_flow_change_required": False,
            "blockers": [],
            "next_safe_slice": "rustification_closed",
        },
        "guardrails": {
            "thin_projection_boundary_preserved": True,
            "cli_hosts_may_not_become_framework_truth": True,
            "claude_host_runtime_semantics_remain_host_owned": True,
        },
        "retirement_gates": {
            "public_runtime_contract_externalized": True,
            "live_primary_contract_externalized": True,
            "compatibility_fallback_contract_externalized": True,
            "rust_only_disabled_mode_externalized": True,
            "response_metadata_surface_externalized": True,
            "delegate_family_impl_metadata_externalized": True,
            "dry_run_delegate_still_python_owned": False,
            "compatibility_fallback_runtime_path_removed": True,
            "explicit_compatibility_requests_rejected": True,
            "dry_run_prompt_preview_still_python_owned": False,
            "compatibility_fallback_agent_factory_still_python_owned": False,
            "compatibility_live_response_serialization_still_python_owned": False,
            "compatibility_fallback_reason_metadata_still_python_owned": False,
            "default_runtime_python_fallback_retired": True,
            "in_process_replacement_complete": True,
        },
    }


def build_execution_kernel_live_response_serialization_contract() -> Dict[str, Any]:
    return {
        "framework_truth": "framework_core",
        "scope": "compatibility_live_response_serialization",
        "artifact_role": "shared-contract-evidence",
        "affected_host_projections": [
            CODEX_DESKTOP_ADAPTER_ID,
            CODEX_CLI_ADAPTER_ID,
            CLAUDE_CODE_ADAPTER_ID,
            GEMINI_CLI_ADAPTER_ID,
        ],
        **build_execution_kernel_live_response_serialization_contract_core(),
        "guardrails": {
            "thin_projection_boundary_preserved": True,
            "cli_hosts_may_not_become_framework_truth": True,
            "claude_host_runtime_semantics_remain_host_owned": True,
        },
    }


def build_control_plane_contract_descriptors() -> Dict[str, Any]:
    """Return the shared control-plane descriptor set used by runtime and artifacts."""

    return {
        EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID: build_execution_controller_contract(),
        DELEGATION_CONTRACT_ARTIFACT_ID: build_delegation_contract(),
        SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID: build_supervisor_state_contract(),
        EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID: (
            build_execution_kernel_live_fallback_retirement_status()
        ),
        EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID: (
            build_execution_kernel_live_response_serialization_contract()
        ),
    }


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


def _build_compatibility_snapshot_entry(spec: HostAdapterSpec) -> Dict[str, Any]:
    return {
        "adapter_id": spec.adapter_id,
        "host_id": spec.host_id,
        "transport": spec.transport,
        "required_capabilities": list(spec.required_capabilities),
        "optional_capabilities": list(spec.optional_capabilities),
        "host_capabilities": list(spec.host_capabilities),
        "works_without_aionrs": spec.protocol_hints.get("works_without_aionrs", False),
        "upgrade_zone": spec.upgrade_zone,
        "legacy_surface": bool(spec.protocol_hints.get("legacy_surface", False)),
        "default_host_peer_set_member": bool(
            spec.protocol_hints.get("default_host_peer_set_member", True)
        ),
    }


def compatibility_snapshot(*, include_legacy_aliases: bool = False) -> Dict[str, Dict[str, Any]]:
    snapshot: Dict[str, Dict[str, Any]] = {}
    for adapter_id, spec in HOST_ADAPTERS.items():
        snapshot[adapter_id] = _build_compatibility_snapshot_entry(spec)
    if include_legacy_aliases:
        desktop_snapshot = snapshot[CODEX_DESKTOP_ADAPTER_ID]
        desktop_snapshot["compatibility_lane"] = {
            "legacy_aliases": {
                LEGACY_CODEX_DESKTOP_ADAPTER_ID: _build_compatibility_snapshot_entry(
                    CODEX_DESKTOP_HOST_ADAPTER
                )
            },
            "default_host_peer_set": list(DEFAULT_HOST_PEER_SET),
            "explicit_opt_in_required": True,
        }
        snapshot["fallback_lane"] = {
            "legacy_adapters": {
                adapter_id: _build_compatibility_snapshot_entry(spec)
                for adapter_id, spec in COMPATIBILITY_HOST_ADAPTERS.items()
                if adapter_id != LEGACY_CODEX_DESKTOP_ADAPTER_ID
            },
            "default_host_peer_set": list(DEFAULT_HOST_PEER_SET),
            "explicit_opt_in_required": True,
        }
    return snapshot


def validate_adapter_compatibility(
    profile: FrameworkProfile,
    adapters: Iterable[HostAdapterSpec | str],
    *,
    include_legacy_aliases: bool = False,
) -> Dict[str, bool]:
    results: Dict[str, bool] = {}
    for adapter in adapters:
        spec = (
            get_host_adapter(adapter, include_legacy_aliases=include_legacy_aliases)
            if isinstance(adapter, str)
            else adapter
        )
        compatible = True
        try:
            ensure_capabilities(profile, spec.required_capabilities)
        except ValueError:
            compatible = False
        requirements = resolve_host_capability_requirements(
            profile,
            host_id=spec.host_id,
            adapter_id=spec.adapter_id,
        )
        required_host_capabilities = requirements.get("required_host_capabilities", [])
        if required_host_capabilities:
            available = set(spec.host_capabilities)
            missing = [cap for cap in required_host_capabilities if cap not in available]
            if missing:
                compatible = False
        results[spec.adapter_id] = compatible
    return results


def build_upgrade_compatibility_matrix(
    profile: FrameworkProfile | None = None,
    *,
    include_legacy_aliases: bool = False,
) -> Dict[str, Dict[str, Any]]:
    inventory_adapters = list_host_adapters(include_legacy_aliases=include_legacy_aliases)
    compatibility = (
        validate_adapter_compatibility(
            profile,
            inventory_adapters,
            include_legacy_aliases=include_legacy_aliases,
        )
        if profile is not None
        else {}
    )
    matrix: Dict[str, Dict[str, Any]] = {}
    for spec in inventory_adapters:
        required = set(spec.required_capabilities)
        optional = set(spec.optional_capabilities)
        legacy_surface = bool(spec.protocol_hints.get("legacy_surface", False))
        matrix[spec.adapter_id] = {
            "adapter_id": spec.adapter_id,
            "host_id": spec.host_id,
            "transport": spec.transport,
            "requires_aionrs": spec.adapter_id == "aionrs_companion_adapter",
            "works_without_aionrs": spec.protocol_hints.get("works_without_aionrs", False),
            "core_runtime": "runtime" in required or "runtime" in optional,
            "memory": "memory" in required or "memory" in optional,
            "artifact": "artifact" in required or "artifact" in optional,
            "orchestration": "orchestration" in required or "orchestration" in optional,
            "upstream_safe_zone": [
                "framework_profile_compilation",
                "artifact_contract_projection",
                *list(spec.protocol_hints.keys()),
            ],
            "thin_patch_zone": list(spec.thin_patch_surfaces),
            "fork_danger_zone": list(spec.fork_danger_surfaces),
            "legacy_surface": legacy_surface,
            "exposure_lane": (
                f"{spec.protocol_hints.get('legacy_lane', 'compatibility')}-only-explicit"
                if legacy_surface
                else "default-peer-set"
            ),
            "default_host_peer_set_member": spec.adapter_id in DEFAULT_HOST_PEER_SET,
            "compatible": compatibility.get(spec.adapter_id),
        }
    return matrix
