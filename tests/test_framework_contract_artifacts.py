from __future__ import annotations

import ast
import json
import subprocess
import sys
from typing import Any, Mapping
from pathlib import Path
from unittest.mock import patch

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
RUST_ADAPTER_TIMEOUT_SECONDS = 120.0
ROUTER_RS_CRATE_ROOT = PROJECT_ROOT / "scripts" / "router-rs"
ROUTER_RS_RELEASE_BIN = ROUTER_RS_CRATE_ROOT / "target" / "release" / "router-rs"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

import framework_runtime.profile_artifacts as profile_artifacts_module
from framework_runtime.framework_profile import build_framework_profile
from framework_runtime.host_adapters import compile_codex_common_adapter
from framework_runtime.profile_artifacts import (
    emit_framework_contract_artifacts,
)
from framework_runtime.framework_profile import FrameworkProfile
from framework_runtime.rust_router import RustRouteAdapter


def _load_json_payload(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_shared_contract_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    assert "shared_contract" in payload or "common_contract" in payload
    return FrameworkProfile.from_dict(payload).shared_contract_surface()


def _ensure_current_router_rs_release_binary() -> None:
    latest_source_mtime = max(
        (
            path.stat().st_mtime
            for path in [ROUTER_RS_CRATE_ROOT / "Cargo.toml", *ROUTER_RS_CRATE_ROOT.joinpath("src").rglob("*.rs")]
        ),
        default=0.0,
    )
    if ROUTER_RS_RELEASE_BIN.is_file() and ROUTER_RS_RELEASE_BIN.stat().st_mtime >= latest_source_mtime:
        return
    subprocess.run(
        ["cargo", "build", "--manifest-path", str(ROUTER_RS_CRATE_ROOT / "Cargo.toml"), "--release"],
        cwd=PROJECT_ROOT,
        check=True,
    )


def _rust_route_adapter() -> RustRouteAdapter:
    _ensure_current_router_rs_release_binary()
    return RustRouteAdapter(PROJECT_ROOT, timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS)


def test_profile_artifacts_avoids_internal_compatibility_escape_hatch_import() -> None:
    module_path = PROJECT_ROOT / "framework_runtime" / "src" / "framework_runtime" / "profile_artifacts.py"
    tree = ast.parse(module_path.read_text(encoding="utf-8"))

    cli_family_imports = [
        node
        for node in tree.body
        if isinstance(node, ast.ImportFrom) and node.module == "framework_runtime.cli_family_contracts"
    ]

    assert cli_family_imports == []

    compatibility_imports = [
        node
        for node in tree.body
        if isinstance(node, ast.ImportFrom) and node.module == "framework_runtime.host_adapter_compatibility"
    ]

    assert compatibility_imports == []

    host_adapter_wrapper_names = {
        "build_codex_desktop_alias_retirement_status",
        "build_upgrade_compatibility_matrix",
        "compile_aionrs_companion_adapter",
        "compile_aionui_host_adapter",
    }
    wrapper_imports = []
    for node in tree.body:
        if not isinstance(node, ast.ImportFrom) or node.module != "framework_runtime.host_adapters":
            continue
        imported = {alias.name for alias in node.names}
        overlap = sorted(imported & host_adapter_wrapper_names)
        if overlap:
            wrapper_imports.extend(overlap)

    assert wrapper_imports == []

    retired_python_projection_helpers = {
        "FrameworkSharedContract",
        "FrameworkSharedContractProjectionReport",
        "FrameworkSharedContractSurface",
        "build_framework_shared_contract_projection_report",
    }
    retired_imports = []
    for node in tree.body:
        if not isinstance(node, ast.ImportFrom):
            continue
        imported = {alias.name for alias in node.names}
        retired_imports.extend(sorted(imported & retired_python_projection_helpers))

    assert retired_imports == []


def test_internal_runtime_modules_avoid_host_adapters_wrapper_imports() -> None:
    runtime_root = PROJECT_ROOT / "framework_runtime" / "src" / "framework_runtime"
    banned_wrapper_names = {
        "build_cli_family_capability_discovery",
        "build_cli_family_parity_snapshot",
        "build_codex_desktop_alias_retirement_status",
        "build_codex_dual_entry_parity_snapshot",
        "build_control_plane_contract_descriptors",
        "build_delegation_contract",
        "build_execution_controller_contract",
        "build_execution_kernel_live_fallback_retirement_status",
        "build_execution_kernel_live_response_serialization_contract",
        "build_supervisor_state_contract",
        "build_upgrade_compatibility_matrix",
        "compatibility_snapshot",
        "compile_aionrs_companion_adapter",
        "compile_aionui_host_adapter",
        "validate_adapter_compatibility",
    }

    offenders: dict[str, list[str]] = {}
    for module_path in sorted(runtime_root.glob("*.py")):
        if module_path.name in {"host_adapters.py", "__init__.py"}:
            continue
        tree = ast.parse(module_path.read_text(encoding="utf-8"))
        hits: set[str] = set()
        for node in tree.body:
            if not isinstance(node, ast.ImportFrom) or node.module != "framework_runtime.host_adapters":
                continue
            hits.update(alias.name for alias in node.names if alias.name in banned_wrapper_names)
        if hits:
            offenders[module_path.name] = sorted(hits)

    assert offenders == {}


def test_emit_framework_contract_artifacts_writes_parity_snapshot_baseline_and_rust_outputs(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile",
        display_name="Artifact Profile",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router", "memory-bridge"]},
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        artifact_contract={"layout": "stable-v1"},
        model_policy={"provider": "openai", "model": "gpt-5"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )

    paths = emit_framework_contract_artifacts(
        tmp_path,
        profile=profile,
        rust_adapter=_rust_route_adapter(),
    )

    expected_keys = {
        "artifact_layout_manifest",
        "framework_profile",
        "framework_surface_policy",
        "cli_common_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
        "cli_family_capability_discovery",
        "codex_desktop_adapter",
        "cli_family_parity_snapshot",
        "codex_dual_entry_parity_snapshot",
        "execution_controller_contract",
        "delegation_contract",
        "supervisor_state_contract",
        "execution_kernel_live_fallback_retirement_status",
        "execution_kernel_live_response_serialization_contract",
        "rust_profile_bundle",
        "rust_cli_common_adapter",
        "rust_codex_common_adapter",
        "rust_codex_desktop_adapter",
        "rust_codex_cli_adapter",
        "rust_claude_code_adapter",
        "rust_gemini_cli_adapter",
        "rust_cli_family_capability_discovery",
        "rust_cli_family_parity_snapshot",
        "rust_codex_dual_entry_parity_snapshot",
        "rust_execution_controller_contract",
        "rust_delegation_contract",
        "rust_supervisor_state_contract",
        "rust_execution_kernel_live_fallback_retirement_status",
        "rust_execution_kernel_live_response_serialization_contract",
    }
    assert expected_keys == set(paths)
    assert "codex_desktop_host_adapter" not in paths
    assert "codex_desktop_alias_inventory" not in paths
    assert "codex_desktop_alias_retirement_status" not in paths
    assert "codex_common_adapter" not in paths
    assert "rust_codex_desktop_host_adapter" not in paths
    assert Path(paths["framework_profile"]).parent.name == "default"
    assert Path(paths["cli_common_adapter"]).parent.name == "default"
    assert Path(paths["framework_surface_policy"]).parent.name == "default"
    assert Path(paths["rust_profile_bundle"]).parent.name == "rust"
    layout_manifest = json.loads(Path(paths["artifact_layout_manifest"]).read_text(encoding="utf-8"))
    assert layout_manifest["directory_policy"] == {
        "default": "default",
        "fallback": "fallback",
        "continuity": "continuity",
        "rust": "rust",
    }
    assert "cli_common_adapter" in layout_manifest["artifacts_by_lane"]["default"]
    assert "codex_common_adapter" not in layout_manifest["artifacts_by_lane"]["default"]
    assert "rust_profile_bundle" in layout_manifest["artifacts_by_lane"]["rust"]

    cli_common = json.loads(Path(paths["cli_common_adapter"]).read_text(encoding="utf-8"))
    surface_policy = json.loads(Path(paths["framework_surface_policy"]).read_text(encoding="utf-8"))
    assert surface_policy["kernel"]["canonical_axes"] == [
        "routing",
        "memory",
        "continuity",
        "host_adapter_payload",
    ]
    assert surface_policy["default_surface"]["default_loadouts"] == ["default_surface_loadout"]
    assert cli_common["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert cli_common["shared_contract"]["framework_surface_policy"]["default_surface"][
        "default_loadouts"
    ] == ["default_surface_loadout"]
    assert cli_common["shared_contract"]["execution_controller_contract"]["status_contract"] == (
        "execution_controller_contract_v1"
    )
    assert cli_common["shared_contract"]["delegation_contract"]["gate"]["gate_skill"] == (
        "subagent-delegation"
    )
    assert cli_common["shared_contract"]["supervisor_state_contract"]["state_artifact_path"] == (
        ".supervisor_state.json"
    )
    assert cli_common["bridge_contract"] == cli_common["shared_contract"]["workspace_bootstrap"][
        "bridges"
    ]

    claude = json.loads(Path(paths["claude_code_adapter"]).read_text(encoding="utf-8"))
    assert "host_projection" not in claude
    assert claude["host_adapter_payload"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert claude["host_adapter_payload"]["settings_scope_order"] == [
        "managed",
        "command_line",
        "local",
        "project",
        "user",
    ]
    assert claude["host_adapter_payload"]["config_root_env_var"] == "CLAUDE_CONFIG_DIR"
    assert claude["host_adapter_payload"]["subagent_paths"] == [
        "~/.claude/agents/",
        ".claude/agents/",
    ]
    assert claude["host_adapter_payload"]["hook_event_names"] == [
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
    ]
    assert claude["host_adapter_payload"]["hook_control_settings"] == [
        "disableAllHooks",
        "allowManagedHooksOnly",
        "allowedHttpHookUrls",
        "httpHookAllowedEnvVars",
    ]
    assert claude["host_adapter_payload"]["hook_inspection_commands"] == ["/hooks"]
    assert claude["host_adapter_payload"]["plugin_hook_manifest_paths"] == ["hooks/hooks.json"]
    assert claude["host_adapter_payload"]["hook_environment_markers"] == [
        "CLAUDE_ENV_FILE",
        "CLAUDE_PROJECT_DIR",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_PLUGIN_DATA",
        "CLAUDE_CODE_REMOTE",
    ]
    assert claude["host_adapter_payload"]["checkpointing_supported"] is True

    gemini = json.loads(Path(paths["gemini_cli_adapter"]).read_text(encoding="utf-8"))
    assert "host_projection" not in gemini
    assert gemini["host_adapter_payload"]["structured_output_modes"] == ["json", "stream-json"]

    cli_discovery = json.loads(
        Path(paths["cli_family_capability_discovery"]).read_text(encoding="utf-8")
    )
    assert cli_discovery["discovery_contract"] == "cli_family_host_capability_contract_v1"
    assert set(cli_discovery["cli_hosts"]) == {
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    }
    assert cli_discovery["controller_boundary"]["host_entrypoints"] == [
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert cli_discovery["cli_hosts"]["codex_cli_adapter"]["supports_cron"] is True
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["transport"] == "headless-exec"
    assert cli_discovery["cli_hosts"]["claude_code_adapter"]["settings_paths"] == [
        "~/.claude/settings.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
    ]
    assert cli_discovery["all_cli_hosts_compatible"] is True

    cli_parity = json.loads(Path(paths["cli_family_parity_snapshot"]).read_text(encoding="utf-8"))
    assert cli_parity["shared_adapter"] == "cli_common_adapter"
    assert cli_parity["cli_hosts"]["claude_code_adapter"]["mcp_config_paths"] == ["~/.claude.json"]
    assert cli_parity["cli_hosts"]["claude_code_adapter"]["settings_scope_order"] == [
        "managed",
        "command_line",
        "local",
        "project",
        "user",
    ]
    assert cli_parity["cli_hosts"]["claude_code_adapter"]["hook_inspection_commands"] == ["/hooks"]
    assert cli_parity["cli_hosts"]["claude_code_adapter"]["checkpointing_supported"] is True
    assert cli_parity["all_shared_contract_checks_pass"] is True

    parity = json.loads(Path(paths["codex_dual_entry_parity_snapshot"]).read_text(encoding="utf-8"))
    assert parity["framework_truth"] == "framework_core"
    assert parity["shared_adapter"] == "cli_common_adapter"
    assert parity["shared_adapter_aliases"] == ["codex_common_adapter"]
    assert parity["desktop"]["adapter_id"] == "codex_desktop_adapter"
    assert parity["desktop"]["entrypoint_kind"] == "interactive"
    assert parity["cli"]["adapter_id"] == "codex_cli_adapter"
    assert parity["cli"]["entrypoint_kind"] == "headless"
    assert parity["controller_boundary"]["single_source_of_truth"] is True
    assert parity["parity_checks"]["artifact_contract"] is True
    assert parity["all_shared_contract_checks_pass"] is True

    execution_controller = json.loads(
        Path(paths["execution_controller_contract"]).read_text(encoding="utf-8")
    )
    assert execution_controller["status_contract"] == "execution_controller_contract_v1"
    assert execution_controller["controller"]["primary_owner"] == "execution-controller-coding"
    assert execution_controller["controller"]["user_facing_aliases"] == ["gsd", "get shit done"]
    assert execution_controller["gsd_execution_posture"]["auto_continue_safe_local_work"] is True
    assert execution_controller["gsd_execution_posture"]["runtime_dependency"] == "none"
    assert execution_controller["boundaries"]["runtime_branching_changes_required"] is False

    delegation = json.loads(Path(paths["delegation_contract"]).read_text(encoding="utf-8"))
    assert delegation["status_contract"] == "delegation_contract_v4"
    assert delegation["gate"]["gate_type"] == "multi_agent_routing"
    assert delegation["gate"]["decision_before_spawn"] is True
    assert delegation["gate"]["route_outcomes"] == ["local", "subagent", "team"]
    assert delegation["gate"]["team_route_skill"] == "team"
    assert delegation["local_supervisor_mode"]["preserves_output_contracts"] is True
    assert delegation["selection_matrix"]["subagent_when"][0] == (
        "bounded sidecars exist with non-overlapping write scopes"
    )
    assert delegation["lane_contract_fields"] == [
        "lane_id",
        "lane_owner",
        "bounded_write_scope",
        "expected_output",
        "integration_status",
        "verification_status",
        "recovery_anchor",
    ]
    assert delegation["delegation_state_fields"] == [
        "routing_decision",
        "orchestration_mode",
        "delegation_plan_created",
        "spawn_attempted",
        "spawn_block_reason",
        "fallback_mode",
        "delegated_sidecars",
        "delegated_lanes",
    ]
    assert delegation["team_contract"] == {
        "supervisor_owned_continuity": True,
        "integration_and_qa_stay_supervisor_led": True,
        "resume_and_recovery_are_first_class": True,
    }

    supervisor_state = json.loads(
        Path(paths["supervisor_state_contract"]).read_text(encoding="utf-8")
    )
    assert supervisor_state["status_contract"] == "supervisor_state_contract_v3"
    assert supervisor_state["schema_expectations"]["execution_contract_fields"] == [
        "goal",
        "scope",
        "forbidden_scope",
        "acceptance_criteria",
        "evidence_required",
    ]
    assert supervisor_state["schema_expectations"]["team_state_fields"] == [
        "delegation_planned",
        "spawn_pending",
        "spawn_blocked",
        "integration_pending",
        "resume_required",
        "cleanup_pending",
    ]
    assert supervisor_state["cross_artifact_alignment"]["delegation_structure_must_be_explicit"] is True
    assert supervisor_state["cross_artifact_alignment"]["lane_outputs_must_remain_lane_local_until_integrated"] is True

    fallback_retirement = json.loads(
        Path(paths["execution_kernel_live_fallback_retirement_status"]).read_text(encoding="utf-8")
    )
    assert fallback_retirement["status_contract"] == (
        "execution_kernel_live_fallback_retirement_status_v1"
    )
    assert fallback_retirement["live_primary"]["contract_mode"] == "rust-live-primary"
    assert fallback_retirement["compatibility_fallback"]["runtime_path_available"] is False
    assert fallback_retirement["compatibility_fallback"]["retired_mode"] == "retired"
    assert fallback_retirement["compatibility_fallback"]["request_behavior"] == "surface-removed"
    assert fallback_retirement["current_contract_truth"]["dry_run_delegate_kind"] == "router-rs"
    assert fallback_retirement["current_contract_truth"]["live_fallback_runtime_path_available"] is (
        False
    )
    assert fallback_retirement["public_runtime_response_metadata_fields"] == [
        "execution_kernel_delegate_family",
        "execution_kernel_delegate_impl",
    ]
    assert fallback_retirement["current_response_metadata_truth"]["dry_run_delegate_family"] == "rust-cli"
    assert fallback_retirement["current_response_metadata_truth"]["dry_run_delegate_impl"] == "router-rs"
    assert fallback_retirement["current_response_metadata_truth"]["live_delegate_impl"] == "router-rs"
    assert fallback_retirement["remaining_python_owned_surfaces"] == []
    assert fallback_retirement["retirement_gates"]["response_metadata_surface_externalized"] is True
    assert fallback_retirement["retirement_gates"]["delegate_family_impl_metadata_externalized"] is True
    assert fallback_retirement["retirement_gates"]["dry_run_delegate_still_python_owned"] is False
    assert fallback_retirement["retirement_gates"]["compatibility_fallback_runtime_path_removed"] is True
    assert fallback_retirement["retirement_gates"]["explicit_compatibility_requests_rejected"] is True
    assert fallback_retirement["retirement_readiness"]["ready"] is True

    response_serialization = json.loads(
        Path(paths["execution_kernel_live_response_serialization_contract"]).read_text(
            encoding="utf-8"
        )
    )
    assert response_serialization["status_contract"] == (
        "execution_kernel_live_response_serialization_contract_v1"
    )
    assert response_serialization["runtime_response_metadata_fields"]["shared"] == [
        "trace_event_count",
        "trace_output_path",
    ]
    assert response_serialization["current_response_shape_truth"]["live_primary"][
        "pass_through_metadata_fields"
    ] == [
        "execution_mode",
        "route_engine",
        "diagnostic_route_mode",
    ]
    assert response_serialization["current_contract_truth"]["steady_state_response_shapes"] == [
        "live_primary",
        "dry_run",
    ]
    assert response_serialization["retirement_gates"][
        "compatibility_live_response_serialization_still_python_owned"
    ] is False
    assert fallback_retirement["guardrails"]["thin_projection_boundary_preserved"] is True

    rust_bundle = json.loads(Path(paths["rust_profile_bundle"]).read_text(encoding="utf-8"))
    assert rust_bundle["profile_id"] == "artifact-profile"
    assert "companion_projection" not in rust_bundle
    assert rust_bundle["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert rust_bundle["codex_common_adapter"]["controller_boundary"]["framework_truth"] == "framework_core"
    assert rust_bundle["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"] == "interactive"
    assert rust_bundle["codex_cli_adapter"]["execution_surface"]["supports_cron"] is True
    assert rust_bundle["claude_code_adapter"]["host_adapter_payload"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert rust_bundle["claude_code_adapter"]["execution_surface"]["supports_cron"] is False
    assert rust_bundle["claude_code_adapter"]["host_adapter_payload"]["subagent_paths"] == [
        "~/.claude/agents/",
        ".claude/agents/",
    ]
    assert rust_bundle["claude_code_adapter"]["host_adapter_payload"]["config_root_env_var"] == (
        "CLAUDE_CONFIG_DIR"
    )
    assert rust_bundle["claude_code_adapter"]["host_adapter_payload"]["hook_event_names"] == [
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
    ]
    assert rust_bundle["claude_code_adapter"]["host_adapter_payload"]["hook_control_settings"] == [
        "disableAllHooks",
        "allowManagedHooksOnly",
        "allowedHttpHookUrls",
        "httpHookAllowedEnvVars",
    ]
    assert rust_bundle["claude_code_adapter"]["host_adapter_payload"]["hook_environment_markers"] == [
        "CLAUDE_ENV_FILE",
        "CLAUDE_PROJECT_DIR",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_PLUGIN_DATA",
        "CLAUDE_CODE_REMOTE",
    ]
    assert rust_bundle["claude_code_adapter"]["host_adapter_payload"]["checkpointing_supported"] is True
    assert "hook_registry" in rust_bundle["claude_code_adapter"]["capabilities"]["host"]
    assert rust_bundle["gemini_cli_adapter"]["host_adapter_payload"]["structured_output_modes"] == [
        "json",
        "stream-json",
    ]
    assert rust_bundle["gemini_cli_adapter"]["execution_surface"]["supports_cron"] is False
    assert rust_bundle["cli_family_parity_snapshot"]["cli_hosts"]["gemini_cli_adapter"]["context_files"] == [
        "GEMINI.md"
    ]
    assert rust_bundle["codex_dual_entry_parity_snapshot"]["desktop"]["adapter_id"] == "codex_desktop_adapter"
    assert rust_bundle["codex_dual_entry_parity_snapshot"]["cli"]["adapter_id"] == "codex_cli_adapter"
    assert rust_bundle["execution_controller_contract"]["status_contract"] == (
        "execution_controller_contract_v1"
    )
    assert rust_bundle["delegation_contract"]["gate"]["gate_skill"] == "subagent-delegation"
    assert rust_bundle["supervisor_state_contract"]["state_artifact_path"] == ".supervisor_state.json"
    assert rust_bundle["execution_kernel_live_fallback_retirement_status"]["retirement_readiness"][
        "status"
    ] == "retired"
    assert rust_bundle["execution_kernel_live_fallback_retirement_status"]["retirement_gates"][
        "public_runtime_contract_externalized"
    ] is True
    assert rust_bundle["execution_kernel_live_response_serialization_contract"]["status_contract"] == (
        "execution_kernel_live_response_serialization_contract_v1"
    )
    assert rust_bundle["execution_kernel_live_response_serialization_contract"][
        "runtime_response_metadata_fields"
    ]["shared"] == [
        "trace_event_count",
        "trace_output_path",
    ]

    rust_common = json.loads(Path(paths["rust_codex_common_adapter"]).read_text(encoding="utf-8"))
    assert rust_common["metadata"]["adapter_alias_of"] == "cli_common_adapter"
    assert rust_common["bridge_contract"] == rust_common["shared_contract"]["workspace_bootstrap"][
        "bridges"
    ]

    rust_cli_common = json.loads(Path(paths["rust_cli_common_adapter"]).read_text(encoding="utf-8"))
    assert rust_cli_common["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert rust_cli_common["bridge_contract"] == rust_cli_common["shared_contract"][
        "workspace_bootstrap"
    ]["bridges"]

    rust_desktop = json.loads(Path(paths["rust_codex_desktop_adapter"]).read_text(encoding="utf-8"))
    assert rust_desktop["entrypoint_contract"]["shared_adapter"] == "cli_common_adapter"

    rust_cli = json.loads(Path(paths["rust_codex_cli_adapter"]).read_text(encoding="utf-8"))
    assert rust_cli["execution_surface"]["controller_is_cli"] is False

    rust_claude = json.loads(Path(paths["rust_claude_code_adapter"]).read_text(encoding="utf-8"))
    assert rust_claude["host_adapter_payload"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert rust_claude["host_adapter_payload"]["hook_inspection_commands"] == ["/hooks"]
    assert rust_claude["host_adapter_payload"]["plugin_hook_manifest_paths"] == ["hooks/hooks.json"]
    assert rust_claude["host_adapter_payload"]["managed_settings_paths"][0] == (
        "/Library/Application Support/ClaudeCode/managed-settings.json"
    )

    rust_gemini = json.loads(Path(paths["rust_gemini_cli_adapter"]).read_text(encoding="utf-8"))
    assert rust_gemini["host_adapter_payload"]["structured_output_modes"] == ["json", "stream-json"]

    rust_cli_discovery = json.loads(
        Path(paths["rust_cli_family_capability_discovery"]).read_text(encoding="utf-8")
    )
    assert rust_cli_discovery["discovery_contract"] == "cli_family_host_capability_contract_v1"
    assert set(rust_cli_discovery["cli_hosts"]) == {
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    }
    assert rust_cli_discovery["controller_boundary"]["host_entrypoints"] == [
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert rust_cli_discovery["cli_hosts"]["codex_cli_adapter"]["supports_cron"] is True
    assert rust_cli_discovery["cli_hosts"]["claude_code_adapter"]["settings_paths"] == [
        "~/.claude/settings.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
    ]
    assert rust_cli_discovery["all_cli_hosts_compatible"] is True

    rust_cli_family = json.loads(
        Path(paths["rust_cli_family_parity_snapshot"]).read_text(encoding="utf-8")
    )
    assert rust_cli_family["all_shared_contract_checks_pass"] is True

    rust_parity = json.loads(
        Path(paths["rust_codex_dual_entry_parity_snapshot"]).read_text(encoding="utf-8")
    )
    assert rust_parity["all_shared_contract_checks_pass"] is True

    rust_execution_controller = json.loads(
        Path(paths["rust_execution_controller_contract"]).read_text(encoding="utf-8")
    )
    assert rust_execution_controller["status_contract"] == "execution_controller_contract_v1"
    assert rust_execution_controller["controller"]["state_artifact"] == ".supervisor_state.json"

    rust_delegation = json.loads(
        Path(paths["rust_delegation_contract"]).read_text(encoding="utf-8")
    )
    assert rust_delegation["status_contract"] == "delegation_contract_v4"
    assert rust_delegation["gate"]["gate_type"] == "multi_agent_routing"
    assert rust_delegation["gate"]["route_outcomes"] == ["local", "subagent", "team"]
    assert rust_delegation["gate"]["team_route_skill"] == "team"
    assert rust_delegation["selection_matrix"]["team_when"][0] == (
        "supervisor-led worker lifecycle management is part of the task"
    )
    assert rust_delegation["local_supervisor_mode"]["allowed_when_runtime_blocks_spawning"] is True
    assert rust_delegation["lane_contract_fields"] == [
        "lane_id",
        "lane_owner",
        "bounded_write_scope",
        "expected_output",
        "integration_status",
        "verification_status",
        "recovery_anchor",
    ]
    assert rust_delegation["delegation_state_fields"] == [
        "routing_decision",
        "orchestration_mode",
        "delegation_plan_created",
        "spawn_attempted",
        "spawn_block_reason",
        "fallback_mode",
        "delegated_sidecars",
        "delegated_lanes",
    ]
    assert rust_delegation["team_contract"] == {
        "supervisor_owned_continuity": True,
        "integration_and_qa_stay_supervisor_led": True,
        "resume_and_recovery_are_first_class": True,
    }

    rust_supervisor = json.loads(
        Path(paths["rust_supervisor_state_contract"]).read_text(encoding="utf-8")
    )
    assert rust_supervisor["status_contract"] == "supervisor_state_contract_v3"
    assert rust_supervisor["compatibility_rules"]["rust_may_validate_or_emit"] is True
    assert rust_supervisor["schema_expectations"]["team_state_fields"] == [
        "delegation_planned",
        "spawn_pending",
        "spawn_blocked",
        "integration_pending",
        "resume_required",
        "cleanup_pending",
    ]

    rust_fallback_retirement = json.loads(
        Path(paths["rust_execution_kernel_live_fallback_retirement_status"]).read_text(
            encoding="utf-8"
        )
    )
    assert rust_fallback_retirement["control_surfaces"]["former_settings_field"] == (
        "rust_execute_fallback_to_python"
    )
    assert rust_fallback_retirement["control_surfaces"]["accepted_after_retirement"] is False
    assert rust_fallback_retirement["retirement_exit_contract"]["surface_status"] == (
        "removed"
    )
    assert rust_fallback_retirement["retirement_exit_contract"]["current_decision"] == (
        "completed"
    )
    assert rust_fallback_retirement["retirement_exit_contract"]["remove_when"] == []
    assert rust_fallback_retirement["current_contract_truth"]["live_primary_kind"] == "router-rs"
    assert rust_fallback_retirement["retirement_gates"]["delegate_family_impl_metadata_externalized"] is True
    assert rust_fallback_retirement["retirement_gates"][
        "compatibility_live_response_serialization_still_python_owned"
    ] is False
    assert rust_fallback_retirement["retirement_gates"][
        "compatibility_fallback_runtime_path_removed"
    ] is True
    assert rust_fallback_retirement["retirement_readiness"][
        "runtime_control_flow_change_required"
    ] is False

    rust_response_serialization = json.loads(
        Path(paths["rust_execution_kernel_live_response_serialization_contract"]).read_text(
            encoding="utf-8"
        )
    )
    assert rust_response_serialization["current_contract_truth"]["public_response_model"] == (
        "RunTaskResponse"
    )
    assert rust_response_serialization["retirement_gates"][
        "compatibility_live_response_serialization_still_python_owned"
    ] is False

    assert cli_common == rust_cli_common
    assert rust_desktop == json.loads(Path(paths["codex_desktop_adapter"]).read_text(encoding="utf-8"))
    assert rust_cli == json.loads(Path(paths["codex_cli_adapter"]).read_text(encoding="utf-8"))
    assert claude == rust_claude
    assert gemini == rust_gemini
    assert cli_discovery == rust_cli_discovery
    assert cli_parity == rust_cli_family
    assert parity == rust_parity
    assert execution_controller == rust_execution_controller
    assert delegation == rust_delegation
    assert supervisor_state == rust_supervisor
    assert fallback_retirement == rust_fallback_retirement
    assert response_serialization == rust_response_serialization

    assert "rust_python_artifact_parity_report" not in paths


def test_emit_framework_contract_artifacts_requires_explicit_opt_in_for_continuity_outputs(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-no-auto-alias",
        display_name="Artifact Profile No Auto Alias",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )
    paths = emit_framework_contract_artifacts(tmp_path, profile=profile)
    assert "codex_desktop_host_adapter" not in paths
    assert "codex_desktop_alias_inventory" not in paths
    assert "codex_desktop_alias_retirement_status" not in paths
    assert "codex_common_adapter" not in paths
    assert "upgrade_compatibility_matrix" not in paths
    assert "aionrs_companion_adapter" not in paths
    assert "aionui_host_adapter" not in paths
    assert "generic_host_adapter" not in paths
    manifest = json.loads(Path(paths["artifact_layout_manifest"]).read_text(encoding="utf-8"))
    assert "fallback" not in manifest["artifacts_by_lane"]
    assert "continuity" not in manifest["artifacts_by_lane"]


def test_emit_framework_contract_artifacts_keeps_codex_common_adapter_out_of_default_lane(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-default-codex-common-hidden",
        display_name="Artifact Profile Default Codex Common Hidden",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )

    paths = emit_framework_contract_artifacts(tmp_path, profile=profile)

    assert "codex_common_adapter" not in paths
    manifest = json.loads(Path(paths["artifact_layout_manifest"]).read_text(encoding="utf-8"))
    assert "codex_common_adapter" not in manifest["artifacts_by_lane"]["default"]


def test_emit_framework_contract_artifacts_rejects_fallback_host_outputs(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-fallbacks",
        display_name="Artifact Profile Fallbacks",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )

    with pytest.raises(ValueError, match="fallback host artifacts are retired"):
        emit_framework_contract_artifacts(
            tmp_path,
            profile=profile,
            include_fallback_artifacts=True,
        )


def test_emit_framework_contract_artifacts_rejects_compatibility_inventory(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-compatibility-inventory",
        display_name="Artifact Profile Compatibility Inventory",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )

    with pytest.raises(ValueError, match="compatibility inventory artifacts are retired"):
        emit_framework_contract_artifacts(
            tmp_path,
            profile=profile,
            include_compatibility_inventory=True,
        )


def test_framework_profile_artifact_bidirectional_shared_contract_consistency(tmp_path: Path) -> None:
    profile = build_framework_profile(
        profile_id="bidirectional-profile",
        display_name="Bidirectional Contract Profile",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router", "memory-bridge"]},
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        tool_policy={"shell": "allow"},
        approval_policy={"mode": "manual"},
        loadout_policy={"default": "framework"},
        artifact_contract={"layout": "stable-v1"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
        workspace_bootstrap={"skill_bridge": {"project_dir": ".codex/skills"}},
    )
    paths = emit_framework_contract_artifacts(
        tmp_path,
        profile=profile,
        rust_adapter=_rust_route_adapter(),
    )

    framework_profile = FrameworkProfile.from_dict(
        _load_json_payload(Path(paths["framework_profile"]))
    )
    canonical_surface = framework_profile.shared_contract_surface()

    contract_payloads = {
        "cli_common_adapter": _load_json_payload(Path(paths["cli_common_adapter"])),
        "codex_common_adapter": compile_codex_common_adapter(framework_profile).host_payload,
        "codex_desktop_adapter": _load_json_payload(Path(paths["codex_desktop_adapter"])),
        "codex_cli_adapter": _load_json_payload(Path(paths["codex_cli_adapter"])),
        "claude_code_adapter": _load_json_payload(Path(paths["claude_code_adapter"])),
        "gemini_cli_adapter": _load_json_payload(Path(paths["gemini_cli_adapter"])),
    }

    for adapter_id, payload in contract_payloads.items():
        assert _read_shared_contract_payload(payload) == canonical_surface, (
            f"{adapter_id} shared-contract payload not aligned with framework_profile"
        )


def test_emit_framework_contract_artifacts_keeps_compatibility_inventory_outputs_retired(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-legacy",
        display_name="Artifact Profile Legacy",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )

    with pytest.raises(ValueError, match="compatibility inventory artifacts are retired"):
        emit_framework_contract_artifacts(
            tmp_path,
            profile=profile,
            rust_adapter=_rust_route_adapter(),
            include_compatibility_inventory=True,
        )


def test_rust_route_adapter_can_compile_profile_bundle(tmp_path: Path) -> None:
    profile = build_framework_profile(
        profile_id="rust-compile-profile",
        display_name="Rust Compile Profile",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )
    profile_path = tmp_path / "framework_profile.json"
    profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

    payload = _rust_route_adapter().compile_profile_bundle(profile_path)

    assert payload["profile_id"] == "rust-compile-profile"
    assert "companion_projection" not in payload
    assert payload["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert payload["codex_common_adapter"]["metadata"]["adapter_alias_of"] == "cli_common_adapter"
    assert payload["cli_family_capability_discovery"]["all_cli_hosts_compatible"] is True
    assert payload["codex_dual_entry_parity_snapshot"]["all_shared_contract_checks_pass"] is True


def test_rust_route_adapter_can_compile_codex_profile_artifacts(tmp_path: Path) -> None:
    profile = build_framework_profile(
        profile_id="rust-artifacts-profile",
        display_name="Rust Artifacts Profile",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )
    profile_path = tmp_path / "framework_profile.json"
    profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

    payload = _rust_route_adapter().compile_codex_profile_artifacts(profile_path)

    assert set(payload) == {
        "cli_common_adapter",
        "codex_common_adapter",
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
        "cli_family_capability_discovery",
        "cli_family_parity_snapshot",
        "codex_dual_entry_parity_snapshot",
        "execution_controller_contract",
        "delegation_contract",
        "supervisor_state_contract",
        "execution_kernel_live_fallback_retirement_status",
        "execution_kernel_live_response_serialization_contract",
    }
    assert payload["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert payload["codex_common_adapter"]["controller_boundary"]["framework_truth"] == "framework_core"
    assert payload["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"] == "interactive"
    assert payload["codex_cli_adapter"]["execution_surface"]["controller_is_cli"] is False
    assert payload["claude_code_adapter"]["host_adapter_payload"]["settings_paths"] == [
        "~/.claude/settings.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
    ]
    assert payload["execution_controller_contract"]["status_contract"] == (
        "execution_controller_contract_v1"
    )
    assert payload["delegation_contract"]["gate"]["gate_skill"] == "subagent-delegation"
    assert payload["supervisor_state_contract"]["state_artifact_path"] == ".supervisor_state.json"
    assert payload["execution_kernel_live_fallback_retirement_status"]["retirement_readiness"][
        "next_safe_slice"
    ] == "rustification_closed"
    assert payload["execution_kernel_live_fallback_retirement_status"][
        "public_runtime_response_metadata_fields"
    ] == [
        "execution_kernel_delegate_family",
        "execution_kernel_delegate_impl",
    ]
    assert payload["execution_kernel_live_fallback_retirement_status"]["compatibility_fallback"][
        "request_behavior"
    ] == "surface-removed"
    assert payload["execution_kernel_live_fallback_retirement_status"]["retirement_exit_contract"][
        "surface_status"
    ] == "removed"
    assert payload["execution_kernel_live_fallback_retirement_status"]["retirement_gates"][
        "dry_run_prompt_preview_still_python_owned"
    ] is False
    assert payload["execution_kernel_live_fallback_retirement_status"]["retirement_gates"][
        "in_process_replacement_complete"
    ] is True
    assert payload["execution_kernel_live_response_serialization_contract"]["status_contract"] == (
        "execution_kernel_live_response_serialization_contract_v1"
    )
    assert payload["execution_kernel_live_response_serialization_contract"][
        "current_response_shape_truth"
    ]["live_primary"]["pass_through_metadata_fields"] == [
        "execution_mode",
        "route_engine",
        "diagnostic_route_mode",
    ]
    assert payload["gemini_cli_adapter"]["host_adapter_payload"]["context_files"] == ["GEMINI.md"]
    assert payload["cli_family_capability_discovery"]["all_cli_hosts_compatible"] is True
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["codex_cli_adapter"][
        "supports_cron"
    ] is True
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["claude_code_adapter"][
        "transport"
    ] == "headless-exec"
    assert payload["cli_family_parity_snapshot"]["all_shared_contract_checks_pass"] is True
    assert payload["codex_dual_entry_parity_snapshot"]["all_shared_contract_checks_pass"] is True


def test_rust_route_adapter_rejects_compatibility_inventory_artifact(tmp_path: Path) -> None:
    profile = build_framework_profile(
        profile_id="rust-artifacts-profile-compat",
        display_name="Rust Artifacts Profile Compat",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )
    profile_path = tmp_path / "framework_profile.json"
    profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

    with pytest.raises(ValueError, match="compatibility inventory artifacts are retired"):
        _rust_route_adapter().compile_codex_profile_artifacts(
            profile_path,
            include_compatibility_inventory=True,
        )


def test_rust_route_adapter_keeps_compatibility_inventory_retired(tmp_path: Path) -> None:
    profile = build_framework_profile(
        profile_id="rust-artifacts-profile-legacy",
        display_name="Rust Artifacts Profile Legacy",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )
    profile_path = tmp_path / "framework_profile.json"
    profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

    with pytest.raises(ValueError, match="compatibility inventory artifacts are retired"):
        _rust_route_adapter().compile_codex_profile_artifacts(
            profile_path,
            include_compatibility_inventory=True,
        )
