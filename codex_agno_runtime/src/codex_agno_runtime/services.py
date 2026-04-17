"""Service boundaries for the Codex Agno runtime."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping

from codex_agno_runtime.agent_factory import AgentFactory
from codex_agno_runtime.checkpoint_store import RuntimeCheckpointer
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.execution_kernel import ExecutionKernelRequest, PythonAgnoExecutionKernel
from codex_agno_runtime.host_adapters import (
    build_delegation_contract,
    build_execution_controller_contract,
    build_supervisor_state_contract,
)
from codex_agno_runtime.memory import FactMemoryStore
from codex_agno_runtime.middleware import MiddlewareContext
from codex_agno_runtime.prompt_builder import PromptBuilder
from codex_agno_runtime.router import SkillRouter
from codex_agno_runtime.rust_router import RustRouteAdapter
from codex_agno_runtime.schemas import (
    BackgroundRunStatus,
    RouteDecisionSnapshot,
    RouteExecutionPolicy,
    RouteDiffReport,
    RoutingResult,
    RunTaskResponse,
)
from codex_agno_runtime.skill_loader import SkillLoader
from codex_agno_runtime.state import BackgroundJobStore
from codex_agno_runtime.trace import InMemoryRuntimeEventBridge, RuntimeEventHandoff, RuntimeEventStreamChunk
from codex_agno_runtime.trace import RuntimeEventTransport

_KERNEL_CONTRACT_FIELDS = (
    "execution_kernel",
    "execution_kernel_authority",
    "execution_kernel_contract_mode",
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


class RouterService:
    """Own skill loading plus route-engine selection."""

    def __init__(self, settings: RuntimeSettings) -> None:
        self.settings = settings
        self.loader = SkillLoader(settings.codex_home / "skills")
        self.prompt_builder = PromptBuilder(loader=self.loader)
        self._rust_adapter = RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        self.skills = []
        self._python_router: SkillRouter | None = None
        self._last_route_report: RouteDiffReport | None = None
        self._route_policy: RouteExecutionPolicy | None = None
        self.reload()

    def startup(self) -> None:
        """Reload skills for a fresh runtime session."""

        self.reload()

    def shutdown(self) -> None:
        """Router service shutdown hook."""

    def reload(self) -> None:
        """Refresh runtime skill metadata and the Python router."""

        self.skills = self.loader.load(
            refresh=True,
            load_bodies=not self.settings.progressive_skill_loading,
        )
        policy = self._resolve_route_policy(refresh=True)
        if self._python_router is not None or self._python_router_required(policy=policy):
            self._python_router = SkillRouter(self.skills)
        else:
            self._python_router = None

    def route(self, *, task: str, session_id: str, allow_overlay: bool, first_turn: bool) -> RoutingResult:
        """Return the configured route decision for one task."""

        self._last_route_report = None
        policy = self._resolve_route_policy()
        python_result = (
            self._route_python(
                task=task,
                session_id=session_id,
                allow_overlay=allow_overlay,
                first_turn=first_turn,
            )
            if policy.python_route_required
            else None
        )
        if policy.primary_authority == "python" and policy.shadow_engine is None:
            if python_result is None:
                raise RuntimeError("Python router is required in python mode.")
            return self._decorate_route_result(
                python_result,
                route_engine="python",
                rollback_to_python=False,
                report=None,
            )

        rust_result = self._route_rust(
            task=task,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )
        report = (
            self._build_route_diff_report(
                mode=policy.mode,
                python_result=python_result,
                rust_result=rust_result,
                rollback_active=policy.rollback_active,
            )
            if policy.diff_report_required
            else None
        )
        self._last_route_report = report
        if policy.verify_parity_required:
            if report is None:
                raise RuntimeError("Rust route policy requires a parity report.")
            self._assert_parity(report)
        if policy.primary_authority == "python":
            if python_result is None:
                raise RuntimeError("Python router is required by the active Rust route policy.")
            return self._decorate_route_result(
                python_result,
                route_engine="python",
                rollback_to_python=policy.rollback_active,
                report=report,
            )
        return self._decorate_route_result(
            rust_result,
            route_engine="rust",
            rollback_to_python=False,
            report=report,
        )

    def health(self) -> dict[str, Any]:
        """Describe router-service health and the active route engine."""

        policy = self._resolve_route_policy()
        return {
            "mode": self.settings.route_engine_mode,
            "rollback_to_python": policy.rollback_active,
            "configured_rollback_to_python": self.settings.rust_route_rollback_to_python,
            "loaded_skill_count": len(self.skills),
            "skill_root": str(self.settings.codex_home / "skills"),
            "primary_authority": policy.primary_authority,
            "route_result_engine": policy.route_result_engine,
            "shadow_engine": policy.shadow_engine,
            "python_router_loaded": self._python_router is not None,
            "python_router_required": self._python_router_required(policy=policy),
            "route_policy": policy.model_dump(mode="json"),
            "rust_adapter": self._rust_adapter.health(),
            "last_route_report": self._last_route_report.model_dump(mode="json") if self._last_route_report else None,
        }

    def _route_python(self, *, task: str, session_id: str, allow_overlay: bool, first_turn: bool) -> RoutingResult:
        result = self._ensure_python_router().route(
            task=task,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )
        if result.route_snapshot is None:
            result = result.model_copy(
                update={
                    "route_snapshot": RouteDecisionSnapshot.model_validate(
                        self._rust_adapter.route_snapshot(
                            engine="python",
                            selected_skill=result.selected_skill.name,
                            overlay_skill=result.overlay_skill.name if result.overlay_skill else None,
                            layer=result.layer,
                            score=float(result.score),
                            reasons=[str(reason) for reason in result.reasons],
                        )
                    )
                }
            )
        return result

    def _route_rust(self, *, task: str, session_id: str, allow_overlay: bool, first_turn: bool) -> RoutingResult:
        decision = self._rust_adapter.route(
            query=task,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )
        selected = next(skill for skill in self.skills if skill.name == decision["selected_skill"])
        overlay = next((skill for skill in self.skills if skill.name == decision["overlay_skill"]), None)
        route_snapshot = (
            RouteDecisionSnapshot.model_validate(decision["route_snapshot"])
            if decision.get("route_snapshot") is not None
            else None
        )
        return RoutingResult(
            task=task,
            session_id=session_id,
            selected_skill=selected,
            overlay_skill=overlay,
            score=float(decision.get("score", 0.0)),
            layer=str(decision["layer"]),
            reasons=[str(reason) for reason in decision.get("reasons", [])],
            route_snapshot=route_snapshot,
        )

    def _decorate_route_result(
        self,
        result: RoutingResult,
        *,
        route_engine: str,
        rollback_to_python: bool,
        report: RouteDiffReport | None,
    ) -> RoutingResult:
        return result.model_copy(
            update={
                "route_engine": route_engine,
                "rollback_to_python": rollback_to_python,
                "shadow_route_report": report,
            }
        )

    def _build_route_diff_report(
        self,
        *,
        mode: str,
        python_result: RoutingResult | None,
        rust_result: RoutingResult,
        rollback_active: bool,
    ) -> RouteDiffReport:
        if python_result is None:
            raise RuntimeError("Python route result is required for diff reporting.")
        python_snapshot = self._build_route_snapshot("python", python_result)
        rust_snapshot = self._build_route_snapshot("rust", rust_result)
        payload = self._rust_adapter.route_report(
            mode=mode,
            python_route_snapshot=python_snapshot.model_dump(mode="json"),
            rust_route_snapshot=rust_snapshot.model_dump(mode="json"),
            rollback_active=rollback_active,
        )
        return RouteDiffReport.model_validate(payload)

    def _build_route_snapshot(self, engine: str, result: RoutingResult) -> RouteDecisionSnapshot:
        if result.route_snapshot is not None:
            return result.route_snapshot
        return RouteDecisionSnapshot.model_validate(
            self._rust_adapter.route_snapshot(
                engine=engine,
                selected_skill=result.selected_skill.name,
                overlay_skill=result.overlay_skill.name if result.overlay_skill else None,
                layer=result.layer,
                score=float(result.score),
                reasons=[str(reason) for reason in result.reasons],
            )
        )

    def _assert_parity(self, report: RouteDiffReport) -> None:
        if report.selected_skill_match and report.overlay_skill_match and report.layer_match:
            return
        if report.mismatch:
            raise RuntimeError(
                "Rust route parity mismatch: "
                f"python={report.python.selected_skill}/{report.python.overlay_skill}/{report.python.layer}/{report.python.score_bucket}/{report.python.reasons_class} "
                f"rust={report.rust.selected_skill}/{report.rust.overlay_skill}/{report.rust.layer}/{report.rust.score_bucket}/{report.rust.reasons_class}"
            )

    def _resolve_route_policy(self, *, refresh: bool = False) -> RouteExecutionPolicy:
        if refresh or self._route_policy is None:
            self._route_policy = RouteExecutionPolicy.model_validate(
                self._rust_adapter.route_policy(
                    mode=self.settings.route_engine_mode,
                    rollback_to_python=self.settings.rust_route_rollback_to_python,
                )
            )
        return self._route_policy

    def _python_router_required(self, *, policy: RouteExecutionPolicy | None = None) -> bool:
        return (policy or self._resolve_route_policy()).python_route_required

    def _ensure_python_router(self) -> SkillRouter:
        if self._python_router is None:
            self._python_router = SkillRouter(self.skills)
        return self._python_router


class StateService:
    """Own durable background-job state and session reservations."""

    def __init__(self, checkpointer: RuntimeCheckpointer) -> None:
        self.checkpointer = checkpointer
        self.state_path = checkpointer.describe_paths().background_state_path
        self.store = BackgroundJobStore(
            state_path=self.state_path,
            storage_backend=getattr(checkpointer, "storage_backend", None),
        )

    def startup(self) -> None:
        """State service startup hook."""

    def shutdown(self) -> None:
        """State service shutdown hook."""

    def set_status(self, job_id: str, **kwargs: Any) -> BackgroundRunStatus:
        return self.store.set_status(job_id, **kwargs)

    def get(self, job_id: str) -> BackgroundRunStatus | None:
        return self.store.get(job_id)

    def snapshot(self) -> dict[str, BackgroundRunStatus]:
        return self.store.snapshot()

    def get_active_job(self, session_id: str) -> str | None:
        return self.store.get_active_job(session_id)

    def health(self) -> dict[str, Any]:
        return {
            "checkpoint_backend_family": self.checkpointer.storage_capabilities().backend_family,
            "state_path": str(self.state_path),
            "job_count": len(self.store.snapshot()),
            "pending_session_takeovers": self.store.pending_session_takeovers(),
        }


class TraceService:
    """Own trace recorder wiring and filesystem paths."""

    def __init__(self, checkpointer: RuntimeCheckpointer) -> None:
        self.checkpointer = checkpointer
        paths = checkpointer.describe_paths()
        self.output_path = paths.trace_output_path
        self.event_stream_path = paths.event_stream_path
        self.resume_manifest_path = paths.resume_manifest_path
        self.event_transport_dir = paths.event_transport_dir
        self.event_bridge = InMemoryRuntimeEventBridge()
        self.recorder = checkpointer.build_trace_recorder(event_bridge=self.event_bridge)
        self.event_bridge.seed(self.recorder.stream_events())

    def startup(self) -> None:
        """Trace service startup hook."""

    def shutdown(self) -> None:
        """Trace service shutdown hook."""
        self.event_bridge.cleanup()

    def subscribe(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
        after_event_id: str | None = None,
        limit: int | None = None,
        heartbeat: bool = False,
    ) -> RuntimeEventStreamChunk:
        """Return one event-bridge delivery window for a subscriber."""

        # Cleanup drops the in-memory cache only; replayable stream state reseeds it on demand.
        self.event_bridge.seed(self.recorder.stream_events(session_id=session_id, job_id=job_id))
        return self.event_bridge.subscribe(
            session_id=session_id,
            job_id=job_id,
            after_event_id=after_event_id,
            limit=limit,
            heartbeat=heartbeat,
        )

    def cleanup_stream(self, *, session_id: str | None = None, job_id: str | None = None) -> None:
        """Release cached bridge events for one stream or for the whole service."""

        self.event_bridge.cleanup(session_id=session_id, job_id=job_id)

    def describe_transport(self, *, session_id: str, job_id: str | None = None) -> RuntimeEventTransport:
        """Describe the host-facing transport binding for one runtime stream."""

        latest_cursor = self.recorder.latest_cursor(session_id=session_id, job_id=job_id)
        stream_key = job_id or session_id
        transport = RuntimeEventTransport(
            stream_id=f"stream::{stream_key}",
            session_id=session_id,
            job_id=job_id,
            binding_backend_family=self.checkpointer.storage_capabilities().backend_family,
            binding_artifact_path=(
                str(path)
                if (path := self.checkpointer.transport_binding_path(session_id=session_id, job_id=job_id)) is not None
                else None
            ),
            latest_cursor=latest_cursor,
        )
        self.checkpointer.write_transport_binding(transport)
        return transport

    def describe_handoff(self, *, session_id: str, job_id: str | None = None) -> RuntimeEventHandoff:
        """Describe the durable handoff surface for one runtime event stream."""

        paths = self.checkpointer.describe_paths()
        transport = self.describe_transport(session_id=session_id, job_id=job_id)
        return RuntimeEventHandoff(
            stream_id=transport.stream_id,
            session_id=session_id,
            job_id=job_id,
            checkpoint_backend_family=self.checkpointer.storage_capabilities().backend_family,
            trace_stream_path=str(paths.event_stream_path) if paths.event_stream_path is not None else None,
            resume_manifest_path=str(paths.resume_manifest_path) if paths.resume_manifest_path is not None else None,
            transport=transport,
        )

    def checkpoint(
        self,
        *,
        session_id: str,
        job_id: str | None,
        status: str,
        artifact_paths: list[str],
        supervisor_projection: dict[str, Any] | None = None,
    ) -> None:
        """Persist the runtime resume checkpoint through the configured backend."""

        transport = self.describe_transport(session_id=session_id, job_id=job_id)
        resolved_artifact_paths = list(artifact_paths)
        if transport.binding_artifact_path is not None and transport.binding_artifact_path not in resolved_artifact_paths:
            resolved_artifact_paths.append(transport.binding_artifact_path)
        self.checkpointer.checkpoint(
            session_id=session_id,
            job_id=job_id,
            status=status,
            generation=self.recorder.current_generation(),
            latest_cursor=self.recorder.latest_cursor(session_id=session_id, job_id=job_id),
            event_transport_path=transport.binding_artifact_path,
            artifact_paths=resolved_artifact_paths,
            supervisor_projection=supervisor_projection,
        )

    def health(self) -> dict[str, Any]:
        paths = self.checkpointer.describe_paths()
        return {
            "checkpoint_backend_family": self.checkpointer.storage_capabilities().backend_family,
            "trace_output_path": str(paths.trace_output_path) if paths.trace_output_path is not None else None,
            "event_stream_path": str(paths.event_stream_path) if paths.event_stream_path is not None else None,
            "resume_manifest_path": (
                str(paths.resume_manifest_path) if paths.resume_manifest_path is not None else None
            ),
            "event_transport_dir": str(paths.event_transport_dir),
            "background_state_path": str(paths.background_state_path),
            "trace_event_schema_version": self.recorder.event_schema_version,
            "trace_metadata_schema_version": self.recorder.metadata_schema_version,
            "replay_supported": self.recorder.describe_stream()["replay_supported"],
            "event_bridge_supported": True,
            "event_bridge_schema_version": self.event_bridge.schema_version,
        }


class MemoryService:
    """Own memory store lifecycle and health surface."""

    def __init__(self, settings: RuntimeSettings) -> None:
        self.store = FactMemoryStore(
            memory_dir=settings.resolved_memory_dir,
            debounce_seconds=settings.memory_debounce_seconds,
        )
        self.memory_dir = settings.resolved_memory_dir

    def startup(self) -> None:
        """Memory service startup hook."""

    def shutdown(self) -> None:
        """Memory service shutdown hook."""

    def health(self) -> dict[str, Any]:
        return {
            "memory_dir": str(self.memory_dir),
        }


class _RustExecutionKernelAuthorityAdapter:
    """Present a Rust-owned kernel seam while live fallback remains compatibility-safe."""

    adapter_kind = "rust-execution-kernel-slice"
    authority = "rust-execution-kernel-authority"

    def __init__(self, delegate: PythonAgnoExecutionKernel) -> None:
        self._delegate = delegate

    @staticmethod
    def _contract_mode() -> str:
        return "rust-live-primary"

    async def execute(self, request: ExecutionKernelRequest) -> RunTaskResponse:
        response = await self._delegate.execute(request)
        delegate_health = self._delegate.health()
        if request.dry_run:
            delegate_kind = delegate_health["kernel_adapter_kind"]
            delegate_authority = delegate_health["kernel_authority"]
        else:
            delegate_kind = str(
                response.metadata.get("execution_kernel")
                or delegate_health.get("kernel_live_delegate_primary_kind")
                or delegate_health["kernel_adapter_kind"]
            )
            delegate_authority = str(
                response.metadata.get("execution_kernel_authority")
                or delegate_health.get("kernel_live_delegate_primary_authority")
                or delegate_health["kernel_authority"]
            )
        response.metadata.update(
            {
                "execution_kernel": self.adapter_kind,
                "execution_kernel_authority": self.authority,
                "execution_kernel_contract_mode": self._contract_mode(),
                "execution_kernel_in_process_replacement_complete": False,
                "execution_kernel_delegate": delegate_kind,
                "execution_kernel_delegate_authority": delegate_authority,
                "execution_kernel_live_primary": delegate_health.get("kernel_live_delegate_primary_kind"),
                "execution_kernel_live_primary_authority": delegate_health.get(
                    "kernel_live_delegate_primary_authority"
                ),
                "execution_kernel_live_fallback": delegate_health.get("kernel_live_delegate_fallback_kind"),
                "execution_kernel_live_fallback_authority": delegate_health.get(
                    "kernel_live_delegate_fallback_authority"
                ),
                "execution_kernel_live_fallback_enabled": delegate_health.get("kernel_live_fallback_enabled", True),
                "execution_kernel_live_fallback_mode": delegate_health.get(
                    "kernel_live_fallback_mode",
                    "compatibility" if delegate_health.get("kernel_live_fallback_enabled", True) else "disabled",
                ),
                "execution_kernel_delegate_family": response.metadata.get(
                    "execution_kernel_delegate_family",
                    (
                        delegate_health.get("kernel_live_backend_family")
                        if not request.dry_run
                        else (
                            delegate_health.get("kernel_live_delegate_fallback_family") or "python"
                        )
                    ),
                ),
                "execution_kernel_delegate_impl": response.metadata.get(
                    "execution_kernel_delegate_impl",
                    (
                        delegate_health.get("kernel_live_backend_impl")
                        if not request.dry_run
                        else delegate_health.get("kernel_live_delegate_fallback_impl") or "agno"
                    ),
                ),
            }
        )
        return response

    def health(self) -> dict[str, Any]:
        delegate_health = self._delegate.health()
        return {
            "kernel_adapter_kind": self.adapter_kind,
            "kernel_authority": self.authority,
            "kernel_owner_family": "rust",
            "kernel_owner_impl": "execution-kernel-slice",
            "kernel_contract_mode": self._contract_mode(),
            "kernel_replace_ready": True,
            "kernel_in_process_replacement_complete": False,
            "kernel_live_backend_family": delegate_health.get("kernel_live_delegate_primary_family", "rust-cli"),
            "kernel_live_backend_impl": delegate_health.get("kernel_live_delegate_primary_kind", "router-rs"),
            "kernel_live_delegate_kind": delegate_health.get("kernel_live_delegate_primary_kind", "router-rs"),
            "kernel_live_delegate_authority": delegate_health.get(
                "kernel_live_delegate_primary_authority",
                "rust-execution-cli",
            ),
            "kernel_live_delegate_family": delegate_health.get("kernel_live_delegate_primary_family", "rust-cli"),
            "kernel_live_delegate_impl": delegate_health.get("kernel_live_delegate_primary_impl", "router-rs"),
            "kernel_live_delegate_mode": delegate_health.get("kernel_live_delegate_mode", "rust-primary"),
            "kernel_live_fallback_kind": delegate_health["kernel_live_delegate_fallback_kind"],
            "kernel_live_fallback_authority": delegate_health["kernel_live_delegate_fallback_authority"],
            "kernel_live_fallback_family": delegate_health.get("kernel_live_delegate_fallback_family"),
            "kernel_live_fallback_impl": delegate_health.get("kernel_live_delegate_fallback_impl"),
            "kernel_live_fallback_enabled": delegate_health.get("kernel_live_fallback_enabled", True),
            "kernel_live_fallback_mode": delegate_health.get(
                "kernel_live_fallback_mode",
                "compatibility" if delegate_health.get("kernel_live_fallback_enabled", True) else "disabled",
            ),
            "kernel_mode_support": delegate_health.get("kernel_mode_support", ["dry_run", "live"]),
        }

    def contract_descriptor(self, *, dry_run: bool = False) -> dict[str, Any]:
        health = self.health()
        dry_run_delegate_family = health["kernel_live_fallback_family"] or "python"
        dry_run_delegate_impl = health["kernel_live_fallback_impl"] or "agno"
        return {
            "execution_kernel": health["kernel_adapter_kind"],
            "execution_kernel_authority": health["kernel_authority"],
            "execution_kernel_contract_mode": health["kernel_contract_mode"],
            "execution_kernel_in_process_replacement_complete": health["kernel_in_process_replacement_complete"],
            "execution_kernel_delegate": (
                self._delegate.adapter_kind if dry_run else health["kernel_live_delegate_kind"]
            ),
            "execution_kernel_delegate_authority": (
                self._delegate.authority if dry_run else health["kernel_live_delegate_authority"]
            ),
            "execution_kernel_delegate_family": (
                dry_run_delegate_family if dry_run else health["kernel_live_delegate_family"]
            ),
            "execution_kernel_delegate_impl": (
                dry_run_delegate_impl if dry_run else health["kernel_live_delegate_impl"]
            ),
            "execution_kernel_live_primary": health["kernel_live_delegate_kind"],
            "execution_kernel_live_primary_authority": health["kernel_live_delegate_authority"],
            "execution_kernel_live_fallback": health["kernel_live_fallback_kind"],
            "execution_kernel_live_fallback_authority": health["kernel_live_fallback_authority"],
            "execution_kernel_live_fallback_enabled": health["kernel_live_fallback_enabled"],
            "execution_kernel_live_fallback_mode": health["kernel_live_fallback_mode"],
        }


class ExecutionEnvironmentService:
    """Own agent-factory construction and execution-environment health."""

    def __init__(
        self,
        settings: RuntimeSettings,
        prompt_builder: PromptBuilder,
        *,
        max_background_jobs: int,
        background_job_timeout_seconds: float,
    ) -> None:
        self.settings = settings
        self.agent_factory = AgentFactory(settings, prompt_builder)
        self.kernel = _RustExecutionKernelAuthorityAdapter(
            PythonAgnoExecutionKernel(
                settings,
                prompt_builder,
                agent_factory=self.agent_factory,
            )
        )
        self.max_background_jobs = max_background_jobs
        self.background_job_timeout_seconds = background_job_timeout_seconds

    def startup(self) -> None:
        """Execution-environment startup hook."""

    def shutdown(self) -> None:
        """Execution-environment shutdown hook."""

    def resolve_dry_run(self, *, request_dry_run: bool) -> bool:
        """Resolve whether one execution should stay in deterministic dry-run mode."""

        return request_dry_run or not self.settings.use_live_model

    async def execute(
        self,
        *,
        ctx: MiddlewareContext,
        dry_run: bool,
        trace_event_count: int,
        trace_output_path: str | None,
    ) -> RunTaskResponse:
        """Run one request through the active execution-kernel adapter."""

        return await self.kernel.execute(
            ExecutionKernelRequest(
                task=ctx.task,
                session_id=ctx.session_id,
                user_id=ctx.user_id,
                routing_result=ctx.routing_result,
                prompt_preview=(ctx.prompt or None) if dry_run else None,
                dry_run=dry_run,
                trace_event_count=trace_event_count,
                trace_output_path=trace_output_path,
            )
        )

    def health(self) -> dict[str, Any]:
        payload = {
            "max_background_jobs": self.max_background_jobs,
            "background_job_timeout_seconds": self.background_job_timeout_seconds,
            "execution_mode_default": "live" if self.settings.use_live_model else "dry_run",
        }
        payload.update(self.kernel.health())
        payload["control_plane_contracts"] = self.describe_control_plane_contracts()
        return payload

    def describe_control_plane_contracts(self) -> dict[str, Any]:
        """Return control-plane-only descriptors for shared execution artifacts."""

        return {
            "execution_controller_contract": build_execution_controller_contract(),
            "delegation_contract": build_delegation_contract(),
            "supervisor_state_contract": build_supervisor_state_contract(),
        }

    def describe_kernel_contract(self, *, dry_run: bool = False) -> dict[str, Any]:
        """Return the stable kernel-owner descriptor used by runtime surfaces."""

        return self.kernel.contract_descriptor(dry_run=dry_run)

    def kernel_payload(
        self,
        *,
        dry_run: bool = False,
        metadata: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Merge explicit execution metadata onto the stable kernel contract."""

        payload = dict(self.describe_kernel_contract(dry_run=dry_run))
        if metadata is not None:
            for field in _KERNEL_CONTRACT_FIELDS:
                if field in metadata:
                    payload[field] = metadata[field]
        return {key: value for key, value in payload.items() if value is not None}
