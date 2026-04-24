"""Runtime background-state helpers."""

from __future__ import annotations

from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Literal, Mapping

from pydantic import BaseModel, Field

from framework_runtime.checkpoint_store import (
    InMemoryRuntimeStorageBackend,
    RuntimeStorageBackend,
    select_runtime_storage_backend,
)
from framework_runtime.paths import default_codex_home
from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.schemas import BackgroundParallelGroupSummary, BackgroundRunStatus, RunTaskResponse


ACTIVE_JOB_STATUSES = {
    "queued",
    "running",
    "interrupt_requested",
    "retry_scheduled",
    "retry_claimed",
}
TERMINAL_JOB_STATUSES = {"completed", "failed", "interrupted", "retry_exhausted"}
BACKGROUND_STATE_SCHEMA_VERSION = "runtime-background-state-v5"
BACKGROUND_STATE_CONTROL_PLANE_SCHEMA_VERSION = "runtime-background-state-control-plane-v1"
BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION = "runtime-background-session-takeover-arbitration-v1"
RUST_BACKGROUND_STATE_REQUEST_SCHEMA_VERSION = "router-rs-background-state-request-v1"
_STATE_SERVICE_NAME = "state"
_DEFAULT_STATE_SERVICE_DESCRIPTOR = {
    "authority": "rust-runtime-control-plane",
    "role": "durable-background-state",
    "projection": "rust-native-projection",
    "delegate_kind": "filesystem-state-store",
}
_DEFAULT_BACKGROUND_JOB_MULTITASK_STRATEGY = "reject"
_DEFAULT_BACKGROUND_JOB_ATTEMPT = 1
_DEFAULT_BACKGROUND_JOB_RETRY_COUNT = 0
_DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS = 1
_DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS = 0.0
_DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER = 2.0
_DEFAULT_IN_MEMORY_BACKGROUND_STATE_PATH = (
    default_codex_home() / ".codex" / "runtime_background_jobs.memory.json"
)
VALID_TRANSITIONS = {
    None: ACTIVE_JOB_STATUSES | TERMINAL_JOB_STATUSES,
    "queued": {"queued", "running", "interrupt_requested", "interrupted", "failed"},
    "running": {
        "running",
        "interrupt_requested",
        "completed",
        "failed",
        "interrupted",
        "retry_scheduled",
        "retry_exhausted",
    },
    "interrupt_requested": {"interrupt_requested", "interrupted"},
    "retry_scheduled": {"retry_scheduled", "retry_claimed", "interrupt_requested", "interrupted", "retry_exhausted"},
    "retry_claimed": {
        "retry_claimed",
        "queued",
        "running",
        "interrupt_requested",
        "interrupted",
        "failed",
        "retry_scheduled",
        "retry_exhausted",
    },
    "completed": {"completed"},
    "failed": {"failed"},
    "interrupted": {"interrupted"},
    "retry_exhausted": {"retry_exhausted"},
}


class SessionConflictError(RuntimeError):
    """Raised when another active job already owns the same session."""


class BackgroundSessionTakeoverArbitration(BaseModel):
    """Versioned reducer result for one session takeover operation."""

    schema_version: str = BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION
    operation: Literal["reserve", "claim", "release"]
    session_id: str
    incoming_job_id: str
    previous_active_job_id: str | None = None
    previous_pending_job_id: str | None = None
    active_job_id: str | None = None
    pending_job_id: str | None = None
    outcome: Literal["available", "pending", "owned", "claimed", "released", "noop"]
    changed: bool = False


class BackgroundStateControlPlaneDescriptor(BaseModel):
    """Rust-owned control-plane projection for the Python background-state host."""

    schema_version: str = BACKGROUND_STATE_CONTROL_PLANE_SCHEMA_VERSION
    runtime_control_plane_schema_version: str | None = None
    runtime_control_plane_authority: str = _DEFAULT_STATE_SERVICE_DESCRIPTOR["authority"]
    service: str = _STATE_SERVICE_NAME
    authority: str = _DEFAULT_STATE_SERVICE_DESCRIPTOR["authority"]
    role: str = _DEFAULT_STATE_SERVICE_DESCRIPTOR["role"]
    projection: str = _DEFAULT_STATE_SERVICE_DESCRIPTOR["projection"]
    delegate_kind: str = _DEFAULT_STATE_SERVICE_DESCRIPTOR["delegate_kind"]
    transport_family: str = "checkpoint-artifact"
    health_family: str = "runtime-health"
    backend_family: str = "filesystem"
    supports_atomic_replace: bool = True
    supports_compaction: bool = False
    supports_snapshot_delta: bool = False
    supports_remote_event_transport: bool = False
    state_path: str | None = None


@dataclass(frozen=True, slots=True)
class BackgroundJobStatusMutation:
    """State-owned reducer contract for one background job row mutation."""

    status: str
    session_id: str | None = None
    parallel_group_id: str | None = None
    lane_id: str | None = None
    parent_job_id: str | None = None
    multitask_strategy: str | None = None
    result: RunTaskResponse | None = None
    error: str | None = None
    timeout_seconds: float | None = None
    claimed_by: str | None = None
    attempt: int | None = None
    retry_count: int | None = None
    max_attempts: int | None = None
    claimed_at: str | None = None
    backoff_base_seconds: float | None = None
    backoff_multiplier: float | None = None
    max_backoff_seconds: float | None = None
    backoff_seconds: float | None = None
    next_retry_at: str | None = None
    retry_scheduled_at: str | None = None
    retry_claimed_at: str | None = None
    interrupt_requested_at: str | None = None
    interrupted_at: str | None = None
    last_attempt_started_at: str | None = None
    last_attempt_finished_at: str | None = None
    last_failure_at: str | None = None

    @classmethod
    def from_transition(
        cls,
        *,
        status: str,
        existing: BackgroundRunStatus | None = None,
        **overrides: Any,
    ) -> "BackgroundJobStatusMutation":
        """Build one mutation payload from an existing row plus explicit overrides."""

        payload: dict[str, Any] = {}
        if existing is not None:
            payload.update(
                existing.model_dump(
                    mode="python",
                    exclude={"job_id", "status", "created_at", "updated_at"},
                )
            )
        payload.update(overrides)
        payload["status"] = status
        return cls(**payload)

    def apply(self, *, job_id: str, existing: BackgroundRunStatus | None) -> BackgroundRunStatus:
        """Reduce the mutation against the current durable row snapshot."""

        if existing is None:
            return BackgroundRunStatus(
                job_id=job_id,
                session_id=self.session_id,
                status=self.status,
                parallel_group_id=self.parallel_group_id,
                lane_id=self.lane_id,
                parent_job_id=self.parent_job_id,
                multitask_strategy=self.multitask_strategy or _DEFAULT_BACKGROUND_JOB_MULTITASK_STRATEGY,
                result=self.result,
                error=self.error,
                attempt=self.attempt if self.attempt is not None else _DEFAULT_BACKGROUND_JOB_ATTEMPT,
                retry_count=self.retry_count
                if self.retry_count is not None
                else _DEFAULT_BACKGROUND_JOB_RETRY_COUNT,
                max_attempts=self.max_attempts
                if self.max_attempts is not None
                else _DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS,
                timeout_seconds=self.timeout_seconds,
                claimed_by=self.claimed_by,
                claimed_at=self.claimed_at,
                backoff_base_seconds=self.backoff_base_seconds
                if self.backoff_base_seconds is not None
                else _DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS,
                backoff_multiplier=self.backoff_multiplier
                if self.backoff_multiplier is not None
                else _DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER,
                max_backoff_seconds=self.max_backoff_seconds,
                backoff_seconds=self.backoff_seconds,
                next_retry_at=self.next_retry_at,
                retry_scheduled_at=self.retry_scheduled_at,
                retry_claimed_at=self.retry_claimed_at,
                interrupt_requested_at=self.interrupt_requested_at,
                interrupted_at=self.interrupted_at,
                last_attempt_started_at=self.last_attempt_started_at,
                last_attempt_finished_at=self.last_attempt_finished_at,
                last_failure_at=self.last_failure_at,
            )
        return existing.touch(
            status=self.status,
            session_id=self.session_id,
            parallel_group_id=(
                self.parallel_group_id
                if self.parallel_group_id is not None
                else existing.parallel_group_id
            ),
            lane_id=self.lane_id if self.lane_id is not None else existing.lane_id,
            parent_job_id=(
                self.parent_job_id
                if self.parent_job_id is not None
                else existing.parent_job_id
            ),
            multitask_strategy=(
                self.multitask_strategy
                if self.multitask_strategy is not None
                else existing.multitask_strategy
            ),
            result=self.result,
            error=self.error,
            attempt=self.attempt if self.attempt is not None else existing.attempt,
            retry_count=self.retry_count if self.retry_count is not None else existing.retry_count,
            max_attempts=self.max_attempts if self.max_attempts is not None else existing.max_attempts,
            timeout_seconds=self.timeout_seconds if self.timeout_seconds is not None else existing.timeout_seconds,
            claimed_by=self.claimed_by if self.claimed_by is not None else existing.claimed_by,
            claimed_at=self.claimed_at if self.claimed_at is not None else existing.claimed_at,
            backoff_base_seconds=(
                self.backoff_base_seconds
                if self.backoff_base_seconds is not None
                else existing.backoff_base_seconds
            ),
            backoff_multiplier=(
                self.backoff_multiplier
                if self.backoff_multiplier is not None
                else existing.backoff_multiplier
            ),
            max_backoff_seconds=(
                self.max_backoff_seconds
                if self.max_backoff_seconds is not None
                else existing.max_backoff_seconds
            ),
            backoff_seconds=self.backoff_seconds,
            next_retry_at=self.next_retry_at,
            retry_scheduled_at=self.retry_scheduled_at,
            retry_claimed_at=self.retry_claimed_at,
            interrupt_requested_at=(
                self.interrupt_requested_at
                if self.interrupt_requested_at is not None
                else existing.interrupt_requested_at
            ),
            interrupted_at=self.interrupted_at if self.interrupted_at is not None else existing.interrupted_at,
            last_attempt_started_at=(
                self.last_attempt_started_at
                if self.last_attempt_started_at is not None
                else existing.last_attempt_started_at
            ),
            last_attempt_finished_at=(
                self.last_attempt_finished_at
                if self.last_attempt_finished_at is not None
                else existing.last_attempt_finished_at
            ),
            last_failure_at=self.last_failure_at if self.last_failure_at is not None else existing.last_failure_at,
        )


def _build_state_control_plane_descriptor(
    *,
    control_plane_descriptor: Mapping[str, Any] | None,
    storage_backend: RuntimeStorageBackend,
    state_path: Path | None,
) -> BackgroundStateControlPlaneDescriptor:
    capabilities = storage_backend.capabilities()
    payload: dict[str, Any] = {
        "delegate_kind": f"{capabilities.backend_family.strip().lower().replace('_', '-')}-state-store",
        "backend_family": capabilities.backend_family,
        "supports_atomic_replace": capabilities.supports_atomic_replace,
        "supports_compaction": capabilities.supports_compaction,
        "supports_snapshot_delta": capabilities.supports_snapshot_delta,
        "supports_remote_event_transport": capabilities.supports_remote_event_transport,
        "state_path": str(state_path) if state_path is not None else None,
    }
    if isinstance(control_plane_descriptor, Mapping):
        payload["runtime_control_plane_schema_version"] = control_plane_descriptor.get("schema_version")
        payload["runtime_control_plane_authority"] = str(
            control_plane_descriptor.get("authority") or _DEFAULT_STATE_SERVICE_DESCRIPTOR["authority"]
        )
        services = control_plane_descriptor.get("services")
        if isinstance(services, Mapping):
            service = services.get(_STATE_SERVICE_NAME)
            if isinstance(service, Mapping):
                for field in ("authority", "role", "projection", "delegate_kind"):
                    value = service.get(field)
                    if value is not None:
                        payload[field] = value
    if (
        capabilities.backend_family != "filesystem"
        and payload.get("delegate_kind") == "filesystem-state-store"
    ):
        payload["delegate_kind"] = (
            f"{capabilities.backend_family.strip().lower().replace('_', '-')}-state-store"
        )
    return BackgroundStateControlPlaneDescriptor.model_validate(payload)


class BackgroundJobStore:
    """Track background job lifecycle states with stable timestamps."""

    def __init__(
        self,
        *,
        state_path: Path | None = None,
        storage_backend: RuntimeStorageBackend | None = None,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self._jobs: dict[str, BackgroundRunStatus] = {}
        self._active_sessions: dict[str, str] = {}
        self._pending_session_takeovers: dict[str, str] = {}
        self._state_payload_text: str | None = None
        resolved_state_path = state_path
        resolved_storage_backend = storage_backend
        if resolved_state_path is None:
            resolved_state_path = _DEFAULT_IN_MEMORY_BACKGROUND_STATE_PATH
            resolved_storage_backend = resolved_storage_backend or InMemoryRuntimeStorageBackend()
        self._state_path = resolved_state_path
        self._storage_backend = resolved_storage_backend or select_runtime_storage_backend(
            storage_root=resolved_state_path.parent
        )
        self._control_plane = _build_state_control_plane_descriptor(
            control_plane_descriptor=control_plane_descriptor,
            storage_backend=self._storage_backend,
            state_path=self._state_path,
        )
        self._use_rust_background_state = True
        self._rust_adapter = RustRouteAdapter(default_codex_home()) if self._use_rust_background_state else None
        self._load_state()

    def set_status(
        self,
        job_id: str,
        *,
        status: str,
        session_id: str | None = None,
        parallel_group_id: str | None = None,
        lane_id: str | None = None,
        parent_job_id: str | None = None,
        multitask_strategy: str | None = None,
        result: RunTaskResponse | None = None,
        error: str | None = None,
        timeout_seconds: float | None = None,
        claimed_by: str | None = None,
        attempt: int | None = None,
        retry_count: int | None = None,
        max_attempts: int | None = None,
        claimed_at: str | None = None,
        backoff_base_seconds: float | None = None,
        backoff_multiplier: float | None = None,
        max_backoff_seconds: float | None = None,
        backoff_seconds: float | None = None,
        next_retry_at: str | None = None,
        retry_scheduled_at: str | None = None,
        retry_claimed_at: str | None = None,
        interrupt_requested_at: str | None = None,
        interrupted_at: str | None = None,
        last_attempt_started_at: str | None = None,
        last_attempt_finished_at: str | None = None,
        last_failure_at: str | None = None,
    ) -> BackgroundRunStatus:
        """Apply a state-owned background job mutation and persist it."""

        mutation = BackgroundJobStatusMutation(
            status=status,
            session_id=session_id,
            parallel_group_id=parallel_group_id,
            lane_id=lane_id,
            parent_job_id=parent_job_id,
            multitask_strategy=multitask_strategy,
            result=result,
            error=error,
            timeout_seconds=timeout_seconds,
            claimed_by=claimed_by,
            attempt=attempt,
            retry_count=retry_count,
            max_attempts=max_attempts,
            claimed_at=claimed_at,
            backoff_base_seconds=backoff_base_seconds,
            backoff_multiplier=backoff_multiplier,
            max_backoff_seconds=max_backoff_seconds,
            backoff_seconds=backoff_seconds,
            next_retry_at=next_retry_at,
            retry_scheduled_at=retry_scheduled_at,
            retry_claimed_at=retry_claimed_at,
            interrupt_requested_at=interrupt_requested_at,
            interrupted_at=interrupted_at,
            last_attempt_started_at=last_attempt_started_at,
            last_attempt_finished_at=last_attempt_finished_at,
            last_failure_at=last_failure_at,
        )
        return self.apply_mutation(job_id, mutation)

    def apply_mutation(self, job_id: str, mutation: BackgroundJobStatusMutation) -> BackgroundRunStatus:
        """Apply a pre-built mutation contract and persist the resulting durable row."""

        if self._use_rust_background_state:
            response = self._invoke_rust_state(
                operation="apply_mutation",
                job_id=job_id,
                mutation=asdict(mutation),
            )
            job = response.get("job")
            if not isinstance(job, dict):
                raise RuntimeError("Rust background state store returned a missing job payload.")
            return BackgroundRunStatus.model_validate(job)
        raise RuntimeError("Python background state fallback is retired.")

    def get(self, job_id: str) -> BackgroundRunStatus | None:
        """Return one background job row."""

        if self._use_rust_background_state:
            response = self._invoke_rust_state(operation="get", job_id=job_id)
            job = response.get("job")
            return BackgroundRunStatus.model_validate(job) if isinstance(job, dict) else None
        return self._jobs.get(job_id)

    def snapshot(self) -> dict[str, BackgroundRunStatus]:
        """Return a snapshot copy of all jobs keyed by job id."""

        if self._use_rust_background_state:
            self._invoke_rust_state(operation="snapshot")
        return dict(self._jobs)

    def get_active_job(self, session_id: str) -> str | None:
        """Return the active job id for a session, if one is reserved."""

        if self._use_rust_background_state:
            response = self._invoke_rust_state(operation="get_active_job", session_id=session_id)
            active_job_id = response.get("active_job_id")
            return active_job_id if isinstance(active_job_id, str) else None
        return self._active_sessions.get(session_id)

    def arbitrate_session_takeover(
        self,
        *,
        session_id: str,
        incoming_job_id: str,
        operation: Literal["reserve", "claim", "release"],
    ) -> BackgroundSessionTakeoverArbitration:
        """Reduce one takeover operation against the current session state."""

        if self._use_rust_background_state:
            response = self._invoke_rust_state(
                operation=operation,
                session_id=session_id,
                incoming_job_id=incoming_job_id,
            )
            takeover = response.get("takeover")
            if not isinstance(takeover, dict):
                raise RuntimeError("Rust background state store returned a missing takeover payload.")
            return BackgroundSessionTakeoverArbitration.model_validate(takeover)
        raise RuntimeError("Python background state fallback is retired.")

    def reserve_session_takeover(self, *, session_id: str, incoming_job_id: str) -> str | None:
        """Reserve the next ownership handoff for one session."""

        decision = self.arbitrate_session_takeover(
            session_id=session_id,
            incoming_job_id=incoming_job_id,
            operation="reserve",
        )
        return decision.previous_active_job_id

    def claim_session_takeover(self, *, session_id: str, incoming_job_id: str) -> None:
        """Claim a previously reserved handoff once the old owner has released."""

        self.arbitrate_session_takeover(
            session_id=session_id,
            incoming_job_id=incoming_job_id,
            operation="claim",
        )

    def release_session_takeover(self, *, session_id: str, incoming_job_id: str) -> None:
        """Release a pending or pre-claimed takeover when enqueue fails."""

        self.arbitrate_session_takeover(
            session_id=session_id,
            incoming_job_id=incoming_job_id,
            operation="release",
        )

    def pending_session_takeovers(self) -> int:
        """Return the number of in-flight replacement reservations."""

        if self._use_rust_background_state:
            self._invoke_rust_state(operation="snapshot")
        return len(self._pending_session_takeovers)

    def active_job_count(self) -> int:
        """Return the number of currently admitted background jobs."""

        if self._use_rust_background_state:
            self._invoke_rust_state(operation="snapshot")
        return sum(1 for job in self._jobs.values() if job.status in ACTIVE_JOB_STATUSES)

    def parallel_group_summary(self, parallel_group_id: str) -> BackgroundParallelGroupSummary | None:
        """Return one aggregate summary for a durable parallel batch."""

        response = self._invoke_rust_state(
            operation="parallel_group_summary",
            parallel_group_id=parallel_group_id,
        )
        summary = response.get("parallel_group_summary")
        return BackgroundParallelGroupSummary.model_validate(summary) if isinstance(summary, dict) else None

    def parallel_group_summaries(self) -> list[BackgroundParallelGroupSummary]:
        """Return aggregate summaries for all durable parallel batches."""

        response = self._invoke_rust_state(operation="parallel_group_summaries")
        summaries = response.get("parallel_group_summaries")
        if not isinstance(summaries, list):
            raise RuntimeError("Rust background state store returned an invalid parallel_group_summaries payload.")
        return [BackgroundParallelGroupSummary.model_validate(item) for item in summaries if isinstance(item, dict)]

    def control_plane_descriptor(self) -> BackgroundStateControlPlaneDescriptor:
        """Return the Rust-owned control-plane projection for this Python store."""

        self._invoke_rust_state(operation="snapshot")
        return self._control_plane.model_copy()

    def health(self) -> dict[str, Any]:
        """Return store health using the shared control-plane boundary."""

        response = self._invoke_rust_state(operation="health")
        health = response.get("health")
        if isinstance(health, dict):
            return health
        raise RuntimeError("Rust background state store returned an invalid health payload.")

    def _load_state(self) -> None:
        """Load durable state from disk when a state path is configured."""

        if (
            self._storage_backend.capabilities().backend_family == "memory"
            and self._state_path is not None
            and self._storage_backend.exists(self._state_path)
        ):
            self._state_payload_text = self._storage_backend.read_text(self._state_path)
        self._invoke_rust_state(operation="snapshot")

    def _invoke_rust_state(self, *, operation: str, **payload: Any) -> dict[str, Any]:
        if self._rust_adapter is None:
            raise RuntimeError("Rust background state store is not configured.")
        request = self._build_rust_state_request(operation=operation, **payload)
        try:
            response = self._rust_adapter.background_state(request)
        except RuntimeError as exc:
            message = str(exc)
            if "Session " in message and (
                "already active in job" in message
                or "already has a pending takeover" in message
                or "is not reserved for incoming job" in message
                or "is still active in job" in message
            ):
                raise SessionConflictError(message) from exc
            raise
        self._refresh_memory_payload(response)
        self._sync_rust_snapshot(response.get("state"))
        return response

    def _build_rust_state_request(self, *, operation: str, **payload: Any) -> dict[str, Any]:
        capabilities = self._storage_backend.capabilities()
        request: dict[str, Any] = {
            "schema_version": RUST_BACKGROUND_STATE_REQUEST_SCHEMA_VERSION,
            "operation": operation,
            "state_path": str(self._state_path) if self._state_path is not None else None,
            "backend_family": capabilities.backend_family,
            "control_plane_descriptor": {
                "schema_version": self._control_plane.runtime_control_plane_schema_version,
                "authority": self._control_plane.runtime_control_plane_authority,
                "services": {
                    "state": {
                        "authority": self._control_plane.authority,
                        "role": self._control_plane.role,
                        "projection": self._control_plane.projection,
                        "delegate_kind": self._control_plane.delegate_kind,
                    }
                },
            },
        }
        if capabilities.backend_family == "memory":
            request["state_payload_text"] = self._state_payload_text
        sqlite_db_path = getattr(self._storage_backend, "_db_path", None)
        if sqlite_db_path is not None:
            request["sqlite_db_path"] = str(sqlite_db_path)
        request.update(payload)
        return request

    def _sync_rust_snapshot(self, snapshot: Any) -> None:
        if not isinstance(snapshot, Mapping):
            return
        control_plane = snapshot.get("control_plane")
        if isinstance(control_plane, Mapping):
            self._control_plane = BackgroundStateControlPlaneDescriptor.model_validate(control_plane)
        jobs = snapshot.get("jobs")
        if isinstance(jobs, list):
            self._jobs = {
                job.job_id: job
                for job in (
                    BackgroundRunStatus.model_validate(item)
                    for item in jobs
                    if isinstance(item, Mapping)
                )
            }
        active_sessions = snapshot.get("active_sessions")
        if isinstance(active_sessions, list):
            self._active_sessions = {
                row["session_id"]: row["job_id"]
                for row in active_sessions
                if isinstance(row, Mapping)
                and isinstance(row.get("session_id"), str)
                and isinstance(row.get("job_id"), str)
            }
        pending_takeovers = snapshot.get("pending_session_takeovers")
        if isinstance(pending_takeovers, list):
            self._pending_session_takeovers = {
                row["session_id"]: row["incoming_job_id"]
                for row in pending_takeovers
                if isinstance(row, Mapping)
                and isinstance(row.get("session_id"), str)
                and isinstance(row.get("incoming_job_id"), str)
            }
        self._sync_memory_backend_state()

    def _sync_memory_backend_state(self) -> None:
        if self._state_path is None or self._storage_backend.capabilities().backend_family != "memory":
            return
        payload = self._state_payload_text
        if payload is None:
            return
        self._storage_backend.write_text(self._state_path, payload)

    def _refresh_memory_payload(self, response: Mapping[str, Any]) -> None:
        if self._storage_backend.capabilities().backend_family != "memory":
            return
        payload_text = response.get("persisted_payload_text")
        if isinstance(payload_text, str):
            self._state_payload_text = payload_text
        elif self._state_path is not None and self._storage_backend.exists(self._state_path):
            self._state_payload_text = self._storage_backend.read_text(self._state_path)
