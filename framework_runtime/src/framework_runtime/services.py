"""Service boundaries for the Codex Agno runtime."""

from __future__ import annotations

import asyncio
import json
import os
from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Mapping
from uuid import uuid4

try:
    import resource as _resource
except ImportError:  # pragma: no cover - exercised via monkeypatched fallback test.
    _resource = None

try:
    import psutil as _psutil
except ImportError:  # pragma: no cover - optional host probe dependency.
    _psutil = None

from framework_runtime.checkpoint_store import RuntimeCheckpointer
from framework_runtime.config import RuntimeSettings
from framework_runtime.execution_kernel import (
    ExecutionKernelRequest,
    SANDBOX_CAPABILITY_CATEGORIES,
    SandboxExecutionPolicy,
    SandboxResourceBudget,
    SandboxRuntimeProbe,
    execute_router_rs_request,
)
from framework_runtime.execution_kernel_contracts import (
    EXECUTION_KERNEL_BRIDGE_AUTHORITY,
    EXECUTION_KERNEL_BRIDGE_KIND,
    EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
    EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
    execution_kernel_steady_state_fields,
    normalize_execution_kernel_metadata_bridge,
    resolve_execution_kernel_expectations,
    validate_execution_kernel_steady_state_metadata,
)
from framework_runtime.control_plane_contracts import build_control_plane_contract_descriptors
from framework_runtime.memory import FactMemoryStore
from framework_runtime.middleware import MiddlewareContext
from framework_runtime.observability import build_runtime_observability_health_snapshot
from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.schemas import (
    BackgroundBatchEnqueueResponse,
    BackgroundParallelGroupSummary,
    BackgroundRunRequest,
    BackgroundRunStatus,
    RouteDecisionContract,
    RouteDecisionSnapshot,
    RouteDiagnosticReport,
    RouteExecutionPolicy,
    RoutingResult,
    RunTaskResponse,
)
from framework_runtime.skill_loader import SkillLoader
from framework_runtime.state import (
    BackgroundJobStatusMutation,
    BackgroundJobStore,
    BackgroundSessionTakeoverArbitration,
    SessionConflictError,
)
from framework_runtime.trace import InMemoryRuntimeEventBridge, RuntimeEventHandoff, RuntimeEventStreamChunk
from framework_runtime.trace import RuntimeEventTransport

_KERNEL_HEALTH_FIELDS = (
    "kernel_adapter_kind",
    "kernel_authority",
    "kernel_owner_family",
    "kernel_owner_impl",
    "kernel_contract_mode",
    "kernel_replace_ready",
    "kernel_in_process_replacement_complete",
    "kernel_live_backend_family",
    "kernel_live_backend_impl",
    "kernel_live_delegate_kind",
    "kernel_live_delegate_authority",
    "kernel_live_delegate_family",
    "kernel_live_delegate_impl",
    "kernel_live_delegate_mode",
    "kernel_mode_support",
    "execution_schema_version",
)
_DEFAULT_SANDBOX_LIFECYCLE_STATES = (
    "created",
    "warm",
    "busy",
    "draining",
    "recycled",
    "failed",
)
_DEFAULT_SANDBOX_ALLOWED_TRANSITIONS = {
    ("created", "warm"),
    ("warm", "busy"),
    ("busy", "draining"),
    ("draining", "recycled"),
    ("draining", "failed"),
    ("warm", "failed"),
    ("busy", "failed"),
    ("recycled", "warm"),
}
_DEFAULT_BACKGROUND_TERMINAL_STATUSES = (
    "completed",
    "failed",
    "interrupted",
    "retry_exhausted",
)
_DEFAULT_BACKGROUND_ACTIVE_STATUSES = (
    "queued",
    "running",
    "interrupt_requested",
    "retry_scheduled",
    "retry_claimed",
)
_DEFAULT_RUNTIME_STARTUP_ORDER = (
    "router",
    "state",
    "trace",
    "memory",
    "execution",
    "background",
)
_DEFAULT_RUNTIME_SHUTDOWN_ORDER = (
    "background",
    "execution",
    "memory",
    "trace",
    "state",
    "router",
)
_DEFAULT_RUNTIME_HEALTH_SECTIONS = (
    "router",
    "state",
    "trace",
    "memory",
    "execution_environment",
    "background",
    "checkpoint",
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


def _runtime_execution_kernel_contract(
    service_descriptor: Mapping[str, Any] | None,
    *,
    dry_run: bool = False,
    response_shape: str | None = None,
) -> dict[str, Any]:
    """Return the Rust-owned execution kernel contract projected by the control plane."""

    if not isinstance(service_descriptor, Mapping):
        raise RuntimeError("runtime control plane execution descriptor is missing.")
    contract_modes = service_descriptor.get("kernel_contract_by_mode")
    resolved_response_shape = (
        str(response_shape)
        if response_shape is not None
        else (
            EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
            if dry_run
            else EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
        )
    )
    contract = None
    if isinstance(contract_modes, Mapping):
        contract = contract_modes.get(resolved_response_shape)
    if not isinstance(contract, Mapping):
        if resolved_response_shape != EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY:
            raise RuntimeError(
                "runtime control plane execution descriptor is missing "
                f"kernel_contract_by_mode.{resolved_response_shape}."
            )
        contract = service_descriptor.get("kernel_contract")
    if not isinstance(contract, Mapping):
        raise RuntimeError("runtime control plane execution descriptor is missing kernel_contract.")
    metadata_bridge = _runtime_execution_kernel_metadata_bridge(service_descriptor)
    return validate_execution_kernel_steady_state_metadata(
        metadata=contract,
        execution_kernel=EXECUTION_KERNEL_BRIDGE_KIND,
        execution_kernel_authority=EXECUTION_KERNEL_BRIDGE_AUTHORITY,
        response_shape=resolved_response_shape,
        metadata_bridge=metadata_bridge,
    )


def _runtime_execution_kernel_metadata_bridge(
    service_descriptor: Mapping[str, Any] | None,
) -> dict[str, Any] | None:
    """Return the Rust-owned execution-kernel naming bridge when available."""

    if not isinstance(service_descriptor, Mapping):
        return None
    bridge = service_descriptor.get("kernel_metadata_bridge")
    if bridge is None:
        return None
    if not isinstance(bridge, Mapping):
        raise RuntimeError(
            "runtime control plane execution descriptor returned an invalid kernel_metadata_bridge."
        )
    return normalize_execution_kernel_metadata_bridge(dict(bridge))


def _runtime_execution_kernel_health(
    service_descriptor: Mapping[str, Any] | None,
    *,
    resolved_binary: str | None,
) -> dict[str, Any]:
    """Return the Rust-owned execution kernel health projection from the control plane."""

    if not isinstance(service_descriptor, Mapping):
        raise RuntimeError("runtime control plane execution descriptor is missing.")
    payload = dict(service_descriptor)
    missing = [field for field in _KERNEL_HEALTH_FIELDS if field not in payload]
    if missing:
        raise RuntimeError(
            "runtime control plane execution descriptor is missing kernel fields: "
            + ", ".join(sorted(missing))
        )
    payload["resolved_binary"] = resolved_binary
    return {field: payload[field] for field in (*_KERNEL_HEALTH_FIELDS, "resolved_binary")}


def _runtime_sandbox_lifecycle_contract(service_descriptor: Mapping[str, Any] | None) -> dict[str, Any]:
    """Return the Rust-owned sandbox lifecycle contract for the execution service."""

    payload = {
        "schema_version": "runtime-sandbox-lifecycle-v1",
        "authority": "rust-runtime-control-plane",
        "role": "sandbox-lifecycle-control",
        "projection": "python-diagnosis-only-projection",
        "delegate_kind": "rust-runtime-control-plane",
        "lifecycle_states": list(_DEFAULT_SANDBOX_LIFECYCLE_STATES),
        "allowed_transitions": [list(edge) for edge in sorted(_DEFAULT_SANDBOX_ALLOWED_TRANSITIONS)],
        "capability_categories": list(SANDBOX_CAPABILITY_CATEGORIES),
        "cleanup_mode": "async-drain-and-recycle",
        "event_log_artifact": "runtime_sandbox_events.jsonl",
        "control_operations": ["transition", "cleanup"],
        "runtime_probe_dimensions": [
            "cpu",
            "memory",
            "wall_clock",
            "output_size",
        ],
    }
    if isinstance(service_descriptor, Mapping):
        contract = service_descriptor.get("sandbox_lifecycle_contract")
        if isinstance(contract, Mapping):
            payload.update(dict(contract))
    return payload


def _runtime_background_orchestration_contract(
    service_descriptor: Mapping[str, Any] | None,
) -> dict[str, Any]:
    """Return the Rust-owned background orchestration contract."""

    payload = {
        "schema_version": "runtime-background-orchestration-v1",
        "authority": "rust-runtime-control-plane",
        "role": "background-orchestration-control",
        "projection": "python-diagnosis-only-projection",
        "delegate_kind": "rust-background-control-policy",
        "policy_schema_version": "router-rs-background-control-v1",
        "queue_model": "bounded-async-host",
        "session_takeover_model": "state-store-lease-arbitration",
        "state_artifact": "runtime_background_jobs.json",
        "max_background_jobs": 16,
        "background_job_timeout_seconds": 600.0,
        "admission_owner": "rust-background-control-policy",
        "queue_concurrency_owner": "rust-control-plane",
        "active_statuses": list(_DEFAULT_BACKGROUND_ACTIVE_STATUSES),
        "terminal_statuses": list(_DEFAULT_BACKGROUND_TERMINAL_STATUSES),
        "policy_operations": [
            "batch-plan",
            "enqueue",
            "claim",
            "interrupt",
            "interrupt-finalize",
            "retry",
            "retry-claim",
            "complete",
            "completion-race",
            "session-release",
        ],
    }
    if isinstance(service_descriptor, Mapping):
        contract = service_descriptor.get("orchestration_contract")
        if isinstance(contract, Mapping):
            payload.update(dict(contract))
    return payload


def _runtime_host_contract(control_plane_descriptor: Mapping[str, Any] | None) -> dict[str, Any]:
    """Return the Rust-owned top-level runtime orchestration contract."""

    payload = {
        "authority": "rust-runtime-control-plane",
        "role": "runtime-orchestration",
        "projection": "python-diagnosis-only-projection",
        "delegate_kind": "rust-runtime-control-plane",
        "startup_order": list(_DEFAULT_RUNTIME_STARTUP_ORDER),
        "shutdown_order": list(_DEFAULT_RUNTIME_SHUTDOWN_ORDER),
        "health_sections": list(_DEFAULT_RUNTIME_HEALTH_SECTIONS),
        "rust_owned_service_count": 0,
    }
    if not isinstance(control_plane_descriptor, Mapping):
        return payload
    payload["authority"] = control_plane_descriptor.get("authority", payload["authority"])
    runtime_host = control_plane_descriptor.get("runtime_host")
    if isinstance(runtime_host, Mapping):
        payload.update(dict(runtime_host))
    services = control_plane_descriptor.get("services")
    if isinstance(services, Mapping):
        payload["rust_owned_service_count"] = len(
            [
                name
                for name, descriptor in services.items()
                if isinstance(name, str)
                and isinstance(descriptor, Mapping)
                and descriptor.get("authority")
            ]
        )
    return payload


def _runtime_service_health_projection(
    service_descriptor: Mapping[str, Any] | None,
    *,
    authority_fallback: Any = None,
) -> dict[str, Any]:
    if not isinstance(service_descriptor, Mapping):
        service_descriptor = {}
    return {
        "control_plane_authority": service_descriptor.get("authority", authority_fallback),
        "control_plane_role": service_descriptor.get("role"),
        "control_plane_projection": service_descriptor.get("projection"),
        "control_plane_delegate_kind": service_descriptor.get("delegate_kind"),
    }


def _runtime_rustification_health(control_plane_descriptor: Mapping[str, Any] | None) -> dict[str, Any]:
    runtime_host = _runtime_host_contract(control_plane_descriptor)
    return {
        "python_host_role": (
            control_plane_descriptor.get("python_host_role")
            if isinstance(control_plane_descriptor, Mapping)
            else None
        ),
        "rustification_status": _runtime_control_plane_rustification_status(control_plane_descriptor),
        "rust_owned_service_count": runtime_host.get("rust_owned_service_count", 0),
    }


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


def _fallback_usage_snapshot() -> dict[str, float]:
    """Return the cross-platform fallback usage probe when getrusage is unavailable."""

    return {
        "self_cpu": float(time.process_time()),
        "child_cpu": 0.0,
        "self_memory": 0.0,
        "child_memory": 0.0,
        "self_peak_memory": 0.0,
        "child_peak_memory": 0.0,
    }


def _normalize_rusage_maxrss(raw_value: float) -> float:
    """Normalize ``ru_maxrss`` to bytes across supported host platforms."""

    if sys.platform == "darwin":
        return float(raw_value)
    return float(raw_value) * 1024.0


def _current_rss_bytes() -> float | None:
    """Return the current process RSS in bytes when the host can provide it."""

    if _psutil is not None:
        try:
            return float(_psutil.Process(os.getpid()).memory_info().rss)
        except (AttributeError, OSError, ValueError):
            pass

    for ps_binary in ("/bin/ps", "ps"):
        try:
            output = subprocess.check_output(
                [ps_binary, "-o", "rss=", "-p", str(os.getpid())],
                text=True,
            )
        except (FileNotFoundError, OSError, subprocess.CalledProcessError):
            continue
        rss_kib = output.strip()
        if not rss_kib:
            continue
        try:
            return float(rss_kib) * 1024.0
        except ValueError:
            continue
    return None


class SandboxLifecycleError(RuntimeError):
    """Raised when the sandbox state machine is driven through an invalid edge."""


class SandboxCapabilityViolation(RuntimeError):
    """Raised when sandbox capability policy denies one execution."""


class SandboxBudgetExceeded(RuntimeError):
    """Raised when runtime sandbox budget enforcement rejects one execution."""


def _runtime_host_concurrency_contract(control_plane_descriptor: Mapping[str, Any] | None) -> dict[str, Any]:
    """Return the Rust-owned runtime concurrency contract when available."""

    runtime_host = _runtime_host_contract(control_plane_descriptor)
    contract = runtime_host.get("concurrency_contract")
    return dict(contract) if isinstance(contract, Mapping) else {}


def _runtime_subagent_limit_contract(control_plane_descriptor: Mapping[str, Any] | None) -> dict[str, Any]:
    """Return the Rust-owned subagent limit contract projected through middleware."""

    service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "middleware")
    contract = service_descriptor.get("subagent_limit_contract")
    return dict(contract) if isinstance(contract, Mapping) else {}


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
        rust_adapter: RustRouteAdapter | None = None,
    ) -> None:
        self.settings = settings
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")
        self._rust_adapter = rust_adapter or RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        self._event_log_path = settings.resolved_data_dir / "runtime_sandbox_events.jsonl"
        self._contract = _runtime_sandbox_lifecycle_contract(self._service_descriptor)
        self.schema_version = str(self._contract["schema_version"])
        self._lifecycle_states = tuple(
            str(state) for state in self._contract.get("lifecycle_states", _DEFAULT_SANDBOX_LIFECYCLE_STATES)
        )
        self._allowed_transitions = {
            (str(edge[0]), str(edge[1]))
            for edge in self._contract.get("allowed_transitions", [])
            if isinstance(edge, (list, tuple)) and len(edge) == 2
        } or set(_DEFAULT_SANDBOX_ALLOWED_TRANSITIONS)
        self._capability_categories = tuple(
            str(category)
            for category in self._contract.get("capability_categories", SANDBOX_CAPABILITY_CATEGORIES)
        )
        self._sandboxes: dict[str, _SandboxRecord] = {}
        self._cleanup_tasks: dict[str, asyncio.Task[None]] = {}
        self._next_cleanup_failure_reason: str | None = None

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self._control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")
        self._contract = _runtime_sandbox_lifecycle_contract(self._service_descriptor)
        self.schema_version = str(self._contract["schema_version"])
        self._lifecycle_states = tuple(
            str(state) for state in self._contract.get("lifecycle_states", _DEFAULT_SANDBOX_LIFECYCLE_STATES)
        )
        self._allowed_transitions = {
            (str(edge[0]), str(edge[1]))
            for edge in self._contract.get("allowed_transitions", [])
            if isinstance(edge, (list, tuple)) and len(edge) == 2
        } or set(_DEFAULT_SANDBOX_ALLOWED_TRANSITIONS)
        self._capability_categories = tuple(
            str(category)
            for category in self._contract.get("capability_categories", SANDBOX_CAPABILITY_CATEGORIES)
        )

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

        state_counts = {state: 0 for state in self._lifecycle_states}
        for record in self._sandboxes.values():
            state_counts[record.state] += 1
        active_cleanup = [
            sandbox_id
            for sandbox_id, task in self._cleanup_tasks.items()
            if not task.done()
        ]
        return {
            "schema_version": self.schema_version,
            "lifecycle_states": list(self._lifecycle_states),
            "allowed_transitions": [list(edge) for edge in sorted(self._allowed_transitions)],
            "capability_categories": list(self._capability_categories),
            "event_log_path": str(self._event_log_path),
            "state_counts": state_counts,
            "active_cleanup_tasks": len(active_cleanup),
            "active_cleanup_sandboxes": active_cleanup,
            "sandbox_count": len(self._sandboxes),
            "latest_records": [record.as_payload() for record in list(self._sandboxes.values())[-5:]],
            "contract": dict(self._contract),
            "background_effect_host_contract": _runtime_background_effect_host_contract(
                self._control_plane_descriptor,
                self._service_descriptor,
                service_name="execution",
            ),
        }

    def _usage_snapshot(self) -> dict[str, float]:
        if _resource is None:
            return _fallback_usage_snapshot()
        try:
            self_usage = _resource.getrusage(_resource.RUSAGE_SELF)
            child_usage = _resource.getrusage(_resource.RUSAGE_CHILDREN)
            self_cpu = float(self_usage.ru_utime + self_usage.ru_stime)
            child_cpu = float(child_usage.ru_utime + child_usage.ru_stime)
            self_peak_memory = _normalize_rusage_maxrss(self_usage.ru_maxrss)
            child_peak_memory = _normalize_rusage_maxrss(child_usage.ru_maxrss)
        except (AttributeError, OSError, TypeError, ValueError):
            return _fallback_usage_snapshot()
        current_rss = _current_rss_bytes()
        return {
            "self_cpu": self_cpu,
            "child_cpu": child_cpu,
            "self_memory": self_peak_memory if current_rss is None else current_rss,
            "child_memory": child_peak_memory,
            "self_peak_memory": self_peak_memory,
            "child_peak_memory": child_peak_memory,
        }

    def _acquire_record(self, request: ExecutionKernelRequest) -> _SandboxRecord:
        request_job_id = request.job_id or request.session_id
        for record in self._sandboxes.values():
            if not self._matches_policy(record, request.sandbox_policy):
                continue
            if record.state == "failed" or record.quarantined:
                continue
            if record.state == "recycled":
                record.reuse_count += 1
                self._transition(record, "warm", event_kind="sandbox.rewarmed", request=request)
                record.current_session_id = request.session_id
                record.current_job_id = request_job_id
                return record
            if record.state == "warm":
                record.current_session_id = request.session_id
                record.current_job_id = request_job_id
                return record

        record = _SandboxRecord(
            sandbox_id=f"sandbox-{uuid4().hex[:12]}",
            policy=request.sandbox_policy,
            budget=request.sandbox_budget,
            current_session_id=request.session_id,
            current_job_id=request_job_id,
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
            if category not in self._capability_categories:
                reason = f"policy_violation:unknown_capability:{category}"
                self._mark_failed(record, request=request, reason=reason)
                raise SandboxCapabilityViolation(reason)
        if request.sandbox_tool_category not in self._capability_categories:
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
            usage_after.get("self_peak_memory", usage_after["self_memory"])
            - usage_before.get("self_peak_memory", usage_before["self_memory"]),
            usage_after.get("child_peak_memory", usage_after["child_memory"])
            - usage_before.get("child_peak_memory", usage_before["child_memory"]),
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
        cleanup_resolution = self._rust_adapter.sandbox_control(
            {
                "schema_version": self.schema_version,
                "operation": "cleanup",
                "current_state": record.state,
                "cleanup_failed": failure_reason is not None,
            }
        )
        if not bool(cleanup_resolution.get("allowed")):
            raise SandboxLifecycleError(
                str(
                    cleanup_resolution.get("error")
                    or f"invalid sandbox cleanup state: {record.state!r}"
                )
            )
        resolved_state = str(cleanup_resolution.get("resolved_state") or ("failed" if failure_reason else "recycled"))
        if failure_reason is not None:
            record.cleanup_pending = False
            record.quarantined = True
            record.last_failure_reason = failure_reason
            self._transition(
                record,
                resolved_state,
                event_kind="sandbox.cleanup_failed",
                request=None,
                detail={"failure_reason": failure_reason},
            )
            return
        record.cleanup_pending = False
        self._transition(
            record,
            resolved_state,
            event_kind="sandbox.cleanup_completed",
            request=None,
        )
        record.current_session_id = None
        record.current_job_id = None

    def _transition(
        self,
        record: _SandboxRecord,
        next_state: str,
        *,
        event_kind: str,
        request: ExecutionKernelRequest | None,
        detail: Mapping[str, Any] | None = None,
    ) -> None:
        decision = self._rust_adapter.sandbox_control(
            {
                "schema_version": self.schema_version,
                "operation": "transition",
                "current_state": record.state,
                "next_state": next_state,
            }
        )
        if not bool(decision.get("allowed")):
            raise SandboxLifecycleError(
                str(decision.get("error") or f"invalid sandbox transition: {record.state!r} -> {next_state!r}")
            )
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
            "job_id": (
                (request.job_id or request.session_id)
                if request is not None
                else record.current_job_id
            ),
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
        self._rust_adapter = rust_adapter or RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        self.control_plane_descriptor = self._rust_adapter.runtime_control_plane()
        self.skills = []
        self._last_route_report: RouteDiagnosticReport | None = None
        self._route_policy: RouteExecutionPolicy | None = None
        self._rust_adapter_health: dict[str, Any] | None = None
        self.reload()

    def startup(self) -> None:
        """Reload skills for a fresh runtime session."""

        self.reload()

    def shutdown(self) -> None:
        """Router service shutdown hook."""

    def reload(self) -> None:
        """Refresh runtime skill metadata and the Rust-owned route policy."""

        self.control_plane_descriptor = self._rust_adapter.runtime_control_plane()
        self.skills = self.loader.load(
            refresh=True,
            load_bodies=not self.settings.progressive_skill_loading,
        )
        self._rust_adapter_health = None
        self._resolve_route_policy(refresh=True)

    def route(self, *, task: str, session_id: str, allow_overlay: bool, first_turn: bool) -> RoutingResult:
        """Return the configured route decision for one task."""

        self._last_route_report = None
        decision, rust_result = self._route_rust(
            task=task,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )
        policy, report = self._rust_adapter.route_resolution_contract(
            mode=self.settings.route_engine_mode,
            route_decision_contract=decision,
        )
        self._route_policy = policy
        self._last_route_report = report
        return self._decorate_route_result(
            rust_result,
            route_engine="rust",
            diagnostic_route_mode=policy.diagnostic_route_mode,
            report=report,
        )

    def health(self) -> dict[str, Any]:
        """Describe router-service health and the active route engine."""

        policy = self._resolve_route_policy()
        service_descriptor = _runtime_control_plane_service_descriptor(
            self.control_plane_descriptor,
            "router",
        )
        payload = _runtime_service_health_projection(
            service_descriptor,
            authority_fallback=self.control_plane_descriptor.get("authority"),
        )
        payload.update(
            {
                "mode": self.settings.route_engine_mode,
                "default_route_mode": self.control_plane_descriptor.get("default_route_mode", "rust"),
                "default_route_authority": self.control_plane_descriptor.get(
                    "default_route_authority",
                    self._rust_adapter.route_authority,
                ),
                "diagnostic_route_mode": policy.diagnostic_route_mode,
                "loaded_skill_count": len(self.skills),
                "skill_root": str(self.settings.codex_home / "skills"),
                "primary_authority": policy.primary_authority,
                "route_result_engine": policy.route_result_engine,
                "diagnostic_report_required": policy.diagnostic_report_required,
                "strict_verification_required": policy.strict_verification_required,
                "python_runtime_role": self.control_plane_descriptor.get("python_host_role"),
                "rustification_status": _runtime_control_plane_rustification_status(self.control_plane_descriptor),
                "route_policy": policy.model_dump(mode="json"),
                "rust_adapter": self._rust_adapter_health_snapshot(),
                "last_route_report": self._last_route_report.model_dump(mode="json") if self._last_route_report else None,
            }
        )
        return payload

    def _route_rust(
        self,
        *,
        task: str,
        session_id: str,
        allow_overlay: bool,
        first_turn: bool,
    ) -> tuple[RouteDecisionContract, RoutingResult]:
        decision = self._rust_adapter.route_contract(
            query=task,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )
        selected = self._resolve_loaded_skill(decision.selected_skill, decision=decision)
        overlay = (
            self._resolve_loaded_skill(decision.overlay_skill, decision=decision)
            if decision.overlay_skill
            else None
        )
        return decision, RoutingResult(
            task=decision.task,
            session_id=decision.session_id,
            selected_skill=selected,
            overlay_skill=overlay,
            score=float(decision.score),
            layer=decision.layer,
            reasons=[str(reason) for reason in decision.reasons],
            route_snapshot=RouteDecisionSnapshot.model_validate(
                decision.route_snapshot.model_dump(mode="json")
            ),
        )

    def _decorate_route_result(
        self,
        result: RoutingResult,
        *,
        route_engine: str,
        diagnostic_route_mode: str,
        report: RouteDiagnosticReport | None,
    ) -> RoutingResult:
        return result.model_copy(
            update={
                "route_engine": route_engine,
                "diagnostic_route_mode": diagnostic_route_mode,
                "route_diagnostic_report": (
                    RouteDiagnosticReport.model_validate(report.model_dump(mode="json"))
                    if report is not None
                    else None
                ),
            }
        )

    def _resolve_route_policy(self, *, refresh: bool = False) -> RouteExecutionPolicy:
        if refresh or self._route_policy is None:
            self._route_policy = self._rust_adapter.route_policy_contract(
                mode=self.settings.route_engine_mode,
            )
        return self._route_policy

    def _rust_adapter_health_snapshot(self) -> dict[str, Any]:
        cached = self._rust_adapter_health
        if cached is None:
            cached = self._rust_adapter.health()
            self._rust_adapter_health = cached
        return dict(cached)

    def _resolve_loaded_skill(
        self,
        skill_name: str,
        *,
        decision: RouteDecisionContract,
    ) -> Any:
        skill = next((item for item in self.skills if item.name == skill_name), None)
        if skill is None:
            raise RuntimeError(
                "Rust route decision referenced a skill that is not loaded by the Python host: "
                f"{skill_name!r} (session_id={decision.session_id!r})"
            )
        return skill

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
        self._session_release_events: dict[str, asyncio.Event] = {}

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

    def parallel_group_summary(self, parallel_group_id: str) -> BackgroundParallelGroupSummary | None:
        """Return one durable parallel-batch summary."""

        return self.store.parallel_group_summary(parallel_group_id)

    def parallel_group_summaries(self) -> list[BackgroundParallelGroupSummary]:
        """Return all durable parallel-batch summaries."""

        return self.store.parallel_group_summaries()

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

    def _session_release_event(self, session_id: str) -> asyncio.Event:
        event = self._session_release_events.get(session_id)
        if event is None or event.is_set():
            event = asyncio.Event()
            self._session_release_events[session_id] = event
        return event

    def notify_session_release(self, session_id: str | None) -> None:
        """Wake any waiters watching the release of one background session."""

        if session_id is None:
            return
        event = self._session_release_events.get(session_id)
        if event is not None:
            event.set()

    async def wait_for_session_release(
        self,
        *,
        session_id: str,
        timeout_seconds: float,
        poll_interval_seconds: float,
    ) -> None:
        """Wait until the session reservation is released by the active job."""

        if self.get_active_job(session_id) is None:
            return
        deadline = asyncio.get_running_loop().time() + timeout_seconds
        while True:
            remaining = deadline - asyncio.get_running_loop().time()
            if remaining <= 0:
                break
            event = self._session_release_event(session_id)
            try:
                await asyncio.wait_for(event.wait(), timeout=min(remaining, poll_interval_seconds))
            except TimeoutError:
                pass
            if self.get_active_job(session_id) is None:
                return
        raise RuntimeError(
            f"Timed out waiting for background session {session_id!r} to become available."
        )

    async def wait_for_retry_backoff(self, *, backoff_seconds: float) -> None:
        """Wait for the retry backoff window resolved by the Rust control plane."""

        await asyncio.sleep(backoff_seconds)

    def health(self) -> dict[str, Any]:
        return {
            **_runtime_service_health_projection(self._service_descriptor),
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
        self._observability_health: dict[str, Any] | None = None
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

        return self.recorder.subscribe_chunk(
            session_id=session_id,
            job_id=job_id,
            after_event_id=after_event_id,
            limit=limit,
            heartbeat=heartbeat,
        )

    def cleanup_stream(self, *, session_id: str | None = None, job_id: str | None = None) -> None:
        """Release cached bridge events for one stream or for the whole service."""

        self.event_bridge.cleanup(session_id=session_id, job_id=job_id)

    def describe_stream_artifacts(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
    ) -> tuple[RuntimeEventTransport, RuntimeEventHandoff]:
        """Resolve transport and handoff together to avoid duplicate hot-path round-trips."""

        latest_cursor = self.recorder.latest_cursor(session_id=session_id, job_id=job_id)
        transport = self.checkpointer.resolve_transport_manifest(
            session_id=session_id,
            job_id=job_id,
            latest_cursor=latest_cursor,
        )
        self.checkpointer.write_transport_binding(transport)
        handoff = self.checkpointer.resolve_handoff_manifest(
            session_id=session_id,
            job_id=job_id,
            transport=transport,
        )
        return transport, handoff

    def describe_transport(self, *, session_id: str, job_id: str | None = None) -> RuntimeEventTransport:
        """Describe the host-facing transport binding for one runtime stream."""

        transport, _ = self.describe_stream_artifacts(session_id=session_id, job_id=job_id)
        return transport

    def describe_handoff(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
        transport: RuntimeEventTransport | None = None,
    ) -> RuntimeEventHandoff:
        """Describe the durable handoff surface for one runtime event stream."""

        if transport is None:
            _, handoff = self.describe_stream_artifacts(session_id=session_id, job_id=job_id)
            return handoff
        return self.checkpointer.resolve_handoff_manifest(
            session_id=session_id,
            job_id=job_id,
            transport=transport,
        )

    def checkpoint(
        self,
        *,
        session_id: str,
        job_id: str | None,
        status: str,
        artifact_paths: list[str],
        parallel_group: dict[str, Any] | None = None,
        supervisor_projection: dict[str, Any] | None = None,
        transport: RuntimeEventTransport | None = None,
    ) -> None:
        """Persist the runtime resume checkpoint through the configured backend."""

        if transport is None:
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
            parallel_group=parallel_group,
            supervisor_projection=supervisor_projection,
        )

    def health(self) -> dict[str, Any]:
        paths = self.checkpointer.describe_paths()
        return {
            **_runtime_service_health_projection(self._service_descriptor),
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
            "observability": self._observability_health_snapshot(),
            "background_effect_host_contract": _runtime_background_effect_host_contract(
                self._control_plane_descriptor,
                self._service_descriptor,
                service_name="trace",
            ),
        }

    def _observability_health_snapshot(self) -> dict[str, Any]:
        cached = self._observability_health
        if cached is None:
            cached = build_runtime_observability_health_snapshot(
                rust_adapter=self.recorder._rust_adapter,
            )
            self._observability_health = cached
        return dict(cached)


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
        payload.update(_runtime_service_health_projection(self._service_descriptor, authority_fallback=payload.get("control_plane_authority")))
        payload["control_plane_contract"] = self.control_plane_contract()
        return payload


class ExecutionEnvironmentService:
    """Own agent-factory construction and execution-environment health."""

    def __init__(
        self,
        settings: RuntimeSettings,
        *,
        max_background_jobs: int,
        background_job_timeout_seconds: float,
        control_plane_descriptor: Mapping[str, Any] | None = None,
        rust_adapter: RustRouteAdapter | None = None,
    ) -> None:
        self.settings = settings
        self.control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")
        self._rust_adapter = rust_adapter or RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        self.sandbox = SandboxLifecycleService(
            settings,
            control_plane_descriptor=control_plane_descriptor,
            rust_adapter=self._rust_adapter,
        )
        self.max_background_jobs = max_background_jobs
        self.background_job_timeout_seconds = background_job_timeout_seconds
        self._rust_adapter_health: dict[str, Any] | None = None
        self._control_plane_contracts: dict[str, Any] | None = None
        self._kernel_descriptor_snapshot: dict[str, Any] | None = None

    def refresh_control_plane(self, control_plane_descriptor: Mapping[str, Any] | None) -> None:
        self.control_plane_descriptor = dict(control_plane_descriptor or {})
        self._service_descriptor = _runtime_control_plane_service_descriptor(control_plane_descriptor, "execution")
        self.sandbox.refresh_control_plane(control_plane_descriptor)
        self._control_plane_contracts = None
        self._kernel_descriptor_snapshot = None

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
                job_id=ctx.job_id,
                user_id=ctx.user_id,
                routing_result=ctx.routing_result,
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

        return await self.sandbox.execute(
            request,
            executor=executor or self._execute_request_via_rust_adapter,
        )

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
        adapter_health = self._rust_adapter_health_snapshot()
        payload = {
            **_runtime_service_health_projection(
                self._service_descriptor,
                authority_fallback=self.control_plane_descriptor.get("authority"),
            ),
            "max_background_jobs": self.max_background_jobs,
            "background_job_timeout_seconds": self.background_job_timeout_seconds,
            "execution_mode_default": "live" if self.settings.use_live_model else "dry_run",
        }
        payload.update(
            _runtime_execution_kernel_health(
                self._service_descriptor,
                resolved_binary=adapter_health.get("resolved_binary"),
            )
        )
        payload["sandbox"] = self.sandbox.health()
        payload["control_plane_contracts"] = self._control_plane_contracts_snapshot()
        return payload

    def describe_control_plane_contracts(self) -> dict[str, Any]:
        """Return control-plane-only descriptors for shared execution artifacts."""

        snapshot = self._execution_kernel_descriptor_snapshot()
        payload = build_control_plane_contract_descriptors(
            execution_kernel_metadata_bridge=snapshot["metadata_bridge"],
            execution_kernel_live_contract=snapshot["contract_by_mode"].get(
                EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
            ),
            execution_kernel_dry_run_contract=snapshot["contract_by_mode"].get(
                EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
            ),
        )
        if self.control_plane_descriptor:
            payload["runtime_control_plane"] = self.control_plane_descriptor
        return payload

    def _rust_adapter_health_snapshot(self) -> dict[str, Any]:
        cached = self._rust_adapter_health
        if cached is None:
            cached = self._rust_adapter.health()
            self._rust_adapter_health = cached
        return dict(cached)

    def _control_plane_contracts_snapshot(self) -> dict[str, Any]:
        cached = self._control_plane_contracts
        if cached is None:
            cached = self.describe_control_plane_contracts()
            self._control_plane_contracts = cached
        return dict(cached)

    def _execution_kernel_descriptor_snapshot(self) -> dict[str, Any]:
        cached = self._kernel_descriptor_snapshot
        if cached is not None:
            return {
                "metadata_bridge": (
                    dict(cached["metadata_bridge"])
                    if isinstance(cached.get("metadata_bridge"), dict)
                    else None
                ),
                "contract_by_mode": {
                    str(shape): dict(contract)
                    for shape, contract in dict(cached["contract_by_mode"]).items()
                },
            }

        metadata_bridge = _runtime_execution_kernel_metadata_bridge(self._service_descriptor)
        contract_by_mode: dict[str, dict[str, Any]] = {}
        for shape in (
            EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY,
            EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN,
        ):
            try:
                contract = _runtime_execution_kernel_contract(
                    self._service_descriptor,
                    response_shape=shape,
                )
            except RuntimeError:
                continue
            contract_by_mode[str(shape)] = dict(contract)
        cached = {
            "metadata_bridge": dict(metadata_bridge) if isinstance(metadata_bridge, dict) else None,
            "contract_by_mode": {
                str(shape): dict(contract) for shape, contract in contract_by_mode.items()
            },
        }
        self._kernel_descriptor_snapshot = cached
        return {
            "metadata_bridge": (
                dict(cached["metadata_bridge"])
                if isinstance(cached.get("metadata_bridge"), dict)
                else None
            ),
            "contract_by_mode": {
                str(shape): dict(contract)
                for shape, contract in dict(cached["contract_by_mode"]).items()
            },
        }

    def _resolved_execution_kernel_response_shape(
        self,
        *,
        dry_run: bool = False,
        response_shape: str | None = None,
    ) -> str:
        if response_shape is not None:
            return str(response_shape)
        return (
            EXECUTION_KERNEL_RESPONSE_SHAPE_DRY_RUN
            if dry_run
            else EXECUTION_KERNEL_RESPONSE_SHAPE_LIVE_PRIMARY
        )

    def _project_kernel_payload(
        self,
        *,
        kernel_contract: Mapping[str, Any],
        metadata_bridge: Mapping[str, Any] | None,
        metadata: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        contract_fields = execution_kernel_steady_state_fields(metadata_bridge)
        payload = dict(kernel_contract)
        if metadata is not None:
            raw_metadata_execution_kernel = metadata.get("execution_kernel")
            raw_metadata_execution_kernel_authority = metadata.get("execution_kernel_authority")
            if (raw_metadata_execution_kernel is None) != (
                raw_metadata_execution_kernel_authority is None
            ):
                provided_field = (
                    "execution_kernel"
                    if raw_metadata_execution_kernel is not None
                    else "execution_kernel_authority"
                )
                provided_value = (
                    raw_metadata_execution_kernel
                    if raw_metadata_execution_kernel is not None
                    else raw_metadata_execution_kernel_authority
                )
                raise RuntimeError(
                    "execution-kernel steady-state metadata returned an unexpected value: "
                    f"{provided_field}={provided_value!r}"
                )
            steady_state_fields = tuple(field for field in contract_fields if field in metadata)
            if steady_state_fields:
                missing_fields = [field for field in contract_fields if field not in metadata]
                if missing_fields:
                    raise RuntimeError(
                        "execution-kernel projection metadata is missing steady-state fields: "
                        + ", ".join(sorted(missing_fields))
                    )
            if steady_state_fields:
                # Steady-state kernel identity remains Rust-contract-owned; runtime metadata
                # may vary on family/impl details, but it may not rename the kernel itself.
                expectations = resolve_execution_kernel_expectations(kernel_contract)
                validated_metadata = validate_execution_kernel_steady_state_metadata(
                    metadata=metadata,
                    execution_kernel=expectations["execution_kernel"],
                    execution_kernel_authority=expectations["execution_kernel_authority"],
                    execution_kernel_delegate=expectations["execution_kernel_delegate"],
                    execution_kernel_delegate_authority=expectations[
                        "execution_kernel_delegate_authority"
                    ],
                    response_shape=str(
                        metadata.get(
                            EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY,
                            payload[EXECUTION_KERNEL_RESPONSE_SHAPE_METADATA_KEY],
                        )
                    ),
                    metadata_bridge=metadata_bridge,
                )
                for field in contract_fields:
                    payload[field] = validated_metadata[field]
            for key, value in metadata.items():
                if key not in contract_fields:
                    payload[key] = value
        return {key: value for key, value in payload.items() if value is not None}

    def describe_kernel_contract(
        self,
        *,
        dry_run: bool = False,
        response_shape: str | None = None,
    ) -> dict[str, Any]:
        """Return the stable kernel-owner descriptor used by runtime surfaces."""

        resolved_shape = self._resolved_execution_kernel_response_shape(
            dry_run=dry_run,
            response_shape=response_shape,
        )
        snapshot = self._execution_kernel_descriptor_snapshot()
        contract_by_mode = dict(snapshot["contract_by_mode"])
        contract = contract_by_mode.get(resolved_shape)
        if isinstance(contract, dict):
            return dict(contract)
        raise RuntimeError(
            "runtime control plane execution descriptor is missing "
            f"kernel_contract_by_mode.{resolved_shape}."
        )

    def kernel_payload(
        self,
        *,
        dry_run: bool = False,
        response_shape: str | None = None,
        metadata: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Merge explicit execution metadata onto the stable kernel contract."""

        snapshot = self._execution_kernel_descriptor_snapshot()
        return self._project_kernel_payload(
            kernel_contract=self.describe_kernel_contract(
                dry_run=dry_run,
                response_shape=response_shape,
            ),
            metadata_bridge=snapshot["metadata_bridge"],
            metadata=metadata,
        )

    def describe_kernel_metadata_bridge(self) -> dict[str, Any] | None:
        """Return the normalized Rust-owned metadata bridge for kernel projection."""

        snapshot = self._execution_kernel_descriptor_snapshot()
        metadata_bridge = snapshot["metadata_bridge"]
        if metadata_bridge is None:
            return None
        return dict(metadata_bridge)

    async def _execute_request_via_rust_adapter(
        self,
        request: ExecutionKernelRequest,
    ) -> RunTaskResponse:
        snapshot = self._execution_kernel_descriptor_snapshot()
        kernel_contract = self.describe_kernel_contract(dry_run=request.dry_run)
        return await execute_router_rs_request(
            request,
            settings=self.settings,
            rust_adapter=self._rust_adapter,
            kernel_contract=kernel_contract,
            metadata_bridge=snapshot["metadata_bridge"],
        )


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
        self._orchestration_contract = _runtime_background_orchestration_contract(self._service_descriptor)
        self._terminal_statuses = {
            str(status)
            for status in self._orchestration_contract.get("terminal_statuses", _DEFAULT_BACKGROUND_TERMINAL_STATUSES)
        }
        self._max_background_jobs = max_background_jobs
        self._background_job_timeout_seconds = background_job_timeout_seconds
        self._job_store = self.state_service.store
        self._trace = self.trace_service.recorder
        self._jobs_lock: asyncio.Lock = asyncio.Lock()
        self._job_semaphore: asyncio.Semaphore = asyncio.Semaphore(self._max_background_jobs)
        self.background_jobs: dict[str, BackgroundRunStatus] = self.state_service.snapshot()
        self.background_requests: dict[str, BackgroundRunRequest] = {}
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
        self._orchestration_contract = _runtime_background_orchestration_contract(self._service_descriptor)
        self._terminal_statuses = {
            str(status)
            for status in self._orchestration_contract.get("terminal_statuses", _DEFAULT_BACKGROUND_TERMINAL_STATUSES)
        }

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

    async def enqueue_job(
        self,
        request: BackgroundRunRequest,
        *,
        session_id_resolver: Callable[[BackgroundRunRequest], str],
        run_task: Callable[[BackgroundRunRequest], Awaitable[RunTaskResponse]],
    ) -> BackgroundRunStatus:
        """Admit one background job under the Rust-owned orchestration policy."""

        job_id = f"job_{uuid4().hex[:12]}"
        effective_session_id = session_id_resolver(request)
        request = request.model_copy(update={"session_id": effective_session_id})
        enqueue_policy = self._background_enqueue_policy(
            multitask_strategy=request.multitask_strategy,
            active_job_count=0,
        )
        multitask_strategy = str(enqueue_policy["normalized_multitask_strategy"])
        requires_takeover = bool(enqueue_policy["requires_takeover"])

        if not bool(enqueue_policy["strategy_supported"]):
            status = self.apply_mutation(
                job_id,
                self.mutation(
                    status="failed",
                    session_id=effective_session_id,
                    parallel_group_id=request.parallel_group_id,
                    lane_id=request.lane_id,
                    parent_job_id=request.parent_job_id,
                    multitask_strategy=multitask_strategy,
                    error=str(enqueue_policy["error"]),
                    timeout_seconds=self._background_job_timeout_seconds,
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
                    **self._background_trace_context(status),
                },
            )
            self.flush_background_admission_failure_trace_metadata(
                request=request,
                status=status,
            )
            return status

        if requires_takeover:
            try:
                takeover = await self._arbitrate_multitask_takeover(
                    session_id=effective_session_id,
                    incoming_job_id=job_id,
                    operation="reserve",
                )
                active_job_id = takeover.previous_active_job_id
            except SessionConflictError as error:
                status = self.apply_mutation(
                    job_id,
                    self.mutation(
                        status="failed",
                        session_id=effective_session_id,
                        parallel_group_id=request.parallel_group_id,
                        lane_id=request.lane_id,
                        parent_job_id=request.parent_job_id,
                        multitask_strategy=multitask_strategy,
                        error=str(error),
                        timeout_seconds=self._background_job_timeout_seconds,
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
                self.flush_background_admission_failure_trace_metadata(
                    request=request,
                    status=status,
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
                await self.request_interrupt(active_job_id)
                try:
                    await self._wait_for_session_release(effective_session_id)
                except Exception:
                    async with self._jobs_lock:
                        self.state_service.arbitrate_session_takeover(
                            session_id=effective_session_id,
                            incoming_job_id=job_id,
                            operation="release",
                        )
                    raise
                await self._arbitrate_multitask_takeover(
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
                    status = self.apply_mutation(
                        job_id,
                        self.mutation(
                            status="failed",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            error=str(admission_policy["error"]),
                            timeout_seconds=self._background_job_timeout_seconds,
                            max_attempts=request.max_attempts,
                            backoff_base_seconds=request.backoff_base_seconds,
                            backoff_multiplier=request.backoff_multiplier,
                            max_backoff_seconds=request.max_backoff_seconds,
                        ),
                    )
                else:
                    status = self.apply_mutation(
                        job_id,
                        self.mutation(
                            status="queued",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            timeout_seconds=self._background_job_timeout_seconds,
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
                    status = self.apply_mutation(
                        job_id,
                        self.mutation(
                            status="failed",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            error=str(error),
                            timeout_seconds=self._background_job_timeout_seconds,
                            max_attempts=request.max_attempts,
                            backoff_base_seconds=request.backoff_base_seconds,
                            backoff_multiplier=request.backoff_multiplier,
                            max_backoff_seconds=request.max_backoff_seconds,
                        ),
                    )
            else:
                async with self._jobs_lock:
                    status = self.apply_mutation(
                        job_id,
                        self.mutation(
                            status="failed",
                            session_id=effective_session_id,
                            parallel_group_id=request.parallel_group_id,
                            lane_id=request.lane_id,
                            parent_job_id=request.parent_job_id,
                            multitask_strategy=multitask_strategy,
                            error=str(error),
                            timeout_seconds=self._background_job_timeout_seconds,
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
                    **self._background_trace_context(status),
                },
            )
            self.flush_background_admission_failure_trace_metadata(
                request=request,
                status=status,
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
                "timeout_seconds": self._background_job_timeout_seconds,
                "capacity_limit": self._max_background_jobs,
                "max_attempts": request.max_attempts,
                "backoff_base_seconds": request.backoff_base_seconds,
                "backoff_multiplier": request.backoff_multiplier,
                "max_backoff_seconds": request.max_backoff_seconds,
                "background_policy_authority": self._background_control_authority,
                **self._background_trace_context(status),
            },
        )

        self.configure_limits(
            max_background_jobs=self._max_background_jobs,
            job_semaphore=self._job_semaphore,
        )
        self.start_job(job_id, request, run_task=run_task)
        return status

    async def enqueue_batch(
        self,
        requests: list[BackgroundRunRequest],
        *,
        session_id_resolver: Callable[[BackgroundRunRequest], str],
        run_task: Callable[[BackgroundRunRequest], Awaitable[RunTaskResponse]],
        parallel_group_id: str | None = None,
        lane_id_prefix: str = "lane",
    ) -> BackgroundBatchEnqueueResponse:
        """Admit a bounded parallel batch and assign lane ids through the host contract."""

        if not requests:
            raise ValueError("enqueue_background_batch requires at least one request.")
        batch_plan = self._background_batch_plan(
            requests=requests,
            parallel_group_id=parallel_group_id,
            lane_id_prefix=lane_id_prefix,
        )
        if not bool(batch_plan.get("accepted")):
            raise ValueError(
                str(
                    batch_plan.get("error")
                    or "enqueue_background_batch requires one consistent parallel_group_id across the whole batch."
                )
            )
        resolved_group_id = str(batch_plan.get("resolved_parallel_group_id") or "")
        if not resolved_group_id:
            raise RuntimeError("Rust background control returned an empty batch parallel_group_id.")
        lane_ids = batch_plan.get("lane_ids")
        if not isinstance(lane_ids, list) or len(lane_ids) != len(requests):
            raise RuntimeError("Rust background control returned an invalid batch lane plan.")
        planned_requests = [
            request.model_copy(
                update={
                    "parallel_group_id": resolved_group_id,
                    "lane_id": str(lane_id_value),
                }
            )
            for request, lane_id_value in zip(requests, lane_ids)
        ]
        statuses = list(
            await asyncio.gather(
                *(
                    self.enqueue_job(
                        planned_request,
                        session_id_resolver=session_id_resolver,
                        run_task=run_task,
                    )
                    for planned_request in planned_requests
                )
            )
        )
        summary = self.parallel_group_summary(resolved_group_id)
        if summary is None:
            raise RuntimeError(f"Background parallel group {resolved_group_id!r} was not persisted.")
        return BackgroundBatchEnqueueResponse(
            parallel_group_id=resolved_group_id,
            statuses=statuses,
            summary=summary,
        )

    def parallel_group_summary(
        self,
        parallel_group_id: str,
    ) -> BackgroundParallelGroupSummary | None:
        """Return one durable parallel-batch summary by group id."""

        return self.state_service.parallel_group_summary(parallel_group_id)

    def parallel_group_summaries(self) -> list[BackgroundParallelGroupSummary]:
        """Return all durable parallel-batch summaries."""

        return self.state_service.parallel_group_summaries()

    def start_job(
        self,
        job_id: str,
        request: BackgroundRunRequest,
        *,
        run_task: Callable[[BackgroundRunRequest], Awaitable[RunTaskResponse]],
    ) -> None:
        """Create the steady-state background runner task for one admitted job."""

        self.background_requests[job_id] = request
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
            if current.status in self._terminal_statuses:
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
                **self._background_trace_context(updated),
            },
        )

        if interrupt_step == "request_interrupt" and bool(interrupt_effect.get("cancel_running_task")) and task is not None and not task.done():
            task.cancel()
        if interrupt_step == "finalize_interrupted" or task is None or task.done():
            return await self._mark_background_interrupted(job_id, session_id=session_id, error="Interrupt requested")
        return updated

    def health(self) -> dict[str, Any]:
        parallel_groups = self.state_service.parallel_group_summaries()
        return {
            **_runtime_service_health_projection(self._service_descriptor),
            "max_background_jobs": self._max_background_jobs,
            "background_job_timeout_seconds": self._background_job_timeout_seconds,
            "active_task_count": len([task for task in self._background_tasks.values() if not task.done()]),
            "parallel_group_count": len(parallel_groups),
            "active_parallel_group_count": len(
                [summary for summary in parallel_groups if summary.active_job_count > 0]
            ),
            "background_policy_authority": self._background_control_authority,
            "orchestration_contract": dict(self._orchestration_contract),
            "background_effect_host_contract": _runtime_background_effect_host_contract(
                self._control_plane_descriptor,
                self._service_descriptor,
                service_name="background",
            ),
        }

    @staticmethod
    def _background_trace_context(job: BackgroundRunStatus | BackgroundRunRequest) -> dict[str, Any]:
        """Project durable parallel-batch identifiers into trace payloads."""

        payload: dict[str, Any] = {}
        for field in ("parallel_group_id", "lane_id", "parent_job_id"):
            value = getattr(job, field, None)
            if value is not None:
                payload[field] = value
        return payload

    def _parallel_group_summary_payload(
        self,
        *,
        parallel_group_id: str | None,
    ) -> dict[str, Any] | None:
        """Return one JSON-safe parallel-batch summary when the job belongs to a group."""

        if parallel_group_id is None:
            return None
        summary = self.state_service.parallel_group_summary(parallel_group_id)
        if summary is None:
            return None
        return summary.model_dump(mode="json")

    def _flush_background_trace_metadata(
        self,
        *,
        request: BackgroundRunRequest,
        result: RunTaskResponse,
        status: BackgroundRunStatus,
    ) -> None:
        """Project one completed background result into the canonical trace metadata artifact."""

        matched_skills = [result.skill]
        if result.overlay:
            matched_skills.append(result.overlay)
        self._trace.flush_metadata(
            task=request.task,
            matched_skills=matched_skills,
            owner=result.skill,
            gate=str(result.metadata.get("routing_gate") or "none"),
            overlay=result.overlay,
            artifact_paths=self._artifact_paths_provider(),
            verification_status="completed" if result.live_run else "dry_run",
            session_id=result.session_id,
            job_id=status.job_id,
            parallel_group=self._parallel_group_summary_payload(
                parallel_group_id=status.parallel_group_id
            ),
            supervisor_projection=self._supervisor_projection_provider(),
        )

    def _flush_background_terminal_trace_metadata(
        self,
        *,
        request: BackgroundRunRequest,
        status: BackgroundRunStatus,
        verification_status: str,
    ) -> None:
        """Flush top-level trace metadata for one terminal background state without a result payload."""

        route_payload = self._trace.latest_route_selection(session_id=request.session_id or status.job_id)
        if route_payload is None:
            return
        matched_skills = [str(route_payload.get("skill") or "unknown")]
        overlay = route_payload.get("overlay")
        if isinstance(overlay, str) and overlay:
            matched_skills.append(overlay)
        self._trace.flush_metadata(
            task=request.task,
            matched_skills=matched_skills,
            owner=str(route_payload.get("routing_owner") or matched_skills[0]),
            gate=str(route_payload.get("routing_gate") or "none"),
            overlay=str(overlay) if isinstance(overlay, str) and overlay else None,
            artifact_paths=self._artifact_paths_provider(),
            verification_status=verification_status,
            session_id=request.session_id,
            job_id=status.job_id,
            parallel_group=self._parallel_group_summary_payload(
                parallel_group_id=status.parallel_group_id
            ),
            supervisor_projection=self._supervisor_projection_provider(),
        )

    def flush_background_admission_failure_trace_metadata(
        self,
        *,
        request: BackgroundRunRequest,
        status: BackgroundRunStatus,
    ) -> None:
        """Project grouped pre-execution failures into top-level trace metadata."""

        if status.parallel_group_id is None:
            return
        control_plane_owner = "background-runtime-host"
        self._trace.flush_metadata(
            task=request.task,
            matched_skills=[control_plane_owner],
            owner=control_plane_owner,
            gate="none",
            overlay=None,
            artifact_paths=self._artifact_paths_provider(),
            verification_status="failed",
            session_id=request.session_id,
            job_id=status.job_id,
            parallel_group=self._parallel_group_summary_payload(
                parallel_group_id=status.parallel_group_id
            ),
            supervisor_projection=self._supervisor_projection_provider(),
        )

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

    async def _arbitrate_multitask_takeover(
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

    def _notify_session_release(self, session_id: str | None) -> None:
        self.state_service.notify_session_release(session_id)

    async def _wait_for_session_release(self, session_id: str, *, timeout_seconds: float = 5.0) -> None:
        """Wait until a session is no longer reserved by active background work."""

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

        return self._background_policy(
            {
                "operation": "enqueue",
                "multitask_strategy": multitask_strategy,
                "active_job_count": active_job_count,
                "capacity_limit": self._max_background_jobs,
            }
        )

    def _background_session_release_policy(self, *, session_id: str) -> dict[str, Any]:
        """Resolve background session-release timing through the Rust control seam."""

        return self._background_policy(
            {
                "operation": "session-release",
                "current_status": "release_pending",
                "task_active": False,
                "task_done": False,
                "active_job_count": self.state_service.active_job_count(),
                "capacity_limit": self._max_background_jobs,
                "session_id": session_id,
            }
        )

    def _background_batch_plan(
        self,
        *,
        requests: list[BackgroundRunRequest],
        parallel_group_id: str | None,
        lane_id_prefix: str,
    ) -> dict[str, Any]:
        """Resolve batch-level parallel group and lane assignment through the Rust control seam."""

        return self._background_policy(
            {
                "operation": "batch-plan",
                "requested_parallel_group_id": parallel_group_id,
                "request_parallel_group_ids": [request.parallel_group_id for request in requests],
                "request_lane_ids": [request.lane_id for request in requests],
                "lane_id_prefix": lane_id_prefix,
                "batch_size": len(requests),
            }
        )

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
                        latest = self._job_store.get(job_id)
                        if latest is None:
                            return
                        claim_policy = self._background_status_transition_policy(
                            operation="claim",
                            current_status=latest.status,
                            task_active=False,
                            task_done=False,
                        )
                        claim_effect = self._background_effect_plan(claim_policy)
                        claim_step = str(claim_effect["next_step"])
                        if claim_step == "finalize_interrupted":
                            running = None
                        elif claim_step == "finalize_terminal":
                            return
                        else:
                            running_status = str(
                                claim_effect.get("resolved_status") or claim_policy.get("resolved_status") or "running"
                            )
                            running = self.apply_mutation(
                                job_id,
                                self.mutation(
                                    status=running_status,
                                    current=latest,
                                    session_id=request.session_id,
                                    error=None,
                                    timeout_seconds=self._background_job_timeout_seconds,
                                    claimed_by=job_id,
                                    claimed_at=started_at,
                                    last_attempt_started_at=started_at,
                                ),
                            )
                    if claim_step == "finalize_interrupted":
                        await self._mark_background_interrupted(
                            job_id,
                            session_id=(latest.session_id if latest is not None else request.session_id) or job_id,
                            error="Interrupt requested before execution",
                        )
                        return
                    if running is None:
                        return
                    self._trace.record(
                        session_id=request.session_id or job_id,
                        job_id=job_id,
                        kind="job.claimed",
                        stage="background",
                        payload={
                            "status": running.status,
                            "attempt": running.attempt,
                            "background_policy_authority": self._background_control_authority,
                            **self._background_trace_context(running),
                            **self.execution_service.kernel_payload(
                                dry_run=bool(request.dry_run),
                            ),
                        },
                    )

                    try:
                        result = await asyncio.wait_for(
                            run_task(request.model_copy(update={"job_id": job_id})),
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
                            **self._background_trace_context(completed),
                            **self.execution_service.kernel_payload(
                                dry_run=not result.live_run,
                                metadata=result.metadata,
                            ),
                        },
                    )
                    self._notify_session_release(completed.session_id)
                    self._flush_background_trace_metadata(
                        request=request,
                        result=result,
                        status=completed,
                    )
                    self._checkpoint_background_resume_manifest(
                        session_id=result.session_id,
                        job_id=job_id,
                        status="completed",
                        parallel_group_id=completed.parallel_group_id,
                    )
                    return
        except asyncio.CancelledError:
            current = self._job_store.get(job_id)
            if current is not None and current.status not in self._terminal_statuses:
                await self._mark_background_interrupted(
                    job_id,
                    session_id=current.session_id or job_id,
                    error="Interrupt requested",
                )
            raise
        finally:
            if self._background_tasks.get(job_id) is current_task:
                self._background_tasks.pop(job_id, None)
            self.background_requests.pop(job_id, None)

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
                    payload={
                        "error": error,
                        "attempt": current.attempt,
                        **self._background_trace_context(current),
                    },
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
                            payload={
                                "attempt": finalized.attempt,
                                "max_attempts": finalized.max_attempts,
                                "error": error,
                                **self._background_trace_context(finalized),
                            },
                        )
                    request = self.background_requests.get(job_id)
                    if request is not None:
                        self._flush_background_terminal_trace_metadata(
                            request=request,
                            status=finalized,
                            verification_status=terminal_status,
                        )
                    self._checkpoint_background_resume_manifest(
                        session_id=session_id,
                        job_id=job_id,
                        status=terminal_status,
                        parallel_group_id=finalized.parallel_group_id,
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
                **self._background_trace_context(scheduled),
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
            payload={
                "attempt": claimed.attempt,
                "retry_count": claimed.retry_count,
                **self._background_trace_context(claimed),
            },
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
                **self._background_trace_context(interrupted),
            },
        )
        self._notify_session_release(interrupted.session_id)
        request = self.background_requests.get(job_id)
        if request is not None:
            self._flush_background_terminal_trace_metadata(
                request=request,
                status=interrupted,
                verification_status="interrupted",
            )
        self._checkpoint_background_resume_manifest(
            session_id=session_id,
            job_id=job_id,
            status="interrupted",
            parallel_group_id=interrupted.parallel_group_id,
        )
        return interrupted

    def _checkpoint_background_resume_manifest(
        self,
        *,
        session_id: str,
        job_id: str,
        status: str,
        parallel_group_id: str | None = None,
    ) -> None:
        """Persist background resume state without routing ownership back through runtime.py."""

        self.trace_service.checkpoint(
            session_id=session_id,
            job_id=job_id,
            status=status,
            artifact_paths=self._artifact_paths_provider(),
            parallel_group=self._parallel_group_summary_payload(parallel_group_id=parallel_group_id),
            supervisor_projection=self._supervisor_projection_provider(),
        )
