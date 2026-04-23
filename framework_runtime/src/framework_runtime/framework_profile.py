from __future__ import annotations

from dataclasses import dataclass, field
from functools import cached_property
from typing import Any, Dict, Iterable, Mapping, MutableMapping, Sequence


FRAMEWORK_PROFILE_VERSION = "0.1.0"
FRAMEWORK_SHARED_CONTRACT_SCHEMA_VERSION = "framework-shared-contract-v1"
FRAMEWORK_SHARED_CONTRACT_FIELDS = (
    "artifact_contract",
    "memory_mounts",
    "mcp_servers",
    "tool_policy",
    "approval_policy",
    "loadout_policy",
    "framework_surface_policy",
    "workspace_bootstrap",
    "session_contract",
)
HOST_SPECIFIC_METADATA_KEYS = frozenset(
    {
        "adapter_id",
        "adapter_alias_of",
        "automation_bridge_required",
        "canonical_adapter_id",
        "checkpointing_supported",
        "claude_directory_features",
        "config_root_env_var",
        "context_files",
        "controller_is_cli",
        "entrypoint_kind",
        "host_cli",
        "host_id",
        "hook_control_settings",
        "hook_definition_sources",
        "hook_environment_markers",
        "hook_event_names",
        "hook_handler_types",
        "hook_inspection_commands",
        "managed_mcp_paths",
        "managed_settings_paths",
        "mcp_config_paths",
        "plugin_hook_manifest_paths",
        "settings_paths",
        "settings_scope_order",
        "settings_scopes",
        "shared_adapter",
        "structured_output_modes",
        "subagent_paths",
        "supports_batch",
        "supports_ci",
        "supports_cron",
        "thread_binding",
        "transport",
    }
)
# Must match scripts/router-rs/src/framework_profile.rs::HOST_SPECIFIC_METADATA_KEYS

CORE_CAPABILITIES = (
    "runtime",
    "memory",
    "artifact",
    "orchestration",
)
FRAMEWORK_NATIVE_ALIAS_SCHEMA_VERSION = "framework-native-aliases-v1"


def _clone_json_like(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {str(key): _clone_json_like(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_clone_json_like(item) for item in value]
    return value


def _merge_nested_mapping(
    base: Mapping[str, Any],
    override: Mapping[str, Any],
) -> Dict[str, Any]:
    merged = _clone_json_like(base)
    for key, value in override.items():
        existing = merged.get(key)
        if isinstance(existing, Mapping) and isinstance(value, Mapping):
            merged[str(key)] = _merge_nested_mapping(existing, value)
            continue
        if isinstance(existing, list) and isinstance(value, (list, tuple)):
            deduped = list(existing)
            for item in value:
                if item not in deduped:
                    deduped.append(_clone_json_like(item))
            merged[str(key)] = deduped
            continue
        merged[str(key)] = _clone_json_like(value)
    return merged


def build_framework_native_aliases(
    aliases: Mapping[str, Mapping[str, Any]] | None = None,
) -> Dict[str, Any]:
    normalized = {
        "schema_version": FRAMEWORK_NATIVE_ALIAS_SCHEMA_VERSION,
        "authority": "framework_profile",
        "aliases": {},
    }
    for alias_name, payload in (aliases or {}).items():
        if not isinstance(payload, Mapping):
            continue
        normalized["aliases"][str(alias_name)] = _clone_json_like(payload)
    return normalized


def normalize_framework_memory_mounts(memory_mounts: Sequence[Any]) -> list[Dict[str, Any]]:
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


def normalize_framework_mcp_servers(mcp_servers: Sequence[Any]) -> list[Dict[str, Any]]:
    normalized: list[Dict[str, Any]] = []
    for server in mcp_servers:
        if isinstance(server, Mapping):
            payload = dict(_clone_json_like(server))
            payload.setdefault("server_id", payload.get("id", "unnamed-mcp-server"))
            normalized.append(payload)
            continue
        normalized.append({"server_id": str(server)})
    return normalized


def build_framework_session_contract(session_policy: Mapping[str, Any]) -> Dict[str, Any]:
    normalized = dict(_clone_json_like(session_policy))
    return {
        "mode": normalized.get("mode", "default"),
        "approval_mode": normalized.get("approval_mode", "inherit"),
        "history_policy": normalized.get("history_policy", "host-managed"),
        "takeover": bool(normalized.get("takeover", False)),
        "extras": {
            key: value
            for key, value in normalized.items()
            if key not in {"mode", "approval_mode", "history_policy", "takeover"}
        },
    }


def build_framework_workspace_bootstrap(
    workspace_bootstrap: Mapping[str, Any],
    memory_mounts: Sequence[Any],
) -> Dict[str, Any]:
    normalized = dict(_clone_json_like(workspace_bootstrap))
    bridges = dict(normalized.get("bridges", {}))
    bridges.setdefault(
        "skills",
        normalized.get(
            "skill_bridge",
            {
                "project_dir": ".codex/skills",
                "user_dir": "~/.codex/skills",
                "bridge_dir": ".aionrs/skills",
            },
        ),
    )
    bridges.setdefault(
        "memory",
        normalized.get(
            "memory_bridge",
            {
                "bridge_dir": ".aionrs-memory-bridge",
                "mounts": normalize_framework_memory_mounts(memory_mounts),
            },
        ),
    )
    compiled = dict(normalized)
    compiled["bridges"] = bridges
    return compiled


def extract_framework_workspace_bridges(workspace_bootstrap: Mapping[str, Any]) -> Dict[str, Any]:
    bridges = workspace_bootstrap.get("bridges", {})
    if not isinstance(bridges, Mapping):
        return {}
    return dict(_clone_json_like(bridges))


@dataclass(frozen=True)
class FrameworkProfile:
    """Host-agnostic framework contract.

    The profile is the stable, outer-framework truth source. Host adapters may
    project this contract into AionUI, Codex Desktop, aionrs companion, or any
    future host, but framework core semantics must not be encoded in any single
    host protocol.
    """

    profile_id: str
    display_name: str
    framework_profile_version: str = FRAMEWORK_PROFILE_VERSION
    runtime_family: str = "portable"
    host_family: str = "generic"
    core_capabilities: tuple[str, ...] = CORE_CAPABILITIES
    optional_capabilities: tuple[str, ...] = ()
    rules_bundle: Any = "default"
    skill_bundle: Any = "default"
    session_policy: Dict[str, Any] = field(default_factory=dict)
    tool_policy: Dict[str, Any] = field(default_factory=dict)
    approval_policy: Dict[str, Any] = field(default_factory=dict)
    loadout_policy: Dict[str, Any] = field(default_factory=dict)
    framework_surface_policy: Dict[str, Any] = field(default_factory=dict)
    artifact_contract: Dict[str, Any] = field(default_factory=dict)
    model_policy: Dict[str, Any] = field(default_factory=dict)
    memory_mounts: tuple[Any, ...] = ()
    mcp_servers: tuple[Any, ...] = ()
    workspace_bootstrap: Dict[str, Any] = field(default_factory=dict)
    host_capability_requirements: Dict[str, Dict[str, Any]] = field(default_factory=dict)
    metadata: Dict[str, Any] = field(default_factory=dict)

    @cached_property
    def _shared_contract_surface(self) -> Dict[str, Any]:
        return {
            "artifact_contract": _clone_json_like(self.artifact_contract),
            "memory_mounts": normalize_framework_memory_mounts(self.memory_mounts),
            "mcp_servers": normalize_framework_mcp_servers(self.mcp_servers),
            "tool_policy": _clone_json_like(self.tool_policy),
            "approval_policy": _clone_json_like(self.approval_policy),
            "loadout_policy": _clone_json_like(self.loadout_policy),
            "framework_surface_policy": _clone_json_like(self.framework_surface_policy),
            "workspace_bootstrap": build_framework_workspace_bootstrap(
                self.workspace_bootstrap,
                self.memory_mounts,
            ),
            "session_contract": build_framework_session_contract(self.session_policy),
        }

    def shared_contract_surface(self) -> Dict[str, Any]:
        return _clone_json_like(self._shared_contract_surface)

    def shared_contract_bridges(self) -> Dict[str, Any]:
        return extract_framework_workspace_bridges(
            self._shared_contract_surface["workspace_bootstrap"]
        )

    def shared_contract_payload(self) -> Dict[str, Any]:
        return {
            "schema_version": FRAMEWORK_SHARED_CONTRACT_SCHEMA_VERSION,
            "authority": "framework_profile",
            "framework_truth": "framework_core",
            "profile_id": self.profile_id,
            "framework_profile_version": self.framework_profile_version,
            "shared_contract_fields": list(FRAMEWORK_SHARED_CONTRACT_FIELDS),
            "shared_contract": _clone_json_like(self._shared_contract_surface),
        }

    def to_dict(self) -> Dict[str, Any]:
        return {
            "profile_id": self.profile_id,
            "display_name": self.display_name,
            "framework_profile_version": self.framework_profile_version,
            "runtime_family": self.runtime_family,
            "host_family": self.host_family,
            "core_capabilities": list(self.core_capabilities),
            "optional_capabilities": list(self.optional_capabilities),
            "rules_bundle": _clone_json_like(self.rules_bundle),
            "skill_bundle": _clone_json_like(self.skill_bundle),
            "session_policy": _clone_json_like(self.session_policy),
            "tool_policy": _clone_json_like(self.tool_policy),
            "approval_policy": _clone_json_like(self.approval_policy),
            "loadout_policy": _clone_json_like(self.loadout_policy),
            "framework_surface_policy": _clone_json_like(self.framework_surface_policy),
            "artifact_contract": _clone_json_like(self.artifact_contract),
            "model_policy": _clone_json_like(self.model_policy),
            "memory_mounts": _clone_json_like(self.memory_mounts),
            "mcp_servers": _clone_json_like(self.mcp_servers),
            "workspace_bootstrap": _clone_json_like(self.workspace_bootstrap),
            "host_capability_requirements": _clone_json_like(self.host_capability_requirements),
            "metadata": _clone_json_like(self.metadata),
        }

    def validate(self) -> None:
        if not self.profile_id:
            raise ValueError("profile_id is required")
        if not self.display_name:
            raise ValueError("display_name is required")
        if not self.framework_profile_version:
            raise ValueError("framework_profile_version is required")
        missing = [cap for cap in CORE_CAPABILITIES if cap not in self.core_capabilities]
        if missing:
            raise ValueError(f"framework profile missing core capabilities: {missing}")
        if self.host_family == "aionrs":
            raise ValueError("framework core must not be pinned directly to aionrs")
        host_specific_metadata = sorted(set(self.metadata) & HOST_SPECIFIC_METADATA_KEYS)
        if host_specific_metadata:
            raise ValueError(
                "framework profile metadata must stay host-neutral; move host-specific "
                f"keys into adapter projections: {host_specific_metadata}"
            )

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "FrameworkProfile":
        return cls(
            profile_id=str(data["profile_id"]),
            display_name=str(data["display_name"]),
            framework_profile_version=str(data.get("framework_profile_version", FRAMEWORK_PROFILE_VERSION)),
            runtime_family=str(data.get("runtime_family", "portable")),
            host_family=str(data.get("host_family", "generic")),
            core_capabilities=tuple(data.get("core_capabilities", CORE_CAPABILITIES)),
            optional_capabilities=tuple(data.get("optional_capabilities", ())),
            rules_bundle=_clone_json_like(data.get("rules_bundle", "default")),
            skill_bundle=_clone_json_like(data.get("skill_bundle", "default")),
            session_policy=_clone_json_like(data.get("session_policy", {})),
            tool_policy=_clone_json_like(data.get("tool_policy", {})),
            approval_policy=_clone_json_like(data.get("approval_policy", {})),
            loadout_policy=_clone_json_like(data.get("loadout_policy", {})),
            framework_surface_policy=_clone_json_like(data.get("framework_surface_policy", {})),
            artifact_contract=_clone_json_like(data.get("artifact_contract", {})),
            model_policy=_clone_json_like(data.get("model_policy", {})),
            memory_mounts=tuple(data.get("memory_mounts", ())),
            mcp_servers=tuple(data.get("mcp_servers", ())),
            workspace_bootstrap=_clone_json_like(data.get("workspace_bootstrap", {})),
            host_capability_requirements=_clone_json_like(data.get("host_capability_requirements", {})),
            metadata=_clone_json_like(data.get("metadata", {})),
        )


def build_framework_profile(
    *,
    profile_id: str,
    display_name: str,
    host_family: str = "generic",
    runtime_family: str = "portable",
    core_capabilities: Sequence[str] = CORE_CAPABILITIES,
    optional_capabilities: Sequence[str] = (),
    rules_bundle: Any = "default",
    skill_bundle: Any = "default",
    session_policy: Mapping[str, Any] | None = None,
    tool_policy: Mapping[str, Any] | None = None,
    approval_policy: Mapping[str, Any] | None = None,
    loadout_policy: Mapping[str, Any] | None = None,
    framework_surface_policy: Mapping[str, Any] | None = None,
    artifact_contract: Mapping[str, Any] | None = None,
    model_policy: Mapping[str, Any] | None = None,
    memory_mounts: Sequence[Any] = (),
    mcp_servers: Sequence[Any] = (),
    workspace_bootstrap: Mapping[str, Any] | None = None,
    host_capability_requirements: Mapping[str, Mapping[str, Any]] | None = None,
    metadata: Mapping[str, Any] | None = None,
) -> FrameworkProfile:
    profile = FrameworkProfile(
        profile_id=profile_id,
        display_name=display_name,
        host_family=host_family,
        runtime_family=runtime_family,
        core_capabilities=tuple(core_capabilities),
        optional_capabilities=tuple(optional_capabilities),
        rules_bundle=_clone_json_like(rules_bundle),
        skill_bundle=_clone_json_like(skill_bundle),
        session_policy=_clone_json_like(session_policy or {}),
        tool_policy=_clone_json_like(tool_policy or {}),
        approval_policy=_clone_json_like(approval_policy or {}),
        loadout_policy=_clone_json_like(loadout_policy or {}),
        framework_surface_policy=_clone_json_like(framework_surface_policy or {}),
        artifact_contract=_clone_json_like(artifact_contract or {}),
        model_policy=_clone_json_like(model_policy or {}),
        memory_mounts=tuple(memory_mounts),
        mcp_servers=tuple(mcp_servers),
        workspace_bootstrap=_clone_json_like(workspace_bootstrap or {}),
        host_capability_requirements=_clone_json_like(host_capability_requirements or {}),
        metadata=_clone_json_like(metadata or {}),
    )
    profile.validate()
    return profile


def merge_profile_overrides(
    profile: FrameworkProfile,
    overrides: Mapping[str, Any],
) -> FrameworkProfile:
    data: MutableMapping[str, Any] = profile.to_dict()
    for key, value in overrides.items():
        if key in {
            "session_policy",
            "tool_policy",
            "approval_policy",
            "loadout_policy",
            "framework_surface_policy",
            "artifact_contract",
            "model_policy",
            "workspace_bootstrap",
            "host_capability_requirements",
            "metadata",
        }:
            merged = _merge_nested_mapping(
                data.get(key, {}),
                value if isinstance(value, Mapping) else {},
            )
            data[key] = merged
        else:
            data[key] = _clone_json_like(value)
    merged_profile = FrameworkProfile.from_dict(data)
    merged_profile.validate()
    return merged_profile


def ensure_capabilities(profile: FrameworkProfile, required: Iterable[str]) -> None:
    available = set(profile.core_capabilities) | set(profile.optional_capabilities)
    missing = [cap for cap in required if cap not in available]
    if missing:
        raise ValueError(f"missing required capabilities: {missing}")


def resolve_host_capability_requirements(
    profile: FrameworkProfile,
    *,
    host_id: str,
    adapter_id: str | None = None,
) -> Dict[str, Any]:
    """Compile the host-facing requirement view for one concrete host projection.

    The raw `profile.host_capability_requirements` mapping stays framework-owned
    truth. Adapter payloads should emit only this resolved slice so host-private
    routing hints do not leak back into the shared contract surface.
    """
    merged: Dict[str, Any] = {}
    merge_order = ["default", host_id]
    if adapter_id:
        merge_order.append(adapter_id)
    for key in merge_order:
        requirements = profile.host_capability_requirements.get(key)
        if not isinstance(requirements, Mapping):
            continue
        merged = _merge_nested_mapping(merged, requirements)
    return merged
