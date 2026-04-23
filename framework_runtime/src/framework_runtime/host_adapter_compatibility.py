"""Compatibility-only host adapter surfaces kept out of the canonical adapter spine."""

from __future__ import annotations

from typing import Any, Dict, Iterable, Mapping

from framework_runtime.framework_profile import (
    FrameworkProfile,
    build_framework_session_contract,
    ensure_capabilities,
    extract_framework_workspace_bridges,
    normalize_framework_mcp_servers,
    resolve_host_capability_requirements,
)
from framework_runtime.host_adapters import (
    AIONRS_COMPANION_ADAPTER,
    AIONUI_HOST_ADAPTER,
    CLI_FAMILY_PARITY_ARTIFACT_ID,
    CODEX_DESKTOP_ADAPTER_ID,
    CODEX_DESKTOP_HOST_ADAPTER,
    COMPATIBILITY_HOST_ADAPTERS,
    COMPATIBILITY_INVENTORY_ARTIFACT_ID,
    DEFAULT_HOST_PEER_SET,
    HostAdapterSpec,
    LEGACY_CODEX_DESKTOP_ADAPTER_ID,
    PARITY_BASELINE_ARTIFACT_ID,
    AdaptedHostProfile,
    _clone_json_like,
    _compile_aionrs_config,
    _compile_tool_approval_mapping,
    _default_event_stream_binding,
    _default_event_translation,
    _default_event_transport,
    _normalize_bundle_items,
    adapt_framework_profile,
    get_host_adapter,
    list_host_adapters,
    CORE_CAPABILITIES,
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
        "sessionMode": build_framework_session_contract(profile.session_policy),
        "aionrsConfig": _compile_aionrs_config(profile),
        "mcpConfig": {"servers": normalize_framework_mcp_servers(profile.mcp_servers)},
        "workspaceBootstrap": payload["workspace_bootstrap"],
        "bridges": extract_framework_workspace_bridges(payload["workspace_bootstrap"]),
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
    companion_contract = compile_aionrs_companion_adapter(
        profile,
        host_overrides=host_overrides,
    ).host_payload["companion_contract"]
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
        "approval_transport": _clone_json_like(companion_contract["toolApprovalMapping"]),
        "event_stream": _clone_json_like(companion_contract["eventTranslation"]),
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
            "legacy_alias_shim_required": None,
        }
    )
    inventory_complete = bool(inventory_summary.get("inventory_complete", False))
    primary_identity_risk_occurrences = inventory_summary.get("primary_identity_risk_occurrences")
    legacy_alias_shim_required = inventory_summary.get("legacy_alias_shim_required")
    runtime_primary_identity_consumers_cleared = (
        primary_identity_risk_occurrences == 0 if inventory_complete else None
    )

    return {
        "canonical_adapter_id": CODEX_DESKTOP_ADAPTER_ID,
        "legacy_alias_id": LEGACY_CODEX_DESKTOP_ADAPTER_ID,
        "alias_lifecycle": "retired-alias-only",
        "alias_mode": "mirror-only",
        "framework_truth": "framework_core",
        "primary_regression_artifact": PARITY_BASELINE_ARTIFACT_ID,
        "codex_dual_entry_parity_artifact": "codex_dual_entry_parity_snapshot",
        "secondary_inventory_artifact": COMPATIBILITY_INVENTORY_ARTIFACT_ID,
        "emitter_contract": {
            "native_emits_alias_artifact": False,
            "rust_emits_alias_artifact": False,
            "drop_requires_joint_emitter_flip": True,
            "legacy_alias_artifact_opt_in": True,
            "alias_may_not_gain_new_host_semantics": True,
        },
        "retirement_gates": {
            "canonical_desktop_identity_locked": True,
            "parity_snapshot_is_primary_baseline": True,
            "legacy_alias_inventory_is_secondary": True,
            "runtime_primary_identity_consumers_cleared": runtime_primary_identity_consumers_cleared,
            "legacy_alias_shim_required": legacy_alias_shim_required,
            "legacy_alias_shim_ready_if_needed": False if legacy_alias_shim_required else True,
        },
        "inventory_summary": inventory_summary,
    }


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
    for adapter_id, spec in {
        adapter.adapter_id: adapter for adapter in list_host_adapters(include_legacy_aliases=False)
    }.items():
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
