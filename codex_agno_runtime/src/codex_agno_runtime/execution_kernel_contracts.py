"""Shared execution-kernel contract helpers used by runtime and descriptor layers."""

from __future__ import annotations

from typing import Any, Mapping

from .schemas import RunTaskResponse, UsageMetrics

EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION = "router-rs-execute-request-v1"
EXECUTION_KERNEL_METADATA_SCHEMA_VERSION = "router-rs-execution-kernel-metadata-v1"
EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY = "execution_kernel_fallback_reason"
EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY = "execution_kernel_contract_mode"
EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY = "execution_kernel_fallback_policy"
EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY = (
    "execution_kernel_metadata_schema_version"
)
EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY = "execution_kernel_response_shape"
EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY = (
    "execution_kernel_prompt_preview_owner"
)
EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY = "execution_kernel_model_id_source"
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
EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY = "live_primary"
EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN = "dry_run"
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
    EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY,
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
    EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
    EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
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
    "diagnostic_route_mode",
    EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
)
LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS = (
    "run_id",
    "status",
    EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
    *RUNTIME_TRACE_METADATA_FIELDS,
)
LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS = (
    "execution_mode",
    "route_engine",
    "diagnostic_route_mode",
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


def build_execution_kernel_runtime_metadata(
    *,
    execution_kernel: str,
    execution_kernel_authority: str,
    trace_event_count: int,
    trace_output_path: str | None,
    response_shape: str = EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
    prompt_preview_owner: str | None = None,
    extra_fields: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Return shared runtime metadata emitted on Python-owned kernel responses."""

    if response_shape not in (
        EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
        EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
    ):
        raise ValueError(f"unsupported execution-kernel response_shape: {response_shape!r}")
    if prompt_preview_owner is None:
        prompt_preview_owner = (
            LIVE_PRIMARY_PROMPT_PREVIEW_OWNER
            if response_shape == EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
            else DRY_RUN_PROMPT_PREVIEW_OWNER
        )

    metadata: dict[str, Any] = {
        EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY: (
            EXECUTION_KERNEL_METADATA_SCHEMA_VERSION
        ),
        "execution_kernel": execution_kernel,
        "execution_kernel_authority": execution_kernel_authority,
        EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY: EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
        EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY: (
            EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
        ),
        "execution_kernel_in_process_replacement_complete": True,
        "execution_kernel_delegate": execution_kernel,
        "execution_kernel_delegate_authority": execution_kernel_authority,
        "execution_kernel_delegate_family": "rust-cli",
        "execution_kernel_delegate_impl": execution_kernel,
        "execution_kernel_live_primary": execution_kernel,
        "execution_kernel_live_primary_authority": execution_kernel_authority,
        "execution_kernel_live_fallback": None,
        "execution_kernel_live_fallback_authority": None,
        "execution_kernel_live_fallback_enabled": False,
        "execution_kernel_live_fallback_mode": "disabled",
        EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY: response_shape,
        EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY: prompt_preview_owner,
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
            "execution_request_schema_version": EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION,
            "live_primary_schema_version": EXECUTION_KERNEL_LIVE_PRIMARY_SCHEMA_VERSION,
            "steady_state_metadata_schema_version": EXECUTION_KERNEL_METADATA_SCHEMA_VERSION,
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
            "compatibility_fallback_request_behavior": "surface-removed",
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
                "request_behavior": "surface-removed",
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
        usage=usage.model_dump(mode="json"),
        prompt_preview=prompt_preview,
        model_id=None,
        metadata=build_execution_kernel_runtime_metadata(
            execution_kernel=execution_kernel,
            execution_kernel_authority=execution_kernel_authority,
            trace_event_count=trace_event_count,
            trace_output_path=trace_output_path,
            response_shape=EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
            extra_fields={
                "reason": "Live model execution is disabled; returned a deterministic dry-run payload."
            }
            | dict(extra_metadata or {}),
        ),
    )


def validate_router_rs_execution_metadata(
    *,
    metadata: Mapping[str, Any],
    live_run: bool,
    usage_mode: str,
    execution_kernel: str,
    execution_kernel_authority: str,
) -> dict[str, Any]:
    """Validate one Rust-owned execution response metadata payload."""

    normalized = dict(metadata)
    required_fields = (
        *EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS,
        *(
            LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS
            if live_run
            else DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS
        ),
    )
    missing = [field for field in required_fields if field not in normalized]
    if missing:
        raise RuntimeError(
            "router-rs execute returned incomplete metadata: " + ", ".join(sorted(missing))
        )

    expected_usage_mode = "live" if live_run else "estimated"
    if usage_mode != expected_usage_mode:
        raise RuntimeError(
            "router-rs execute returned an unexpected usage mode: "
            f"{usage_mode!r} != {expected_usage_mode!r}"
        )

    expected_shape = (
        EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
        if live_run
        else EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
    )
    expected_execution_mode = "live" if live_run else "dry_run"
    expected_prompt_preview_owner = (
        LIVE_PRIMARY_PROMPT_PREVIEW_OWNER
        if live_run
        else DRY_RUN_PROMPT_PREVIEW_OWNER
    )
    expected_pairs = {
        EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY: (
            EXECUTION_KERNEL_METADATA_SCHEMA_VERSION
        ),
        "execution_kernel": execution_kernel,
        "execution_kernel_authority": execution_kernel_authority,
        EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY: EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
        EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY: (
            EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY
        ),
        "execution_kernel_in_process_replacement_complete": True,
        "execution_kernel_delegate": execution_kernel,
        "execution_kernel_delegate_authority": execution_kernel_authority,
        "execution_kernel_delegate_family": "rust-cli",
        "execution_kernel_delegate_impl": execution_kernel,
        "execution_kernel_live_primary": execution_kernel,
        "execution_kernel_live_primary_authority": execution_kernel_authority,
        "execution_kernel_live_fallback_enabled": False,
        "execution_kernel_live_fallback_mode": "disabled",
        EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY: expected_shape,
        EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY: expected_prompt_preview_owner,
        "execution_mode": expected_execution_mode,
    }
    if live_run:
        expected_pairs[EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY] = (
            LIVE_PRIMARY_MODEL_ID_SOURCE
        )
    for field, expected in expected_pairs.items():
        if normalized.get(field) != expected:
            raise RuntimeError(
                "router-rs execute returned an unexpected metadata value: "
                f"{field}={normalized.get(field)!r}"
            )
    if normalized.get("execution_kernel_live_fallback") is not None:
        raise RuntimeError("router-rs execute returned an unexpected live fallback marker.")
    if normalized.get("execution_kernel_live_fallback_authority") is not None:
        raise RuntimeError("router-rs execute returned an unexpected live fallback authority.")
    return normalized


def decode_router_rs_execution_response(
    payload: Mapping[str, Any],
    *,
    execution_kernel: str,
    execution_kernel_authority: str,
) -> RunTaskResponse:
    """Decode one validated router-rs execute payload into ``RunTaskResponse``."""

    usage_payload = dict(payload.get("usage") or {})
    live_run = bool(payload["live_run"])
    metadata = validate_router_rs_execution_metadata(
        metadata=payload.get("metadata") or {},
        live_run=live_run,
        usage_mode=str(usage_payload.get("mode", "live")),
        execution_kernel=execution_kernel,
        execution_kernel_authority=execution_kernel_authority,
    )
    return RunTaskResponse(
        session_id=str(payload["session_id"]),
        user_id=str(payload["user_id"]),
        skill=str(payload["skill"]),
        overlay=str(payload["overlay"]) if payload.get("overlay") is not None else None,
        live_run=live_run,
        content=str(payload.get("content", "")),
        usage=UsageMetrics(
            input_tokens=int(usage_payload.get("input_tokens", 0)),
            output_tokens=int(usage_payload.get("output_tokens", 0)),
            total_tokens=int(usage_payload.get("total_tokens", 0)),
            mode=str(usage_payload.get("mode", "live")),
        ),
        prompt_preview=(
            str(payload.get("prompt_preview"))
            if payload.get("prompt_preview") is not None
            else None
        ),
        model_id=str(payload.get("model_id")) if payload.get("model_id") is not None else None,
        metadata=metadata,
    )
