"""Rust-owned execution-kernel contract surfaces exposed to Python."""

from __future__ import annotations

from functools import lru_cache
from pathlib import Path
from typing import Any, Mapping

from framework_runtime.paths import default_codex_home
from framework_runtime.schemas import RunTaskResponse

EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION = "router-rs-execute-request-v1"
EXECUTION_KERNEL_RESPONSE_SCHEMA_VERSION = "router-rs-execute-response-v1"
EXECUTION_KERNEL_METADATA_SCHEMA_VERSION = "router-rs-execution-kernel-metadata-v1"
EXECUTION_KERNEL_METADATA_CONTRACT_SCHEMA_VERSION = (
    "router-rs-execution-kernel-metadata-contract-v1"
)
EXECUTION_KERNEL_CONTRACT_BUNDLE_SCHEMA_VERSION = (
    "router-rs-execution-kernel-contract-bundle-v1"
)
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
EXECUTION_KERNEL_KIND = "rust-execution-kernel-slice"
EXECUTION_KERNEL_AUTHORITY = "rust-execution-kernel-authority"
EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND = "router-rs"
EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY = "rust-execution-cli"
EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY = "rust-cli"
EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL = "router-rs"
EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_VERSION = (
    "execution-kernel-compatibility-agent-v1"
)
EXECUTION_KERNEL_LIVE_PRIMARY_SCHEMA_VERSION = "router-rs-execute-response-v1"
EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY = "live_primary"
EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN = "dry_run"
EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_STATUS_CONTRACT = (
    "execution_kernel_live_response_serialization_contract_v1"
)
LIVE_PRIMARY_PROMPT_PREVIEW_OWNER = "rust-execution-cli"
DRY_RUN_PROMPT_PREVIEW_OWNER = "rust-execution-cli"
LIVE_PRIMARY_MODEL_ID_SOURCE = "aggregator-response.model"
RUNTIME_TRACE_METADATA_FIELDS = (
    "trace_event_count",
    "trace_output_path",
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
DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS = (
    "reason",
    EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
    EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
    *RUNTIME_TRACE_METADATA_FIELDS,
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
EXECUTION_KERNEL_RUST_CANONICAL_STEADY_STATE_METADATA_FIELDS = (
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
    EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
    EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
)
EXECUTION_KERNEL_PUBLIC_RUNTIME_CONTRACT_FIELDS = (
    "execution_kernel",
    "execution_kernel_authority",
    EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
    "execution_kernel_in_process_replacement_complete",
    "execution_kernel_delegate",
    "execution_kernel_delegate_authority",
    "execution_kernel_live_primary",
    "execution_kernel_live_primary_authority",
)
EXECUTION_KERNEL_PUBLIC_RUNTIME_RESPONSE_METADATA_FIELDS = (
    "execution_kernel_delegate_family",
    "execution_kernel_delegate_impl",
)
EXECUTION_KERNEL_STEADY_STATE_METADATA_FIELDS = (
    *EXECUTION_KERNEL_RUST_CANONICAL_STEADY_STATE_METADATA_FIELDS,
)


@lru_cache(maxsize=1)
def _execution_contract_adapter() -> Any:
    from framework_runtime.rust_router import RustRouteAdapter

    return RustRouteAdapter(default_codex_home())


def _bundle() -> dict[str, Any]:
    return dict(_execution_contract_adapter().execution_contract_bundle())


def execution_contract_bundle() -> dict[str, Any]:
    return dict(_bundle())


def normalize_execution_kernel_metadata_contract(
    kernel_metadata_contract: Mapping[str, Any] | None,
) -> dict[str, Any]:
    payload = None if kernel_metadata_contract is None else dict(kernel_metadata_contract)
    return dict(_execution_contract_adapter().normalize_execution_kernel_metadata_contract(payload))


def execution_kernel_steady_state_fields() -> tuple[str, ...]:
    contract = normalize_execution_kernel_metadata_contract(None)
    return tuple(str(field) for field in contract["steady_state_fields"])


def execution_kernel_runtime_metadata_fields() -> dict[str, tuple[str, ...]]:
    contract = normalize_execution_kernel_metadata_contract(None)
    runtime_fields = dict(contract["runtime_fields"])
    return {
        str(name): tuple(str(field) for field in fields)
        for name, fields in runtime_fields.items()
    }


def resolve_execution_kernel_expectations(
    kernel_contract: Mapping[str, Any] | None = None,
) -> dict[str, str]:
    contract = dict(kernel_contract or {})

    def _field(key: str, fallback: str) -> str:
        value = contract.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
        if value is None:
            return fallback
        return str(value)

    return {
        "execution_kernel": _field("execution_kernel", EXECUTION_KERNEL_KIND),
        "execution_kernel_authority": _field(
            "execution_kernel_authority",
            EXECUTION_KERNEL_AUTHORITY,
        ),
        "execution_kernel_delegate": _field(
            "execution_kernel_delegate",
            EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
        ),
        "execution_kernel_delegate_authority": _field(
            "execution_kernel_delegate_authority",
            EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
        ),
        "execution_kernel_delegate_family": _field(
            "execution_kernel_delegate_family",
            EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
        ),
        "execution_kernel_delegate_impl": _field(
            "execution_kernel_delegate_impl",
            EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
        ),
    }


def build_trace_runtime_metadata(
    *,
    trace_event_count: int,
    trace_output_path: str | None,
) -> dict[str, Any]:
    return {
        "trace_event_count": trace_event_count,
        "trace_output_path": trace_output_path,
    }


def build_execution_kernel_runtime_metadata(
    *,
    execution_kernel: str,
    execution_kernel_authority: str,
    execution_kernel_delegate: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
    execution_kernel_delegate_authority: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    execution_kernel_delegate_family: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
    execution_kernel_delegate_impl: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
    trace_event_count: int,
    trace_output_path: str | None,
    response_shape: str = EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
    prompt_preview_owner: str | None = None,
    extra_fields: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
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
        "execution_kernel_delegate": execution_kernel_delegate,
        "execution_kernel_delegate_authority": execution_kernel_delegate_authority,
        "execution_kernel_delegate_family": execution_kernel_delegate_family,
        "execution_kernel_delegate_impl": execution_kernel_delegate_impl,
        "execution_kernel_live_primary": execution_kernel_delegate,
        "execution_kernel_live_primary_authority": execution_kernel_delegate_authority,
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
    return dict(_bundle()["live_response_serialization_contract"])


def validate_execution_kernel_steady_state_metadata(
    *,
    metadata: Mapping[str, Any],
    execution_kernel: str,
    execution_kernel_authority: str,
    execution_kernel_delegate: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
    execution_kernel_delegate_authority: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    response_shape: str | None = None,
) -> dict[str, Any]:
    kernel_contract = {
        "execution_kernel": execution_kernel,
        "execution_kernel_authority": execution_kernel_authority,
        "execution_kernel_delegate": execution_kernel_delegate,
        "execution_kernel_delegate_authority": execution_kernel_delegate_authority,
    }
    if isinstance(metadata, Mapping):
        for field in (
            "execution_kernel_delegate_family",
            "execution_kernel_delegate_impl",
            "execution_kernel_live_primary",
            "execution_kernel_live_primary_authority",
        ):
            if field in metadata:
                kernel_contract[field] = metadata[field]
    return dict(
        _execution_contract_adapter().validate_execution_kernel_steady_state_metadata(
            metadata=dict(metadata),
            kernel_contract=kernel_contract,
            response_shape=response_shape,
        )
    )


def validate_router_rs_execution_metadata(
    *,
    metadata: Mapping[str, Any],
    live_run: bool,
    usage_mode: str,
    execution_kernel: str,
    execution_kernel_authority: str,
    execution_kernel_delegate: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
    execution_kernel_delegate_authority: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    execution_kernel_delegate_family: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
    execution_kernel_delegate_impl: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
) -> dict[str, Any]:
    payload = {
        "session_id": "validation-session",
        "user_id": "validation-user",
        "skill": "validation-skill",
        "overlay": None,
        "live_run": live_run,
        "content": "",
        "usage": {
            "input_tokens": 0,
            "output_tokens": 0,
            "total_tokens": 0,
            "mode": usage_mode,
        },
        "prompt_preview": None,
        "model_id": "validation-model" if live_run else None,
        "metadata": dict(metadata),
    }
    kernel_contract = {
        "execution_kernel": execution_kernel,
        "execution_kernel_authority": execution_kernel_authority,
        "execution_kernel_delegate": execution_kernel_delegate,
        "execution_kernel_delegate_authority": execution_kernel_delegate_authority,
        "execution_kernel_delegate_family": execution_kernel_delegate_family,
        "execution_kernel_delegate_impl": execution_kernel_delegate_impl,
    }
    return dict(
        execution_kernel_response_decode(
            payload,
            kernel_contract=kernel_contract,
            dry_run=not live_run,
        ).metadata
    )


def execution_kernel_response_model_validate(payload: Mapping[str, Any]) -> RunTaskResponse:
    return RunTaskResponse.model_validate(payload)


def execution_kernel_response_decode(
    payload: Mapping[str, Any],
    *,
    kernel_contract: Mapping[str, Any] | None = None,
    dry_run: bool | None = None,
) -> RunTaskResponse:
    normalized = _execution_contract_adapter().decode_execution_response(
        dict(payload),
        kernel_contract=None if kernel_contract is None else dict(kernel_contract),
        dry_run=dry_run,
    )
    return execution_kernel_response_model_validate(normalized.model_dump(mode="json"))


def decode_router_rs_execution_response(
    payload: Mapping[str, Any],
    *,
    execution_kernel: str,
    execution_kernel_authority: str,
    execution_kernel_delegate: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
    execution_kernel_delegate_authority: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    execution_kernel_delegate_family: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
    execution_kernel_delegate_impl: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
) -> RunTaskResponse:
    return execution_kernel_response_decode(
        payload,
        kernel_contract={
            "execution_kernel": execution_kernel,
            "execution_kernel_authority": execution_kernel_authority,
            "execution_kernel_delegate": execution_kernel_delegate,
            "execution_kernel_delegate_authority": execution_kernel_delegate_authority,
            "execution_kernel_delegate_family": execution_kernel_delegate_family,
            "execution_kernel_delegate_impl": execution_kernel_delegate_impl,
        },
        dry_run=not bool(payload.get("live_run")),
    )
