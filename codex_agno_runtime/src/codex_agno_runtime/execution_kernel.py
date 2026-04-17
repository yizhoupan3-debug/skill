"""Execution-kernel adapters for the Codex Agno runtime."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
import json
import subprocess
from pathlib import Path
from typing import Any

try:  # pragma: no cover - import is environment-dependent
    from agno.run.agent import RunOutput as AgnoRunOutput
except Exception:  # pragma: no cover - local dev often runs without Agno installed
    AgnoRunOutput = None

from codex_agno_runtime.agent_factory import AgentFactory
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.prompt_builder import PromptBuilder
from codex_agno_runtime.schemas import RoutingResult, RunTaskResponse, UsageMetrics
from codex_agno_runtime.utils import estimate_tokens


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

    def health(self) -> dict[str, Any]:
        payload = super().health()
        payload.update(
            {
                "kernel_replace_ready": False,
                "kernel_live_backend_family": "rust-cli",
                "kernel_live_backend_impl": "router-rs",
                "kernel_mode_support": ["live"],
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
        completed = subprocess.run(
            command,
            cwd=self.settings.codex_home,
            capture_output=True,
            text=True,
            timeout=self.settings.rust_router_timeout_seconds,
            check=False,
        )
        if completed.returncode != 0:
            stderr = completed.stderr.strip() or completed.stdout.strip() or "unknown router-rs failure"
            raise RuntimeError(f"router-rs execute failed: {stderr}")
        try:
            parsed = json.loads(completed.stdout)
        except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
            raise RuntimeError(f"router-rs execute returned invalid JSON: {exc}") from exc
        if parsed.get("execution_schema_version") != self.execution_schema_version:
            raise RuntimeError(
                "router-rs execute returned an unknown schema: "
                f"{parsed.get('execution_schema_version')!r}"
            )
        if parsed.get("authority") != self.authority:
            raise RuntimeError(
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
    """Compatibility wrapper: Rust-first live kernel with a narrow Python fallback."""

    adapter_kind = "python-agno"
    authority = "python-agno-kernel-adapter"

    def _live_fallback_enabled(self) -> bool:
        return bool(getattr(self.settings, "rust_execute_fallback_to_python", True))

    def _live_fallback_mode(self) -> str:
        return "compatibility" if self._live_fallback_enabled() else "disabled"

    def __init__(
        self,
        settings: RuntimeSettings,
        prompt_builder: PromptBuilder,
        *,
        agent_factory: AgentFactory,
    ) -> None:
        self.settings = settings
        self.prompt_builder = prompt_builder
        self.agent_factory = agent_factory
        self._rust_live_kernel = RouterRsExecutionKernel(settings)

    async def execute(self, request: ExecutionKernelRequest) -> RunTaskResponse:
        if request.dry_run:
            prompt_preview = (
                request.prompt_preview
                or request.routing_result.prompt_preview
                or self.prompt_builder.build_prompt(request.routing_result)
            )
            return self._dry_run_response(request, prompt_preview=prompt_preview)
        live_request = ExecutionKernelRequest(
            task=request.task,
            session_id=request.session_id,
            user_id=request.user_id,
            routing_result=request.routing_result,
            prompt_preview=None,
            dry_run=False,
            trace_event_count=request.trace_event_count,
            trace_output_path=request.trace_output_path,
        )
        try:
            return await self._rust_live_kernel.execute(live_request)
        except Exception as error:
            if not self._live_fallback_enabled():
                raise RuntimeError(
                    "router-rs execute failed while Python live fallback is disabled: "
                    f"{error}"
                ) from error
            prompt_preview = (
                request.prompt_preview
                or request.routing_result.prompt_preview
                or self.prompt_builder.build_prompt(request.routing_result)
            )
            response = await self._live_run_response(live_request, prompt_preview=prompt_preview)
            response.metadata.setdefault("execution_kernel", self.adapter_kind)
            response.metadata.setdefault("execution_kernel_authority", self.authority)
            response.metadata["execution_kernel_primary"] = self._rust_live_kernel.adapter_kind
            response.metadata["execution_kernel_primary_authority"] = self._rust_live_kernel.authority
            response.metadata["execution_kernel_fallback_reason"] = str(error)
            return response

    def health(self) -> dict[str, Any]:
        payload = super().health()
        live_fallback_enabled = self._live_fallback_enabled()
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
                "kernel_live_delegate_fallback_kind": self.adapter_kind if live_fallback_enabled else None,
                "kernel_live_delegate_fallback_authority": self.authority if live_fallback_enabled else None,
                "kernel_live_delegate_fallback_family": "python" if live_fallback_enabled else None,
                "kernel_live_delegate_fallback_impl": "agno" if live_fallback_enabled else None,
                "kernel_live_fallback_enabled": live_fallback_enabled,
                "kernel_live_delegate_mode": "rust-primary",
                "kernel_live_fallback_mode": self._live_fallback_mode(),
            }
        )
        return payload

    def _dry_run_response(
        self,
        request: ExecutionKernelRequest,
        *,
        prompt_preview: str,
    ) -> RunTaskResponse:
        routing_result = request.routing_result
        input_tokens = estimate_tokens(routing_result.task + "\n" + prompt_preview)
        output_tokens = min(self.settings.default_output_tokens, 96)
        content = (
            f"[dry-run] Routed to `{routing_result.selected_skill.name}` on {routing_result.layer}. "
            f"Session `{routing_result.session_id}` is ready for kernel execution."
        )
        return RunTaskResponse(
            session_id=routing_result.session_id,
            user_id=request.user_id,
            skill=routing_result.selected_skill.name,
            overlay=routing_result.overlay_skill.name if routing_result.overlay_skill else None,
            live_run=False,
            content=content,
            usage=UsageMetrics(
                input_tokens=input_tokens,
                output_tokens=output_tokens,
                total_tokens=input_tokens + output_tokens,
                mode="estimated",
            ),
            prompt_preview=prompt_preview,
            model_id=None,
            metadata={
                "reason": "Live model execution is disabled; returned a deterministic dry-run payload.",
                "trace_event_count": request.trace_event_count,
                "trace_output_path": request.trace_output_path,
                "execution_kernel": self.adapter_kind,
                "execution_kernel_authority": self.authority,
            },
        )

    async def _live_run_response(
        self,
        request: ExecutionKernelRequest,
        *,
        prompt_preview: str,
    ) -> RunTaskResponse:
        routing_result = request.routing_result
        agent = self.agent_factory.build_compatibility_agent(routing_result, request.user_id)
        if prompt_preview:
            agent.instructions = [prompt_preview]
        run_output = await agent.arun(
            request.task,
            session_id=request.session_id,
            user_id=request.user_id,
            stream=False,
        )
        if AgnoRunOutput is not None and not isinstance(run_output, AgnoRunOutput):
            raise TypeError("Expected Agno to return a RunOutput object.")
        return RunTaskResponse(
            session_id=routing_result.session_id,
            user_id=request.user_id,
            skill=routing_result.selected_skill.name,
            overlay=routing_result.overlay_skill.name if routing_result.overlay_skill else None,
            live_run=True,
            content=run_output.get_content_as_string() or "",
            usage=self._serialize_metrics(getattr(run_output, "metrics", None)),
            prompt_preview=prompt_preview,
            model_id=run_output.model,
            metadata={
                "run_id": run_output.run_id,
                "status": run_output.status.value if hasattr(run_output.status, "value") else str(run_output.status),
                "trace_event_count": request.trace_event_count,
                "trace_output_path": request.trace_output_path,
                "execution_kernel": self.adapter_kind,
                "execution_kernel_authority": self.authority,
            },
        )

    @staticmethod
    def _serialize_metrics(metrics: Any | None) -> UsageMetrics:
        """Normalize Agno metrics into the public API schema."""

        if metrics is None:
            return UsageMetrics(input_tokens=0, output_tokens=0, total_tokens=0, mode="live")
        return UsageMetrics(
            input_tokens=metrics.input_tokens,
            output_tokens=metrics.output_tokens,
            total_tokens=metrics.total_tokens,
            mode="live",
        )
