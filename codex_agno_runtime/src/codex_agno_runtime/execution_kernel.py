"""Execution-kernel adapters for the Codex Agno runtime."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any

from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.execution_kernel_contracts import (
    EXECUTION_KERNEL_BRIDGE_AUTHORITY,
    EXECUTION_KERNEL_BRIDGE_KIND,
    EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY,
    EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
    EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
    EXECUTION_KERNEL_REQUEST_SCHEMA_VERSION,
    decode_router_rs_execution_response,
)
from codex_agno_runtime.rust_router import RustRouteAdapter
from codex_agno_runtime.schemas import RoutingResult, RunTaskResponse


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
    prompt_preview: str | None = None
    dry_run: bool = False
    trace_event_count: int = 0
    trace_output_path: str | None = None
    sandbox_policy: SandboxExecutionPolicy = field(default_factory=SandboxExecutionPolicy)
    sandbox_budget: SandboxResourceBudget = field(default_factory=SandboxResourceBudget)
    sandbox_tool_category: str = "workspace_mutating"
    sandbox_runtime_probe: SandboxRuntimeProbe | None = None


class ExecutionKernel:
    """Base execution-kernel adapter contract."""

    adapter_kind = "unknown"
    authority = "unknown"

    async def execute(self, request: ExecutionKernelRequest) -> RunTaskResponse:
        """Run one execution request through the active kernel."""

        raise NotImplementedError

    def health(self) -> dict[str, Any]:
        """Describe the active kernel adapter."""

        return {
            "kernel_adapter_kind": self.adapter_kind,
            "kernel_authority": self.authority,
        }


class RouterRsExecutionError(RuntimeError):
    """Base error raised when router-rs execution cannot complete."""


class RouterRsInfrastructureError(RouterRsExecutionError):
    """Router-rs failed before a valid execution result could be produced."""


class RouterRsExecutionKernel(ExecutionKernel):
    """Rust-owned execution slice invoked out-of-process through router-rs."""

    adapter_kind = EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL
    authority = EXECUTION_KERNEL_PRIMARY_DELEGATE_AUTHORITY
    bridge_kind = EXECUTION_KERNEL_BRIDGE_KIND
    bridge_authority = EXECUTION_KERNEL_BRIDGE_AUTHORITY
    execution_schema_version = RustRouteAdapter.execution_schema_version

    def __init__(
        self,
        settings: RuntimeSettings,
        *,
        rust_adapter: RustRouteAdapter | None = None,
    ) -> None:
        self.settings = settings
        self._rust_adapter = rust_adapter or RustRouteAdapter(
            self.settings.codex_home,
            timeout_seconds=self.settings.rust_router_timeout_seconds,
        )

    async def execute(self, request: ExecutionKernelRequest) -> RunTaskResponse:
        payload = self._build_payload(request)
        response_payload = await asyncio.to_thread(self._run_execute_command, payload)
        return self._decode_response(response_payload)

    def preview_prompt(self, request: ExecutionKernelRequest) -> str | None:
        """Synchronously ask router-rs for the dry-run prompt preview."""

        if not request.dry_run:
            raise ValueError("preview_prompt requires a dry-run execution request")
        payload = self._build_payload(request)
        response_payload = self._run_execute_command(payload)
        return self._decode_response(response_payload).prompt_preview

    def health(self) -> dict[str, Any]:
        payload = super().health()
        adapter_health = self._rust_adapter.health()
        payload.update(
            {
                "kernel_replace_ready": False,
                "kernel_live_backend_family": "rust-cli",
                "kernel_live_backend_impl": "router-rs",
                "kernel_mode_support": ["dry_run", "live"],
                "execution_schema_version": self.execution_schema_version,
                "resolved_binary": adapter_health.get("resolved_binary"),
            }
        )
        return payload

    def _build_payload(self, request: ExecutionKernelRequest) -> dict[str, Any]:
        routing_result = request.routing_result
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
            "reasons": [str(reason) for reason in routing_result.reasons],
            "prompt_preview": request.prompt_preview if request.dry_run else None,
            "dry_run": request.dry_run,
            "trace_event_count": request.trace_event_count,
            "trace_output_path": request.trace_output_path,
            "default_output_tokens": self.settings.default_output_tokens,
            "model_id": self.settings.model_id,
            "aggregator_base_url": self.settings.aggregator_base_url,
            "aggregator_api_key": self.settings.aggregator_api_key,
        }

    def _run_execute_command(self, payload: dict[str, Any]) -> dict[str, Any]:
        try:
            return self._rust_adapter.execute(payload)
        except RuntimeError as exc:
            message = str(exc)
            if message.startswith("router-rs execute failed:"):
                raise RouterRsExecutionError(message) from exc
            raise RouterRsInfrastructureError(message) from exc

    def _decode_response(self, payload: dict[str, Any]) -> RunTaskResponse:
        try:
            return decode_router_rs_execution_response(
                payload,
                execution_kernel=self.bridge_kind,
                execution_kernel_authority=self.bridge_authority,
                execution_kernel_delegate=self.adapter_kind,
                execution_kernel_delegate_authority=self.authority,
                execution_kernel_delegate_family=EXECUTION_KERNEL_PRIMARY_DELEGATE_FAMILY,
                execution_kernel_delegate_impl=EXECUTION_KERNEL_PRIMARY_DELEGATE_IMPL,
            )
        except RuntimeError as exc:
            raise RouterRsInfrastructureError(str(exc)) from exc
