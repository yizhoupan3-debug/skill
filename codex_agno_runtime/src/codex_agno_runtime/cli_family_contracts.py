"""CLI-family parity and discovery artifacts kept outside the canonical host-adapter spine."""

from __future__ import annotations

from typing import Any, Dict, Mapping

from codex_agno_runtime.framework_profile import (
    CORE_CAPABILITIES,
    FrameworkProfile,
    resolve_host_capability_requirements,
)
from codex_agno_runtime.host_adapters import (
    CLAUDE_CODE_ADAPTER,
    CLAUDE_CODE_ADAPTER_ID,
    CLI_COMMON_ADAPTER_ID,
    CLI_FAMILY_PARITY_ARTIFACT_ID,
    CODEX_CLI_ADAPTER,
    CODEX_CLI_ADAPTER_ID,
    CODEX_COMMON_ADAPTER_ID,
    GEMINI_CLI_ADAPTER,
    GEMINI_CLI_ADAPTER_ID,
    LEGACY_CODEX_DESKTOP_ADAPTER_ID,
    COMMON_PARITY_FIELDS,
    HostAdapterSpec,
    _clone_json_like,
    compile_claude_code_adapter,
    compile_cli_common_adapter,
    compile_codex_cli_adapter,
    compile_codex_desktop_adapter,
    compile_gemini_cli_adapter,
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
    supervisor_capabilities = {
        "external_session_supervisor": "external_session_supervisor" in available_set,
        "rate_limit_auto_resume": "rate_limit_auto_resume" in available_set,
        "host_resume_entrypoint": "host_resume_entrypoint" in available_set,
        "host_tmux_worker_management": "host_tmux_worker_management" in available_set,
    }
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
        "supervisor_capabilities": supervisor_capabilities,
        "session_supervisor_driver": adapter_spec.protocol_hints.get("session_supervisor_driver"),
        "resume_command_examples": _clone_json_like(
            adapter_spec.protocol_hints.get("resume_command_examples", ())
        ),
        "framework_alias_entrypoints": _clone_json_like(
            adapter_spec.protocol_hints.get("framework_alias_entrypoints", {})
        ),
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
