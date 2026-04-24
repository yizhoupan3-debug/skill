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
    ensure_capabilities,
)
from .runtime_registry import default_host_peer_set, host_adapter_records
UPSTREAM_SAFE_ZONE = "upstream-safe-zone"
THIN_PATCH_ZONE = "thin-patch-zone"
FORK_DANGER_ZONE = "fork-danger-zone"
_HOST_PRIVATE_OVERRIDE_KEY = "host_private"
HOST_ADAPTER_PAYLOAD_KEY = "host_adapter_payload"

COMMON_FORK_DANGER_SURFACES: tuple[str, ...] = ()

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
CLI_FAMILY_PARITY_ARTIFACT_ID = "cli_family_parity_snapshot"
PARITY_BASELINE_ARTIFACT_ID = CLI_FAMILY_PARITY_ARTIFACT_ID
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
DEFAULT_HOST_PEER_SET = default_host_peer_set()
PROJECT_ROOT = Path(__file__).resolve().parents[3]


def _clone_json_like(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {str(key): _clone_json_like(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_clone_json_like(item) for item in value]
    return value


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
    if "host_projection" in host_private:
        raise ValueError("host_projection output is retired; use host_adapter_payload.")
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


def _merge_rust_adapter_payload(
    rust_payload: Mapping[str, Any],
    host_overrides: Mapping[str, Any] | None,
) -> Dict[str, Any]:
    payload = dict(_clone_json_like(rust_payload))
    if host_overrides:
        public_overrides, host_private_overrides = _split_host_overrides(host_overrides)
        if public_overrides:
            payload = _merge_mapping(payload, public_overrides)
        if host_private_overrides:
            payload = _merge_mapping(payload, host_private_overrides)
    return payload


def _compile_rust_owned_adapter(
    profile: FrameworkProfile,
    adapter_spec: HostAdapterSpec,
    artifact_id: str,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> AdaptedHostProfile:
    payload = _merge_rust_adapter_payload(
        _compile_rust_codex_artifact(profile, artifact_id),
        host_overrides,
    )
    return AdaptedHostProfile(
        framework_profile=profile,
        adapter=adapter_spec,
        host_payload=payload,
    )


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


def _tuple_from_record(record: Mapping[str, Any], key: str) -> tuple[str, ...]:
    values = record.get(key, ())
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
        return ()
    return tuple(str(value) for value in values)


def _host_adapter_spec_from_registry_record(record: Mapping[str, Any]) -> HostAdapterSpec:
    protocol_hints = record.get("protocol_hints")
    return HostAdapterSpec(
        adapter_id=str(record["adapter_id"]),
        host_id=str(record["host_id"]),
        transport=str(record["transport"]),
        required_capabilities=_tuple_from_record(record, "required_capabilities"),
        optional_capabilities=_tuple_from_record(record, "optional_capabilities"),
        host_capabilities=_tuple_from_record(record, "host_capabilities"),
        emits_artifacts=bool(record.get("emits_artifacts", True)),
        supports_memory_mounts=bool(record.get("supports_memory_mounts", True)),
        supports_orchestration=bool(record.get("supports_orchestration", True)),
        upgrade_zone=str(record.get("upgrade_zone", UPSTREAM_SAFE_ZONE)),
        thin_patch_surfaces=_tuple_from_record(record, "thin_patch_surfaces"),
        fork_danger_surfaces=(
            _tuple_from_record(record, "fork_danger_surfaces") or COMMON_FORK_DANGER_SURFACES
        ),
        protocol_hints=(
            dict(_clone_json_like(protocol_hints)) if isinstance(protocol_hints, Mapping) else {}
        ),
        notes=str(record.get("notes", "")),
    )


def _host_adapter_registry_from_runtime_registry(
    *,
    include_legacy_aliases: bool,
) -> Dict[str, HostAdapterSpec]:
    if include_legacy_aliases:
        raise ValueError("legacy host adapter aliases are retired; use the default Rust lane only.")
    records = host_adapter_records(include_legacy_aliases=False)
    return {
        spec.adapter_id: spec
        for spec in (
            _host_adapter_spec_from_registry_record(record)
            for record in records
            if isinstance(record, Mapping)
        )
    }


HOST_ADAPTERS: Dict[str, HostAdapterSpec] = _host_adapter_registry_from_runtime_registry(
    include_legacy_aliases=False,
)
CLI_COMMON_ADAPTER = HOST_ADAPTERS[CLI_COMMON_ADAPTER_ID]
CODEX_COMMON_ADAPTER = HOST_ADAPTERS[CODEX_COMMON_ADAPTER_ID]
CODEX_DESKTOP_ADAPTER = HOST_ADAPTERS[CODEX_DESKTOP_ADAPTER_ID]
CODEX_CLI_ADAPTER = HOST_ADAPTERS[CODEX_CLI_ADAPTER_ID]
CLAUDE_CODE_ADAPTER = HOST_ADAPTERS[CLAUDE_CODE_ADAPTER_ID]
GEMINI_CLI_ADAPTER = HOST_ADAPTERS[GEMINI_CLI_ADAPTER_ID]

RUST_OWNED_HOST_ADAPTER_ARTIFACTS = {
    CLI_COMMON_ADAPTER_ID: CLI_COMMON_ADAPTER_ID,
    CODEX_COMMON_ADAPTER_ID: CODEX_COMMON_ADAPTER_ID,
    CODEX_DESKTOP_ADAPTER_ID: CODEX_DESKTOP_ADAPTER_ID,
    CODEX_CLI_ADAPTER_ID: CODEX_CLI_ADAPTER_ID,
    CLAUDE_CODE_ADAPTER_ID: CLAUDE_CODE_ADAPTER_ID,
    GEMINI_CLI_ADAPTER_ID: GEMINI_CLI_ADAPTER_ID,
}


def _select_host_adapter_registry(*, include_legacy_aliases: bool) -> Dict[str, HostAdapterSpec]:
    if include_legacy_aliases:
        raise ValueError("legacy host adapter aliases are retired; use the default Rust lane only.")
    return HOST_ADAPTERS


def get_host_adapter(
    adapter_id: str,
    *,
    include_legacy_aliases: bool = False,
) -> HostAdapterSpec:
    registry = _select_host_adapter_registry(include_legacy_aliases=include_legacy_aliases)
    try:
        return registry[adapter_id]
    except KeyError as exc:
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
    rust_artifact_id = RUST_OWNED_HOST_ADAPTER_ARTIFACTS.get(adapter_spec.adapter_id)
    if rust_artifact_id is None:
        raise ValueError(
            f"host adapter {adapter_spec.adapter_id!r} is not a Rust-owned runtime artifact."
        )
    return _compile_rust_owned_adapter(
        profile,
        adapter_spec,
        rust_artifact_id,
        host_overrides=host_overrides,
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
