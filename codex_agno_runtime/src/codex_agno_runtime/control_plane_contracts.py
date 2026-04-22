"""Shared control-plane contract artifacts kept outside the canonical host-adapter spine."""

from __future__ import annotations

from typing import Any, Dict

from codex_agno_runtime.execution_kernel_contracts import (
    EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    build_execution_kernel_live_response_serialization_contract_core,
)
from codex_agno_runtime.host_adapters import (
    CLAUDE_CODE_ADAPTER_ID,
    CODEX_CLI_ADAPTER_ID,
    CODEX_DESKTOP_ADAPTER_ID,
    DELEGATION_CONTRACT_ARTIFACT_ID,
    EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
    EXECUTION_KERNEL_LIVE_FALLBACK_RETIREMENT_ARTIFACT_ID,
    EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_ARTIFACT_ID,
    GEMINI_CLI_ADAPTER_ID,
    SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID,
)


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
                    "runtime_control_plane.services.execution.kernel_contract",
                    "ExecutionEnvironmentService.health().kernel_live_backend_impl",
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
