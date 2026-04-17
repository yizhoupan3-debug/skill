"""Runtime background-state helpers."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from pydantic import BaseModel, Field

from codex_agno_runtime.checkpoint_store import FilesystemRuntimeStorageBackend, RuntimeStorageBackend
from codex_agno_runtime.schemas import BackgroundRunStatus, RunTaskResponse


ACTIVE_JOB_STATUSES = {
    "queued",
    "running",
    "interrupt_requested",
    "retry_scheduled",
    "retry_claimed",
}
TERMINAL_JOB_STATUSES = {"completed", "failed", "interrupted", "retry_exhausted"}
BACKGROUND_STATE_SCHEMA_VERSION = "runtime-background-state-v4"
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


@dataclass(frozen=True)
class PendingSessionTakeover:
    """Represent one in-flight session handoff to a replacement job."""

    session_id: str
    incoming_job_id: str


class _PersistedActiveSession(BaseModel):
    """One durable active-session reservation row."""

    session_id: str
    job_id: str


class _PersistedPendingTakeover(BaseModel):
    """One durable pending session handoff row."""

    session_id: str
    incoming_job_id: str


class _PersistedBackgroundState(BaseModel):
    """Versioned durable contract for background job state."""

    version: int = 2
    schema_version: str = BACKGROUND_STATE_SCHEMA_VERSION
    jobs: list[BackgroundRunStatus] = Field(default_factory=list)
    active_sessions: list[_PersistedActiveSession] = Field(default_factory=list)
    pending_session_takeovers: list[_PersistedPendingTakeover] = Field(default_factory=list)


class BackgroundJobStore:
    """Track background job lifecycle states with stable timestamps."""

    def __init__(
        self,
        *,
        state_path: Path | None = None,
        storage_backend: RuntimeStorageBackend | None = None,
    ) -> None:
        self._jobs: dict[str, BackgroundRunStatus] = {}
        self._active_sessions: dict[str, str] = {}
        self._pending_session_takeovers: dict[str, str] = {}
        self._state_path = state_path
        self._storage_backend = storage_backend or FilesystemRuntimeStorageBackend()
        self._load_state()

    def set_status(
        self,
        job_id: str,
        *,
        status: str,
        session_id: str | None = None,
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
        """Create or update a background status row."""

        existing = self._jobs.get(job_id)
        previous_status = existing.status if existing is not None else None
        previous_session_id = existing.session_id if existing is not None else None
        resolved_session_id = session_id if session_id is not None else (existing.session_id if existing is not None else None)

        self._assert_transition(previous_status, status)
        self._reserve_session(job_id, resolved_session_id, status)

        if existing is None:
            existing = BackgroundRunStatus(
                job_id=job_id,
                session_id=resolved_session_id,
                status=status,
                multitask_strategy=multitask_strategy or "reject",
                result=result,
                error=error,
                attempt=attempt if attempt is not None else 1,
                retry_count=retry_count if retry_count is not None else 0,
                max_attempts=max_attempts if max_attempts is not None else 1,
                timeout_seconds=timeout_seconds,
                claimed_by=claimed_by,
                claimed_at=claimed_at,
                backoff_base_seconds=backoff_base_seconds if backoff_base_seconds is not None else 0.0,
                backoff_multiplier=backoff_multiplier if backoff_multiplier is not None else 2.0,
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
        else:
            existing = existing.touch(
                status=status,
                session_id=resolved_session_id,
                multitask_strategy=(
                    multitask_strategy
                    if multitask_strategy is not None
                    else existing.multitask_strategy
                ),
                result=result,
                error=error,
                attempt=attempt if attempt is not None else existing.attempt,
                retry_count=retry_count if retry_count is not None else existing.retry_count,
                max_attempts=max_attempts if max_attempts is not None else existing.max_attempts,
                timeout_seconds=timeout_seconds if timeout_seconds is not None else existing.timeout_seconds,
                claimed_by=claimed_by if claimed_by is not None else existing.claimed_by,
                claimed_at=claimed_at if claimed_at is not None else existing.claimed_at,
                backoff_base_seconds=(
                    backoff_base_seconds if backoff_base_seconds is not None else existing.backoff_base_seconds
                ),
                backoff_multiplier=(
                    backoff_multiplier if backoff_multiplier is not None else existing.backoff_multiplier
                ),
                max_backoff_seconds=(
                    max_backoff_seconds if max_backoff_seconds is not None else existing.max_backoff_seconds
                ),
                backoff_seconds=backoff_seconds,
                next_retry_at=next_retry_at,
                retry_scheduled_at=retry_scheduled_at,
                retry_claimed_at=retry_claimed_at,
                interrupt_requested_at=(
                    interrupt_requested_at
                    if interrupt_requested_at is not None
                    else existing.interrupt_requested_at
                ),
                interrupted_at=interrupted_at if interrupted_at is not None else existing.interrupted_at,
                last_attempt_started_at=(
                    last_attempt_started_at
                    if last_attempt_started_at is not None
                    else existing.last_attempt_started_at
                ),
                last_attempt_finished_at=(
                    last_attempt_finished_at
                    if last_attempt_finished_at is not None
                    else existing.last_attempt_finished_at
                ),
                last_failure_at=last_failure_at if last_failure_at is not None else existing.last_failure_at,
            )
        self._jobs[job_id] = existing
        self._release_previous_session(job_id, previous_session_id, resolved_session_id)
        self._finalize_session(job_id, resolved_session_id, status)
        self._persist_state()
        return existing

    def get(self, job_id: str) -> BackgroundRunStatus | None:
        """Return one background job row."""

        return self._jobs.get(job_id)

    def snapshot(self) -> dict[str, BackgroundRunStatus]:
        """Return a snapshot copy of all jobs keyed by job id."""

        return dict(self._jobs)

    def get_active_job(self, session_id: str) -> str | None:
        """Return the active job id for a session, if one is reserved."""

        return self._active_sessions.get(session_id)

    def reserve_session_takeover(self, *, session_id: str, incoming_job_id: str) -> str | None:
        """Reserve the next ownership handoff for one session."""

        pending_owner = self._pending_session_takeovers.get(session_id)
        if pending_owner is not None and pending_owner != incoming_job_id:
            raise SessionConflictError(
                f"Session {session_id!r} already has a pending takeover for job {pending_owner!r}."
            )

        active_owner = self._active_sessions.get(session_id)
        if active_owner is None or active_owner == incoming_job_id:
            return active_owner

        self._pending_session_takeovers[session_id] = incoming_job_id
        self._persist_state()
        return active_owner

    def claim_session_takeover(self, *, session_id: str, incoming_job_id: str) -> None:
        """Claim a previously reserved handoff once the old owner has released."""

        pending_owner = self._pending_session_takeovers.get(session_id)
        if pending_owner != incoming_job_id:
            raise SessionConflictError(
                f"Session {session_id!r} is not reserved for incoming job {incoming_job_id!r}."
            )

        active_owner = self._active_sessions.get(session_id)
        if active_owner is not None and active_owner != incoming_job_id:
            raise SessionConflictError(
                f"Session {session_id!r} is still active in job {active_owner!r}."
            )

        self._active_sessions[session_id] = incoming_job_id
        self._pending_session_takeovers.pop(session_id, None)
        self._persist_state()

    def release_session_takeover(self, *, session_id: str, incoming_job_id: str) -> None:
        """Release a pending or pre-claimed takeover when enqueue fails."""

        changed = False
        if self._pending_session_takeovers.get(session_id) == incoming_job_id:
            self._pending_session_takeovers.pop(session_id, None)
            changed = True
        if self._active_sessions.get(session_id) == incoming_job_id and incoming_job_id not in self._jobs:
            self._active_sessions.pop(session_id, None)
            changed = True
        if changed:
            self._persist_state()

    def pending_session_takeovers(self) -> int:
        """Return the number of in-flight replacement reservations."""

        return len(self._pending_session_takeovers)

    def active_job_count(self) -> int:
        """Return the number of currently admitted background jobs."""

        return sum(1 for job in self._jobs.values() if job.status in ACTIVE_JOB_STATUSES)

    def _assert_transition(self, previous_status: str | None, next_status: str) -> None:
        """Validate background job lifecycle transitions."""

        allowed = VALID_TRANSITIONS.get(previous_status, {next_status})
        if next_status not in allowed:
            raise ValueError(f"Invalid background job transition: {previous_status!r} -> {next_status!r}")

    def _reserve_session(self, job_id: str, session_id: str | None, status: str) -> None:
        """Reserve an active session for queued or running work."""

        if session_id is None or status not in ACTIVE_JOB_STATUSES:
            return
        owner = self._active_sessions.get(session_id)
        if owner is not None and owner != job_id:
            raise SessionConflictError(f"Session {session_id!r} is already active in job {owner!r}.")
        self._active_sessions[session_id] = job_id

    def _release_previous_session(self, job_id: str, previous_session_id: str | None, next_session_id: str | None) -> None:
        """Release the old session reservation if the job moved to a new session id."""

        if previous_session_id is None or previous_session_id == next_session_id:
            return
        if self._active_sessions.get(previous_session_id) == job_id:
            self._active_sessions.pop(previous_session_id, None)

    def _finalize_session(self, job_id: str, session_id: str | None, status: str) -> None:
        """Release the reservation once the job reaches a terminal state."""

        if session_id is None or status not in TERMINAL_JOB_STATUSES:
            return
        if self._active_sessions.get(session_id) == job_id:
            self._active_sessions.pop(session_id, None)

    def _load_state(self) -> None:
        """Load durable state from disk when a state path is configured."""

        if self._state_path is None or not self._storage_backend.exists(self._state_path):
            return

        payload = json.loads(self._storage_backend.read_text(self._state_path))
        persisted = _PersistedBackgroundState.model_validate(payload)
        self._jobs = {job.job_id: job for job in persisted.jobs}

        if persisted.active_sessions:
            self._active_sessions = {
                row.session_id: row.job_id for row in persisted.active_sessions
            }
        else:
            self._active_sessions = self._rebuild_active_sessions()

        self._active_sessions = {
            session_id: job_id
            for session_id, job_id in self._active_sessions.items()
            if job_id in self._jobs and self._jobs[job_id].status in ACTIVE_JOB_STATUSES
        }
        self._pending_session_takeovers = {
            row.session_id: row.incoming_job_id
            for row in persisted.pending_session_takeovers
            if row.session_id in self._active_sessions
            and row.incoming_job_id in self._jobs
            and self._jobs[row.incoming_job_id].status in ACTIVE_JOB_STATUSES
        }

    def _persist_state(self) -> None:
        """Persist state to a deterministic, versioned JSON contract."""

        if self._state_path is None:
            return
        self._state_path.parent.mkdir(parents=True, exist_ok=True)
        persisted = _PersistedBackgroundState(
            jobs=[self._jobs[job_id] for job_id in sorted(self._jobs)],
            active_sessions=[
                _PersistedActiveSession(session_id=session_id, job_id=job_id)
                for session_id, job_id in sorted(self._active_sessions.items())
            ],
            pending_session_takeovers=[
                _PersistedPendingTakeover(session_id=session_id, incoming_job_id=job_id)
                for session_id, job_id in sorted(self._pending_session_takeovers.items())
            ],
        )
        payload = json.dumps(persisted.model_dump(mode="json"), ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        self._storage_backend.write_text(self._state_path, payload)

    def _rebuild_active_sessions(self) -> dict[str, str]:
        """Rebuild active session reservations from active job rows."""

        candidates = sorted(
            (
                job.updated_at,
                job.job_id,
                job.session_id,
            )
            for job in self._jobs.values()
            if job.session_id is not None and job.status in ACTIVE_JOB_STATUSES
        )
        active_sessions: dict[str, str] = {}
        for _, job_id, session_id in candidates:
            if session_id is None:
                continue
            active_sessions[session_id] = job_id
        return active_sessions
