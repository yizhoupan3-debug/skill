"""Shared execution-kernel contract helpers used by runtime and descriptor layers."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Mapping

from .schemas import RoutingResult, RunTaskResponse, UsageMetrics

EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY = "execution_kernel_fallback_reason"
EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY = "execution_kernel_contract_mode"
EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY = "execution_kernel_fallback_policy"
EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY = (
    "execution_kernel_compatibility_agent_contract"
)
EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY = (
    "execution_kernel_compatibility_agent_kind"
)
EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY = (
    "execution_kernel_compatibility_agent_authority"
)
EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE = "rust-live-primary"
EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY = "infrastructure-only-explicit"
EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_VERSION = (
    "execution-kernel-compatibility-agent-v1"
)
EXECUTION_KERNEL_LIVE_PRIMARY_SCHEMA_VERSION = "router-rs-execute-response-v1"
EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_STATUS_CONTRACT = (
    "execution_kernel_live_response_serialization_contract_v1"
)
EXECUTION_KERNEL_PUBLIC_RESPONSE_FIELDS = (
    "session_id",
    "user_id",
    "skill",
    "overlay",
    "live_run",
    "content",
    "usage",
    "prompt_preview",
    "model_id",
    "metadata",
)
EXECUTION_KERNEL_USAGE_FIELDS = (
    "input_tokens",
    "output_tokens",
    "total_tokens",
    "mode",
)
EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS = (
    "execution_kernel",
    "execution_kernel_authority",
    EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
    "execution_kernel_in_process_replacement_complete",
    "execution_kernel_delegate",
    "execution_kernel_delegate_authority",
    "execution_kernel_delegate_family",
    "execution_kernel_delegate_impl",
    "execution_kernel_live_primary",
    "execution_kernel_live_primary_authority",
    "execution_kernel_live_fallback",
    "execution_kernel_live_fallback_authority",
    "execution_kernel_live_fallback_enabled",
    "execution_kernel_live_fallback_mode",
)
RUNTIME_TRACE_METADATA_FIELDS = (
    "trace_event_count",
    "trace_output_path",
)
LIVE_PRIMARY_RUNTIME_METADATA_FIELDS = (
    "run_id",
    "status",
    "execution_mode",
    "route_engine",
    "diagnostic_python_lane_active",
)
LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS = (
    "run_id",
    "status",
    *RUNTIME_TRACE_METADATA_FIELDS,
)
LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS = (
    "execution_mode",
    "route_engine",
    "diagnostic_python_lane_active",
)
DRY_RUN_RUNTIME_METADATA_FIELDS = (
    "reason",
    EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
)
DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS = (
    *DRY_RUN_RUNTIME_METADATA_FIELDS,
    *RUNTIME_TRACE_METADATA_FIELDS,
)
COMPATIBILITY_FALLBACK_RUNTIME_METADATA_FIELDS = (
    "run_id",
    "status",
    EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
    "execution_kernel_primary",
    "execution_kernel_primary_authority",
    EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY,
    EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY,
)
COMPATIBILITY_FALLBACK_REQUIRED_RUNTIME_METADATA_FIELDS = (
    "run_id",
    "status",
    *RUNTIME_TRACE_METADATA_FIELDS,
    *COMPATIBILITY_FALLBACK_RUNTIME_METADATA_FIELDS[2:],
)
LIVE_PRIMARY_PROMPT_PREVIEW_OWNER = "rust-execution-cli"
COMPATIBILITY_FALLBACK_PROMPT_PREVIEW_OWNER = "python-agno-kernel-adapter"
DRY_RUN_PROMPT_PREVIEW_OWNER = "rust-execution-cli"
LIVE_PRIMARY_MODEL_ID_SOURCE = "aggregator-response.model"
COMPATIBILITY_FALLBACK_MODEL_ID_SOURCE = "agno-run-output.model"
EXECUTION_KERNEL_RETIRED_COMPATIBILITY_FALLBACK_MODE = "retired"


@dataclass(frozen=True, slots=True)
class ExecutionKernelCompatibilityAgentSpec:
    """Python compatibility-agent contract emitted by Rust-primary execution lanes."""

    instructions: tuple[str, ...]
    metadata: dict[str, Any]


def build_trace_runtime_metadata(
    *,
    trace_event_count: int,
    trace_output_path: str | None,
) -> dict[str, Any]:
    """Return the shared trace metadata emitted on execution-kernel responses."""

    return {
        "trace_event_count": trace_event_count,
        "trace_output_path": trace_output_path,
    }


def build_compatibility_fallback_metadata(
    *,
    primary_adapter_kind: str,
    primary_authority: str,
    error: Exception | str,
    compatibility_agent_metadata: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Return the compatibility-only metadata added when Python handles a live fallback."""

    metadata: dict[str, Any] = {
        EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY: EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
        EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY: (
            EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
        ),
        "execution_kernel_primary": primary_adapter_kind,
        "execution_kernel_primary_authority": primary_authority,
        EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY: str(error),
    }
    if compatibility_agent_metadata:
        metadata.update(dict(compatibility_agent_metadata))
    return metadata


def build_execution_kernel_compatibility_projection_metadata() -> dict[str, str]:
    """Return shared metadata for Python compatibility projections of Rust-primary execution."""

    return {
        EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY: EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
        EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY: (
            EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
        ),
    }


def build_execution_kernel_runtime_metadata(
    *,
    execution_kernel: str,
    execution_kernel_authority: str,
    trace_event_count: int,
    trace_output_path: str | None,
    extra_fields: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Return shared runtime metadata emitted on Python-owned kernel responses."""

    metadata: dict[str, Any] = {
        "execution_kernel": execution_kernel,
        "execution_kernel_authority": execution_kernel_authority,
    }
    if extra_fields:
        metadata.update(dict(extra_fields))
    metadata.update(
        build_trace_runtime_metadata(
            trace_event_count=trace_event_count,
            trace_output_path=trace_output_path,
        )
    )
    return metadata


def build_execution_kernel_live_response_serialization_contract_core() -> dict[str, Any]:
    """Return the shared execution-kernel response serialization contract core."""

    return {
        "status_contract": EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_STATUS_CONTRACT,
        "public_response_fields": [*EXECUTION_KERNEL_PUBLIC_RESPONSE_FIELDS],
        "usage_contract": {
            "fields": [*EXECUTION_KERNEL_USAGE_FIELDS],
            "live_mode": "live",
            "dry_run_mode": "estimated",
        },
        "runtime_response_metadata_fields": {
            "shared": [*RUNTIME_TRACE_METADATA_FIELDS],
            "steady_state_kernel": [*EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS],
            "live_primary": [*LIVE_PRIMARY_RUNTIME_METADATA_FIELDS],
            "dry_run": [*DRY_RUN_RUNTIME_METADATA_FIELDS],
            "retired_compatibility_fallback": [*COMPATIBILITY_FALLBACK_RUNTIME_METADATA_FIELDS],
        },
        "current_contract_truth": {
            "public_response_model": "RunTaskResponse",
            "live_primary_schema_version": EXECUTION_KERNEL_LIVE_PRIMARY_SCHEMA_VERSION,
            "live_primary_prompt_preview_owner": LIVE_PRIMARY_PROMPT_PREVIEW_OWNER,
            "steady_state_response_shapes": ["live_primary", "dry_run"],
            "retired_compatibility_fallback_prompt_preview_owner": (
                COMPATIBILITY_FALLBACK_PROMPT_PREVIEW_OWNER
            ),
            "dry_run_prompt_preview_owner": DRY_RUN_PROMPT_PREVIEW_OWNER,
            "live_primary_model_id_source": LIVE_PRIMARY_MODEL_ID_SOURCE,
            "retired_compatibility_fallback_model_id_source": (
                COMPATIBILITY_FALLBACK_MODEL_ID_SOURCE
            ),
            "compatibility_fallback_runtime_path": EXECUTION_KERNEL_RETIRED_COMPATIBILITY_FALLBACK_MODE,
            "compatibility_fallback_request_behavior": "explicit-request-rejected",
            "retired_compatibility_fallback_policy": EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY,
            "retired_compatibility_agent_contract_version": (
                EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_VERSION
            ),
            "compatibility_fallback_reason_metadata_key": (
                EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY
            ),
        },
        "current_response_shape_truth": {
            "live_primary": {
                "live_run": True,
                "usage_mode": "live",
                "content_type": "string",
                "prompt_preview_source": "rust-owned-live-prompt",
                "model_id_present": True,
                "required_metadata_fields": [
                    *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS,
                    *LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS,
                ],
                "steady_state_metadata_fields": [
                    *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS
                ],
                "pass_through_metadata_fields": [
                    *LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS
                ],
            },
            "retired_compatibility_fallback": {
                "runtime_path_available": False,
                "request_behavior": "explicit-request-rejected",
                "legacy_live_run": True,
                "legacy_usage_mode": "live",
                "legacy_content_type": "string",
                "legacy_prompt_preview_source": "python-prompt-builder",
                "legacy_model_id_present": True,
                "legacy_required_metadata_fields": [
                    *COMPATIBILITY_FALLBACK_REQUIRED_RUNTIME_METADATA_FIELDS
                ],
                "legacy_fallback_reason_present": True,
            },
            "dry_run": {
                "live_run": False,
                "usage_mode": "estimated",
                "content_type": "string",
                "prompt_preview_source": "rust-owned-dry-run-prompt",
                "model_id_present": False,
                "required_metadata_fields": [
                    *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS,
                    *DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS,
                ],
                "steady_state_metadata_fields": [
                    *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS
                ],
                "fallback_reason_present": False,
            },
        },
        "retirement_gates": {
            "response_shape_contract_externalized": True,
            "live_primary_response_contract_externalized": True,
            "compatibility_fallback_response_contract_externalized": True,
            "compatibility_fallback_runtime_path_removed": True,
            "explicit_compatibility_requests_rejected": True,
            "compatibility_live_response_serialization_still_python_owned": False,
            "runtime_control_flow_change_required_for_removal": False,
        },
    }


def resolve_execution_kernel_prompt_preview(
    *,
    prompt_preview: str | None,
    routing_result: RoutingResult,
    build_prompt: Callable[[RoutingResult], str],
) -> str:
    """Resolve the prompt preview for Python-owned dry-run and fallback paths."""

    if prompt_preview:
        return prompt_preview
    if routing_result.prompt_preview:
        return routing_result.prompt_preview
    routing_result.prompt_preview = build_prompt(routing_result)
    return routing_result.prompt_preview


def build_execution_kernel_compatibility_agent_instructions(
    *,
    routing_result: RoutingResult,
    build_prompt: Callable[[RoutingResult], str],
    prompt_preview: str | None = None,
) -> list[str]:
    """Return shared fallback-agent instructions for Python compatibility execution."""

    spec = build_execution_kernel_compatibility_agent_spec(
        routing_result=routing_result,
        build_prompt=build_prompt,
        prompt_preview=prompt_preview,
    )
    return [*spec.instructions]


def build_execution_kernel_compatibility_agent_spec(
    *,
    routing_result: RoutingResult,
    build_prompt: Callable[[RoutingResult], str],
    prompt_preview: str | None = None,
) -> ExecutionKernelCompatibilityAgentSpec:
    """Return the explicit Python compatibility-agent construction contract."""

    resolved_prompt_preview = resolve_execution_kernel_prompt_preview(
        prompt_preview=prompt_preview or routing_result.prompt_preview,
        routing_result=routing_result,
        build_prompt=build_prompt,
    )
    return ExecutionKernelCompatibilityAgentSpec(
        instructions=(resolved_prompt_preview,),
        metadata={
            **build_execution_kernel_compatibility_projection_metadata(),
            EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY: (
                EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_VERSION
            ),
            EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY: "python-agno",
            EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY: (
                COMPATIBILITY_FALLBACK_PROMPT_PREVIEW_OWNER
            ),
        },
    )


def build_execution_kernel_dry_run_response(
    *,
    session_id: str,
    user_id: str,
    skill: str,
    overlay: str | None,
    content: str,
    prompt_preview: str,
    input_tokens: int,
    output_tokens: int,
    execution_kernel: str,
    execution_kernel_authority: str,
    trace_event_count: int,
    trace_output_path: str | None,
    extra_metadata: Mapping[str, Any] | None = None,
) -> RunTaskResponse:
    """Return the shared dry-run response shape for Python-owned execution-kernel paths."""

    usage = UsageMetrics(
        input_tokens=input_tokens,
        output_tokens=output_tokens,
        total_tokens=input_tokens + output_tokens,
        mode="estimated",
    )
    return RunTaskResponse(
        session_id=session_id,
        user_id=user_id,
        skill=skill,
        overlay=overlay,
        live_run=False,
        content=content,
        usage=usage,
        prompt_preview=prompt_preview,
        model_id=None,
        metadata=build_execution_kernel_runtime_metadata(
            execution_kernel=execution_kernel,
            execution_kernel_authority=execution_kernel_authority,
            trace_event_count=trace_event_count,
            trace_output_path=trace_output_path,
            extra_fields={
                "reason": "Live model execution is disabled; returned a deterministic dry-run payload."
            }
            | dict(extra_metadata or {}),
        ),
    )


def build_execution_kernel_compatibility_live_response(
    *,
    session_id: str,
    user_id: str,
    skill: str,
    overlay: str | None,
    content: str,
    usage: UsageMetrics,
    prompt_preview: str,
    model_id: str | None,
    run_id: str,
    status: str,
    execution_kernel: str,
    execution_kernel_authority: str,
    trace_event_count: int,
    trace_output_path: str | None,
    extra_metadata: Mapping[str, Any] | None = None,
) -> RunTaskResponse:
    """Return the shared compatibility live-response shape for Python-owned execution-kernel paths."""

    return RunTaskResponse(
        session_id=session_id,
        user_id=user_id,
        skill=skill,
        overlay=overlay,
        live_run=True,
        content=content,
        usage=usage,
        prompt_preview=prompt_preview,
        model_id=model_id,
        metadata=build_execution_kernel_runtime_metadata(
            execution_kernel=execution_kernel,
            execution_kernel_authority=execution_kernel_authority,
            trace_event_count=trace_event_count,
            trace_output_path=trace_output_path,
            extra_fields={
                "run_id": run_id,
                "status": status,
            }
            | dict(extra_metadata or {}),
        ),
    )
