"""Core runtime orchestration for Codex on Agno."""

from __future__ import annotations

import asyncio
import json
import os
import uuid
from datetime import UTC, datetime, timedelta
from typing import Any

from codex_agno_runtime.checkpoint_store import FilesystemRuntimeCheckpointer
from codex_agno_runtime.trace import TraceSupervisorProjection
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.middleware import (
    ContextCompressionMiddleware,
    MemoryMiddleware,
    MiddlewareChain,
    MiddlewareContext,
    SkillInjectionMiddleware,
    SubagentLimitMiddleware,
)
from codex_agno_runtime.schemas import (
    BackgroundRunRequest,
    BackgroundRunStatus,
    PrepareSessionRequest,
    PrepareSessionResponse,
    RoutingResult,
    RunTaskRequest,
    RunTaskResponse,
)
from codex_agno_runtime.services import (
    ExecutionEnvironmentService,
    MemoryService,
    RouterService,
    StateService,
    TraceService,
)
from codex_agno_runtime.state import SessionConflictError, TERMINAL_JOB_STATUSES
from codex_agno_runtime.utils import build_session_id

# Maximum number of concurrently running background jobs.
_MAX_BACKGROUND_JOBS = int(os.environ.get("CODEX_MAX_BACKGROUND_JOBS", 16))

# Timeout (seconds) for a single background job run.
_BACKGROUND_JOB_TIMEOUT = float(os.environ.get("CODEX_BACKGROUND_JOB_TIMEOUT", 600))

_SUPPORTED_BACKGROUND_MULTITASK_STRATEGIES = {"reject", "interrupt"}


def _now_iso() -> str:
    """Return a canonical UTC timestamp."""

    return datetime.now(UTC).isoformat()


class CodexAgnoRuntime:
    """High-level runtime that routes tasks and delegates execution through a kernel seam."""

    def __init__(self, settings: RuntimeSettings) -> None:
        self.settings = settings

        self.checkpointer = FilesystemRuntimeCheckpointer(
            data_dir=self.settings.resolved_data_dir,
            trace_output_path=self.settings.resolved_trace_output_path,
        )
        self.router_service = RouterService(settings)
        self.state_service = StateService(self.checkpointer)
        self.trace_service = TraceService(self.checkpointer)
        self.memory_service = MemoryService(settings)
        self.execution_service = ExecutionEnvironmentService(
            settings,
            self.router_service.prompt_builder,
            max_background_jobs=_MAX_BACKGROUND_JOBS,
            background_job_timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
        )

        self.loader = self.router_service.loader
        self.prompt_builder = self.router_service.prompt_builder
        self.skills = self.router_service.skills
        self.router = self.router_service._python_router
        self._job_store = self.state_service.store
        self._trace = self.trace_service.recorder
        self._memory_store = self.memory_service.store

        self._max_background_jobs = _MAX_BACKGROUND_JOBS
        self._jobs_lock: asyncio.Lock = asyncio.Lock()
        self._job_semaphore: asyncio.Semaphore = asyncio.Semaphore(self._max_background_jobs)
        self.background_jobs: dict[str, BackgroundRunStatus] = self.state_service.snapshot()
        self._background_tasks: dict[str, asyncio.Task[None]] = {}

        self._middleware_chain = self._build_middleware_chain()

    def startup(self) -> None:
        """Start runtime service boundaries."""

        for service in (
            self.router_service,
            self.state_service,
            self.trace_service,
            self.memory_service,
            self.execution_service,
        ):
            service.startup()
        self._refresh_router()

    def shutdown(self) -> None:
        """Shutdown runtime service boundaries."""

        for task in list(self._background_tasks.values()):
            if not task.done():
                task.cancel()
        for service in (
            self.execution_service,
            self.memory_service,
            self.trace_service,
            self.state_service,
            self.router_service,
        ):
            service.shutdown()

    def health(self) -> dict[str, Any]:
        """Return health information for each runtime service seam."""

        return {
            "router": self.router_service.health(),
            "state": self.state_service.health(),
            "trace": self.trace_service.health(),
            "memory": self.memory_service.health(),
            "execution_environment": self.execution_service.health(),
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
        if include_prompt_preview:
            routing_result.prompt_preview = self.prompt_builder.build_prompt(routing_result)
        user_id = request.user_id or request.project_id or "codex-user"
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
                "rollback_to_python": routing_result.rollback_to_python,
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
                "rollback_to_python": routing_result.rollback_to_python,
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
            rollback_to_python=routing_result.rollback_to_python,
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
            user_id=prepared.user_id,
            routing_result=routing_result,
            execution_kernel=kernel_contract.get("execution_kernel"),
            execution_kernel_authority=kernel_contract.get("execution_kernel_authority"),
            execution_kernel_delegate=kernel_contract.get("execution_kernel_delegate"),
            execution_kernel_delegate_authority=kernel_contract.get("execution_kernel_delegate_authority"),
        )
        ctx.metadata["dry_run"] = execution_is_dry_run
        ctx.metadata["python_prompt_required"] = execution_is_dry_run

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
                kind="run.failed",
                stage="execution",
                payload={"error": str(error)},
            )
            raise

        self._trace.record(
            session_id=prepared.session_id,
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
        multitask_strategy = request.multitask_strategy.casefold().strip()

        if multitask_strategy not in _SUPPORTED_BACKGROUND_MULTITASK_STRATEGIES:
            status = self._job_store.set_status(
                job_id,
                status="failed",
                session_id=effective_session_id,
                multitask_strategy=request.multitask_strategy,
                error=(
                    "Unsupported multitask strategy: "
                    f"{request.multitask_strategy}. Supported strategies: "
                    f"{', '.join(sorted(_SUPPORTED_BACKGROUND_MULTITASK_STRATEGIES))}"
                ),
                timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                max_attempts=request.max_attempts,
                backoff_base_seconds=request.backoff_base_seconds,
                backoff_multiplier=request.backoff_multiplier,
                max_backoff_seconds=request.max_backoff_seconds,
            )
            async with self._jobs_lock:
                self.background_jobs[job_id] = status
            self._trace.record(
                session_id=effective_session_id,
                job_id=job_id,
                kind="job.failed",
                stage="background",
                payload={"error": status.error, "multitask_strategy": request.multitask_strategy},
            )
            return status

        if multitask_strategy == "interrupt":
            try:
                active_job_id = await self._reserve_background_multitask_takeover(
                    session_id=effective_session_id,
                    incoming_job_id=job_id,
                )
            except SessionConflictError as error:
                status = self._job_store.set_status(
                    job_id,
                    status="failed",
                    session_id=effective_session_id,
                    multitask_strategy=request.multitask_strategy,
                    error=str(error),
                    timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                    max_attempts=request.max_attempts,
                    backoff_base_seconds=request.backoff_base_seconds,
                    backoff_multiplier=request.backoff_multiplier,
                    max_backoff_seconds=request.max_backoff_seconds,
                )
                async with self._jobs_lock:
                    self.background_jobs[job_id] = status
                self._trace.record(
                    session_id=effective_session_id,
                    job_id=job_id,
                    kind="job.failed",
                    stage="background",
                    payload={"error": str(error)},
                )
                return status
            if active_job_id is not None:
                self._trace.record(
                    session_id=effective_session_id,
                    job_id=active_job_id,
                    kind="job.multitask_preempt_requested",
                    stage="background",
                    payload={"multitask_strategy": multitask_strategy, "incoming_job_id": job_id},
                )
                await self.request_background_interrupt(active_job_id)
                try:
                    await self._wait_for_background_session_release(effective_session_id)
                except Exception:
                    async with self._jobs_lock:
                        self._job_store.release_session_takeover(
                            session_id=effective_session_id,
                            incoming_job_id=job_id,
                        )
                    raise
                async with self._jobs_lock:
                    self._job_store.claim_session_takeover(
                        session_id=effective_session_id,
                        incoming_job_id=job_id,
                    )

        try:
            async with self._jobs_lock:
                if self._job_store.active_job_count() >= self._max_background_jobs:
                    if multitask_strategy == "interrupt":
                        self._job_store.release_session_takeover(
                            session_id=effective_session_id,
                            incoming_job_id=job_id,
                        )
                    status = self._job_store.set_status(
                        job_id,
                        status="failed",
                        session_id=effective_session_id,
                        multitask_strategy=request.multitask_strategy,
                        error=(
                            "Too many admitted background jobs "
                            f"({self._job_store.active_job_count()}/{self._max_background_jobs})"
                        ),
                        timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                        max_attempts=request.max_attempts,
                        backoff_base_seconds=request.backoff_base_seconds,
                        backoff_multiplier=request.backoff_multiplier,
                        max_backoff_seconds=request.max_backoff_seconds,
                    )
                else:
                    status = self._job_store.set_status(
                        job_id,
                        status="queued",
                        session_id=effective_session_id,
                        multitask_strategy=request.multitask_strategy,
                        timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                        max_attempts=request.max_attempts,
                        backoff_base_seconds=request.backoff_base_seconds,
                        backoff_multiplier=request.backoff_multiplier,
                        max_backoff_seconds=request.max_backoff_seconds,
                    )
                self.background_jobs[job_id] = status
        except SessionConflictError as error:
            if multitask_strategy == "interrupt":
                async with self._jobs_lock:
                    self._job_store.release_session_takeover(
                        session_id=effective_session_id,
                        incoming_job_id=job_id,
                    )
                    status = self._job_store.set_status(
                        job_id,
                        status="failed",
                        session_id=effective_session_id,
                        multitask_strategy=request.multitask_strategy,
                        error=str(error),
                        timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                        max_attempts=request.max_attempts,
                        backoff_base_seconds=request.backoff_base_seconds,
                        backoff_multiplier=request.backoff_multiplier,
                        max_backoff_seconds=request.max_backoff_seconds,
                    )
                    self.background_jobs[job_id] = status
            else:
                async with self._jobs_lock:
                    status = self._job_store.set_status(
                        job_id,
                        status="failed",
                        session_id=effective_session_id,
                        multitask_strategy=request.multitask_strategy,
                        error=str(error),
                        timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                        max_attempts=request.max_attempts,
                        backoff_base_seconds=request.backoff_base_seconds,
                        backoff_multiplier=request.backoff_multiplier,
                        max_backoff_seconds=request.max_backoff_seconds,
                    )
                    self.background_jobs[job_id] = status
        if status.status == "failed":
            self._trace.record(
                session_id=effective_session_id,
                job_id=job_id,
                kind="job.failed",
                stage="background",
                payload={"error": status.error},
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
                "multitask_strategy": request.multitask_strategy,
                "timeout_seconds": _BACKGROUND_JOB_TIMEOUT,
                "capacity_limit": self._max_background_jobs,
                "max_attempts": request.max_attempts,
                "backoff_base_seconds": request.backoff_base_seconds,
                "backoff_multiplier": request.backoff_multiplier,
                "max_backoff_seconds": request.max_backoff_seconds,
            },
        )

        task = asyncio.create_task(self._run_background_job(job_id, request))
        self._background_tasks[job_id] = task
        return status

    async def _reserve_background_multitask_takeover(
        self,
        *,
        session_id: str,
        incoming_job_id: str,
    ) -> str | None:
        """Reserve a handoff slot so session preemption stays single-owner."""

        async with self._jobs_lock:
            return self._job_store.reserve_session_takeover(
                session_id=session_id,
                incoming_job_id=incoming_job_id,
            )

    async def _wait_for_background_session_release(self, session_id: str, *, timeout_seconds: float = 5.0) -> None:
        """Wait until a session is no longer reserved by an active background job."""

        deadline = asyncio.get_running_loop().time() + timeout_seconds
        while asyncio.get_running_loop().time() < deadline:
            if self._job_store.get_active_job(session_id) is None:
                return
            await asyncio.sleep(0.01)
        raise RuntimeError(
            f"Timed out waiting for background session {session_id!r} to become available."
        )

    async def request_background_interrupt(self, job_id: str) -> BackgroundRunStatus | None:
        """Request interruption of a queued, running, or retry-scheduled job."""

        async with self._jobs_lock:
            current = self._job_store.get(job_id)
            if current is None:
                return None
            if current.status in TERMINAL_JOB_STATUSES:
                self.background_jobs[job_id] = current
                return current

            session_id = current.session_id or job_id
            finalize_immediately = current.status in {"queued", "retry_scheduled"}
            updated = self._job_store.set_status(
                job_id,
                status="interrupt_requested",
                session_id=current.session_id,
                result=current.result,
                error=current.error,
                timeout_seconds=current.timeout_seconds,
                claimed_by=current.claimed_by,
                attempt=current.attempt,
                retry_count=current.retry_count,
                max_attempts=current.max_attempts,
                claimed_at=current.claimed_at,
                backoff_base_seconds=current.backoff_base_seconds,
                backoff_multiplier=current.backoff_multiplier,
                max_backoff_seconds=current.max_backoff_seconds,
                backoff_seconds=current.backoff_seconds,
                next_retry_at=current.next_retry_at,
                retry_scheduled_at=current.retry_scheduled_at,
                retry_claimed_at=current.retry_claimed_at,
                interrupt_requested_at=_now_iso(),
                interrupted_at=current.interrupted_at,
                last_attempt_started_at=current.last_attempt_started_at,
                last_attempt_finished_at=current.last_attempt_finished_at,
                last_failure_at=current.last_failure_at,
            )
            self.background_jobs[job_id] = updated

        self._trace.record(
            session_id=session_id,
            job_id=job_id,
            kind="job.interrupt_requested",
            stage="background",
            payload={"attempt": updated.attempt, "status": updated.status},
        )

        task = self._background_tasks.get(job_id)
        if task is not None and not task.done():
            task.cancel()
        if finalize_immediately or task is None or task.done():
            return await self._mark_background_interrupted(job_id, session_id=session_id, error="Interrupt requested")
        return updated

    def get_background_status(self, job_id: str) -> BackgroundRunStatus | None:
        """Return the latest background job state."""

        status = self._job_store.get(job_id)
        if status is not None:
            self.background_jobs[job_id] = status
            return status
        return self.background_jobs.get(job_id)

    async def _run_background_job(self, job_id: str, request: BackgroundRunRequest) -> None:
        """Execute one background job with explicit retry/backoff semantics."""

        current_task = asyncio.current_task()
        try:
            while True:
                current = self._job_store.get(job_id)
                if current is None:
                    return
                if current.status == "interrupt_requested":
                    await self._mark_background_interrupted(
                        job_id,
                        session_id=current.session_id or job_id,
                        error="Interrupt requested before execution",
                    )
                    return

                async with self._job_semaphore:
                    started_at = _now_iso()
                    async with self._jobs_lock:
                        running = self._job_store.set_status(
                            job_id,
                            status="running",
                            session_id=request.session_id,
                            result=current.result,
                            error=None,
                            timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                            claimed_by=job_id,
                            attempt=current.attempt,
                            retry_count=current.retry_count,
                            max_attempts=current.max_attempts,
                            claimed_at=started_at,
                            backoff_base_seconds=current.backoff_base_seconds,
                            backoff_multiplier=current.backoff_multiplier,
                            max_backoff_seconds=current.max_backoff_seconds,
                            backoff_seconds=current.backoff_seconds,
                            next_retry_at=current.next_retry_at,
                            retry_scheduled_at=current.retry_scheduled_at,
                            retry_claimed_at=current.retry_claimed_at,
                            interrupt_requested_at=current.interrupt_requested_at,
                            interrupted_at=current.interrupted_at,
                            last_attempt_started_at=started_at,
                            last_attempt_finished_at=current.last_attempt_finished_at,
                            last_failure_at=current.last_failure_at,
                        )
                        self.background_jobs[job_id] = running
                    self._trace.record(
                        session_id=request.session_id or job_id,
                        job_id=job_id,
                        kind="job.claimed",
                        stage="background",
                        payload={
                            "status": "running",
                            "attempt": running.attempt,
                            **self.execution_service.kernel_payload(
                                dry_run=self.execution_service.resolve_dry_run(
                                    request_dry_run=request.dry_run
                                ),
                            ),
                        },
                    )

                    try:
                        result = await asyncio.wait_for(
                            self.run_task(request),
                            timeout=_BACKGROUND_JOB_TIMEOUT,
                        )
                    except asyncio.CancelledError:
                        await self._mark_background_interrupted(
                            job_id,
                            session_id=request.session_id or job_id,
                            error="Interrupt requested",
                        )
                        raise
                    except TimeoutError:
                        if await self._schedule_retry_or_finalize(
                            job_id,
                            session_id=request.session_id or job_id,
                            error=f"Job timed out after {_BACKGROUND_JOB_TIMEOUT}s",
                        ):
                            continue
                        return
                    except Exception as error:
                        if await self._schedule_retry_or_finalize(
                            job_id,
                            session_id=request.session_id or job_id,
                            error=str(error),
                        ):
                            continue
                        return

                    completed_at = _now_iso()
                    should_interrupt = False
                    async with self._jobs_lock:
                        latest = self._job_store.get(job_id)
                        if latest is None:
                            return
                        if latest.status == "interrupt_requested":
                            should_interrupt = True
                        else:
                            completed = self._job_store.set_status(
                                job_id,
                                status="completed",
                                session_id=result.session_id,
                                result=result,
                                timeout_seconds=_BACKGROUND_JOB_TIMEOUT,
                                claimed_by=job_id,
                                attempt=latest.attempt,
                                retry_count=latest.retry_count,
                                max_attempts=latest.max_attempts,
                                claimed_at=latest.claimed_at,
                                backoff_base_seconds=latest.backoff_base_seconds,
                                backoff_multiplier=latest.backoff_multiplier,
                                max_backoff_seconds=latest.max_backoff_seconds,
                                backoff_seconds=latest.backoff_seconds,
                                next_retry_at=None,
                                retry_scheduled_at=latest.retry_scheduled_at,
                                retry_claimed_at=latest.retry_claimed_at,
                                interrupt_requested_at=latest.interrupt_requested_at,
                                interrupted_at=latest.interrupted_at,
                                last_attempt_started_at=latest.last_attempt_started_at,
                                last_attempt_finished_at=completed_at,
                                last_failure_at=latest.last_failure_at,
                            )
                            self.background_jobs[job_id] = completed
                    if should_interrupt:
                        await self._mark_background_interrupted(
                            job_id,
                            session_id=result.session_id,
                            error="Interrupt requested",
                        )
                        return
                    self._trace.record(
                        session_id=result.session_id,
                        job_id=job_id,
                        kind="job.completed",
                        stage="background",
                        payload={
                            "status": "completed",
                            "attempt": completed.attempt,
                            **self.execution_service.kernel_payload(
                                dry_run=not result.live_run,
                                metadata=result.metadata,
                            ),
                        },
                    )
                    self._write_resume_manifest(
                        session_id=result.session_id,
                        job_id=job_id,
                        status="completed",
                    )
                    return
        except asyncio.CancelledError:
            current = self._job_store.get(job_id)
            if current is not None and current.status not in TERMINAL_JOB_STATUSES:
                await self._mark_background_interrupted(
                    job_id,
                    session_id=current.session_id or job_id,
                    error="Interrupt requested",
                )
            raise
        finally:
            if self._background_tasks.get(job_id) is current_task:
                self._background_tasks.pop(job_id, None)

    async def _schedule_retry_or_finalize(self, job_id: str, *, session_id: str, error: str) -> bool:
        """Record failure, then either schedule deterministic retry or finalize."""

        failed_at = _now_iso()
        should_interrupt = False
        async with self._jobs_lock:
            current = self._job_store.get(job_id)
            if current is None:
                return False
            if current.status == "interrupt_requested":
                should_interrupt = True
            else:
                self._trace.record(
                    session_id=session_id,
                    job_id=job_id,
                    kind="job.failed",
                    stage="background",
                    payload={"error": error, "attempt": current.attempt},
                )

            if should_interrupt:
                pass
            elif current.attempt >= current.max_attempts:
                terminal_status = "retry_exhausted" if current.max_attempts > 1 else "failed"
                finalized = self._job_store.set_status(
                    job_id,
                    status=terminal_status,
                    session_id=current.session_id,
                    result=current.result,
                    error=error,
                    timeout_seconds=current.timeout_seconds,
                    claimed_by=current.claimed_by,
                    attempt=current.attempt,
                    retry_count=current.retry_count,
                    max_attempts=current.max_attempts,
                    claimed_at=current.claimed_at,
                    backoff_base_seconds=current.backoff_base_seconds,
                    backoff_multiplier=current.backoff_multiplier,
                    max_backoff_seconds=current.max_backoff_seconds,
                    backoff_seconds=current.backoff_seconds,
                    next_retry_at=None,
                    retry_scheduled_at=current.retry_scheduled_at,
                    retry_claimed_at=current.retry_claimed_at,
                    interrupt_requested_at=current.interrupt_requested_at,
                    interrupted_at=current.interrupted_at,
                    last_attempt_started_at=current.last_attempt_started_at,
                    last_attempt_finished_at=failed_at,
                    last_failure_at=failed_at,
                )
                self.background_jobs[job_id] = finalized
                if terminal_status == "retry_exhausted":
                    self._trace.record(
                        session_id=session_id,
                        job_id=job_id,
                        kind="job.retry_exhausted",
                        stage="background",
                        payload={"attempt": finalized.attempt, "max_attempts": finalized.max_attempts, "error": error},
                    )
                self._write_resume_manifest(
                    session_id=session_id,
                    job_id=job_id,
                    status=terminal_status,
                )
                return False
            else:
                next_retry_count = current.retry_count + 1
                backoff_seconds = self._compute_backoff_seconds(
                    base=current.backoff_base_seconds,
                    multiplier=current.backoff_multiplier,
                    retry_count=next_retry_count,
                    maximum=current.max_backoff_seconds,
                )
                next_retry_at = (
                    datetime.now(UTC) + timedelta(seconds=backoff_seconds)
                ).isoformat()
                scheduled = self._job_store.set_status(
                    job_id,
                    status="retry_scheduled",
                    session_id=current.session_id,
                    result=current.result,
                    error=error,
                    timeout_seconds=current.timeout_seconds,
                    claimed_by=current.claimed_by,
                    attempt=current.attempt,
                    retry_count=next_retry_count,
                    max_attempts=current.max_attempts,
                    claimed_at=current.claimed_at,
                    backoff_base_seconds=current.backoff_base_seconds,
                    backoff_multiplier=current.backoff_multiplier,
                    max_backoff_seconds=current.max_backoff_seconds,
                    backoff_seconds=backoff_seconds,
                    next_retry_at=next_retry_at,
                    retry_scheduled_at=failed_at,
                    retry_claimed_at=current.retry_claimed_at,
                    interrupt_requested_at=current.interrupt_requested_at,
                    interrupted_at=current.interrupted_at,
                    last_attempt_started_at=current.last_attempt_started_at,
                    last_attempt_finished_at=failed_at,
                    last_failure_at=failed_at,
                )
                self.background_jobs[job_id] = scheduled

        if should_interrupt:
            await self._mark_background_interrupted(job_id, session_id=session_id, error="Interrupt requested")
            return False

        self._trace.record(
            session_id=session_id,
            job_id=job_id,
            kind="job.retry_scheduled",
            stage="background",
            payload={
                "attempt": scheduled.attempt,
                "retry_count": scheduled.retry_count,
                "backoff_seconds": scheduled.backoff_seconds,
                "next_retry_at": scheduled.next_retry_at,
                "error": error,
            },
        )

        try:
            await asyncio.sleep(scheduled.backoff_seconds or 0.0)
        except asyncio.CancelledError:
            await self._mark_background_interrupted(
                job_id,
                session_id=session_id,
                error="Interrupt requested during retry backoff",
            )
            raise

        should_interrupt = False
        async with self._jobs_lock:
            current = self._job_store.get(job_id)
            if current is None:
                return False
            if current.status == "interrupt_requested":
                should_interrupt = True
            else:
                retry_claimed_at = _now_iso()
                claimed = self._job_store.set_status(
                    job_id,
                    status="retry_claimed",
                    session_id=current.session_id,
                    result=current.result,
                    error=current.error,
                    timeout_seconds=current.timeout_seconds,
                    claimed_by=job_id,
                    attempt=current.attempt + 1,
                    retry_count=current.retry_count,
                    max_attempts=current.max_attempts,
                    claimed_at=current.claimed_at,
                    backoff_base_seconds=current.backoff_base_seconds,
                    backoff_multiplier=current.backoff_multiplier,
                    max_backoff_seconds=current.max_backoff_seconds,
                    backoff_seconds=current.backoff_seconds,
                    next_retry_at=current.next_retry_at,
                    retry_scheduled_at=current.retry_scheduled_at,
                    retry_claimed_at=retry_claimed_at,
                    interrupt_requested_at=current.interrupt_requested_at,
                    interrupted_at=current.interrupted_at,
                    last_attempt_started_at=current.last_attempt_started_at,
                    last_attempt_finished_at=current.last_attempt_finished_at,
                    last_failure_at=current.last_failure_at,
                )
                self.background_jobs[job_id] = claimed

        if should_interrupt:
            await self._mark_background_interrupted(job_id, session_id=session_id, error="Interrupt requested")
            return False

        self._trace.record(
            session_id=session_id,
            job_id=job_id,
            kind="job.retry_claimed",
            stage="background",
            payload={"attempt": claimed.attempt, "retry_count": claimed.retry_count},
        )
        return True

    async def _mark_background_interrupted(
        self,
        job_id: str,
        *,
        session_id: str,
        error: str,
    ) -> BackgroundRunStatus | None:
        """Finalize a job as interrupted and emit the matching trace event."""

        async with self._jobs_lock:
            current = self._job_store.get(job_id)
            if current is None:
                return None
            if current.status == "interrupted":
                self.background_jobs[job_id] = current
                return current
            interrupted_at = _now_iso()
            interrupted = self._job_store.set_status(
                job_id,
                status="interrupted",
                session_id=current.session_id,
                result=current.result,
                error=error,
                timeout_seconds=current.timeout_seconds,
                claimed_by=current.claimed_by,
                attempt=current.attempt,
                retry_count=current.retry_count,
                max_attempts=current.max_attempts,
                claimed_at=current.claimed_at,
                backoff_base_seconds=current.backoff_base_seconds,
                backoff_multiplier=current.backoff_multiplier,
                max_backoff_seconds=current.max_backoff_seconds,
                backoff_seconds=current.backoff_seconds,
                next_retry_at=current.next_retry_at,
                retry_scheduled_at=current.retry_scheduled_at,
                retry_claimed_at=current.retry_claimed_at,
                interrupt_requested_at=current.interrupt_requested_at or interrupted_at,
                interrupted_at=interrupted_at,
                last_attempt_started_at=current.last_attempt_started_at,
                last_attempt_finished_at=interrupted_at,
                last_failure_at=current.last_failure_at,
            )
            self.background_jobs[job_id] = interrupted

        self._trace.record(
            session_id=session_id,
            job_id=job_id,
            kind="job.interrupted",
            stage="background",
            payload={"attempt": interrupted.attempt, "error": error},
        )
        self._write_resume_manifest(
            session_id=session_id,
            job_id=job_id,
            status="interrupted",
        )
        return interrupted

    def _attach_trace_metadata(self, response: RunTaskResponse, routing_result: RoutingResult) -> None:
        """Stamp final trace metadata onto a response payload."""

        reroute_count = self._trace.count_reroutes(response.session_id)
        retry_count = self._trace.count_retries(response.session_id)
        latest_cursor = self._trace.latest_cursor(session_id=response.session_id)
        stream_state = self._trace.describe_stream()
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
                "trace_event_bridge_supported": True,
                "trace_event_bridge_schema_version": self.trace_service.event_bridge.schema_version,
                "trace_event_schema_version": self._trace.event_schema_version,
                "trace_metadata_schema_version": self._trace.metadata_schema_version,
                "trace_event_sink_schema_version": (
                    self._trace.event_sink.schema_version if self._trace.event_sink is not None else None
                ),
                "trace_replay_cursor_schema_version": (
                    latest_cursor.schema_version if latest_cursor is not None else None
                ),
                "trace_replay_supported": stream_state["replay_supported"],
                "trace_generation": stream_state["generation"],
                "trace_latest_seq": stream_state["latest_seq"],
                "trace_resume_cursor": latest_cursor.model_dump(mode="json") if latest_cursor is not None else None,
                "reroute_count": reroute_count,
                "retry_count": retry_count,
                "route_engine_mode": self.settings.route_engine_mode,
                "route_engine": routing_result.route_engine,
                "rollback_to_python": routing_result.rollback_to_python,
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
            rollback_to_python=prepared.rollback_to_python,
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
        delegation = payload.get("delegation") or {}
        verification = payload.get("verification") or {}
        delegated_sidecars = delegation.get("delegated_sidecars")
        if not isinstance(delegated_sidecars, list):
            delegated_sidecars = []
        return TraceSupervisorProjection(
            supervisor_state_path=str(supervisor_state_path.resolve()),
            active_phase=payload.get("active_phase"),
            verification_status=(
                verification.get("verification_status")
                if isinstance(verification, dict)
                else payload.get("verification_status")
            ),
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

    def _compute_backoff_seconds(
        self,
        *,
        base: float,
        multiplier: float,
        retry_count: int,
        maximum: float | None,
    ) -> float:
        """Return deterministic backoff seconds for the next retry attempt."""

        if retry_count <= 0:
            return 0.0
        if base <= 0:
            return 0.0
        multiplier = multiplier if multiplier > 0 else 1.0
        delay = base * (multiplier ** max(0, retry_count - 1))
        if maximum is not None:
            delay = min(delay, maximum)
        return delay
