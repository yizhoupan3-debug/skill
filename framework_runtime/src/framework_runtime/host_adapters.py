from __future__ import annotations

import json
from dataclasses import dataclass, field
from functools import lru_cache
from pathlib import Path
from tempfile import NamedTemporaryFile
from typing import Any, Dict, Mapping, Sequence

from .framework_profile import (
    CORE_CAPABILITIES,
    FrameworkProfile,
    build_framework_session_contract,
    ensure_capabilities,
    resolve_host_capability_requirements,
)
from .runtime_registry import framework_native_aliases
from .trace import (
    TRACE_EVENT_STREAM_SCHEMA_VERSION,
    TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
)
UPSTREAM_SAFE_ZONE = "upstream-safe-zone"
THIN_PATCH_ZONE = "thin-patch-zone"
FORK_DANGER_ZONE = "fork-danger-zone"
_HOST_PRIVATE_OVERRIDE_KEY = "host_private"
HOST_ADAPTER_PAYLOAD_KEY = "host_adapter_payload"
LEGACY_HOST_PROJECTION_KEY = "host_projection"

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
PROJECT_ROOT = Path(__file__).resolve().parents[3]


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


def _profile_cache_key(profile: FrameworkProfile) -> str:
    return json.dumps(profile.to_dict(), ensure_ascii=False, sort_keys=True, separators=(",", ":"))


@lru_cache(maxsize=64)
def _compile_rust_codex_artifacts_cached(profile_payload: str) -> Dict[str, Any]:
    from framework_runtime.rust_router import route_adapter

    with NamedTemporaryFile("w", encoding="utf-8", suffix=".json", delete=False) as handle:
        handle.write(profile_payload)
        handle.flush()
        profile_path = Path(handle.name)
    try:
        return route_adapter(codex_home=PROJECT_ROOT).compile_codex_profile_artifacts(profile_path)
    finally:
        profile_path.unlink(missing_ok=True)


def _compile_rust_codex_artifact(
    profile: FrameworkProfile,
    artifact_id: str,
) -> Dict[str, Any]:
    artifacts = _compile_rust_codex_artifacts_cached(_profile_cache_key(profile))
    payload = artifacts.get(artifact_id)
    if not isinstance(payload, Mapping):
        raise RuntimeError(f"router-rs codex artifact payload missing: {artifact_id}")
    return dict(_clone_json_like(payload))


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
    if LEGACY_HOST_PROJECTION_KEY in host_private:
        raise ValueError(
            f"{LEGACY_HOST_PROJECTION_KEY} is a legacy read surface; use "
            f"{HOST_ADAPTER_PAYLOAD_KEY} under {_HOST_PRIVATE_OVERRIDE_KEY} instead."
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


def _normalize_host_adapter_payload_aliases(payload: Mapping[str, Any]) -> Dict[str, Any]:
    """Project the canonical host adapter payload to the legacy output key."""

    normalized = dict(_clone_json_like(payload))
    adapter_payload = normalized.get(HOST_ADAPTER_PAYLOAD_KEY)

    if isinstance(adapter_payload, Mapping):
        normalized[LEGACY_HOST_PROJECTION_KEY] = _clone_json_like(adapter_payload)
    return normalized


def _merge_rust_adapter_payload(
    rust_payload: Mapping[str, Any],
    adapted_payload: Mapping[str, Any],
) -> Dict[str, Any]:
    payload = dict(_clone_json_like(rust_payload))
    for key in _CANONICAL_HOST_ADAPTER_PAYLOAD_FIELDS:
        if key == "metadata":
            continue
        if key in adapted_payload:
            payload[key] = _clone_json_like(adapted_payload[key])
    rust_metadata = payload.get("metadata", {})
    adapted_metadata = adapted_payload.get("metadata", {})
    if isinstance(rust_metadata, Mapping) and isinstance(adapted_metadata, Mapping):
        payload["metadata"] = _merge_mapping(rust_metadata, adapted_metadata)
    elif "metadata" in adapted_payload:
        payload["metadata"] = _clone_json_like(adapted_metadata)
    for key, value in adapted_payload.items():
        if key in _CANONICAL_HOST_ADAPTER_PAYLOAD_FIELDS or key in payload:
            continue
        payload[key] = _clone_json_like(value)
    return _normalize_host_adapter_payload_aliases(payload)


def _compile_rust_owned_adapter(
    profile: FrameworkProfile,
    adapter_spec: HostAdapterSpec,
    artifact_id: str,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    adapted = adapt_framework_profile(profile, adapter_spec, host_overrides=host_overrides)
    payload = _merge_rust_adapter_payload(
        _compile_rust_codex_artifact(profile, artifact_id),
        adapted.host_payload,
    )
    return AdaptedHostProfile(
        framework_profile=adapted.framework_profile,
        adapter=adapted.adapter,
        host_payload=payload,
    )


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
        "transport_contract_kind": "runtime_event_stream",
        "transport_family": "host-facing-transport",
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
        "cleanup_semantics": "stream_cache_only",
        "cleanup_preserves_replay": True,
        "replay_reseed_supported": True,
        "chunk_schema_version": TRACE_EVENT_STREAM_SCHEMA_VERSION,
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
    payload = _normalize_host_adapter_payload_aliases(payload)
    return AdaptedHostProfile(
        framework_profile=profile,
        adapter=adapter_spec,
        host_payload=payload,
    )


def compile_cli_common_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_rust_owned_adapter(
        profile,
        CLI_COMMON_ADAPTER,
        CLI_COMMON_ADAPTER_ID,
        host_overrides=host_overrides,
    )


def compile_codex_common_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_rust_owned_adapter(
        profile,
        CODEX_COMMON_ADAPTER,
        CODEX_COMMON_ADAPTER_ID,
        host_overrides=host_overrides,
    )


def compile_codex_desktop_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_rust_owned_adapter(
        profile,
        CODEX_DESKTOP_ADAPTER,
        CODEX_DESKTOP_ADAPTER_ID,
        host_overrides=host_overrides,
    )


def compile_codex_cli_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_rust_owned_adapter(
        profile,
        CODEX_CLI_ADAPTER,
        CODEX_CLI_ADAPTER_ID,
        host_overrides=host_overrides,
    )


def compile_claude_code_adapter(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    return _compile_rust_owned_adapter(
        profile,
        CLAUDE_CODE_ADAPTER,
        CLAUDE_CODE_ADAPTER_ID,
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
    return _compile_rust_owned_adapter(
        profile,
        GEMINI_CLI_ADAPTER,
        GEMINI_CLI_ADAPTER_ID,
        host_overrides=host_overrides,
    )


def should_emit_codex_desktop_alias_artifact(
    alias_inventory_summary: Mapping[str, Any] | None,
) -> bool:
    if alias_inventory_summary is None:
        return True
    if not bool(alias_inventory_summary.get("inventory_complete", False)):
        return True
    if alias_inventory_summary.get("primary_identity_risk_occurrences") != 0:
        return True
    if alias_inventory_summary.get("legacy_alias_shim_required") is not False:
        return True
    return False
