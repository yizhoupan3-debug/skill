"""Execution-kernel adapters for the Codex Agno runtime."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
import json
import subprocess
from pathlib import Path
from typing import Any

from codex_agno_runtime.agent_factory import AgentFactory
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.execution_kernel_contracts import (
    resolve_execution_kernel_prompt_preview,
)
from codex_agno_runtime.prompt_builder import PromptBuilder
from codex_agno_runtime.schemas import RoutingResult, RunTaskResponse, UsageMetrics


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

    adapter_kind = "router-rs"
    authority = "rust-execution-cli"
    execution_schema_version = "router-rs-execute-response-v1"

    def __init__(self, settings: RuntimeSettings) -> None:
        self.settings = settings
        self.router_dir = self.settings.codex_home / "scripts" / "router-rs"
        self.release_bin = self.router_dir / "target" / "release" / "router-rs"
        self.debug_bin = self.router_dir / "target" / "debug" / "router-rs"

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
        payload.update(
            {
                "kernel_replace_ready": False,
                "kernel_live_backend_family": "rust-cli",
                "kernel_live_backend_impl": "router-rs",
                "kernel_mode_support": ["dry_run", "live"],
                "execution_schema_version": self.execution_schema_version,
                "resolved_binary": str(self._resolved_binary()) if self._resolved_binary() is not None else None,
            }
        )
        return payload

    def _resolved_binary(self) -> Path | None:
        for candidate in (self.release_bin, self.debug_bin):
            if candidate.is_file():
                return candidate
        return None

    def _binary_command(self) -> list[str]:
        resolved_binary = self._resolved_binary()
        if resolved_binary is not None:
            return [str(resolved_binary)]
        return [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(self.router_dir / "Cargo.toml"),
            "--",
        ]

    def _build_payload(self, request: ExecutionKernelRequest) -> dict[str, Any]:
        routing_result = request.routing_result
        return {
            "schema_version": "router-rs-execute-request-v1",
            "task": request.task,
            "session_id": request.session_id,
            "user_id": request.user_id,
            "selected_skill": routing_result.selected_skill.name,
            "overlay_skill": routing_result.overlay_skill.name if routing_result.overlay_skill else None,
            "layer": routing_result.layer,
            "route_engine": routing_result.route_engine,
            "rollback_to_python": routing_result.rollback_to_python,
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
        command = [
            *self._binary_command(),
            "--execute-json",
            "--execute-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        try:
            completed = subprocess.run(
                command,
                cwd=self.settings.codex_home,
                capture_output=True,
                text=True,
                timeout=self.settings.rust_router_timeout_seconds,
                check=False,
            )
        except subprocess.TimeoutExpired as exc:
            raise RouterRsInfrastructureError(
                "router-rs execute timed out before returning a response"
            ) from exc
        except OSError as exc:
            raise RouterRsInfrastructureError(
                f"router-rs execute could not be launched: {exc}"
            ) from exc
        if completed.returncode != 0:
            stderr = completed.stderr.strip() or completed.stdout.strip() or "unknown router-rs failure"
            raise RouterRsExecutionError(f"router-rs execute failed: {stderr}")
        try:
            parsed = json.loads(completed.stdout)
        except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
            raise RouterRsInfrastructureError(f"router-rs execute returned invalid JSON: {exc}") from exc
        if parsed.get("execution_schema_version") != self.execution_schema_version:
            raise RouterRsInfrastructureError(
                "router-rs execute returned an unknown schema: "
                f"{parsed.get('execution_schema_version')!r}"
            )
        if parsed.get("authority") != self.authority:
            raise RouterRsInfrastructureError(
                "router-rs execute returned an unexpected authority marker: "
                f"{parsed.get('authority')!r}"
            )
        return parsed

    def _decode_response(self, payload: dict[str, Any]) -> RunTaskResponse:
        usage_payload = payload.get("usage") or {}
        metadata = dict(payload.get("metadata") or {})
        metadata.setdefault("execution_kernel", self.adapter_kind)
        metadata.setdefault("execution_kernel_authority", self.authority)
        return RunTaskResponse(
            session_id=str(payload["session_id"]),
            user_id=str(payload["user_id"]),
            skill=str(payload["skill"]),
            overlay=str(payload["overlay"]) if payload.get("overlay") is not None else None,
            live_run=bool(payload["live_run"]),
            content=str(payload.get("content", "")),
            usage=UsageMetrics(
                input_tokens=int(usage_payload.get("input_tokens", 0)),
                output_tokens=int(usage_payload.get("output_tokens", 0)),
                total_tokens=int(usage_payload.get("total_tokens", 0)),
                mode=str(usage_payload.get("mode", "live")),
            ),
            prompt_preview=str(payload.get("prompt_preview")) if payload.get("prompt_preview") is not None else None,
            model_id=str(payload.get("model_id")) if payload.get("model_id") is not None else None,
            metadata=metadata,
        )


class PythonAgnoExecutionKernel(ExecutionKernel):
    """Thin Python projection over the Rust-owned execution kernel."""

    adapter_kind = "python-agno"
    authority = "python-agno-kernel-adapter"

    @staticmethod
    def _compatibility_fallback_requested(settings: RuntimeSettings) -> bool:
        return bool(getattr(settings, "rust_execute_fallback_to_python", False))

    def _live_fallback_enabled(self) -> bool:
        return False

    def _live_fallback_mode(self) -> str:
        return "retired"

    def live_fallback_enabled(self) -> bool:
        """Expose that the compatibility live fallback is no longer available."""

        return False

    def __init__(
        self,
        settings: RuntimeSettings,
        prompt_builder: PromptBuilder,
        *,
        agent_factory: AgentFactory,
    ) -> None:
        self.settings = settings
        self.prompt_builder = prompt_builder
        # Keep the Python-side handle only as a retired compatibility-agent contract
        # surface for older callers. The live kernel no longer delegates to it.
        self.agent_factory = agent_factory
        self._rust_live_kernel = RouterRsExecutionKernel(settings)

    def _retired_fallback_error(self, *, phase: str, error: Exception | str) -> RuntimeError:
        if self._compatibility_fallback_requested(self.settings):
            return RuntimeError(
                f"router-rs {phase} failed and the requested Python compatibility fallback "
                f"is rejected after retirement: {error}"
            )
        return RuntimeError(
            f"router-rs {phase} failed after Python compatibility fallback retirement: {error}"
        )

    async def execute(self, request: ExecutionKernelRequest) -> RunTaskResponse:
        if request.dry_run:
            prompt_preview = resolve_execution_kernel_prompt_preview(
                prompt_preview=request.prompt_preview,
                routing_result=request.routing_result,
                build_prompt=self.prompt_builder.build_prompt,
            )
            dry_run_request = ExecutionKernelRequest(
                task=request.task,
                session_id=request.session_id,
                user_id=request.user_id,
                routing_result=request.routing_result,
                prompt_preview=prompt_preview,
                dry_run=True,
                trace_event_count=request.trace_event_count,
                trace_output_path=request.trace_output_path,
                sandbox_policy=request.sandbox_policy,
                sandbox_budget=request.sandbox_budget,
                sandbox_tool_category=request.sandbox_tool_category,
                sandbox_runtime_probe=request.sandbox_runtime_probe,
            )
            try:
                return await self._rust_live_kernel.execute(dry_run_request)
            except Exception as error:
                raise self._retired_fallback_error(phase="dry-run execution", error=error) from error
        live_request = ExecutionKernelRequest(
            task=request.task,
            session_id=request.session_id,
            user_id=request.user_id,
            routing_result=request.routing_result,
            prompt_preview=None,
            dry_run=False,
            trace_event_count=request.trace_event_count,
            trace_output_path=request.trace_output_path,
            sandbox_policy=request.sandbox_policy,
            sandbox_budget=request.sandbox_budget,
            sandbox_tool_category=request.sandbox_tool_category,
            sandbox_runtime_probe=request.sandbox_runtime_probe,
        )
        try:
            return await self._rust_live_kernel.execute(live_request)
        except Exception as error:
            raise self._retired_fallback_error(phase="live execution", error=error) from error

    async def execute_live_fallback(
        self,
        request: ExecutionKernelRequest,
        *,
        error: Exception | str,
    ) -> RunTaskResponse:
        """Reject the retired Python compatibility fallback lane."""

        raise self._retired_fallback_error(phase="live execution", error=error)

    def health(self) -> dict[str, Any]:
        payload = super().health()
        compatibility_request_rejected = self._compatibility_fallback_requested(self.settings)
        payload.update(
            {
                "kernel_replace_ready": True,
                "kernel_live_backend_family": "rust-cli",
                "kernel_live_backend_impl": "router-rs",
                "kernel_mode_support": ["dry_run", "live"],
                "kernel_live_delegate_primary_kind": self._rust_live_kernel.adapter_kind,
                "kernel_live_delegate_primary_authority": self._rust_live_kernel.authority,
                "kernel_live_delegate_primary_family": "rust-cli",
                "kernel_live_delegate_primary_impl": "router-rs",
                "kernel_live_delegate_fallback_kind": None,
                "kernel_live_delegate_fallback_authority": None,
                "kernel_live_delegate_fallback_family": None,
                "kernel_live_delegate_fallback_impl": None,
                "kernel_live_fallback_enabled": False,
                "kernel_live_delegate_mode": "rust-primary-only",
                "kernel_live_fallback_mode": self._live_fallback_mode(),
                "kernel_live_fallback_retired": True,
                "kernel_live_fallback_request_status": (
                    "explicit-request-rejected"
                    if compatibility_request_rejected
                    else "not-supported"
                ),
            }
        )
        return payload
