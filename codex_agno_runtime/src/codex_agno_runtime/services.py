"""Service boundaries for the Codex Agno runtime."""

from __future__ import annotations

import asyncio
import json
from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from pathlib import Path
import resource
from typing import Any, Mapping
from uuid import uuid4

from codex_agno_runtime.checkpoint_store import RuntimeCheckpointer
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.execution_kernel import (
    ExecutionKernelRequest,
    RouterRsExecutionKernel,
    SANDBOX_CAPABILITY_CATEGORIES,
    SandboxExecutionPolicy,
    SandboxResourceBudget,
    SandboxRuntimeProbe,
)
from codex_agno_runtime.host_adapters import (
    build_control_plane_contract_descriptors,
)
from codex_agno_runtime.memory import FactMemoryStore
from codex_agno_runtime.middleware import MiddlewareContext
from codex_agno_runtime.prompt_builder import PromptBuilder
from codex_agno_runtime.router import SkillRouter
from codex_agno_runtime.rust_router import RustRouteAdapter
from codex_agno_runtime.schemas import (
    BackgroundRunRequest,
    BackgroundRunStatus,
    RouteDecisionSnapshot,
    RouteExecutionPolicy,
    RouteDiffReport,
    RoutingResult,
    RunTaskResponse,
)
from codex_agno_runtime.skill_loader import SkillLoader
from codex_agno_runtime.state import (
    BackgroundJobStatusMutation,
    BackgroundJobStore,
    BackgroundSessionTakeoverArbitration,
    TERMINAL_JOB_STATUSES,
)
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


def _runtime_control_plane_service_descriptor(
    control_plane_descriptor: Mapping[str, Any] | None,
    service_name: str,
) -> dict[str, Any]:
    if not isinstance(control_plane_descriptor, Mapping):
        return {}
    services = control_plane_descriptor.get("services")
    if not isinstance(services, Mapping):
        return {}
    service = services.get(service_name)
    if not isinstance(service, Mapping):
        return {}
    return dict(service)


def _runtime_control_plane_rustification_status(
    control_plane_descriptor: Mapping[str, Any] | None,
) -> dict[str, Any]:
    if not isinstance(control_plane_descriptor, Mapping):
        return {}
    status = control_plane_descriptor.get("rustification_status")
    if not isinstance(status, Mapping):
        return {}
    return dict(status)


def _runtime_background_effect_host_contract(
    control_plane_descriptor: Mapping[str, Any] | None,
    service_descriptor: Mapping[str, Any] | None,
    *,
    service_name: str,
) -> dict[str, Any]:
    """Project the runtime-wide host ownership signal into one service seam."""

    rustification_status = _runtime_control_plane_rustification_status(control_plane_descriptor)
    if isinstance(control_plane_descriptor, Mapping):
        runtime_control_plane_authority = control_plane_descriptor.get("authority")
        runtime_control_plane_schema_version = control_plane_descriptor.get("schema_version")
        python_host_role = control_plane_descriptor.get("python_host_role")
    else:
        runtime_control_plane_authority = None
        runtime_control_plane_schema_version = None
        python_host_role = None
    if not isinstance(service_descriptor, Mapping):
        service_descriptor = {}
    progression = {
        "runtime_primary_owner": rustification_status.get("runtime_primary_owner"),
        "runtime_primary_owner_authority": rustification_status.get("runtime_primary_owner_authority"),
        "python_runtime_role": rustification_status.get("python_runtime_role"),
        "steady_state_python_allowed": rustification_status.get("steady_state_python_allowed"),
    }
    return {
        "service": service_name,
        "control_plane_authority": service_descriptor.get("authority"),
        "control_plane_role": service_descriptor.get("role"),
        "control_plane_projection": service_descriptor.get("projection"),
        "control_plane_delegate_kind": service_descriptor.get("delegate_kind"),
        "runtime_control_plane_authority": runtime_control_plane_authority,
        "runtime_control_plane_schema_version": runtime_control_plane_schema_version,
        "python_host_role": python_host_role,
        "steady_state_owner": progression["runtime_primary_owner"],
        "steady_state_owner_authority": progression["runtime_primary_owner_authority"],
        "remaining_python_role": progression["python_runtime_role"],
        "progression": progression,
    }


def _now_iso() -> str:
    """Return a canonical UTC timestamp."""

    return datetime.now(UTC).isoformat()


_SANDBOX_LIFECYCLE_STATES = (
    "created",
    "warm",
    "busy",
    "draining",
    "recycled",
    "failed",
)
_SANDBOX_ALLOWED_TRANSITIONS = {
    ("created", "warm"),
    ("warm", "busy"),
    ("busy", "draining"),
    ("draining", "recycled"),
    ("draining", "failed"),
    ("warm", "failed"),
    ("busy", "failed"),
    ("recycled", "warm"),
}


class SandboxLifecycleError(RuntimeError):
    """Raised when the sandbox state machine is driven through an invalid edge."""


class SandboxCapabilityViolation(RuntimeError):
    """Raised when sandbox capability policy denies one execution."""


class SandboxBudgetExceeded(RuntimeError):
    """Raised when runtime sandbox budget enforcement rejects one execution."""


@dataclass(slots=True)
class _SandboxRecord:
    """Mutable sandbox lifecycle record kept by the execution service."""

    sandbox_id: str
    policy: SandboxExecutionPolicy
    budget: SandboxResourceBudget
    state: str = "created"
    created_at: str = field(default_factory=_now_iso)
    last_event_at: str = field(default_factory=_now_iso)
    current_session_id: str | None = None
    current_job_id: str | None = None
    reuse_count: int = 0
    cleanup_attempts: int = 0
    cleanup_pending: bool = False
    quarantined: bool = False
    last_failure_reason: str | None = None
    last_budget_violation: str | None = None

    def as_payload(self) -> dict[str, Any]:
        """Serialize the record for health surfaces."""

        return {
            "sandbox_id": self.sandbox_id,
            "profile": self.policy.profile,
            "capability_categories": list(self.policy.capability_categories),
            "dedicated_profile": self.policy.dedicated_profile,
            "reusable": self.policy.reusable,
            "state": self.state,
            "created_at": self.created_at,
            "last_event_at": self.last_event_at,
            "current_session_id": self.current_session_id,
            "current_job_id": self.current_job_id,
            "reuse_count": self.reuse_count,
            "cleanup_attempts": self.cleanup_attempts,
            "cleanup_pending": self.cleanup_pending,
            "quarantined": self.quarantined,
            "last_failure_reason": self.last_failure_reason,
            "last_budget_violation": self.last_budget_violation,
            "budget": self.budget.to_metadata(),
        }


class SandboxLifecycleService:
    """Minimal sandbox lifecycle manager backing the frozen runtime contract."""

    schema_version = "runtime-sandbox-lifecycle-v1"

    def __init__(
        self,
        settings: RuntimeSettings,
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.settings = settings
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")
        self._event_log_path = settings.resolved_data_dir / "runtime_sandbox_events.jsonl"
        self._sandboxes: dict[str, _SandboxRecord] = {}
        self._cleanup_tasks: dict[str, asyncio.Task[None]] = {}
        self._next_cleanup_failure_reason: str | None = None

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")

    def startup(self) -> None:
        """Ensure the durable sandbox event log directory exists."""

        self._event_log_path.parent.mkdir(parents=True, exist_ok=True)

    def shutdown(self) -> None:
        """Cancel pending cleanup tasks during runtime shutdown."""

        for task in list(self._cleanup_tasks.values()):
            if not task.done():
                task.cancel()

    def fail_next_cleanup(self, reason: str) -> None:
        """Inject one cleanup failure for deterministic tests."""

        self._next_cleanup_failure_reason = reason

    async def execute(
        self,
        request: ExecutionKernelRequest,
        *,
        executor: Callable[[ExecutionKernelRequest], Awaitable[RunTaskResponse]],
    ) -> RunTaskResponse:
        """Admit one execution into the sandbox state machine and enforce its contract."""

        record = self._acquire_record(request)
        self._validate_policy(request, record)
        self._validate_budget(request.sandbox_budget, record=record, request=request)
        self._transition(record, "busy", event_kind="sandbox.execution_started", request=request)
        usage_before = self._usage_snapshot()
        started_at = asyncio.get_running_loop().time()
        try:
            response = await executor(request)
        except Exception as error:
            self._handle_execution_failure(record, request=request, error=error)
            raise

        runtime_probe = self._build_runtime_probe(
            request=request,
            response=response,
            started_at=started_at,
            usage_before=usage_before,
        )
        violation = self._detect_budget_violation(request.sandbox_budget, runtime_probe)
        if violation is not None:
            record.last_failure_reason = violation
            record.last_budget_violation = violation
            self._transition(
                record,
                "draining",
                event_kind="sandbox.budget_exceeded",
                request=request,
                detail={"failure_reason": violation, "runtime_probe": runtime_probe.to_metadata()},
            )
            self._schedule_cleanup(record)
            raise SandboxBudgetExceeded(violation)

        self._transition(
            record,
            "draining",
            event_kind="sandbox.execution_completed",
            request=request,
            detail={"runtime_probe": runtime_probe.to_metadata()},
        )
        self._schedule_cleanup(record)
        response.metadata.update(
            {
                "sandbox_schema_version": self.schema_version,
                "sandbox_id": record.sandbox_id,
                "sandbox_profile": record.policy.profile,
                "sandbox_tool_category": request.sandbox_tool_category,
                "sandbox_state": record.state,
                "sandbox_capability_categories": list(record.policy.capability_categories),
                "sandbox_budget": request.sandbox_budget.to_metadata(),
                "sandbox_runtime_probe": runtime_probe.to_metadata(),
                "sandbox_cleanup_pending": record.cleanup_pending,
                "sandbox_event_log_path": str(self._event_log_path),
            }
        )
        return response

    async def await_cleanup(self, sandbox_id: str) -> dict[str, Any]:
        """Wait until one sandbox cleanup task settles and return the latest record."""

        task = self._cleanup_tasks.get(sandbox_id)
        if task is not None:
            await task
        return self.describe_sandbox(sandbox_id)

    def describe_sandbox(self, sandbox_id: str) -> dict[str, Any]:
        """Return one sandbox record for tests and health reporting."""

        record = self._sandboxes.get(sandbox_id)
        if record is None:
            raise KeyError(f"unknown sandbox {sandbox_id!r}")
        return record.as_payload()

    def health(self) -> dict[str, Any]:
        """Summarize sandbox lifecycle state, durability, and cleanup activity."""

        state_counts = {state: 0 for state in _SANDBOX_LIFECYCLE_STATES}
        for record in self._sandboxes.values():
            state_counts[record.state] += 1
        active_cleanup = [
            sandbox_id
            for sandbox_id, task in self._cleanup_tasks.items()
            if not task.done()
        ]
        return {
            "schema_version": self.schema_version,
            "lifecycle_states": list(_SANDBOX_LIFECYCLE_STATES),
            "allowed_transitions": [list(edge) for edge in sorted(_SANDBOX_ALLOWED_TRANSITIONS)],
            "capability_categories": list(SANDBOX_CAPABILITY_CATEGORIES),
            "event_log_path": str(self._event_log_path),
            "state_counts": state_counts,
            "active_cleanup_tasks": len(active_cleanup),
            "active_cleanup_sandboxes": active_cleanup,
            "sandbox_count": len(self._sandboxes),
            "latest_records": [record.as_payload() for record in list(self._sandboxes.values())[-5:]],
            "background_effect_host_contract": _runtime_background_effect_host_contract(
                self._control_plane_descriptor,
                self._service_descriptor,
                service_name="execution",
            ),
        }

    def _usage_snapshot(self) -> dict[str, float]:
        self_usage = resource.getrusage(resource.RUSAGE_SELF)
        child_usage = resource.getrusage(resource.RUSAGE_CHILDREN)
        return {
            "self_cpu": float(self_usage.ru_utime + self_usage.ru_stime),
            "child_cpu": float(child_usage.ru_utime + child_usage.ru_stime),
            "self_memory": float(self_usage.ru_maxrss),
            "child_memory": float(child_usage.ru_maxrss),
        }

    def _acquire_record(self, request: ExecutionKernelRequest) -> _SandboxRecord:
        for record in self._sandboxes.values():
            if not self._matches_policy(record, request.sandbox_policy):
                continue
            if record.state == "failed" or record.quarantined:
                continue
            if record.state == "recycled":
                record.reuse_count += 1
                self._transition(record, "warm", event_kind="sandbox.rewarmed", request=request)
                record.current_session_id = request.session_id
                record.current_job_id = request.session_id
                return record
            if record.state == "warm":
                record.current_session_id = request.session_id
                record.current_job_id = request.session_id
                return record

        record = _SandboxRecord(
            sandbox_id=f"sandbox-{uuid4().hex[:12]}",
            policy=request.sandbox_policy,
            budget=request.sandbox_budget,
            current_session_id=request.session_id,
            current_job_id=request.session_id,
        )
        self._sandboxes[record.sandbox_id] = record
        self._record_event(
            record,
            event_kind="sandbox.created",
            request=request,
            detail={"budget": request.sandbox_budget.to_metadata()},
        )
        self._transition(record, "warm", event_kind="sandbox.warmed", request=request)
        return record

    @staticmethod
    def _matches_policy(record: _SandboxRecord, policy: SandboxExecutionPolicy) -> bool:
        return (
            record.policy.profile == policy.profile
            and record.policy.capability_categories == policy.capability_categories
            and record.policy.dedicated_profile == policy.dedicated_profile
            and record.policy.reusable == policy.reusable
        )

    def _validate_policy(self, request: ExecutionKernelRequest, record: _SandboxRecord) -> None:
        categories = tuple(request.sandbox_policy.capability_categories)
        if not categories:
            self._mark_failed(
                record,
                request=request,
                reason="policy_violation:missing_capability_declaration",
            )
            raise SandboxCapabilityViolation("policy_violation:missing_capability_declaration")
        for category in categories:
            if category not in SANDBOX_CAPABILITY_CATEGORIES:
                reason = f"policy_violation:unknown_capability:{category}"
                self._mark_failed(record, request=request, reason=reason)
                raise SandboxCapabilityViolation(reason)
        if request.sandbox_tool_category not in SANDBOX_CAPABILITY_CATEGORIES:
            reason = f"policy_violation:unknown_tool_category:{request.sandbox_tool_category}"
            self._mark_failed(record, request=request, reason=reason)
            raise SandboxCapabilityViolation(reason)
        if request.sandbox_tool_category not in categories:
            reason = f"policy_violation:capability_denied:{request.sandbox_tool_category}"
            self._mark_failed(record, request=request, reason=reason)
            raise SandboxCapabilityViolation(reason)
        if request.sandbox_tool_category == "high_risk" and not request.sandbox_policy.dedicated_profile:
            reason = "policy_violation:high_risk_requires_dedicated_profile"
            self._mark_failed(record, request=request, reason=reason)
            raise SandboxCapabilityViolation(reason)

    def _validate_budget(
        self,
        budget: SandboxResourceBudget,
        *,
        record: _SandboxRecord,
        request: ExecutionKernelRequest,
    ) -> None:
        for dimension, value in (
            ("cpu", budget.cpu),
            ("memory", budget.memory),
            ("wall_clock", budget.wall_clock),
            ("output_size", budget.output_size),
        ):
            if value <= 0:
                reason = f"budget_admission_failed:{dimension}_non_positive"
                self._mark_failed(record, request=request, reason=reason)
                raise SandboxBudgetExceeded(reason)

    def _build_runtime_probe(
        self,
        *,
        request: ExecutionKernelRequest,
        response: RunTaskResponse,
        started_at: float,
        usage_before: Mapping[str, float],
    ) -> SandboxRuntimeProbe:
        elapsed = asyncio.get_running_loop().time() - started_at
        usage_after = self._usage_snapshot()
        cpu_used = (usage_after["self_cpu"] - usage_before["self_cpu"]) + (
            usage_after["child_cpu"] - usage_before["child_cpu"]
        )
        memory_used = max(
            usage_after["self_memory"] - usage_before["self_memory"],
            usage_after["child_memory"] - usage_before["child_memory"],
            0.0,
        )
        output_size = len((response.content or "").encode("utf-8"))
        probe = request.sandbox_runtime_probe
        return SandboxRuntimeProbe(
            cpu=probe.cpu if probe is not None and probe.cpu is not None else cpu_used,
            memory=probe.memory if probe is not None and probe.memory is not None else int(memory_used),
            wall_clock=probe.wall_clock if probe is not None and probe.wall_clock is not None else elapsed,
            output_size=probe.output_size if probe is not None and probe.output_size is not None else output_size,
            source=probe.source if probe is not None else "host-runtime-rusage",
        )

    @staticmethod
    def _detect_budget_violation(
        budget: SandboxResourceBudget,
        probe: SandboxRuntimeProbe,
    ) -> str | None:
        for dimension, limit, observed in (
            ("cpu", budget.cpu, probe.cpu),
            ("memory", budget.memory, probe.memory),
            ("wall_clock", budget.wall_clock, probe.wall_clock),
            ("output_size", budget.output_size, probe.output_size),
        ):
            if observed is not None and observed > limit:
                return f"{dimension}_exceeded"
        return None

    def _handle_execution_failure(
        self,
        record: _SandboxRecord,
        *,
        request: ExecutionKernelRequest,
        error: Exception,
    ) -> None:
        if isinstance(error, (TimeoutError, asyncio.TimeoutError)):
            record.last_failure_reason = "wall_clock_exceeded"
            self._transition(
                record,
                "draining",
                event_kind="sandbox.timeout",
                request=request,
                detail={"failure_reason": record.last_failure_reason},
            )
            self._schedule_cleanup(record)
            return
        self._mark_failed(record, request=request, reason=f"execution_failed:{type(error).__name__}")

    def _mark_failed(self, record: _SandboxRecord, *, request: ExecutionKernelRequest, reason: str) -> None:
        record.last_failure_reason = reason
        record.quarantined = True
        self._transition(
            record,
            "failed",
            event_kind="sandbox.failed",
            request=request,
            detail={"failure_reason": reason},
        )

    def _schedule_cleanup(self, record: _SandboxRecord) -> None:
        if record.cleanup_pending:
            return
        record.cleanup_pending = True
        failure_reason = self._next_cleanup_failure_reason
        self._next_cleanup_failure_reason = None
        task = asyncio.create_task(self._cleanup(record.sandbox_id, failure_reason=failure_reason))
        self._cleanup_tasks[record.sandbox_id] = task

    async def _cleanup(self, sandbox_id: str, *, failure_reason: str | None) -> None:
        record = self._sandboxes[sandbox_id]
        record.cleanup_attempts += 1
        self._record_event(
            record,
            event_kind="sandbox.cleanup_started",
            request=None,
        )
        await asyncio.sleep(0)
        if failure_reason is not None:
            record.cleanup_pending = False
            record.quarantined = True
            record.last_failure_reason = failure_reason
            self._transition(
                record,
                "failed",
                event_kind="sandbox.cleanup_failed",
                request=None,
                detail={"failure_reason": failure_reason},
            )
            return
        record.cleanup_pending = False
        record.current_session_id = None
        record.current_job_id = None
        self._transition(
            record,
            "recycled",
            event_kind="sandbox.cleanup_completed",
            request=None,
        )

    def _transition(
        self,
        record: _SandboxRecord,
        next_state: str,
        *,
        event_kind: str,
        request: ExecutionKernelRequest | None,
        detail: Mapping[str, Any] | None = None,
    ) -> None:
        edge = (record.state, next_state)
        if edge not in _SANDBOX_ALLOWED_TRANSITIONS:
            raise SandboxLifecycleError(f"invalid sandbox transition: {record.state!r} -> {next_state!r}")
        record.state = next_state
        record.last_event_at = _now_iso()
        self._record_event(record, event_kind=event_kind, request=request, detail=detail)

    def _record_event(
        self,
        record: _SandboxRecord,
        *,
        event_kind: str,
        request: ExecutionKernelRequest | None,
        detail: Mapping[str, Any] | None = None,
    ) -> None:
        self._event_log_path.parent.mkdir(parents=True, exist_ok=True)
        event = {
            "schema_version": self.schema_version,
            "ts": _now_iso(),
            "kind": event_kind,
            "sandbox_id": record.sandbox_id,
            "state": record.state,
            "profile": record.policy.profile,
            "capability_categories": list(record.policy.capability_categories),
            "dedicated_profile": record.policy.dedicated_profile,
            "session_id": request.session_id if request is not None else record.current_session_id,
            "job_id": request.session_id if request is not None else record.current_job_id,
        }
        if detail:
            event.update(detail)
        with self._event_log_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(event, ensure_ascii=False) + "\n")


class RouterService:
    """Own skill loading plus route-engine selection."""

    def __init__(self, settings: RuntimeSettings, *, rust_adapter: RustRouteAdapter | None = None) -> None:
        self.settings = settings
        self.loader = SkillLoader(settings.codex_home / "skills")
        self.prompt_builder = PromptBuilder(loader=self.loader)
        self._rust_adapter = rust_adapter or RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        self.control_plane_descriptor = self._rust_adapter.runtime_control_plane()
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
        """Refresh runtime skill metadata and the Python router for the legacy lane."""

        self.control_plane_descriptor = self._rust_adapter.runtime_control_plane()
        self.skills = self.loader.load(
            refresh=True,
            load_bodies=not self.settings.progressive_skill_loading,
        )
        policy = self._resolve_route_policy(refresh=True)
        if self._python_router_required(policy=policy):
            self._python_router = SkillRouter(
                self.skills,
                control_plane_descriptor=self.control_plane_descriptor,
            )
        else:
            self._python_router = None

    def route(self, *, task: str, session_id: str, allow_overlay: bool, first_turn: bool) -> RoutingResult:
        """Return the configured route decision for one task."""

        self._last_route_report = None
        policy = self._resolve_route_policy()
        if policy.primary_authority == "python":
            python_result = self._route_python(
                task=task,
                session_id=session_id,
                allow_overlay=allow_overlay,
                first_turn=first_turn,
            )
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
        python_result: RoutingResult | None = None
        # Diagnostic lanes may still snapshot Python for comparison, but Rust owns the route result.
        if policy.diagnostic_python_lane or policy.diff_report_required or policy.verify_parity_required:
            python_result = self._route_python(
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
        return self._decorate_route_result(
            rust_result,
            route_engine="rust",
            rollback_to_python=False,
            report=report,
        )

    def health(self) -> dict[str, Any]:
        """Describe router-service health and the active route engine."""

        policy = self._resolve_route_policy()
        service_descriptor = _runtime_control_plane_service_descriptor(
            self.control_plane_descriptor,
            "router",
        )
        rustification_status = _runtime_control_plane_rustification_status(self.control_plane_descriptor)
        return {
            "mode": self.settings.route_engine_mode,
            "default_route_mode": self.control_plane_descriptor.get("default_route_mode", "rust"),
            "default_route_authority": self.control_plane_descriptor.get(
                "default_route_authority",
                self._rust_adapter.route_authority,
            ),
            "rollback_to_python": policy.primary_authority == "python" and policy.rollback_active,
            "configured_rollback_to_python": self.settings.rust_route_rollback_to_python,
            "loaded_skill_count": len(self.skills),
            "skill_root": str(self.settings.codex_home / "skills"),
            "primary_authority": policy.primary_authority,
            "route_result_engine": policy.route_result_engine,
            "shadow_engine": policy.shadow_engine,
            "python_router_loaded": self._python_router is not None,
            "python_router_required": self._python_router_required(policy=policy),
            "control_plane_authority": service_descriptor.get(
                "authority",
                self.control_plane_descriptor.get("authority"),
            ),
            "control_plane_projection": service_descriptor.get("projection"),
            "control_plane_delegate_kind": service_descriptor.get("delegate_kind"),
            "python_runtime_role": self.control_plane_descriptor.get("python_host_role"),
            "rustification_status": rustification_status,
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
            self._python_router = SkillRouter(
                self.skills,
                control_plane_descriptor=self.control_plane_descriptor,
            )
        return self._python_router


class StateService:
    """Own durable background-job state and session reservations."""

    def __init__(
        self,
        checkpointer: RuntimeCheckpointer,
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.checkpointer = checkpointer
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "state")
        self.state_path = checkpointer.describe_paths().background_state_path
        self.store = BackgroundJobStore(
            state_path=self.state_path,
            storage_backend=getattr(checkpointer, "storage_backend", None),
            control_plane_descriptor=control_plane_descriptor,
        )

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "state")

    def startup(self) -> None:
        """State service startup hook."""

    def shutdown(self) -> None:
        """State service shutdown hook."""

    def set_status(self, job_id: str, **kwargs: Any) -> BackgroundRunStatus:
        return self.store.set_status(job_id, **kwargs)

    def apply_mutation(self, job_id: str, mutation: BackgroundJobStatusMutation) -> BackgroundRunStatus:
        """Apply one pre-built background mutation through the durable state host lane."""

        return self.store.apply_mutation(job_id, mutation)

    def get(self, job_id: str) -> BackgroundRunStatus | None:
        return self.store.get(job_id)

    def snapshot(self) -> dict[str, BackgroundRunStatus]:
        return self.store.snapshot()

    def get_active_job(self, session_id: str) -> str | None:
        return self.store.get_active_job(session_id)

    def active_job_count(self) -> int:
        """Return the number of currently admitted background jobs."""

        return self.store.active_job_count()

    def arbitrate_session_takeover(
        self,
        *,
        session_id: str,
        incoming_job_id: str,
        operation: str,
    ) -> BackgroundSessionTakeoverArbitration:
        """Apply one takeover reducer step through the durable state host lane."""

        return self.store.arbitrate_session_takeover(
            session_id=session_id,
            incoming_job_id=incoming_job_id,
            operation=operation,
        )

    async def wait_for_session_release(
        self,
        *,
        session_id: str,
        timeout_seconds: float,
        poll_interval_seconds: float,
    ) -> None:
        """Wait until the session reservation is released by the active job."""

        deadline = asyncio.get_running_loop().time() + timeout_seconds
        while asyncio.get_running_loop().time() < deadline:
            if self.get_active_job(session_id) is None:
                return
            await asyncio.sleep(poll_interval_seconds)
        raise RuntimeError(
            f"Timed out waiting for background session {session_id!r} to become available."
        )

    async def wait_for_retry_backoff(self, *, backoff_seconds: float) -> None:
        """Wait for the retry backoff window resolved by the Rust control plane."""

        await asyncio.sleep(backoff_seconds)

    def health(self) -> dict[str, Any]:
        return {
            "control_plane_authority": self._service_descriptor.get("authority"),
            "control_plane_role": self._service_descriptor.get("role"),
            "control_plane_projection": self._service_descriptor.get("projection"),
            "control_plane_delegate_kind": self._service_descriptor.get("delegate_kind"),
            "checkpoint_backend_family": self.checkpointer.storage_capabilities().backend_family,
            "state_path": str(self.state_path),
            "job_count": len(self.store.snapshot()),
            "pending_session_takeovers": self.store.pending_session_takeovers(),
            "background_effect_host_contract": _runtime_background_effect_host_contract(
                self._control_plane_descriptor,
                self._service_descriptor,
                service_name="state",
            ),
        }


class TraceService:
    """Own trace recorder wiring and filesystem paths."""

    def __init__(
        self,
        checkpointer: RuntimeCheckpointer,
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.checkpointer = checkpointer
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "trace")
        paths = checkpointer.describe_paths()
        self.output_path = paths.trace_output_path
        self.event_stream_path = paths.event_stream_path
        self.resume_manifest_path = paths.resume_manifest_path
        self.event_transport_dir = paths.event_transport_dir
        self.event_bridge = InMemoryRuntimeEventBridge(control_plane_descriptor=control_plane_descriptor)
        self.recorder = checkpointer.build_trace_recorder(event_bridge=self.event_bridge)
        self.event_bridge.seed(self.recorder.stream_events())

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "trace")

    def startup(self) -> None:
        """Trace service startup hook."""

    def shutdown(self) -> None:
        """Trace service shutdown hook."""
        self.event_bridge.cleanup()

    def control_plane_contract(self) -> dict[str, Any]:
        """Return the Rust-first trace contract projected into the steady-state host."""

        recorder_contract = self.recorder.control_plane_descriptor().model_dump(mode="json")
        bridge_contract = self.event_bridge.control_plane_descriptor().model_dump(mode="json")
        recorder_semantics = dict(recorder_contract)
        bridge_semantics = dict(bridge_contract)
        for payload in (recorder_semantics, bridge_semantics):
            payload.pop("event_stream_path", None)
            payload.pop("trace_output_path", None)
        return {
            "recorder": recorder_contract,
            "bridge": bridge_contract,
            "aligned": recorder_semantics == bridge_semantics,
        }

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
            "control_plane_authority": self._service_descriptor.get("authority"),
            "control_plane_role": self._service_descriptor.get("role"),
            "control_plane_projection": self._service_descriptor.get("projection"),
            "control_plane_delegate_kind": self._service_descriptor.get("delegate_kind"),
            "control_plane_contract": self.control_plane_contract(),
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
            "background_effect_host_contract": _runtime_background_effect_host_contract(
                self._control_plane_descriptor,
                self._service_descriptor,
                service_name="trace",
            ),
        }


class MemoryService:
    """Own memory store lifecycle and health surface."""

    def __init__(
        self,
        settings: RuntimeSettings,
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "memory")
        self.store = FactMemoryStore(
            memory_dir=settings.resolved_memory_dir,
            debounce_seconds=settings.memory_debounce_seconds,
            control_plane_descriptor=control_plane_descriptor,
        )
        self.memory_dir = settings.resolved_memory_dir

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "memory")

    def startup(self) -> None:
        """Memory service startup hook."""

    def shutdown(self) -> None:
        """Memory service shutdown hook."""

    def control_plane_contract(self) -> dict[str, Any]:
        """Return the Rust-first memory contract projected into the steady-state host."""

        return self.store.control_plane_descriptor().model_dump(mode="json")

    def health(self) -> dict[str, Any]:
        payload = self.store.health()
        payload["control_plane_contract"] = self.control_plane_contract()
        return payload


class _RustExecutionKernelAuthorityAdapter:
    """Present a Rust-owned kernel seam while live fallback remains compatibility-safe."""

    adapter_kind = "rust-execution-kernel-slice"
    authority = "rust-execution-kernel-authority"

    def __init__(self, delegate: RouterRsExecutionKernel) -> None:
        self._delegate = delegate

    @staticmethod
    def _contract_mode() -> str:
        return "rust-live-primary"

    async def execute(self, request: ExecutionKernelRequest) -> RunTaskResponse:
        response = await self._delegate.execute(request)
        delegate_health = self._delegate.health()

        def _response_metadata(field: str, default: Any) -> Any:
            if field in response.metadata:
                return response.metadata[field]
            return default

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
        delegate_family_default = delegate_health.get("kernel_live_backend_family")
        delegate_impl_default = delegate_health.get("kernel_live_backend_impl")
        live_fallback_enabled = False
        live_fallback_mode = "disabled"
        response.metadata.update(
            {
                "execution_kernel": self.adapter_kind,
                "execution_kernel_authority": self.authority,
                "execution_kernel_contract_mode": self._contract_mode(),
                "execution_kernel_in_process_replacement_complete": True,
                "execution_kernel_delegate": delegate_kind,
                "execution_kernel_delegate_authority": delegate_authority,
                "execution_kernel_live_primary": _response_metadata(
                    "execution_kernel_live_primary",
                    response.metadata.get("execution_kernel_primary")
                    or delegate_health.get("kernel_live_delegate_primary_kind")
                    or delegate_health.get("kernel_adapter_kind"),
                ),
                "execution_kernel_live_primary_authority": _response_metadata(
                    "execution_kernel_live_primary_authority",
                    response.metadata.get("execution_kernel_primary_authority")
                    or delegate_health.get("kernel_live_delegate_primary_authority")
                    or delegate_health.get("kernel_authority"),
                ),
                "execution_kernel_live_fallback": _response_metadata(
                    "execution_kernel_live_fallback",
                    None,
                ),
                "execution_kernel_live_fallback_authority": _response_metadata(
                    "execution_kernel_live_fallback_authority",
                    None,
                ),
                "execution_kernel_live_fallback_enabled": live_fallback_enabled,
                "execution_kernel_live_fallback_mode": live_fallback_mode,
                "execution_kernel_delegate_family": response.metadata.get(
                    "execution_kernel_delegate_family",
                    delegate_family_default,
                ),
                "execution_kernel_delegate_impl": response.metadata.get(
                    "execution_kernel_delegate_impl",
                    delegate_impl_default,
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
            "kernel_in_process_replacement_complete": True,
            "kernel_live_backend_family": delegate_health.get("kernel_live_backend_family", "rust-cli"),
            "kernel_live_backend_impl": delegate_health.get("kernel_live_backend_impl", "router-rs"),
            "kernel_live_delegate_kind": delegate_health.get("kernel_adapter_kind", "router-rs"),
            "kernel_live_delegate_authority": delegate_health.get("kernel_authority", "rust-execution-cli"),
            "kernel_live_delegate_family": delegate_health.get("kernel_live_backend_family", "rust-cli"),
            "kernel_live_delegate_impl": delegate_health.get("kernel_live_backend_impl", "router-rs"),
            "kernel_live_delegate_mode": "rust-primary",
            "kernel_live_fallback_kind": None,
            "kernel_live_fallback_authority": None,
            "kernel_live_fallback_family": None,
            "kernel_live_fallback_impl": None,
            "kernel_live_fallback_enabled": False,
            "kernel_live_fallback_mode": "disabled",
            "kernel_mode_support": ["dry_run", "live"],
        }

    def contract_descriptor(self, *, dry_run: bool = False) -> dict[str, Any]:
        health = self.health()
        return {
            "execution_kernel": health["kernel_adapter_kind"],
            "execution_kernel_authority": health["kernel_authority"],
            "execution_kernel_contract_mode": health["kernel_contract_mode"],
            "execution_kernel_in_process_replacement_complete": health["kernel_in_process_replacement_complete"],
            "execution_kernel_delegate": health["kernel_live_delegate_kind"],
            "execution_kernel_delegate_authority": health["kernel_live_delegate_authority"],
            "execution_kernel_delegate_family": health["kernel_live_delegate_family"],
            "execution_kernel_delegate_impl": health["kernel_live_delegate_impl"],
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
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.settings = settings
        self.control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")
        self.primary_kernel = RouterRsExecutionKernel(settings)
        self.kernel = _RustExecutionKernelAuthorityAdapter(self.primary_kernel)
        self.sandbox = SandboxLifecycleService(
            settings,
            control_plane_descriptor=control_plane_descriptor,
        )
        self.max_background_jobs = max_background_jobs
        self.background_job_timeout_seconds = background_job_timeout_seconds

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self.control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")
        self.sandbox.refresh_control_plane(control_plane_descriptor)

    def startup(self) -> None:
        """Execution-environment startup hook."""

        self.sandbox.startup()

    def shutdown(self) -> None:
        """Execution-environment shutdown hook."""

        self.sandbox.shutdown()

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

        return await self.execute_request(
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

    async def execute_request(
        self,
        request: ExecutionKernelRequest,
        *,
        executor: Callable[[ExecutionKernelRequest], Awaitable[RunTaskResponse]] | None = None,
    ) -> RunTaskResponse:
        """Run one pre-built kernel request through sandbox admission and cleanup."""

        return await self.sandbox.execute(request, executor=executor or self.kernel.execute)

    async def await_sandbox_cleanup(self, sandbox_id: str) -> dict[str, Any]:
        """Wait for one sandbox cleanup task to settle."""

        return await self.sandbox.await_cleanup(sandbox_id)

    def describe_sandbox(self, sandbox_id: str) -> dict[str, Any]:
        """Return one sandbox lifecycle record."""

        return self.sandbox.describe_sandbox(sandbox_id)

    def fail_next_sandbox_cleanup(self, reason: str) -> None:
        """Inject one cleanup failure for tests without reopening inline handling."""

        self.sandbox.fail_next_cleanup(reason)

    def health(self) -> dict[str, Any]:
        payload = {
            "max_background_jobs": self.max_background_jobs,
            "background_job_timeout_seconds": self.background_job_timeout_seconds,
            "execution_mode_default": "live" if self.settings.use_live_model else "dry_run",
            "control_plane_authority": self._service_descriptor.get(
                "authority",
                self.control_plane_descriptor.get("authority"),
            ),
            "control_plane_role": self._service_descriptor.get("role"),
            "control_plane_projection": self._service_descriptor.get("projection"),
            "control_plane_delegate_kind": self._service_descriptor.get("delegate_kind"),
        }
        payload.update(self.kernel.health())
        payload["sandbox"] = self.sandbox.health()
        payload["control_plane_contracts"] = self.describe_control_plane_contracts()
        return payload

    def describe_control_plane_contracts(self) -> dict[str, Any]:
        """Return control-plane-only descriptors for shared execution artifacts."""

        payload = build_control_plane_contract_descriptors()
        if self.control_plane_descriptor:
            payload["runtime_control_plane"] = self.control_plane_descriptor
        return payload

    def describe_kernel_contract(self, *, dry_run: bool = False) -> dict[str, Any]:
        """Return the stable kernel-owner descriptor used by runtime surfaces."""

        return self.kernel.contract_descriptor(dry_run=dry_run)

    def preview_prompt(
        self,
        *,
        task: str,
        session_id: str,
        user_id: str,
        routing_result: RoutingResult,
    ) -> str | None:
        """Build the dry-run prompt preview through router-rs instead of Python prompt assembly."""

        return self.primary_kernel.preview_prompt(
            ExecutionKernelRequest(
                task=task,
                session_id=session_id,
                user_id=user_id,
                routing_result=routing_result,
                prompt_preview=None,
                dry_run=True,
            )
        )

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


class BackgroundRuntimeHost:
    """Own background task lifecycle under the Rust background-control descriptor."""

    def __init__(
        self,
        *,
        state_service: StateService,
        trace_service: TraceService,
        execution_service: ExecutionEnvironmentService,
        background_control_provider: Callable[[], Callable[[dict[str, Any]], dict[str, Any]]],
        background_control_schema_version: str,
        background_control_authority: str,
        max_background_jobs: int,
        background_job_timeout_seconds: float,
        artifact_paths_provider: Callable[[], list[str]],
        supervisor_projection_provider: Callable[[], dict[str, Any] | None],
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.state_service = state_service
        self.trace_service = trace_service
        self.execution_service = execution_service
        self._background_control_provider = background_control_provider
        self._background_control_schema_version = background_control_schema_version
        self._background_control_authority = background_control_authority
        self._artifact_paths_provider = artifact_paths_provider
        self._supervisor_projection_provider = supervisor_projection_provider
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "background")
        self._max_background_jobs = max_background_jobs
        self._background_job_timeout_seconds = background_job_timeout_seconds
        self._job_store = self.state_service.store
        self._trace = self.trace_service.recorder
        self._jobs_lock: asyncio.Lock = asyncio.Lock()
        self._job_semaphore: asyncio.Semaphore = asyncio.Semaphore(self._max_background_jobs)
        self.background_jobs: dict[str, BackgroundRunStatus] = self.state_service.snapshot()
        self._background_tasks: dict[str, asyncio.Task[None]] = {}

    @property
    def jobs_lock(self) -> asyncio.Lock:
        return self._jobs_lock

    @property
    def job_semaphore(self) -> asyncio.Semaphore:
        return self._job_semaphore

    @job_semaphore.setter
    def job_semaphore(self, semaphore: asyncio.Semaphore) -> None:
        self._job_semaphore = semaphore

    @property
    def background_tasks(self) -> dict[str, asyncio.Task[None]]:
        return self._background_tasks

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "background")

    def configure_limits(
        self,
        *,
        max_background_jobs: int,
        job_semaphore: asyncio.Semaphore | None = None,
    ) -> None:
        self._max_background_jobs = max_background_jobs
        if job_semaphore is not None:
            self._job_semaphore = job_semaphore

    def startup(self) -> None:
        """Background host startup hook."""

    def shutdown(self) -> None:
        """Cancel tracked background tasks during runtime shutdown."""

        for task in list(self._background_tasks.values()):
            if not task.done():
                task.cancel()

    def mutation(
        self,
        *,
        status: str,
        current: BackgroundRunStatus | None = None,
        **overrides: Any,
    ) -> BackgroundJobStatusMutation:
        """Build one descriptor-driven background mutation from the latest row snapshot."""

        return BackgroundJobStatusMutation.from_transition(
            status=status,
            existing=current,
            **overrides,
        )

    def apply_mutation(
        self,
        job_id: str,
        mutation: BackgroundJobStatusMutation,
    ) -> BackgroundRunStatus:
        """Apply one durable mutation through the state-service host lane."""

        updated = self.state_service.apply_mutation(job_id, mutation)
        self.background_jobs[job_id] = updated
        return updated

    def get_status(self, job_id: str) -> BackgroundRunStatus | None:
        """Return the latest background job status projection."""

        status = self._job_store.get(job_id)
        if status is not None:
            self.background_jobs[job_id] = status
            return status
        return self.background_jobs.get(job_id)

    def start_job(
        self,
        job_id: str,
        request: BackgroundRunRequest,
        *,
        run_task: Callable[[BackgroundRunRequest], Awaitable[RunTaskResponse]],
    ) -> None:
        """Create the steady-state background runner task for one admitted job."""

        task = asyncio.create_task(self._run_background_job(job_id, request, run_task=run_task))
        self._background_tasks[job_id] = task

    async def request_interrupt(self, job_id: str) -> BackgroundRunStatus | None:
        """Request interruption of a queued, running, or retry-scheduled job."""

        task = self._background_tasks.get(job_id)
        task_active = task is not None and not task.done()
        task_done = task is not None and task.done()
        async with self._jobs_lock:
            current = self._job_store.get(job_id)
            if current is None:
                return None
            if current.status in TERMINAL_JOB_STATUSES:
                self.background_jobs[job_id] = current
                return current

            session_id = current.session_id or job_id
            interrupt_policy = self._background_status_transition_policy(
                operation="interrupt",
                current_status=current.status,
                task_active=task_active,
                task_done=task_done,
            )
            interrupt_effect = self._background_effect_plan(interrupt_policy)
            interrupt_step = str(interrupt_effect["next_step"])
            updated = self.apply_mutation(
                job_id,
                self.mutation(
                    status=str(interrupt_effect.get("resolved_status") or interrupt_policy["resolved_status"]),
                    current=current,
                    interrupt_requested_at=_now_iso(),
                ),
            )

        self._trace.record(
            session_id=session_id,
            job_id=job_id,
            kind="job.interrupt_requested",
            stage="background",
            payload={
                "attempt": updated.attempt,
                "status": updated.status,
                "background_policy_authority": self._background_control_authority,
            },
        )

        if interrupt_step == "request_interrupt" and bool(interrupt_effect.get("cancel_running_task")) and task is not None and not task.done():
            task.cancel()
        if interrupt_step == "finalize_interrupted" or task is None or task.done():
            return await self._mark_background_interrupted(job_id, session_id=session_id, error="Interrupt requested")
        return updated

    def health(self) -> dict[str, Any]:
        return {
            "max_background_jobs": self._max_background_jobs,
            "background_job_timeout_seconds": self._background_job_timeout_seconds,
            "active_task_count": len([task for task in self._background_tasks.values() if not task.done()]),
            "control_plane_authority": self._service_descriptor.get("authority"),
            "control_plane_role": self._service_descriptor.get("role"),
            "control_plane_projection": self._service_descriptor.get("projection"),
            "control_plane_delegate_kind": self._service_descriptor.get("delegate_kind"),
            "background_policy_authority": self._background_control_authority,
            "background_effect_host_contract": _runtime_background_effect_host_contract(
                self._control_plane_descriptor,
                self._service_descriptor,
                service_name="background",
            ),
        }

    def _background_policy(self, payload: dict[str, Any]) -> dict[str, Any]:
        return self._background_control_provider()(
            {
                "schema_version": self._background_control_schema_version,
                **payload,
            }
        )

    def _background_status_transition_policy(
        self,
        *,
        operation: str,
        current_status: str,
        task_active: bool,
        task_done: bool,
    ) -> dict[str, Any]:
        return self._background_policy(
            {
                "operation": operation,
                "current_status": current_status,
                "task_active": task_active,
                "task_done": task_done,
            }
        )

    def _background_retry_policy(self, status: BackgroundRunStatus) -> dict[str, Any]:
        return self._background_policy(
            {
                "operation": "retry",
                "attempt": status.attempt,
                "retry_count": status.retry_count,
                "max_attempts": status.max_attempts,
                "backoff_base_seconds": status.backoff_base_seconds,
                "backoff_multiplier": status.backoff_multiplier,
                "max_backoff_seconds": status.max_backoff_seconds,
            }
        )

    def _background_effect_plan(self, policy: dict[str, Any]) -> dict[str, Any]:
        effect_plan = policy.get("effect_plan")
        if not isinstance(effect_plan, dict):
            raise RuntimeError("Rust background control response missing effect_plan.")
        return effect_plan

    async def _run_background_job(
        self,
        job_id: str,
        request: BackgroundRunRequest,
        *,
        run_task: Callable[[BackgroundRunRequest], Awaitable[RunTaskResponse]],
    ) -> None:
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
                        running = self.apply_mutation(
                            job_id,
                            self.mutation(
                                status="running",
                                current=current,
                                session_id=request.session_id,
                                error=None,
                                timeout_seconds=self._background_job_timeout_seconds,
                                claimed_by=job_id,
                                claimed_at=started_at,
                                last_attempt_started_at=started_at,
                            ),
                        )
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
                            run_task(request),
                            timeout=self._background_job_timeout_seconds,
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
                            error=f"Job timed out after {self._background_job_timeout_seconds}s",
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
                    async with self._jobs_lock:
                        latest = self._job_store.get(job_id)
                        if latest is None:
                            return
                        completion_policy = self._background_status_transition_policy(
                            operation="completion-race",
                            current_status=latest.status,
                            task_active=False,
                            task_done=True,
                        )
                        completion_effect = self._background_effect_plan(completion_policy)
                        if str(completion_effect["next_step"]) == "finalize_interrupted":
                            completed = None
                        else:
                            terminal_status = str(
                                completion_effect.get("terminal_status") or completion_policy["terminal_status"]
                            )
                            completed = self.apply_mutation(
                                job_id,
                                self.mutation(
                                    status=terminal_status,
                                    current=latest,
                                    session_id=result.session_id,
                                    result=result,
                                    timeout_seconds=self._background_job_timeout_seconds,
                                    claimed_by=job_id,
                                    next_retry_at=None,
                                    last_attempt_finished_at=completed_at,
                                ),
                            )
                    if completed is None:
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
                    self._checkpoint_background_resume_manifest(
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
        scheduled: BackgroundRunStatus | None = None
        retry_claimed = False
        async with self._jobs_lock:
            current = self._job_store.get(job_id)
            if current is None:
                return False
            if current.status in {"interrupt_requested", "interrupted"}:
                scheduled = None
            else:
                self._trace.record(
                    session_id=session_id,
                    job_id=job_id,
                    kind="job.failed",
                    stage="background",
                    payload={"error": error, "attempt": current.attempt},
                )

                retry_policy = self._background_retry_policy(current)
                retry_effect = self._background_effect_plan(retry_policy)
                if str(retry_effect["next_step"]) != "schedule_retry":
                    terminal_status = str(
                        retry_effect.get("terminal_status") or retry_policy["terminal_status"]
                    )
                    finalized = self.apply_mutation(
                        job_id,
                        self.mutation(
                            status=terminal_status,
                            current=current,
                            error=error,
                            next_retry_at=None,
                            last_attempt_finished_at=failed_at,
                            last_failure_at=failed_at,
                        ),
                    )
                    if terminal_status == "retry_exhausted":
                        self._trace.record(
                            session_id=session_id,
                            job_id=job_id,
                            kind="job.retry_exhausted",
                            stage="background",
                            payload={"attempt": finalized.attempt, "max_attempts": finalized.max_attempts, "error": error},
                        )
                    self._checkpoint_background_resume_manifest(
                        session_id=session_id,
                        job_id=job_id,
                        status=terminal_status,
                    )
                    return False
                next_retry_count = int(
                    retry_effect.get("next_retry_count") or retry_policy["next_retry_count"]
                )
                backoff_seconds = float(retry_effect.get("backoff_seconds") or retry_policy["backoff_seconds"])
                next_retry_at = (
                    datetime.now(UTC) + timedelta(seconds=backoff_seconds)
                ).isoformat()
                scheduled = self.apply_mutation(
                    job_id,
                    self.mutation(
                        status="retry_scheduled",
                        current=current,
                        error=error,
                        retry_count=next_retry_count,
                        backoff_seconds=backoff_seconds,
                        next_retry_at=next_retry_at,
                        retry_scheduled_at=failed_at,
                        last_attempt_finished_at=failed_at,
                        last_failure_at=failed_at,
                    ),
                )

        if scheduled is None:
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
                "background_policy_authority": self._background_control_authority,
            },
        )

        try:
            await self.state_service.wait_for_retry_backoff(backoff_seconds=scheduled.backoff_seconds or 0.0)
        except asyncio.CancelledError:
            await self._mark_background_interrupted(
                job_id,
                session_id=session_id,
                error="Interrupt requested during retry backoff",
            )
            raise

        async with self._jobs_lock:
            current = self._job_store.get(job_id)
            if current is None:
                return False
            retry_claim_policy = self._background_status_transition_policy(
                operation="retry-claim",
                current_status=current.status,
                task_active=False,
                task_done=False,
            )
            retry_claim_effect = self._background_effect_plan(retry_claim_policy)
            if str(retry_claim_effect["next_step"]) == "finalize_interrupted":
                retry_claimed = False
            else:
                retry_claimed_at = _now_iso()
                claimed = self.apply_mutation(
                    job_id,
                    self.mutation(
                        status=str(retry_claim_effect.get("terminal_status") or retry_claim_policy["terminal_status"]),
                        current=current,
                        claimed_by=job_id,
                        attempt=current.attempt + 1,
                        retry_claimed_at=retry_claimed_at,
                    ),
                )
                retry_claimed = True

        if not retry_claimed:
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
            finalize_policy = self._background_status_transition_policy(
                operation="interrupt-finalize",
                current_status=current.status,
                task_active=False,
                task_done=True,
            )
            finalize_effect = self._background_effect_plan(finalize_policy)
            interrupted_at = _now_iso()
            interrupted = self.apply_mutation(
                job_id,
                self.mutation(
                    status=str(finalize_effect.get("terminal_status") or finalize_policy["terminal_status"]),
                    current=current,
                    error=error,
                    interrupt_requested_at=current.interrupt_requested_at or interrupted_at,
                    interrupted_at=interrupted_at,
                    last_attempt_finished_at=interrupted_at,
                ),
            )

        self._trace.record(
            session_id=session_id,
            job_id=job_id,
            kind="job.interrupted",
            stage="background",
            payload={
                "attempt": interrupted.attempt,
                "error": error,
                "background_policy_authority": self._background_control_authority,
            },
        )
        self._checkpoint_background_resume_manifest(
            session_id=session_id,
            job_id=job_id,
            status="interrupted",
        )
        return interrupted

    def _checkpoint_background_resume_manifest(
        self,
        *,
        session_id: str,
        job_id: str,
        status: str,
    ) -> None:
        """Persist background resume state without routing ownership back through runtime.py."""

        self.trace_service.checkpoint(
            session_id=session_id,
            job_id=job_id,
            status=status,
            artifact_paths=self._artifact_paths_provider(),
            supervisor_projection=self._supervisor_projection_provider(),
        )
