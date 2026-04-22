from __future__ import annotations

import json
import sys
from pathlib import Path
from unittest.mock import patch

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
RUST_ADAPTER_TIMEOUT_SECONDS = 120.0
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

import codex_agno_runtime.profile_artifacts as profile_artifacts_module
from codex_agno_runtime.framework_profile import build_framework_profile
from codex_agno_runtime.profile_artifacts import (
    build_framework_shared_contract_projection_report,
    emit_framework_contract_artifacts,
)
from codex_agno_runtime.rust_router import RustRouteAdapter


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
        rust_adapter=RustRouteAdapter(PROJECT_ROOT, timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS),
    )

    expected_keys = {
        "artifact_layout_manifest",
        "framework_profile",
        "framework_surface_policy",
        "cli_common_adapter",
        "codex_common_adapter",
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
        "rust_python_artifact_parity_report",
    }
    assert expected_keys == set(paths)
    assert "codex_desktop_host_adapter" not in paths
    assert "codex_desktop_alias_inventory" not in paths
    assert "codex_desktop_alias_retirement_status" not in paths
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
    assert "rust_profile_bundle" in layout_manifest["artifacts_by_lane"]["rust"]

    cli_common = json.loads(Path(paths["cli_common_adapter"]).read_text(encoding="utf-8"))
    surface_policy = json.loads(Path(paths["framework_surface_policy"]).read_text(encoding="utf-8"))
    assert surface_policy["kernel"]["canonical_axes"] == [
        "routing",
        "memory",
        "continuity",
        "host_projection",
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

    common = json.loads(Path(paths["codex_common_adapter"]).read_text(encoding="utf-8"))
    assert common["controller_boundary"]["framework_truth"] == "framework_core"
    assert common["metadata"]["adapter_alias_of"] == "cli_common_adapter"
    assert common["bridge_contract"] == common["shared_contract"]["workspace_bootstrap"]["bridges"]

    claude = json.loads(Path(paths["claude_code_adapter"]).read_text(encoding="utf-8"))
    assert claude["host_projection"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert claude["host_projection"]["settings_scope_order"] == [
        "managed",
        "command_line",
        "local",
        "project",
        "user",
    ]
    assert claude["host_projection"]["config_root_env_var"] == "CLAUDE_CONFIG_DIR"
    assert claude["host_projection"]["subagent_paths"] == [
        "~/.claude/agents/",
        ".claude/agents/",
    ]
    assert claude["host_projection"]["hook_event_names"] == [
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
    assert claude["host_projection"]["hook_control_settings"] == [
        "disableAllHooks",
        "allowManagedHooksOnly",
        "allowedHttpHookUrls",
        "httpHookAllowedEnvVars",
    ]
    assert claude["host_projection"]["hook_inspection_commands"] == ["/hooks"]
    assert claude["host_projection"]["plugin_hook_manifest_paths"] == ["hooks/hooks.json"]
    assert claude["host_projection"]["hook_environment_markers"] == [
        "CLAUDE_ENV_FILE",
        "CLAUDE_PROJECT_DIR",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_PLUGIN_DATA",
        "CLAUDE_CODE_REMOTE",
    ]
    assert claude["host_projection"]["checkpointing_supported"] is True

    gemini = json.loads(Path(paths["gemini_cli_adapter"]).read_text(encoding="utf-8"))
    assert gemini["host_projection"]["structured_output_modes"] == ["json", "stream-json"]

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
    assert parity["desktop"]["legacy_aliases"] == ["codex_desktop_host_adapter"]
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
    assert delegation["status_contract"] == "delegation_contract_v1"
    assert delegation["gate"]["decision_before_spawn"] is True
    assert delegation["local_supervisor_mode"]["preserves_output_contracts"] is True

    supervisor_state = json.loads(
        Path(paths["supervisor_state_contract"]).read_text(encoding="utf-8")
    )
    assert supervisor_state["status_contract"] == "supervisor_state_contract_v2"
    assert supervisor_state["schema_expectations"]["execution_contract_fields"] == [
        "goal",
        "scope",
        "forbidden_scope",
        "acceptance_criteria",
        "evidence_required",
    ]
    assert supervisor_state["cross_artifact_alignment"]["delegation_structure_must_be_explicit"] is True

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
    assert fallback_retirement["current_contract_truth"]["compatibility_fallback_reason_metadata_key"] == (
        "execution_kernel_fallback_reason"
    )
    assert fallback_retirement["public_runtime_response_metadata_fields"] == [
        "execution_kernel_delegate_family",
        "execution_kernel_delegate_impl",
    ]
    assert fallback_retirement["retired_runtime_response_metadata_fields"] == [
        "execution_kernel_fallback_reason",
    ]
    assert fallback_retirement["current_response_metadata_truth"]["dry_run_delegate_family"] == "rust-cli"
    assert fallback_retirement["current_response_metadata_truth"]["dry_run_delegate_impl"] == "router-rs"
    assert fallback_retirement["current_response_metadata_truth"]["live_delegate_impl"] == "router-rs"
    assert (
        fallback_retirement["current_response_metadata_truth"][
            "compatibility_fallback_reason_present_in_steady_state"
        ]
        is False
    )
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
    assert response_serialization["current_response_shape_truth"]["retired_compatibility_fallback"][
        "runtime_path_available"
    ] is False
    assert response_serialization["retirement_gates"][
        "compatibility_live_response_serialization_still_python_owned"
    ] is False
    assert fallback_retirement["guardrails"]["thin_projection_boundary_preserved"] is True

    rust_bundle = json.loads(Path(paths["rust_profile_bundle"]).read_text(encoding="utf-8"))
    assert rust_bundle["profile_id"] == "artifact-profile"
    assert rust_bundle["companion_projection"]["enabledSkills"][1]["skill_id"] == "memory-bridge"
    assert rust_bundle["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert rust_bundle["codex_common_adapter"]["controller_boundary"]["framework_truth"] == "framework_core"
    assert rust_bundle["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"] == "interactive"
    assert rust_bundle["codex_cli_adapter"]["execution_surface"]["supports_cron"] is True
    assert rust_bundle["claude_code_adapter"]["host_projection"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert rust_bundle["claude_code_adapter"]["execution_surface"]["supports_cron"] is False
    assert rust_bundle["claude_code_adapter"]["host_projection"]["subagent_paths"] == [
        "~/.claude/agents/",
        ".claude/agents/",
    ]
    assert rust_bundle["claude_code_adapter"]["host_projection"]["config_root_env_var"] == (
        "CLAUDE_CONFIG_DIR"
    )
    assert rust_bundle["claude_code_adapter"]["host_projection"]["hook_event_names"] == [
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
    assert rust_bundle["claude_code_adapter"]["host_projection"]["hook_control_settings"] == [
        "disableAllHooks",
        "allowManagedHooksOnly",
        "allowedHttpHookUrls",
        "httpHookAllowedEnvVars",
    ]
    assert rust_bundle["claude_code_adapter"]["host_projection"]["hook_environment_markers"] == [
        "CLAUDE_ENV_FILE",
        "CLAUDE_PROJECT_DIR",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_PLUGIN_DATA",
        "CLAUDE_CODE_REMOTE",
    ]
    assert rust_bundle["claude_code_adapter"]["host_projection"]["checkpointing_supported"] is True
    assert "hook_registry" in rust_bundle["claude_code_adapter"]["capabilities"]["host"]
    assert rust_bundle["gemini_cli_adapter"]["host_projection"]["structured_output_modes"] == [
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
    assert rust_claude["host_projection"]["context_files"] == [
        "CLAUDE.md",
        "CLAUDE.local.md",
    ]
    assert rust_claude["host_projection"]["hook_inspection_commands"] == ["/hooks"]
    assert rust_claude["host_projection"]["plugin_hook_manifest_paths"] == ["hooks/hooks.json"]
    assert rust_claude["host_projection"]["managed_settings_paths"][0] == (
        "/Library/Application Support/ClaudeCode/managed-settings.json"
    )

    rust_gemini = json.loads(Path(paths["rust_gemini_cli_adapter"]).read_text(encoding="utf-8"))
    assert rust_gemini["host_projection"]["structured_output_modes"] == ["json", "stream-json"]

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
    assert rust_parity["desktop"]["legacy_aliases"] == ["codex_desktop_host_adapter"]
    assert rust_parity["all_shared_contract_checks_pass"] is True

    rust_execution_controller = json.loads(
        Path(paths["rust_execution_controller_contract"]).read_text(encoding="utf-8")
    )
    assert rust_execution_controller["status_contract"] == "execution_controller_contract_v1"
    assert rust_execution_controller["controller"]["state_artifact"] == ".supervisor_state.json"

    rust_delegation = json.loads(
        Path(paths["rust_delegation_contract"]).read_text(encoding="utf-8")
    )
    assert rust_delegation["status_contract"] == "delegation_contract_v1"
    assert rust_delegation["local_supervisor_mode"]["allowed_when_runtime_blocks_spawning"] is True

    rust_supervisor = json.loads(
        Path(paths["rust_supervisor_state_contract"]).read_text(encoding="utf-8")
    )
    assert rust_supervisor["status_contract"] == "supervisor_state_contract_v2"
    assert rust_supervisor["compatibility_rules"]["rust_may_validate_or_emit"] is True

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
    assert rust_response_serialization["current_response_shape_truth"]["retired_compatibility_fallback"][
        "legacy_required_metadata_fields"
    ] == [
        "run_id",
        "status",
        "trace_event_count",
        "trace_output_path",
        "execution_kernel_contract_mode",
        "execution_kernel_fallback_policy",
        "execution_kernel_primary",
        "execution_kernel_primary_authority",
        "execution_kernel_fallback_reason",
        "execution_kernel_compatibility_agent_contract",
        "execution_kernel_compatibility_agent_kind",
        "execution_kernel_compatibility_agent_authority",
    ]
    assert rust_response_serialization["retirement_gates"][
        "compatibility_live_response_serialization_still_python_owned"
    ] is False

    rust_parity_report = json.loads(
        Path(paths["rust_python_artifact_parity_report"]).read_text(encoding="utf-8")
    )
    assert rust_parity_report["schema_version"] == "rust-python-artifact-parity-report-v1"
    assert rust_parity_report["raw_all_artifacts_match"] is True
    assert rust_parity_report["all_artifacts_match_after_normalization"] is True
    assert rust_parity_report["artifacts"]["cli_common_adapter"]["normalized_match"] is True
    assert rust_parity_report["artifacts"]["codex_cli_adapter"]["normalized_match"] is True
    assert rust_parity_report["artifacts"]["claude_code_adapter"]["normalized_match"] is True
    assert rust_parity_report["artifacts"]["cli_family_capability_discovery"]["normalized_match"] is True
    assert rust_parity_report["artifacts"]["gemini_cli_adapter"]["normalized_match"] is True
    assert rust_parity_report["artifacts"]["cli_family_parity_snapshot"]["normalized_match"] is True
    assert rust_parity_report["artifacts"]["execution_controller_contract"]["raw_match"] is True
    assert rust_parity_report["artifacts"]["delegation_contract"]["raw_match"] is True
    assert rust_parity_report["artifacts"]["supervisor_state_contract"]["raw_match"] is True
    assert rust_parity_report["artifacts"]["execution_kernel_live_fallback_retirement_status"][
        "raw_match"
    ] is True
    assert rust_parity_report["artifacts"]["execution_kernel_live_response_serialization_contract"][
        "raw_match"
    ] is True
    assert "codex_desktop_alias_retirement_status" not in rust_parity_report["artifacts"]


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
    risky_inventory = {
        "canonical_adapter_id": "codex_desktop_adapter",
        "legacy_alias_id": "codex_desktop_host_adapter",
        "scan_root": str(PROJECT_ROOT),
        "summary": {
            "inventory_complete": False,
            "primary_identity_risk_occurrences": 3,
            "translation_shim_required": True,
        },
        "references": [],
    }

    with patch.object(
        profile_artifacts_module,
        "build_codex_desktop_alias_inventory",
        return_value=risky_inventory,
    ):
        paths = emit_framework_contract_artifacts(tmp_path, profile=profile)

    assert "codex_desktop_host_adapter" not in paths
    assert "codex_desktop_alias_inventory" not in paths
    assert "codex_desktop_alias_retirement_status" not in paths
    assert "upgrade_compatibility_matrix" not in paths
    assert "aionrs_companion_adapter" not in paths
    assert "aionui_host_adapter" not in paths
    assert "generic_host_adapter" not in paths
    manifest = json.loads(Path(paths["artifact_layout_manifest"]).read_text(encoding="utf-8"))
    assert "fallback" not in manifest["artifacts_by_lane"]
    assert "continuity" not in manifest["artifacts_by_lane"]


def test_emit_framework_contract_artifacts_can_opt_in_fallback_host_outputs(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-fallbacks",
        display_name="Artifact Profile Fallbacks",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )

    paths = emit_framework_contract_artifacts(
        tmp_path,
        profile=profile,
        include_fallback_artifacts=True,
    )

    companion = json.loads(Path(paths["aionrs_companion_adapter"]).read_text(encoding="utf-8"))
    assert Path(paths["aionrs_companion_adapter"]).parent.name == "fallback"
    assert companion["companion_contract"]["fallbackSemantics"]["fallback_adapter"] == "codex_desktop_adapter"
    assert companion["companion_contract"]["fallbackSemantics"]["default_host_peer_set"] == [
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert companion["legacy_boundary"]["exposure_lane"] == "fallback-only-explicit"
    assert companion["legacy_boundary"]["default_host_peer_set_member"] is False

    aionui = json.loads(Path(paths["aionui_host_adapter"]).read_text(encoding="utf-8"))
    assert aionui["host_runtime_contract"]["preferred_backend"] == "aionrs_companion_adapter"
    assert aionui["host_runtime_contract"]["fallback_semantics"]["default_host_peer_set"] == [
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    ]
    assert aionui["legacy_boundary"]["exposure_lane"] == "fallback-only-explicit"
    assert aionui["legacy_boundary"]["default_host_peer_set_member"] is False

    generic = json.loads(Path(paths["generic_host_adapter"]).read_text(encoding="utf-8"))
    assert Path(paths["generic_host_adapter"]).parent.name == "fallback"
    assert generic["metadata"]["adapter_id"] == "generic_host_adapter"
    assert generic["metadata"]["host_id"] == "generic"
    assert generic["capabilities"]["host"] == ["local_runtime", "artifact_contract", "memory_mounts"]


def test_emit_framework_contract_artifacts_can_opt_in_compatibility_inventory(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-compatibility-inventory",
        display_name="Artifact Profile Compatibility Inventory",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )

    paths = emit_framework_contract_artifacts(
        tmp_path,
        profile=profile,
        include_compatibility_inventory=True,
    )

    matrix = json.loads(Path(paths["upgrade_compatibility_matrix"]).read_text(encoding="utf-8"))
    assert Path(paths["upgrade_compatibility_matrix"]).parent.name == "continuity"
    assert matrix["cli_common_adapter"]["compatible"] is True
    assert matrix["codex_desktop_adapter"]["compatible"] is True
    assert matrix["codex_cli_adapter"]["compatible"] is True
    assert matrix["claude_code_adapter"]["compatible"] is True
    assert matrix["gemini_cli_adapter"]["compatible"] is True
    assert "aionrs_companion_adapter" not in matrix
    assert "aionui_host_adapter" not in matrix
    assert "codex_desktop_host_adapter" not in matrix


def test_framework_shared_contract_projection_report_keeps_hosts_on_one_outer_truth() -> None:
    profile = build_framework_profile(
        profile_id="shared-contract-report",
        display_name="Shared Contract Report",
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

    report = build_framework_shared_contract_projection_report(profile)
    projections = {item["adapter_id"]: item for item in report["adapter_projections"]}

    assert report["shared_contract_schema_version"] == "framework-shared-contract-v1"
    assert report["all_shared_contract_projections_match"] is True
    assert set(projections) == {
        "cli_common_adapter",
        "codex_common_adapter",
        "codex_desktop_adapter",
        "codex_cli_adapter",
        "claude_code_adapter",
        "gemini_cli_adapter",
    }
    assert projections["cli_common_adapter"]["projection_field"] == "shared_contract"
    assert projections["cli_common_adapter"]["shared_contract_match"] is True
    assert projections["codex_desktop_adapter"]["projection_field"] == "common_contract"
    assert projections["codex_desktop_adapter"]["runtime_surface_match"] is True
    assert projections["codex_cli_adapter"]["runtime_surface_match"] is True
    assert projections["claude_code_adapter"]["runtime_surface_match"] is True
    assert projections["gemini_cli_adapter"]["runtime_surface_match"] is True
    assert projections["gemini_cli_adapter"]["projected_contract"]["workspace_bootstrap"][
        "bridges"
    ]["memory"]["mounts"] == [
        {
            "mount_id": "project",
            "source": "project",
            "bridge_kind": "framework-memory-mount",
        }
    ]


def test_emit_framework_contract_artifacts_can_opt_in_continuity_alias_outputs(
    tmp_path: Path,
) -> None:
    profile = build_framework_profile(
        profile_id="artifact-profile-legacy",
        display_name="Artifact Profile Legacy",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )

    paths = emit_framework_contract_artifacts(
        tmp_path,
        profile=profile,
        rust_adapter=RustRouteAdapter(PROJECT_ROOT, timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS),
        include_compatibility_inventory=True,
        include_legacy_alias_artifact=True,
    )

    assert "codex_desktop_host_adapter" not in paths
    alias_inventory = json.loads(
        Path(paths["codex_desktop_alias_inventory"]).read_text(encoding="utf-8")
    )
    assert alias_inventory["canonical_adapter_id"] == "codex_desktop_adapter"
    assert alias_inventory["summary"]["inventory_complete"] is True
    assert alias_inventory["summary"]["primary_identity_risk_occurrences"] == 0

    alias_retirement = json.loads(
        Path(paths["codex_desktop_alias_retirement_status"]).read_text(encoding="utf-8")
    )
    assert alias_retirement["legacy_alias_id"] == "codex_desktop_host_adapter"
    assert alias_retirement["emitter_contract"]["legacy_alias_artifact_opt_in"] is True
    assert json.loads(Path(paths["cli_common_adapter"]).read_text(encoding="utf-8"))[
        "controller_boundary"
    ]["shared_adapter"] == "cli_common_adapter"
    assert json.loads(Path(paths["codex_common_adapter"]).read_text(encoding="utf-8"))[
        "metadata"
    ]["adapter_alias_of"] == "cli_common_adapter"
    assert json.loads(Path(paths["codex_desktop_adapter"]).read_text(encoding="utf-8"))[
        "entrypoint_contract"
    ]["entrypoint_kind"] == "interactive"
    assert json.loads(Path(paths["codex_cli_adapter"]).read_text(encoding="utf-8"))[
        "execution_surface"
    ]["entrypoint_kind"] == "headless"
    assert json.loads(Path(paths["claude_code_adapter"]).read_text(encoding="utf-8"))[
        "host_projection"
    ]["context_files"][0] == "CLAUDE.md"
    assert json.loads(Path(paths["gemini_cli_adapter"]).read_text(encoding="utf-8"))[
        "host_projection"
    ]["settings_paths"] == ["~/.gemini/settings.json"]
    matrix = json.loads(Path(paths["upgrade_compatibility_matrix"]).read_text(encoding="utf-8"))
    assert Path(paths["upgrade_compatibility_matrix"]).parent.name == "continuity"
    assert matrix["codex_desktop_host_adapter"]["compatible"] is True

    rust_alias_payload = json.loads(
        Path(paths["rust_codex_desktop_host_adapter"]).read_text(encoding="utf-8")
    )
    assert rust_alias_payload["metadata"]["adapter_alias_of"] == "codex_desktop_adapter"
    rust_alias_retirement = json.loads(
        Path(paths["rust_codex_desktop_alias_retirement_status"]).read_text(encoding="utf-8")
    )
    assert rust_alias_retirement["canonical_adapter_id"] == "codex_desktop_adapter"
    assert rust_alias_retirement["retirement_gates"]["runtime_primary_identity_consumers_cleared"] is True
    assert rust_alias_retirement["emitter_contract"]["legacy_alias_artifact_opt_in"] is True


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

    payload = RustRouteAdapter(
        PROJECT_ROOT,
        timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS,
    ).compile_profile_bundle(profile_path)

    assert payload["profile_id"] == "rust-compile-profile"
    assert payload["companion_projection"]["presetRules"][0]["id"] == "outer-owned"
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

    payload = RustRouteAdapter(
        PROJECT_ROOT,
        timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS,
    ).compile_codex_profile_artifacts(profile_path)

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
    assert payload["claude_code_adapter"]["host_projection"]["settings_paths"] == [
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
    assert payload["execution_kernel_live_fallback_retirement_status"][
        "retired_runtime_response_metadata_fields"
    ] == ["execution_kernel_fallback_reason"]
    assert payload["execution_kernel_live_fallback_retirement_status"]["compatibility_fallback"][
        "request_behavior"
    ] == "surface-removed"
    assert payload["execution_kernel_live_fallback_retirement_status"]["retirement_exit_contract"][
        "surface_status"
    ] == "removed"
    assert payload["execution_kernel_live_fallback_retirement_status"]["current_response_metadata_truth"][
        "compatibility_fallback_reason_present_in_steady_state"
    ] is False
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
    assert payload["gemini_cli_adapter"]["host_projection"]["context_files"] == ["GEMINI.md"]
    assert payload["cli_family_capability_discovery"]["all_cli_hosts_compatible"] is True
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["codex_cli_adapter"][
        "supports_cron"
    ] is True
    assert payload["cli_family_capability_discovery"]["cli_hosts"]["claude_code_adapter"][
        "transport"
    ] == "headless-exec"
    assert payload["cli_family_parity_snapshot"]["all_shared_contract_checks_pass"] is True
    assert payload["codex_dual_entry_parity_snapshot"]["all_shared_contract_checks_pass"] is True
def test_rust_route_adapter_can_opt_in_continuity_alias_artifact(tmp_path: Path) -> None:
    profile = build_framework_profile(
        profile_id="rust-artifacts-profile-legacy",
        display_name="Rust Artifacts Profile Legacy",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router"]},
        session_policy={"mode": "bounded"},
    )
    profile_path = tmp_path / "framework_profile.json"
    profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")

    payload = RustRouteAdapter(
        PROJECT_ROOT,
        timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS,
    ).compile_codex_profile_artifacts(
        profile_path,
        include_legacy_alias_artifact=True,
    )

    assert payload["cli_common_adapter"]["controller_boundary"]["shared_adapter"] == "cli_common_adapter"
    assert payload["codex_common_adapter"]["controller_boundary"]["framework_truth"] == "framework_core"
    assert payload["codex_desktop_adapter"]["entrypoint_contract"]["entrypoint_kind"] == "interactive"
    assert payload["codex_cli_adapter"]["execution_surface"]["entrypoint_kind"] == "headless"
    assert payload["claude_code_adapter"]["host_projection"]["context_files"][0] == "CLAUDE.md"
    assert payload["gemini_cli_adapter"]["host_projection"]["structured_output_modes"] == [
        "json",
        "stream-json",
    ]
    assert payload["codex_desktop_host_adapter"]["metadata"]["adapter_alias_of"] == "codex_desktop_adapter"
