"""Execution-kernel adapters for the Codex Agno runtime."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any, Mapping

from framework_runtime.config import RuntimeSettings
from framework_runtime.execution_kernel_contracts import (
    EXECUTION_KERNEL_BRIDGE_AUTHORITY,
    EXECUTION_KERNEL_BRIDGE_KIND,
    EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
    EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
    EXECUTION_KERNEL_PRIMARY_DELEGATE_KIND,
    EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION,
    EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
    decode_router_rs_execution_response,
    normalize_execution_kernel_metadata_bridge,
    resolve_execution_kernel_expectations,
    validate_execution_kernel_steady_state_metadata,
)
from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.schemas import RoutingResult, RunTaskResponse


SANDBOX_CAPABILITY_CATEGORIES = (
    "read_only",
    "workspace_mutating",
    "networked",
    "high_risk",
)

@dataclass(slots=True, frozen=True)
class SandboxExecutionPolicy:
    """Explicit sandbox capability policy carried with one kernel request."""

    profile: str = "workspace-default"
    capability_categories: tuple[str, ...] = ("read_only", "workspace_mutating", "networked")
    dedicated_profile: bool = False
    reusable: bool = True
    schema_version: str = "runtime-sandbox-policy-v1"

    def to_metadata(self) -> dict[str, Any]:
        """Serialize the policy for response metadata and event logs."""

        return {
            "schema_version": self.schema_version,
            "profile": self.profile,
            "capability_categories": list(self.capability_categories),
            "dedicated_profile": self.dedicated_profile,
            "reusable": self.reusable,
        }


@dataclass(slots=True, frozen=True)
class SandboxResourceBudget:
    """Runtime sandbox resource budgets attached to one execution."""

    cpu: float = 30.0
    memory: int = 512 * 1024 * 1024
    wall_clock: float = 30.0
    output_size: int = 64 * 1024
    schema_version: str = "runtime-sandbox-budget-v1"

    def to_metadata(self) -> dict[str, Any]:
        """Serialize the budget for response metadata and event logs."""

        return {
            "schema_version": self.schema_version,
            "cpu": self.cpu,
            "memory": self.memory,
            "wall_clock": self.wall_clock,
            "output_size": self.output_size,
        }


@dataclass(slots=True, frozen=True)
class SandboxRuntimeProbe:
    """Optional runtime measurements supplied by the host around kernel execution."""

    cpu: float | None = None
    memory: int | None = None
    wall_clock: float | None = None
    output_size: int | None = None
    source: str = "host-runtime"
    schema_version: str = "runtime-sandbox-runtime-probe-v1"

    def to_metadata(self) -> dict[str, Any]:
        """Serialize the runtime probe for metadata and logs."""

        return {
            "schema_version": self.schema_version,
            "cpu": self.cpu,
            "memory": self.memory,
            "wall_clock": self.wall_clock,
            "output_size": self.output_size,
            "source": self.source,
        }


@dataclass(slots=True)
class ExecutionKernelRequest:
    """Normalized execution payload passed to the active kernel adapter."""

    task: str
    session_id: str
    user_id: str
    routing_result: RoutingResult
    job_id: str | None = None
    dry_run: bool = False
    trace_event_count: int = 0
    trace_output_path: str | None = None
    sandbox_policy: SandboxExecutionPolicy = field(default_factory=SandboxExecutionPolicy)
    sandbox_budget: SandboxResourceBudget = field(default_factory=SandboxResourceBudget)
    sandbox_tool_category: str = "workspace_mutating"
    sandbox_runtime_probe: SandboxRuntimeProbe | None = None


class RouterRsExecutionError(RuntimeError):
    """Base error raised when router-rs execution cannot complete."""


class RouterRsInfrastructureError(RouterRsExecutionError):
    """Router-rs failed before a valid execution result could be produced."""


def build_router_rs_execution_request_payload(
    request: ExecutionKernelRequest,
    *,
    settings: RuntimeSettings,
) -> dict[str, Any]:
    """Serialize one execution request into the Rust router-rs request payload."""

    routing_result = request.routing_result
    route_snapshot = routing_result.route_snapshot
    snapshot_reasons: list[str] = []
    if route_snapshot is not None:
        if hasattr(route_snapshot, "reasons"):
            snapshot_reasons = [str(reason) for reason in route_snapshot.reasons]
        elif isinstance(route_snapshot, dict):
            snapshot_reasons = [str(reason) for reason in route_snapshot.get("reasons") or []]
    prompt_reasons = (
        snapshot_reasons
        if snapshot_reasons
        else [str(reason) for reason in routing_result.reasons]
    )
    return {
        "schema_version": EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION,
        "task": request.task,
        "session_id": request.session_id,
        "user_id": request.user_id,
        "selected_skill": routing_result.selected_skill.name,
        "overlay_skill": routing_result.overlay_skill.name if routing_result.overlay_skill else None,
        "layer": routing_result.layer,
        "route_engine": routing_result.route_engine,
        "diagnostic_route_mode": routing_result.diagnostic_route_mode,
        "reasons": prompt_reasons,
        # Prompt construction is Rust-owned on both steady-state paths.
        "prompt_preview": None,
        "dry_run": request.dry_run,
        "trace_event_count": request.trace_event_count,
        "trace_output_path": request.trace_output_path,
        "default_output_tokens": settings.default_output_tokens,
        "model_id": settings.model_id,
        "aggregator_base_url": settings.aggregator_base_url,
        "aggregator_api_key": settings.aggregator_api_key,
    }


def decode_router_rs_execution_payload(
    payload: Mapping[str, Any],
    *,
    kernel_contract: Mapping[str, Any] | None = None,
    metadata_bridge: Mapping[str, Any] | None = None,
) -> RunTaskResponse:
    """Decode one router-rs execution payload through the shared bridge contract."""

    return decode_router_rs_execution_payload_with_contract(
        payload,
        kernel_contract=kernel_contract,
        metadata_bridge=metadata_bridge,
    )


def decode_router_rs_execution_payload_with_contract(
    payload: Mapping[str, Any],
    *,
    kernel_contract: Mapping[str, Any] | None = None,
    metadata_bridge: Mapping[str, Any] | None = None,
) -> RunTaskResponse:
    """Decode one router-rs payload against the Rust-owned kernel contract bundle."""

    resolved_bridge = (
        normalize_execution_kernel_metadata_bridge(metadata_bridge)
        if metadata_bridge is not None
        else None
    )
    expectations = resolve_execution_kernel_expectations(kernel_contract)
    return decode_router_rs_execution_response(
        payload,
        execution_kernel=expectations["execution_kernel"],
        execution_kernel_authority=expectations["execution_kernel_authority"],
        execution_kernel_delegate=expectations["execution_kernel_delegate"],
        execution_kernel_delegate_authority=expectations["execution_kernel_delegate_authority"],
        execution_kernel_delegate_family=expectations["execution_kernel_delegate_family"],
        execution_kernel_delegate_impl=expectations["execution_kernel_delegate_impl"],
        metadata_bridge=resolved_bridge,
    )


def _resolve_runtime_execution_contract_bundle(
    *,
    rust_adapter: RustRouteAdapter,
    dry_run: bool,
    kernel_contract: Mapping[str, Any] | None = None,
    metadata_bridge: Mapping[str, Any] | None = None,
) -> tuple[dict[str, Any] | None, dict[str, Any] | None]:
    """Resolve the Rust-owned execution-kernel contract bundle for one response shape."""

    resolved_contract = dict(kernel_contract) if isinstance(kernel_contract, Mapping) else None
    resolved_bridge = (
        normalize_execution_kernel_metadata_bridge(metadata_bridge)
        if metadata_bridge is not None
        else None
    )
    if resolved_contract is not None and resolved_bridge is not None:
        response_shape = (
            EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
            if dry_run
            else EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
        )
        expectations = resolve_execution_kernel_expectations(resolved_contract)
        validated_contract = validate_execution_kernel_steady_state_metadata(
            metadata=resolved_contract,
            execution_kernel=expectations["execution_kernel"],
            execution_kernel_authority=expectations["execution_kernel_authority"],
            execution_kernel_delegate=expectations["execution_kernel_delegate"],
            execution_kernel_delegate_authority=expectations["execution_kernel_delegate_authority"],
            response_shape=response_shape,
            metadata_bridge=resolved_bridge,
        )
        return dict(validated_contract), resolved_bridge

    control_plane_descriptor = rust_adapter.runtime_control_plane()
    services = control_plane_descriptor.get("services")
    if not isinstance(services, Mapping):
        raise RuntimeError("runtime control plane is missing services.")
    service_descriptor = services.get("execution")
    if not isinstance(service_descriptor, Mapping):
        raise RuntimeError("runtime control plane is missing execution service descriptor.")

    if resolved_bridge is None:
        bridge_payload = service_descriptor.get("kernel_metadata_bridge")
        if bridge_payload is not None:
            if not isinstance(bridge_payload, Mapping):
                raise RuntimeError(
                    "runtime control plane execution descriptor returned an invalid kernel_metadata_bridge."
                )
            resolved_bridge = normalize_execution_kernel_metadata_bridge(bridge_payload)

    if resolved_contract is None:
        response_shape = (
            EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
            if dry_run
            else EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
        )
        contract_modes = service_descriptor.get("kernel_contract_by_mode")
        contract_payload = (
            contract_modes.get(response_shape)
            if isinstance(contract_modes, Mapping)
            else None
        )
        if not isinstance(contract_payload, Mapping):
            raise RuntimeError(
                "runtime control plane execution descriptor is missing "
                f"kernel_contract_by_mode.{response_shape}."
            )
        expectations = resolve_execution_kernel_expectations(contract_payload)
        resolved_contract = validate_execution_kernel_steady_state_metadata(
            metadata=contract_payload,
            execution_kernel=expectations["execution_kernel"],
            execution_kernel_authority=expectations["execution_kernel_authority"],
            execution_kernel_delegate=expectations["execution_kernel_delegate"],
            execution_kernel_delegate_authority=expectations["execution_kernel_delegate_authority"],
            response_shape=response_shape,
            metadata_bridge=resolved_bridge,
        )

    return (
        dict(resolved_contract) if resolved_contract is not None else None,
        resolved_bridge,
    )


def run_router_rs_execution_payload(
    payload: dict[str, Any],
    *,
    rust_adapter: RustRouteAdapter,
) -> dict[str, Any]:
    """Run one serialized execution payload through the shared Rust adapter."""

    try:
        return rust_adapter.execute(payload)
    except RuntimeError as exc:
        message = str(exc)
        if message.startswith("router-rs execute failed:"):
            raise RouterRsExecutionError(message) from exc
        raise RouterRsInfrastructureError(message) from exc


async def execute_router_rs_request(
    request: ExecutionKernelRequest,
    *,
    settings: RuntimeSettings,
    rust_adapter: RustRouteAdapter,
    kernel_contract: Mapping[str, Any] | None = None,
    metadata_bridge: Mapping[str, Any] | None = None,
) -> RunTaskResponse:
    """Execute one normalized request through router-rs and decode the result."""

    payload = build_router_rs_execution_request_payload(request, settings=settings)
    response_payload = await asyncio.to_thread(
        run_router_rs_execution_payload,
        payload,
        rust_adapter=rust_adapter,
    )
    try:
        resolved_contract, resolved_bridge = await asyncio.to_thread(
            _resolve_runtime_execution_contract_bundle,
            rust_adapter=rust_adapter,
            dry_run=request.dry_run,
            kernel_contract=kernel_contract,
            metadata_bridge=metadata_bridge,
        )
        return decode_router_rs_execution_payload_with_contract(
            response_payload,
            kernel_contract=resolved_contract,
            metadata_bridge=resolved_bridge,
        )
    except RuntimeError as exc:
        raise RouterRsInfrastructureError(str(exc)) from exc


def preview_router_rs_request_prompt(
    request: ExecutionKernelRequest,
    *,
    settings: RuntimeSettings,
    rust_adapter: RustRouteAdapter,
    kernel_contract: Mapping[str, Any] | None = None,
    metadata_bridge: Mapping[str, Any] | None = None,
) -> str | None:
    """Synchronously resolve the Rust-owned dry-run prompt preview for one request."""

    if not request.dry_run:
        raise ValueError("preview_prompt requires a dry-run execution request")
    payload = build_router_rs_execution_request_payload(request, settings=settings)
    response_payload = run_router_rs_execution_payload(payload, rust_adapter=rust_adapter)
    try:
        resolved_contract, resolved_bridge = _resolve_runtime_execution_contract_bundle(
            rust_adapter=rust_adapter,
            dry_run=True,
            kernel_contract=kernel_contract,
            metadata_bridge=metadata_bridge,
        )
        return decode_router_rs_execution_payload_with_contract(
            response_payload,
            kernel_contract=resolved_contract,
            metadata_bridge=resolved_bridge,
        ).prompt_preview
    except RuntimeError as exc:
        raise RouterRsInfrastructureError(str(exc)) from exc
