"""Shared execution-kernel contract helpers used by runtime and descriptor layers."""

from __future__ import annotations

from typing import Any, Mapping

from .schemas import RunTaskResponse, UsageMetrics

EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION = "router-rs-execute-request-v1"
EXECUTION_KERNEL_METADATA_SCHEMA_VERSION = "router-rs-execution-kernel-metadata-v1"
EXECUTION_KERNEL_METADATA_BRIDGE_SCHEMA_VERSION = (
    "router-rs-execution-kernel-metadata-bridge-v1"
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
EXECUTION_KERNEL_BRIDGE_KIND = "rust-execution-kernel-slice"
EXECUTION_KERNEL_BRIDGE_AUTHORITY = "rust-execution-kernel-authority"
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
LIVE_PRIMARY_PROMPT_PREVIEW_OWNER = "rust-execution-cli"
DRY_RUN_PROMPT_PREVIEW_OWNER = "rust-execution-cli"
LIVE_PRIMARY_MODEL_ID_SOURCE = "aggregator-response.model"


def _normalize_runtime_field_group(
    runtime_fields_payload: Mapping[str, Any] | None,
    *,
    name: str,
    fallback: tuple[str, ...],
) -> tuple[str, ...]:
    payload = fallback if runtime_fields_payload is None else runtime_fields_payload.get(name, fallback)
    if not isinstance(payload, (list, tuple)) or any(
        not isinstance(field, str) or not field.strip() for field in payload
    ):
        raise RuntimeError(
            "execution-kernel metadata bridge returned an invalid runtime field group: "
            f"{name}={payload!r}"
        )
    return tuple(str(field) for field in payload)


def _normalize_execution_kernel_runtime_fields(
    runtime_fields_payload: Mapping[str, Any] | None,
) -> dict[str, tuple[str, ...]]:
    return {
        "shared": _normalize_runtime_field_group(
            runtime_fields_payload,
            name="shared",
            fallback=RUNTIME_TRACE_METADATA_FIELDS,
        ),
        "live_primary_required": _normalize_runtime_field_group(
            runtime_fields_payload,
            name="live_primary_required",
            fallback=LIVE_PRIMARY_REQUIRED_RUNTIME_METADATA_FIELDS,
        ),
        "live_primary_passthrough": _normalize_runtime_field_group(
            runtime_fields_payload,
            name="live_primary_passthrough",
            fallback=LIVE_PRIMARY_PASSTHROUGH_RUNTIME_METADATA_FIELDS,
        ),
        "dry_run_required": _normalize_runtime_field_group(
            runtime_fields_payload,
            name="dry_run_required",
            fallback=DRY_RUN_REQUIRED_RUNTIME_METADATA_FIELDS,
        ),
    }


def resolve_execution_kernel_expectations(
    kernel_contract: Mapping[str, Any] | None = None,
) -> dict[str, str]:
    """Resolve the Rust-owned execution-kernel identity contract for response decode."""

    contract = kernel_contract or {}

    def _field(key: str, fallback: str) -> str:
        value = contract.get(key)
        if value is None:
            return fallback
        if isinstance(value, str):
            stripped = value.strip()
            if stripped:
                return stripped
            return fallback
        return str(value)

    return {
        "execution_kernel": _field("execution_kernel", EXECUTION_KERNEL_BRIDGE_KIND),
        "execution_kernel_authority": _field(
            "execution_kernel_authority",
            EXECUTION_KERNEL_BRIDGE_AUTHORITY,
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


def normalize_execution_kernel_metadata_bridge(
    metadata_bridge: Mapping[str, Any] | None,
) -> dict[str, Any]:
    """Resolve the Rust-owned metadata bridge, falling back to compatibility defaults."""

    if metadata_bridge is None:
        return {
            "steady_state_fields": EXECUTION_KERNEL_RUST_CANONICAL_STEADY_STATE_METADATA_FIELDS,
            "runtime_fields": _normalize_execution_kernel_runtime_fields(None),
            "metadata_keys": {
                "metadata_schema_version": (
                    EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY
                ),
                "contract_mode": EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
                "fallback_policy": EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
                "response_shape": EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
                "prompt_preview_owner": (
                    EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY
                ),
                "model_id_source": EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
            },
            "defaults": {
                "contract_mode": EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
                "fallback_policy": EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY,
                "prompt_preview_owner_by_mode": {
                    EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY: (
                        LIVE_PRIMARY_PROMPT_PREVIEW_OWNER
                    ),
                    EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN: DRY_RUN_PROMPT_PREVIEW_OWNER,
                },
                "live_primary_model_id_source": LIVE_PRIMARY_MODEL_ID_SOURCE,
                "supported_response_shapes": (
                    EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
                    EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
                ),
            },
        }

    steady_state_fields = metadata_bridge.get("steady_state_fields")
    if not isinstance(steady_state_fields, (list, tuple)) or any(
        not isinstance(field, str) or not field.strip() for field in steady_state_fields
    ):
        raise RuntimeError("execution-kernel metadata bridge is missing steady_state_fields.")
    metadata_keys_payload = metadata_bridge.get("metadata_keys")
    if not isinstance(metadata_keys_payload, Mapping):
        raise RuntimeError("execution-kernel metadata bridge is missing metadata_keys.")
    defaults_payload = metadata_bridge.get("defaults")
    if not isinstance(defaults_payload, Mapping):
        raise RuntimeError("execution-kernel metadata bridge is missing defaults.")
    runtime_fields_payload = metadata_bridge.get("runtime_fields")
    if runtime_fields_payload is not None and not isinstance(runtime_fields_payload, Mapping):
        raise RuntimeError("execution-kernel metadata bridge returned invalid runtime_fields.")
    prompt_preview_owner_by_mode = defaults_payload.get("prompt_preview_owner_by_mode")
    if not isinstance(prompt_preview_owner_by_mode, Mapping):
        raise RuntimeError(
            "execution-kernel metadata bridge is missing defaults.prompt_preview_owner_by_mode."
        )

    def _bridge_key(name: str, fallback: str) -> str:
        value = metadata_keys_payload.get(name, fallback)
        if not isinstance(value, str) or not value.strip():
            raise RuntimeError(
                f"execution-kernel metadata bridge returned an invalid metadata key: {name}={value!r}"
            )
        return value

    def _bridge_default(name: str, fallback: str) -> str:
        value = defaults_payload.get(name, fallback)
        if not isinstance(value, str) or not value.strip():
            raise RuntimeError(
                f"execution-kernel metadata bridge returned an invalid default: {name}={value!r}"
            )
        return value

    supported_response_shapes = defaults_payload.get("supported_response_shapes")
    if not isinstance(supported_response_shapes, (list, tuple)) or any(
        not isinstance(shape, str) or not shape.strip() for shape in supported_response_shapes
    ):
        raise RuntimeError(
            "execution-kernel metadata bridge is missing defaults.supported_response_shapes."
        )

    normalized_prompt_owners: dict[str, str] = {}
    for shape in supported_response_shapes:
        owner = prompt_preview_owner_by_mode.get(shape)
        if not isinstance(owner, str) or not owner.strip():
            raise RuntimeError(
                "execution-kernel metadata bridge returned an invalid prompt preview owner: "
                f"{shape}={owner!r}"
            )
        normalized_prompt_owners[str(shape)] = owner

    return {
        "steady_state_fields": tuple(str(field) for field in steady_state_fields),
        "runtime_fields": _normalize_execution_kernel_runtime_fields(runtime_fields_payload),
        "metadata_keys": {
            "metadata_schema_version": _bridge_key(
                "metadata_schema_version",
                EXECUTION_KERNEL_METADATA_SCHEMA_VERSION_METADATA_KEY,
            ),
            "contract_mode": _bridge_key(
                "contract_mode",
                EXECUTION_KERNEL_CONTRACT_MODE_METADATA_KEY,
            ),
            "fallback_policy": _bridge_key(
                "fallback_policy",
                EXECUTION_KERNEL_FALLBACK_POLICY_METADATA_KEY,
            ),
            "response_shape": _bridge_key(
                "response_shape",
                EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
            ),
            "prompt_preview_owner": _bridge_key(
                "prompt_preview_owner",
                EXECUTION_KERNEL_PROMPT_PREVIEW_OWNER_METADATA_KEY,
            ),
            "model_id_source": _bridge_key(
                "model_id_source",
                EXECUTION_KERNEL_MODEL_ID_SOURCE_METADATA_KEY,
            ),
        },
        "defaults": {
            "contract_mode": _bridge_default(
                "contract_mode",
                EXECUTION_KERNEL_RUST_PRIMARY_CONTRACT_MODE,
            ),
            "fallback_policy": _bridge_default(
                "fallback_policy",
                EXECUTION_KERNEL_COMPATIBILITY_FALLBACK_POLICY,
            ),
            "prompt_preview_owner_by_mode": normalized_prompt_owners,
            "live_primary_model_id_source": _bridge_default(
                "live_primary_model_id_source",
                LIVE_PRIMARY_MODEL_ID_SOURCE,
            ),
            "supported_response_shapes": tuple(str(shape) for shape in supported_response_shapes),
        },
    }


def execution_kernel_steady_state_fields(
    metadata_bridge: Mapping[str, Any] | None,
) -> tuple[str, ...]:
    """Return the canonical steady-state metadata field list for one bridge payload."""

    bridge = normalize_execution_kernel_metadata_bridge(metadata_bridge)
    return tuple(str(field) for field in bridge["steady_state_fields"])


def execution_kernel_runtime_metadata_fields(
    metadata_bridge: Mapping[str, Any] | None,
) -> dict[str, tuple[str, ...]]:
    """Return the Rust-owned runtime metadata field groups for one bridge payload."""

    bridge = normalize_execution_kernel_metadata_bridge(metadata_bridge)
    runtime_fields = bridge["runtime_fields"]
    return {
        str(name): tuple(str(field) for field in fields)
        for name, fields in dict(runtime_fields).items()
    }


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
    """Return Rust canonical steady-state metadata for projection responses."""

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


def build_execution_kernel_live_response_serialization_contract_core(
    metadata_bridge: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Return the shared execution-kernel response serialization contract core."""

    bridge = normalize_execution_kernel_metadata_bridge(metadata_bridge)
    defaults = dict(bridge["defaults"])
    steady_state_fields = list(execution_kernel_steady_state_fields(bridge))
    runtime_fields = execution_kernel_runtime_metadata_fields(bridge)
    return {
        "status_contract": EXECUTION_KERNEL_LIVE_RESPONSE_SERIALIZATION_STATUS_CONTRACT,
        "public_response_fields": [*EXECUTION_KERNEL_PUBLIC_RESPONSE_FIELDS],
        "usage_contract": {
            "fields": [*EXECUTION_KERNEL_USAGE_FIELDS],
            "live_mode": "live",
            "dry_run_mode": "estimated",
        },
        "runtime_response_metadata_fields": {
            "shared": [*runtime_fields["shared"]],
            "steady_state_kernel": [*steady_state_fields],
            "live_primary": [*runtime_fields["live_primary_required"]],
            "dry_run": [*runtime_fields["dry_run_required"]],
        },
        "current_contract_truth": {
            "public_response_model": "RunTaskResponse",
            "execution_request_schema_version": EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION,
            "live_primary_schema_version": EXECUTION_KERNEL_LIVE_PRIMARY_SCHEMA_VERSION,
            "steady_state_metadata_schema_version": EXECUTION_KERNEL_METADATA_SCHEMA_VERSION,
            "live_primary_prompt_preview_owner": defaults["prompt_preview_owner_by_mode"][
                EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
            ],
            "steady_state_response_shapes": [*defaults["supported_response_shapes"]],
            "dry_run_prompt_preview_owner": defaults["prompt_preview_owner_by_mode"][
                EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
            ],
            "live_primary_model_id_source": defaults["live_primary_model_id_source"],
        },
        "current_response_shape_truth": {
            "live_primary": {
                "live_run": True,
                "usage_mode": "live",
                "content_type": "string",
                "prompt_preview_source": "rust-owned-live-prompt",
                "model_id_present": True,
                "required_metadata_fields": [
                    *steady_state_fields,
                    *runtime_fields["live_primary_required"],
                ],
                "steady_state_metadata_fields": [*steady_state_fields],
                "pass_through_metadata_fields": [
                    *runtime_fields["live_primary_passthrough"]
                ],
            },
            "dry_run": {
                "live_run": False,
                "usage_mode": "estimated",
                "content_type": "string",
                "prompt_preview_source": "rust-owned-dry-run-prompt",
                "model_id_present": False,
                "required_metadata_fields": [
                    *steady_state_fields,
                    *runtime_fields["dry_run_required"],
                ],
                "steady_state_metadata_fields": [*steady_state_fields],
                "fallback_reason_present": False,
            },
        },
        "retirement_gates": {
            "response_shape_contract_externalized": True,
            "live_primary_response_contract_externalized": True,
            "compatibility_live_response_serialization_still_python_owned": False,
            "runtime_control_flow_change_required_for_removal": False,
        },
    }

def validate_execution_kernel_steady_state_metadata(
    *,
    metadata: Mapping[str, Any],
    execution_kernel: str,
    execution_kernel_authority: str,
    execution_kernel_delegate: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
    execution_kernel_delegate_authority: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    response_shape: str | None = None,
    metadata_bridge: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Validate the steady-state execution-kernel metadata owned by Rust."""

    bridge = normalize_execution_kernel_metadata_bridge(metadata_bridge)
    steady_state_fields = tuple(bridge["steady_state_fields"])
    metadata_keys = dict(bridge["metadata_keys"])
    defaults = dict(bridge["defaults"])
    normalized = dict(metadata)
    missing = [field for field in steady_state_fields if field not in normalized]
    if missing:
        raise RuntimeError(
            "execution-kernel steady-state metadata is incomplete: "
            + ", ".join(sorted(missing))
        )

    response_shape_key = metadata_keys["response_shape"]
    actual_shape = normalized.get(response_shape_key)
    if response_shape is None:
        response_shape = str(actual_shape)
    supported_shapes = tuple(defaults["supported_response_shapes"])
    if response_shape not in supported_shapes:
        raise RuntimeError(
            "execution-kernel steady-state metadata returned an unsupported response_shape: "
            f"{actual_shape!r}"
        )
    expected_prompt_preview_owner = defaults["prompt_preview_owner_by_mode"][response_shape]
    expected_pairs = {
        metadata_keys["metadata_schema_version"]: (
            EXECUTION_KERNEL_METADATA_SCHEMA_VERSION
        ),
        "execution_kernel": execution_kernel,
        "execution_kernel_authority": execution_kernel_authority,
        metadata_keys["contract_mode"]: defaults["contract_mode"],
        metadata_keys["fallback_policy"]: defaults["fallback_policy"],
        "execution_kernel_in_process_replacement_complete": True,
        "execution_kernel_delegate": execution_kernel_delegate,
        "execution_kernel_delegate_authority": execution_kernel_delegate_authority,
        response_shape_key: response_shape,
        metadata_keys["prompt_preview_owner"]: expected_prompt_preview_owner,
    }
    for field, expected in expected_pairs.items():
        if normalized.get(field) != expected:
            raise RuntimeError(
                "execution-kernel steady-state metadata returned an unexpected value: "
                f"{field}={normalized.get(field)!r}"
            )
    for field in (
        "execution_kernel_delegate_family",
        "execution_kernel_delegate_impl",
        "execution_kernel_live_primary",
        "execution_kernel_live_primary_authority",
    ):
        value = normalized.get(field)
        if not isinstance(value, str) or not value.strip():
            raise RuntimeError(
                "execution-kernel steady-state metadata returned an invalid value: "
                f"{field}={value!r}"
            )
    for field in (
        EXECUTION_KERNEL_FALLBACK_REASON_METADATA_KEY,
        EXECUTION_KERNEL_COMPATIBILITY_AGENT_CONTRACT_METADATA_KEY,
        EXECUTION_KERNEL_COMPATIBILITY_AGENT_KIND_METADATA_KEY,
        EXECUTION_KERNEL_COMPATIBILITY_AGENT_AUTHORITY_METADATA_KEY,
    ):
        if field in normalized:
            raise RuntimeError(
                "execution-kernel steady-state metadata returned a retired compatibility "
                f"fallback field: {field}"
            )
    return normalized


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
    metadata_bridge: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Validate one Rust-owned execution response metadata payload."""

    bridge = normalize_execution_kernel_metadata_bridge(metadata_bridge)
    metadata_keys = dict(bridge["metadata_keys"])
    defaults = dict(bridge["defaults"])
    normalized = validate_execution_kernel_steady_state_metadata(
        metadata=metadata,
        execution_kernel=execution_kernel,
        execution_kernel_authority=execution_kernel_authority,
        execution_kernel_delegate=execution_kernel_delegate,
        execution_kernel_delegate_authority=execution_kernel_delegate_authority,
        response_shape=(
            EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
            if live_run
            else EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
        ),
        metadata_bridge=bridge,
    )
    runtime_fields = execution_kernel_runtime_metadata_fields(bridge)
    required_fields = runtime_fields[
        "live_primary_required" if live_run else "dry_run_required"
    ]
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

    expected_execution_mode = "live" if live_run else "dry_run"
    expected_pairs = {
        "execution_mode": expected_execution_mode,
    }
    if live_run:
        expected_pairs[metadata_keys["model_id_source"]] = defaults[
            "live_primary_model_id_source"
        ]
    for field, expected in expected_pairs.items():
        if normalized.get(field) != expected:
            raise RuntimeError(
                "router-rs execute returned an unexpected metadata value: "
                f"{field}={normalized.get(field)!r}"
            )
    return normalized


def decode_router_rs_execution_response(
    payload: Mapping[str, Any],
    *,
    execution_kernel: str,
    execution_kernel_authority: str,
    execution_kernel_delegate: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
    execution_kernel_delegate_authority: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    execution_kernel_delegate_family: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
    execution_kernel_delegate_impl: str = EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
    metadata_bridge: Mapping[str, Any] | None = None,
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
        execution_kernel_delegate=execution_kernel_delegate,
        execution_kernel_delegate_authority=execution_kernel_delegate_authority,
        execution_kernel_delegate_family=execution_kernel_delegate_family,
        execution_kernel_delegate_impl=execution_kernel_delegate_impl,
        metadata_bridge=metadata_bridge,
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
