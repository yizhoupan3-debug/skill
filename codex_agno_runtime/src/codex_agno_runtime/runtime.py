"""Core runtime orchestration for Codex on Agno."""

from __future__ import annotations

import asyncio
import json
import os
import uuid
from datetime import UTC, datetime
from typing import Any

from codex_agno_runtime.checkpoint_store import FilesystemRuntimeCheckpointer
from codex_agno_runtime.event_transport import ExternalRuntimeEventTransportBridge
from codex_agno_runtime.trace import (
    TRACE_EVENT_HANDOFF_SCHEMA_VERSION,
    TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
    TraceSupervisorProjection,
)
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.middleware import (
    ContextCompressionMiddleware,
    MemoryMiddleware,
    MiddlewareChain,
    MiddlewareContext,
    SkillInjectionMiddleware,
    SubagentLimitMiddleware,
)
from codex_agno_runtime.rust_router import RustRouteAdapter
from codex_agno_runtime.schemas import (
    BackgroundBatchEnqueueResponse,
    BackgroundParallelGroupSummary,
    BackgroundRunRequest,
    BackgroundRunStatus,
    PrepareSessionRequest,
    PrepareSessionResponse,
    RoutingResult,
    RunTaskRequest,
    RunTaskResponse,
)
from codex_agno_runtime.services import (
    BackgroundRuntimeHost,
    ExecutionEnvironmentService,
    MemoryService,
    RouterService,
    StateService,
    TraceService,
)
from codex_agno_runtime.state import (
    BackgroundJobStatusMutation,
    BackgroundSessionTakeoverArbitration,
    SessionConflictError,
)
from codex_agno_runtime.utils import build_session_id

# Maximum number of concurrently running background jobs.
_MAX_BACKGROUND_JOBS = int(os.environ.get("CODEX_MAX_BACKGROUND_JOBS", 16))

# Timeout (seconds) for a single background job run.
_BACKGROUND_JOB_TIMEOUT = float(os.environ.get("CODEX_BACKGROUND_JOB_TIMEOUT", 600))

def _now_iso() -> str:
    """Return a canonical UTC timestamp."""

    return datetime.now(UTC).isoformat()


class CodexAgnoRuntime:
    """High-level runtime that routes tasks and delegates execution through a kernel seam."""

    def __init__(self, settings: RuntimeSettings) -> None:
        self.settings = settings

        self.rust_adapter = RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        self.router_service = RouterService(settings, rust_adapter=self.rust_adapter)
        self.control_plane_descriptor = self.router_service.control_plane_descriptor
        self.checkpointer = FilesystemRuntimeCheckpointer(
            data_dir=self.settings.resolved_data_dir,
            trace_output_path=self.settings.resolved_trace_output_path,
            control_plane_descriptor=self.control_plane_descriptor,
        )
        self.state_service = StateService(
            self.checkpointer,
            control_plane_descriptor=self.control_plane_descriptor,
        )
        self.trace_service = TraceService(
            self.checkpointer,
            control_plane_descriptor=self.control_plane_descriptor,
        )
        self.memory_service = MemoryService(
            settings,
            control_plane_descriptor=self.control_plane_descriptor,
        )
        self.execution_service = ExecutionEnvironmentService(
            settings,
            self.router_service.prompt_builder,
            max_background_jobs=_MAX_BACKGROUND_JOBS,
            background_job_timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
            control_plane_descriptor=self.control_plane_descriptor,
        )
        self.background_service = BackgroundRuntimeHost(
            state_service=self.state_service,
            trace_service=self.trace_service,
            execution_service=self.execution_service,
            background_control_provider=lambda: self.rust_adapter.background_control,
            background_control_schema_version=self.rust_adapter.background_control_schema_version,
            background_control_authority=self.rust_adapter.background_control_authority,
            max_background_jobs=_MAX_BACKGROUND_JOBS,
            background_job_timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
            artifact_paths_provider=self._runtime_artifact_paths,
            supervisor_projection_provider=lambda: self._build_supervisor_projection().model_dump(mode="json"),
            control_plane_descriptor=self.control_plane_descriptor,
        )

        self.loader = self.router_service.loader
        self.prompt_builder = self.router_service.prompt_builder
        self.skills = self.router_service.skills
        self.router = self.router_service._python_router
        self._job_store = self.state_service.store
        self._trace = self.trace_service.recorder
        self._memory_store = self.memory_service.store

        self._max_background_jobs = _MAX_BACKGROUND_JOBS
        self._jobs_lock = self.background_service.jobs_lock
        self._job_semaphore = self.background_service.job_semaphore
        self.background_jobs = self.background_service.background_jobs
        self._background_tasks = self.background_service.background_tasks

        self._middleware_chain = self._build_middleware_chain()

    def _apply_control_plane_descriptor(self, descriptor: dict[str, Any]) -> None:
        """Propagate the active Rust control-plane descriptor across runtime seams."""

        self.control_plane_descriptor = dict(descriptor)
        self.state_service.refresh_control_plane(self.control_plane_descriptor)
        self.trace_service.refresh_control_plane(self.control_plane_descriptor)
        self.memory_service.refresh_control_plane(self.control_plane_descriptor)
        self.execution_service.refresh_control_plane(self.control_plane_descriptor)
        self.background_service.refresh_control_plane(self.control_plane_descriptor)

    def startup(self) -> None:
        """Start runtime service boundaries."""

        for service in (
            self.router_service,
            self.state_service,
            self.trace_service,
            self.memory_service,
            self.execution_service,
            self.background_service,
        ):
            service.startup()
        self._refresh_router()

    def shutdown(self) -> None:
        """Shutdown runtime service boundaries."""

        for service in (
            self.background_service,
            self.execution_service,
            self.memory_service,
            self.trace_service,
            self.state_service,
            self.router_service,
        ):
            service.shutdown()

    def health(self) -> dict[str, Any]:
        """Return health information for each runtime service seam."""

        services = self.control_plane_descriptor.get("services")
        rust_owned_services = (
            len(
                [
                    name
                    for name, descriptor in services.items()
                    if isinstance(name, str)
                    and isinstance(descriptor, dict)
                    and descriptor.get("authority")
                ]
            )
            if isinstance(services, dict)
            else 0
        )
        return {
            "control_plane": self.control_plane_descriptor,
            "rustification": {
                "python_host_role": self.control_plane_descriptor.get("python_host_role"),
                "rustification_status": self.control_plane_descriptor.get("rustification_status"),
                "rust_owned_service_count": rust_owned_services,
            },
            "router": self.router_service.health(),
            "state": self.state_service.health(),
            "trace": self.trace_service.health(),
            "memory": self.memory_service.health(),
            "execution_environment": self.execution_service.health(),
            "background": self.background_service.health(),
            "checkpoint": self.checkpointer.health(),
        }

    def subscribe_runtime_events(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
        after_event_id: str | None = None,
        limit: int | None = 100,
        heartbeat: bool = False,
    ) -> dict[str, Any]:
        """Expose the live event-bridge seam to host adapters and local callers."""

        return self.trace_service.subscribe(
            session_id=session_id,
            job_id=job_id,
            after_event_id=after_event_id,
            limit=limit,
            heartbeat=heartbeat,
        ).model_dump(mode="json")

    def cleanup_runtime_events(self, *, session_id: str | None = None, job_id: str | None = None) -> None:
        """Release cached event-bridge state for one stream or the entire runtime."""

        self.trace_service.cleanup_stream(session_id=session_id, job_id=job_id)

    def describe_runtime_event_transport(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
    ) -> dict[str, Any]:
        """Expose the host-facing event transport descriptor for one runtime stream."""

        return self.trace_service.describe_transport(session_id=session_id, job_id=job_id).model_dump(mode="json")

    def describe_runtime_event_handoff(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
    ) -> dict[str, Any]:
        """Expose the host/remote handoff descriptor for one runtime stream."""

        return self.trace_service.describe_handoff(session_id=session_id, job_id=job_id).model_dump(mode="json")

    def attach_runtime_event_transport(
        self,
        *,
        attach_descriptor: dict[str, Any] | None = None,
        binding_artifact_path: str | None = None,
        handoff_path: str | None = None,
        resume_manifest_path: str | None = None,
    ) -> dict[str, Any]:
        """Resolve a process-external attach bridge from persisted runtime artifacts."""

        return self._attach_runtime_event_bridge(
            attach_descriptor=attach_descriptor,
            binding_artifact_path=binding_artifact_path,
            handoff_path=handoff_path,
            resume_manifest_path=resume_manifest_path,
        ).describe()

    def subscribe_attached_runtime_events(
        self,
        *,
        attach_descriptor: dict[str, Any] | None = None,
        binding_artifact_path: str | None = None,
        handoff_path: str | None = None,
        resume_manifest_path: str | None = None,
        after_event_id: str | None = None,
        limit: int | None = 100,
        heartbeat: bool = False,
    ) -> dict[str, Any]:
        """Replay runtime events through the process-external attach bridge."""

        return self._attach_runtime_event_bridge(
            attach_descriptor=attach_descriptor,
            binding_artifact_path=binding_artifact_path,
            handoff_path=handoff_path,
            resume_manifest_path=resume_manifest_path,
        ).subscribe(
            after_event_id=after_event_id,
            limit=limit,
            heartbeat=heartbeat,
        ).model_dump(mode="json")

    def cleanup_attached_runtime_event_transport(
        self,
        *,
        attach_descriptor: dict[str, Any] | None = None,
        binding_artifact_path: str | None = None,
        handoff_path: str | None = None,
        resume_manifest_path: str | None = None,
    ) -> dict[str, Any]:
        """Describe cleanup semantics for the process-external attach bridge."""

        return self._attach_runtime_event_bridge(
            attach_descriptor=attach_descriptor,
            binding_artifact_path=binding_artifact_path,
            handoff_path=handoff_path,
            resume_manifest_path=resume_manifest_path,
        ).cleanup()

    def _attach_runtime_event_bridge(
        self,
        *,
        attach_descriptor: dict[str, Any] | None = None,
        binding_artifact_path: str | None = None,
        handoff_path: str | None = None,
        resume_manifest_path: str | None = None,
    ) -> ExternalRuntimeEventTransportBridge:
        """Resolve one external attach bridge from either a stable descriptor or explicit paths."""

        return ExternalRuntimeEventTransportBridge.attach(
            attach_descriptor=attach_descriptor,
            binding_artifact_path=binding_artifact_path,
            handoff_path=handoff_path,
            resume_manifest_path=resume_manifest_path,
        )

    def _build_middleware_chain(self) -> MiddlewareChain:
        """Build the ordered middleware pipeline."""

        s = self.settings
        middlewares = [
            SkillInjectionMiddleware(self.prompt_builder),
            MemoryMiddleware(self._memory_store) if s.memory_enabled else None,
            ContextCompressionMiddleware(
                budget_tokens=s.context_budget_tokens,
                threshold=s.compression_threshold,
            ),
            SubagentLimitMiddleware(
                max_concurrent=s.max_concurrent_subagents,
                timeout_seconds=s.subagent_timeout_seconds,
            ),
        ]
        return MiddlewareChain(
            [m for m in middlewares if m is not None],
            trace_recorder=self._trace,
        )

    def _refresh_router(self) -> None:
        """Reload skill metadata and rebuild router-facing compatibility handles."""

        self.router_service.reload()
        self._apply_control_plane_descriptor(self.router_service.control_plane_descriptor)
        self.skills = self.router_service.skills
        self.router = self.router_service._python_router

    def _prepare_session(
        self,
        request: PrepareSessionRequest,
        *,
        include_prompt_preview: bool,
    ) -> PrepareSessionResponse:
        """Route a task and optionally build a session prompt preview."""

        session_id = build_session_id(
            request.project_id,
            request.task,
            self.settings.codex_home,
            request.session_id,
        )
        routing_result = self.router_service.route(
            task=request.task,
            session_id=session_id,
            allow_overlay=request.allow_overlay,
            first_turn=True,
        )
        user_id = request.user_id or request.project_id or "codex-user"
        if include_prompt_preview:
            routing_result.prompt_preview = self.execution_service.preview_prompt(
                task=request.task,
                session_id=session_id,
                user_id=user_id,
                routing_result=routing_result,
            )
        self._trace.record(
            session_id=session_id,
            kind="session.prepared",
            stage="routing",
            payload={
                "user_id": user_id,
                "allow_overlay": request.allow_overlay,
                "loaded_skill_count": len(self.skills),
                "route_engine_mode": self.settings.route_engine_mode,
                "route_engine": routing_result.route_engine,
                "diagnostic_python_lane_active": routing_result.diagnostic_python_lane_active,
                "python_lane_kind": routing_result.python_lane_kind,
            },
        )
        self._trace.record(
            session_id=session_id,
            kind="route.selected",
            stage="routing",
            payload={
                "skill": routing_result.selected_skill.name,
                "overlay": routing_result.overlay_skill.name if routing_result.overlay_skill else None,
                "layer": routing_result.layer,
                "reasons": routing_result.reasons,
                "route_engine": routing_result.route_engine,
                "route_engine_mode": self.settings.route_engine_mode,
                "diagnostic_python_lane_active": routing_result.diagnostic_python_lane_active,
                "python_lane_kind": routing_result.python_lane_kind,
                "shadow_route_report": (
                    routing_result.shadow_route_report.model_dump(mode="json")
                    if routing_result.shadow_route_report is not None
                    else None
                ),
            },
        )
        return PrepareSessionResponse(
            session_id=session_id,
            user_id=user_id,
            skill=routing_result.selected_skill.name,
            overlay=routing_result.overlay_skill.name if routing_result.overlay_skill else None,
            layer=routing_result.layer,
            reasons=routing_result.reasons,
            prompt_preview=routing_result.prompt_preview,
            loaded_skill_count=len(self.skills),
            route_engine=routing_result.route_engine,
            diagnostic_python_lane_active=routing_result.diagnostic_python_lane_active,
            python_lane_kind=routing_result.python_lane_kind,
            shadow_route_report=routing_result.shadow_route_report,
        )

    def prepare_session(self, request: PrepareSessionRequest) -> PrepareSessionResponse:
        """Route a task and build a session preview."""

        return self._prepare_session(request, include_prompt_preview=True)

    async def run_task(self, request: RunTaskRequest) -> RunTaskResponse:
        """Run a routed task through the middleware chain."""

        execution_is_dry_run = self.execution_service.resolve_dry_run(request_dry_run=request.dry_run)
        prepared = self._prepare_session(
            PrepareSessionRequest(
                task=request.task,
                project_id=request.project_id,
                session_id=request.session_id,
                user_id=request.user_id,
                allow_overlay=request.allow_overlay,
            ),
            include_prompt_preview=execution_is_dry_run,
        )
        routing_result = self._to_routing_result(request.task, prepared)
        kernel_contract = self.execution_service.describe_kernel_contract(dry_run=execution_is_dry_run)
        self._trace.record(
            session_id=prepared.session_id,
            job_id=request.job_id,
            kind="run.started",
            stage="execution",
            payload={
                "skill": prepared.skill,
                "live_run": not execution_is_dry_run,
                **self.execution_service.kernel_payload(
                    dry_run=execution_is_dry_run,
                    metadata=kernel_contract,
                ),
            },
        )

        ctx = MiddlewareContext(
            task=request.task,
            session_id=prepared.session_id,
            job_id=request.job_id,
            user_id=prepared.user_id,
            routing_result=routing_result,
            prompt=prepared.prompt_preview or "",
            execution_kernel=kernel_contract.get("execution_kernel"),
            execution_kernel_authority=kernel_contract.get("execution_kernel_authority"),
            execution_kernel_delegate=kernel_contract.get("execution_kernel_delegate"),
            execution_kernel_delegate_authority=kernel_contract.get("execution_kernel_delegate_authority"),
        )
        ctx.metadata["dry_run"] = execution_is_dry_run
        ctx.metadata["python_prompt_required"] = False

        async def _core_agent_fn(mw_ctx: MiddlewareContext) -> RunTaskResponse:
            return await self.execution_service.execute(
                ctx=mw_ctx,
                dry_run=execution_is_dry_run,
                trace_event_count=len(self._trace.events),
                trace_output_path=(
                    str(self.trace_service.output_path) if self.trace_service.output_path is not None else None
                ),
            )

        try:
            result = await self._middleware_chain.execute(ctx, _core_agent_fn)
        except Exception as error:
            self._trace.record(
                session_id=prepared.session_id,
                job_id=request.job_id,
                kind="run.failed",
                stage="execution",
                payload={"error": str(error)},
            )
            raise

        self._trace.record(
            session_id=prepared.session_id,
            job_id=request.job_id,
            kind="run.completed",
            stage="execution",
            payload={
                "live_run": not execution_is_dry_run,
                "mode": "dry-run" if execution_is_dry_run else "live",
                **self.execution_service.kernel_payload(
                    dry_run=not result.live_run,
                    metadata=result.metadata,
                ),
                **({"model_id": result.model_id} if result.live_run else {}),
            },
        )
        self._attach_trace_metadata(result, routing_result)
        self._maybe_flush_trace(result, routing_result)
        return result

    async def enqueue_background_run(self, request: BackgroundRunRequest) -> BackgroundRunStatus:
        """Schedule a background task execution."""

        job_id = f"job_{uuid.uuid4().hex[:12]}"
        effective_session_id = build_session_id(
            request.project_id,
            request.task,
            self.settings.codex_home,
            request.session_id,
        )
        request = request.model_copy(update={"session_id": effective_session_id})
        enqueue_policy = self._background_enqueue_policy(
            multitask_strategy=request.multitask_strategy,
            active_job_count=0,
        )
        multitask_strategy = str(enqueue_policy["normalized_multitask_strategy"])
        requires_takeover = bool(enqueue_policy["requires_takeover"])

        if not bool(enqueue_policy["strategy_supported"]):
            status = self._apply_background_mutation(
                job_id,
                self._background_mutation(
                    status="failed",
                    session_id=effective_session_id,
                    parallel_group_id=request.parallel_group_id,
                    lane_id=request.lane_id,
                    parent_job_id=request.parent_job_id,
                    multitask_strategy=multitask_strategy,
                    error=str(enqueue_policy["error"]),
                    timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                    max_attempts=request.max_attempts,
                    backoff_base_seconds=request.backoff_base_seconds,
                    backoff_multiplier=request.backoff_multiplier,
                    max_backoff_seconds=request.max_backoff_seconds,
                ),
            )
            async with self._jobs_lock:
                self.background_jobs[job_id] = status
            self._trace.record(
                session_id=effective_session_id,
                job_id=job_id,
                kind="job.failed",
                stage="background",
                payload={
                    "error": status.error,
                    "multitask_strategy": multitask_strategy,
                    **self.background_service._background_trace_context(status),
                },
            )
            return status

        if requires_takeover:
            try:
                takeover = await self._arbitrate_background_multitask_takeover(
                    session_id=effective_session_id,
                    incoming_job_id=job_id,
                    operation="reserve",
                )
                active_job_id = takeover.previous_active_job_id
            except SessionConflictError as error:
                status = self._apply_background_mutation(
                    job_id,
                    self._background_mutation(
                    status="failed",
                    session_id=effective_session_id,
                    parallel_group_id=request.parallel_group_id,
                    lane_id=request.lane_id,
                    parent_job_id=request.parent_job_id,
                    multitask_strategy=multitask_strategy,
                    error=str(error),
                        timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                        max_attempts=request.max_attempts,
                        backoff_base_seconds=request.backoff_base_seconds,
                        backoff_multiplier=request.backoff_multiplier,
                        max_backoff_seconds=request.max_backoff_seconds,
                    ),
                )
                async with self._jobs_lock:
                    self.background_jobs[job_id] = status
                self._trace.record(
                    session_id=effective_session_id,
                    job_id=job_id,
                    kind="job.failed",
                    stage="background",
                    payload={
                        "error": str(error),
                        "parallel_group_id": request.parallel_group_id,
                        "lane_id": request.lane_id,
                        "parent_job_id": request.parent_job_id,
                    },
                )
                return status
            if active_job_id is not None:
                self._trace.record(
                    session_id=effective_session_id,
                    job_id=active_job_id,
                    kind="job.multitask_preempt_requested",
                    stage="background",
                    payload={
                        "multitask_strategy": multitask_strategy,
                        "incoming_job_id": job_id,
                        "parallel_group_id": request.parallel_group_id,
                        "lane_id": request.lane_id,
                        "parent_job_id": request.parent_job_id,
                    },
                )
                await self.request_background_interrupt(active_job_id)
                try:
                    await self._wait_for_background_session_release(effective_session_id)
                except Exception:
                    async with self._jobs_lock:
                        self.state_service.arbitrate_session_takeover(
                            session_id=effective_session_id,
                            incoming_job_id=job_id,
                            operation="release",
                        )
                    raise
                await self._arbitrate_background_multitask_takeover(
                    session_id=effective_session_id,
                    incoming_job_id=job_id,
                    operation="claim",
                )

        try:
            async with self._jobs_lock:
                admission_policy = self._background_enqueue_policy(
                    multitask_strategy=multitask_strategy,
                    active_job_count=self.state_service.active_job_count(),
                )
                if not bool(admission_policy["accepted"]):
                    if requires_takeover:
                        self.state_service.arbitrate_session_takeover(
                            session_id=effective_session_id,
                            incoming_job_id=job_id,
                            operation="release",
                        )
                    status = self._apply_background_mutation(
                        job_id,
                        self._background_mutation(
                            status="failed",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            error=str(admission_policy["error"]),
                            timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                            max_attempts=request.max_attempts,
                            backoff_base_seconds=request.backoff_base_seconds,
                            backoff_multiplier=request.backoff_multiplier,
                            max_backoff_seconds=request.max_backoff_seconds,
                        ),
                    )
                else:
                    status = self._apply_background_mutation(
                        job_id,
                        self._background_mutation(
                            status="queued",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                            max_attempts=request.max_attempts,
                            backoff_base_seconds=request.backoff_base_seconds,
                            backoff_multiplier=request.backoff_multiplier,
                            max_backoff_seconds=request.max_backoff_seconds,
                        ),
                    )
        except SessionConflictError as error:
            if requires_takeover:
                async with self._jobs_lock:
                    self.state_service.arbitrate_session_takeover(
                        session_id=effective_session_id,
                        incoming_job_id=job_id,
                        operation="release",
                    )
                    status = self._apply_background_mutation(
                        job_id,
                        self._background_mutation(
                            status="failed",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            error=str(error),
                            timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                            max_attempts=request.max_attempts,
                            backoff_base_seconds=request.backoff_base_seconds,
                            backoff_multiplier=request.backoff_multiplier,
                            max_backoff_seconds=request.max_backoff_seconds,
                        ),
                    )
            else:
                async with self._jobs_lock:
                    status = self._apply_background_mutation(
                        job_id,
                        self._background_mutation(
                            status="failed",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            error=str(error),
                            timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                            max_attempts=request.max_attempts,
                            backoff_base_seconds=request.backoff_base_seconds,
                            backoff_multiplier=request.backoff_multiplier,
                            max_backoff_seconds=request.max_backoff_seconds,
                        ),
                    )
        if status.status == "failed":
            self._trace.record(
                session_id=effective_session_id,
                job_id=job_id,
                kind="job.failed",
                stage="background",
                payload={
                    "error": status.error,
                    **self.background_service._background_trace_context(status),
                },
            )
            return status

        async with self._jobs_lock:
            self.background_jobs[job_id] = status
        self._trace.record(
            session_id=effective_session_id,
            job_id=job_id,
            kind="job.queued",
            stage="background",
            payload={
                "multitask_strategy": multitask_strategy,
                "timeout_seconds": _BACKGROUND_JOB_TIMEOUT,
                "capacity_limit": self._max_background_jobs,
                "max_attempts": request.max_attempts,
                "backoff_base_seconds": request.backoff_base_seconds,
                "backoff_multiplier": request.backoff_multiplier,
                "max_backoff_seconds": request.max_backoff_seconds,
                "background_policy_authority": self.rust_adapter.background_control_authority,
                **self.background_service._background_trace_context(status),
            },
        )

        self.background_service.configure_limits(
            max_background_jobs=self._max_background_jobs,
            job_semaphore=self._job_semaphore,
        )
        self.background_service.start_job(job_id, request, run_task=self.run_task)
        return status

    async def enqueue_background_batch(
        self,
        requests: list[BackgroundRunRequest],
        *,
        parallel_group_id: str | None = None,
        lane_id_prefix: str = "lane",
    ) -> BackgroundBatchEnqueueResponse:
        """Admit a bounded parallel batch and auto-assign lane ids when needed."""

        if not requests:
            raise ValueError("enqueue_background_batch requires at least one request.")
        resolved_group_id = parallel_group_id or f"pgroup_{uuid.uuid4().hex[:12]}"
        statuses: list[BackgroundRunStatus] = []
        for index, request in enumerate(requests, start=1):
            request_group_id = request.parallel_group_id
            if request_group_id is not None and request_group_id != resolved_group_id:
                raise ValueError(
                    "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                )
            lane_id = request.lane_id or f"{lane_id_prefix}-{index}"
            status = await self.enqueue_background_run(
                request.model_copy(
                    update={
                        "parallel_group_id": resolved_group_id,
                        "lane_id": lane_id,
                    }
                )
            )
            statuses.append(status)
        summary = self.get_background_parallel_group_summary(resolved_group_id)
        if summary is None:
            raise RuntimeError(f"Background parallel group {resolved_group_id!r} was not persisted.")
        return BackgroundBatchEnqueueResponse(
            parallel_group_id=resolved_group_id,
            statuses=statuses,
            summary=summary,
        )

    def get_background_parallel_group_summary(
        self,
        parallel_group_id: str,
    ) -> BackgroundParallelGroupSummary | None:
        """Return one durable parallel-batch summary by group id."""

        return self.state_service.parallel_group_summary(parallel_group_id)

    def list_background_parallel_groups(self) -> list[BackgroundParallelGroupSummary]:
        """Return all durable parallel-batch summaries."""

        return self.state_service.parallel_group_summaries()

    async def _arbitrate_background_multitask_takeover(
        self,
        *,
        session_id: str,
        incoming_job_id: str,
        operation: str,
    ) -> BackgroundSessionTakeoverArbitration:
        """Execute one state-owned takeover reducer step under the jobs lock."""

        async with self._jobs_lock:
            return self.state_service.arbitrate_session_takeover(
                session_id=session_id,
                incoming_job_id=incoming_job_id,
                operation=operation,
            )

    def _background_mutation(
        self,
        *,
        status: str,
        current: BackgroundRunStatus | None = None,
        **overrides: Any,
    ) -> BackgroundJobStatusMutation:
        """Build one descriptor-driven background mutation from the latest row snapshot."""

        return self.background_service.mutation(status=status, current=current, **overrides)

    def _apply_background_mutation(
        self,
        job_id: str,
        mutation: BackgroundJobStatusMutation,
    ) -> BackgroundRunStatus:
        """Apply one durable mutation through the state service host lane."""

        return self.background_service.apply_mutation(job_id, mutation)

    async def _wait_for_background_session_release(self, session_id: str, *, timeout_seconds: float = 5.0) -> None:
        """Wait until a session is no longer reserved by an active background job."""

        release_policy = self._background_session_release_policy(session_id=session_id)
        effect_plan = self._background_effect_plan(release_policy)
        timeout_seconds = float(effect_plan.get("wait_timeout_seconds") or timeout_seconds)
        poll_interval_seconds = float(effect_plan.get("wait_poll_interval_seconds") or 0.01)
        await self.state_service.wait_for_session_release(
            session_id=session_id,
            timeout_seconds=timeout_seconds,
            poll_interval_seconds=poll_interval_seconds,
        )

    def _background_enqueue_policy(
        self,
        *,
        multitask_strategy: str,
        active_job_count: int,
    ) -> dict[str, Any]:
        """Resolve enqueue admission through the Rust background-control seam."""

        return self.rust_adapter.background_control(
            {
                "schema_version": self.rust_adapter.background_control_schema_version,
                "operation": "enqueue",
                "multitask_strategy": multitask_strategy,
                "active_job_count": active_job_count,
                "capacity_limit": self._max_background_jobs,
            }
        )

    def _background_effect_plan(self, policy: dict[str, Any]) -> dict[str, Any]:
        """Extract the Rust-owned background effect plan from a control response."""

        effect_plan = policy.get("effect_plan")
        if not isinstance(effect_plan, dict):
            raise RuntimeError("Rust background control response missing effect_plan.")
        return effect_plan

    def _background_session_release_policy(self, *, session_id: str) -> dict[str, Any]:
        """Resolve background session-release timing through the Rust control seam."""

        return self.rust_adapter.background_control(
            {
                "schema_version": self.rust_adapter.background_control_schema_version,
                "operation": "session-release",
                "current_status": "release_pending",
                "task_active": False,
                "task_done": False,
                "active_job_count": self.state_service.active_job_count(),
                "capacity_limit": self._max_background_jobs,
                "session_id": session_id,
            }
        )

    async def request_background_interrupt(self, job_id: str) -> BackgroundRunStatus | None:
        """Request interruption of a queued, running, or retry-scheduled job."""

        return await self.background_service.request_interrupt(job_id)

    def get_background_status(self, job_id: str) -> BackgroundRunStatus | None:
        """Return the latest background job state."""

        return self.background_service.get_status(job_id)

    def _attach_trace_metadata(self, response: RunTaskResponse, routing_result: RoutingResult) -> None:
        """Stamp final trace metadata onto a response payload."""

        reroute_count = self._trace.count_reroutes(response.session_id)
        retry_count = self._trace.count_retries(response.session_id)
        latest_cursor = self._trace.latest_cursor(session_id=response.session_id)
        stream_state = self._trace.describe_stream()
        transport = self.trace_service.describe_transport(session_id=response.session_id)
        handoff = self.trace_service.describe_handoff(session_id=response.session_id)
        response.metadata.update(
            {
                **self.execution_service.kernel_payload(
                    dry_run=not response.live_run,
                    metadata=response.metadata,
                ),
                "trace_event_count": len(self._trace.events),
                "trace_output_path": str(self.trace_service.output_path) if self.trace_service.output_path else None,
                "trace_stream_path": (
                    str(self.trace_service.event_stream_path) if self.trace_service.event_stream_path else None
                ),
                "trace_resume_manifest_path": handoff.resume_manifest_path,
                "trace_resume_manifest_role": handoff.resume_manifest_role,
                "trace_resume_manifest_binding_path": transport.binding_artifact_path,
                "trace_event_transport_path": transport.binding_artifact_path,
                "trace_event_bridge_supported": True,
                "trace_event_bridge_schema_version": self.trace_service.event_bridge.schema_version,
                "trace_event_transport_schema_version": TRACE_EVENT_TRANSPORT_SCHEMA_VERSION,
                "trace_event_transport_family": transport.transport_family,
                "trace_event_transport_endpoint_kind": transport.endpoint_kind,
                "trace_event_transport_remote_capable": transport.remote_capable,
                "trace_event_transport_handoff_supported": transport.handoff_supported,
                "trace_event_transport_attach_mode": transport.attach_mode,
                "trace_event_transport_binding_role": transport.binding_artifact_role,
                "trace_event_transport_recommended_method": transport.recommended_remote_attach_method,
                "trace_event_handoff_schema_version": TRACE_EVENT_HANDOFF_SCHEMA_VERSION,
                "trace_event_schema_version": self._trace.event_schema_version,
                "trace_metadata_schema_version": self._trace.metadata_schema_version,
                "trace_event_sink_schema_version": (
                    self._trace.event_sink.schema_version if self._trace.event_sink is not None else None
                ),
                "trace_replay_cursor_schema_version": (
                    latest_cursor.schema_version if latest_cursor is not None else None
                ),
                "trace_replay_supported": stream_state["replay_supported"],
                "trace_replay_anchor_kind": "trace_replay_cursor",
                "trace_replay_resume_mode": stream_state["resume_mode"],
                "trace_generation": stream_state["generation"],
                "trace_latest_seq": stream_state["latest_seq"],
                "trace_resume_cursor": latest_cursor.model_dump(mode="json") if latest_cursor is not None else None,
                "reroute_count": reroute_count,
                "retry_count": retry_count,
                "route_engine_mode": self.settings.route_engine_mode,
                "route_engine": routing_result.route_engine,
                "diagnostic_python_lane_active": routing_result.diagnostic_python_lane_active,
                "python_lane_kind": routing_result.python_lane_kind,
                "shadow_route_report": (
                    routing_result.shadow_route_report.model_dump(mode="json")
                    if routing_result.shadow_route_report is not None
                    else None
                ),
            }
        )

    def _to_routing_result(self, task: str, prepared: PrepareSessionResponse) -> RoutingResult:
        """Convert a prepared session payload back into a routing result."""

        selected = next(skill for skill in self.skills if skill.name == prepared.skill)
        overlay = next((skill for skill in self.skills if skill.name == prepared.overlay), None)
        return RoutingResult(
            task=task,
            session_id=prepared.session_id,
            selected_skill=selected,
            overlay_skill=overlay,
            score=0,
            layer=prepared.layer,
            reasons=prepared.reasons,
            prompt_preview=prepared.prompt_preview,
            route_engine=prepared.route_engine,
            diagnostic_python_lane_active=prepared.diagnostic_python_lane_active,
            python_lane_kind=prepared.python_lane_kind,
            shadow_route_report=prepared.shadow_route_report,
        )

    def _maybe_flush_trace(self, result: RunTaskResponse, routing_result: RoutingResult) -> None:
        """Flush canonical trace metadata when configured."""

        reroute_count = self._trace.count_reroutes(result.session_id)
        retry_count = self._trace.count_retries(result.session_id)
        artifact_paths = self._runtime_artifact_paths()
        self._trace.flush_metadata(
            task=routing_result.task,
            matched_skills=[
                routing_result.selected_skill.name,
                *([routing_result.overlay_skill.name] if routing_result.overlay_skill else []),
            ],
            owner=routing_result.selected_skill.name,
            gate=routing_result.selected_skill.routing_gate,
            overlay=routing_result.overlay_skill.name if routing_result.overlay_skill else None,
            artifact_paths=artifact_paths,
            verification_status="completed" if result.live_run else "dry_run",
            session_id=result.session_id,
            supervisor_projection=self._build_supervisor_projection().model_dump(mode="json"),
            reroute_count=reroute_count,
            retry_count=retry_count,
        )
        self._write_resume_manifest(
            session_id=result.session_id,
            job_id=None,
            status="completed" if result.live_run else "dry_run",
            artifact_paths=artifact_paths,
        )

    @staticmethod
    def _normalize_supervisor_verification_status(value: Any) -> str | None:
        """Reduce nested supervisor verification payloads to a stable summary string."""

        if value is None or isinstance(value, str):
            return value
        if isinstance(value, dict):
            nested_status = value.get("verification_status")
            if isinstance(nested_status, str):
                return nested_status
            verdicts = [item.strip().lower() for item in value.values() if isinstance(item, str)]
            if verdicts:
                if all(verdict in {"passed", "completed", "ok"} or verdict.endswith(" passed") for verdict in verdicts):
                    return "passed"
                if any(
                    token in verdict
                    for verdict in verdicts
                    for token in ("fail", "failed", "error", "timeout")
                ):
                    return "failed"
            return "composite"
        return str(value)

    def _build_supervisor_projection(self) -> TraceSupervisorProjection:
        """Project the minimal supervisor/control-plane descriptor for trace/resume artifacts."""

        supervisor_state_path = self.settings.codex_home / ".supervisor_state.json"
        if not supervisor_state_path.exists():
            return TraceSupervisorProjection()

        payload = json.loads(supervisor_state_path.read_text(encoding="utf-8"))
        if not isinstance(payload, dict):
            return TraceSupervisorProjection(
                supervisor_state_path=str(supervisor_state_path.resolve())
            )
        schema_version = payload.get("schema_version")
        delegation = payload.get("delegation")
        if schema_version != "supervisor-state-v2" and not isinstance(delegation, dict):
            delegation = {
                "delegation_plan_created": payload.get("delegation_plan_created"),
                "spawn_attempted": payload.get("spawn_attempted"),
                "fallback_mode": payload.get("fallback_mode"),
                "delegated_sidecars": payload.get("delegated_sidecars"),
            }
        verification = payload.get("verification")
        if schema_version != "supervisor-state-v2" and not isinstance(verification, dict):
            verification = {"verification_status": payload.get("verification_status")}
        delegated_sidecars = delegation.get("delegated_sidecars")
        if not isinstance(delegated_sidecars, list):
            delegated_sidecars = []
        verification_status = (
            verification.get("verification_status")
            if isinstance(verification, dict)
            else payload.get("verification_status")
        )
        return TraceSupervisorProjection(
            supervisor_state_path=str(supervisor_state_path.resolve()),
            active_phase=payload.get("active_phase"),
            verification_status=self._normalize_supervisor_verification_status(verification_status),
            delegation={
                "plan_created": delegation.get("delegation_plan_created"),
                "spawn_attempted": delegation.get("spawn_attempted"),
                "fallback_mode": delegation.get("fallback_mode"),
                "sidecar_count": len(delegated_sidecars),
            },
        )

    def _runtime_artifact_paths(self) -> list[str]:
        """Return the existing runtime/session artifacts relevant for replay and recovery."""

        return self.checkpointer.artifact_paths(codex_home=self.settings.codex_home)

    def _write_resume_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        status: str,
        artifact_paths: list[str] | None = None,
    ) -> None:
        """Write the runtime-owned resume manifest when trace artifacts are configured."""

        artifact_paths = artifact_paths or self._runtime_artifact_paths()
        self.trace_service.checkpoint(
            session_id=session_id,
            job_id=job_id,
            status=status,
            artifact_paths=artifact_paths,
            supervisor_projection=self._build_supervisor_projection().model_dump(mode="json"),
        )
