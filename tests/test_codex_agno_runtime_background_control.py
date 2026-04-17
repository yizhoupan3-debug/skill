"""Targeted retry, backoff, and interrupt regression tests for background jobs."""

from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.runtime import CodexAgnoRuntime
from codex_agno_runtime.schemas import BackgroundRunRequest, RunTaskResponse, UsageMetrics


async def _wait_for_status(
    runtime: CodexAgnoRuntime,
    job_id: str,
    expected: set[str],
    *,
    timeout: float = 2.0,
) -> object:
    deadline = asyncio.get_running_loop().time() + timeout
    while asyncio.get_running_loop().time() < deadline:
        status = runtime.get_background_status(job_id)
        if status is not None and status.status in expected:
            return status
        await asyncio.sleep(0.01)
    raise AssertionError(f"Timed out waiting for {job_id} to reach one of {sorted(expected)}")


def _build_runtime(tmp_path: Path) -> CodexAgnoRuntime:
    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        live_model_override=False,
    )
    return CodexAgnoRuntime(settings)


def test_background_interrupt_while_queued(tmp_path: Path) -> None:
    """A queued job should expose interrupt_requested before becoming interrupted."""

    runtime = _build_runtime(tmp_path)

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="后台排队任务",
                user_id="tester",
                session_id="queued-interrupt-session",
                dry_run=True,
            )
        )
        interrupt_status = await runtime.request_background_interrupt(status.job_id)
        final = await _wait_for_status(runtime, status.job_id, {"interrupted"})

        assert interrupt_status is not None
        assert interrupt_status.status == "interrupted"
        assert final.status == "interrupted"
        assert final.interrupt_requested_at is not None
        assert final.interrupted_at is not None
        assert [event.kind for event in runtime._trace.events if event.job_id == status.job_id][-2:] == [
            "job.interrupt_requested",
            "job.interrupted",
        ]

    asyncio.run(_run())


def test_background_interrupt_while_running(tmp_path: Path) -> None:
    """A running job should be cancelable through the explicit interrupt lifecycle."""

    runtime = _build_runtime(tmp_path)
    started = asyncio.Event()
    cancelled = asyncio.Event()

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        started.set()
        try:
            await asyncio.sleep(10)
        except asyncio.CancelledError:
            cancelled.set()
            raise
        return RunTaskResponse(
            session_id="running-interrupt-session",
            user_id="tester",
            skill="test-skill",
            live_run=False,
            content="ok",
            usage=UsageMetrics(),
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="长运行任务",
                user_id="tester",
                session_id="running-interrupt-session",
                dry_run=True,
            )
        )
        await started.wait()
        running = await _wait_for_status(runtime, status.job_id, {"running"})
        assert running.status == "running"

        interrupt_status = await runtime.request_background_interrupt(status.job_id)
        final = await _wait_for_status(runtime, status.job_id, {"interrupted"})

        assert interrupt_status is not None
        assert interrupt_status.status == "interrupt_requested"
        assert cancelled.is_set()
        assert final.status == "interrupted"
        assert final.last_attempt_started_at is not None
        assert final.last_attempt_finished_at is not None

    asyncio.run(_run())


def test_background_retry_after_failure(tmp_path: Path) -> None:
    """The runtime should schedule and claim a retry before succeeding."""

    runtime = _build_runtime(tmp_path)
    attempts: list[int] = []

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        attempts.append(1)
        if len(attempts) == 1:
            raise RuntimeError("first attempt failed")
        return RunTaskResponse(
            session_id="retry-success-session",
            user_id="tester",
            skill="test-skill",
            live_run=False,
            content="ok",
            usage=UsageMetrics(),
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="失败后重试一次",
                user_id="tester",
                session_id="retry-success-session",
                dry_run=True,
                max_attempts=2,
                backoff_base_seconds=0.01,
            )
        )
        final = await _wait_for_status(runtime, status.job_id, {"completed"})

        assert len(attempts) == 2
        assert final.status == "completed"
        assert final.attempt == 2
        assert final.retry_count == 1
        assert final.retry_scheduled_at is not None
        assert final.retry_claimed_at is not None
        assert any(event.kind == "job.retry_scheduled" for event in runtime._trace.events if event.job_id == status.job_id)
        assert any(event.kind == "job.retry_claimed" for event in runtime._trace.events if event.job_id == status.job_id)

    asyncio.run(_run())


def test_background_claim_records_rust_kernel_authority(tmp_path: Path) -> None:
    """Background control should surface the Rust-owned kernel contract during admission/execution."""

    runtime = _build_runtime(tmp_path)

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        return RunTaskResponse(
            session_id="background-kernel-session",
            user_id="tester",
            skill="test-skill",
            live_run=False,
            content="ok",
            usage=UsageMetrics(),
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="背景执行 authority",
                user_id="tester",
                session_id="background-kernel-session",
                dry_run=True,
            )
        )
        final = await _wait_for_status(runtime, status.job_id, {"completed"})
        assert final.status == "completed"

        claimed = next(event for event in runtime._trace.events if event.job_id == status.job_id and event.kind == "job.claimed")
        completed = next(
            event for event in runtime._trace.events if event.job_id == status.job_id and event.kind == "job.completed"
        )
        for event in (claimed, completed):
            assert event.payload["execution_kernel"] == "rust-execution-kernel-slice"
            assert event.payload["execution_kernel_authority"] == "rust-execution-kernel-authority"
            assert event.payload["execution_kernel_delegate"] == "router-rs"
            assert event.payload["execution_kernel_delegate_authority"] == "rust-execution-cli"

    asyncio.run(_run())


def test_background_backoff_state_is_persisted(tmp_path: Path) -> None:
    """Retry scheduling should persist deterministic backoff fields before the next claim."""

    runtime = _build_runtime(tmp_path)

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        raise RuntimeError("force retry scheduling")

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="持久化 backoff 字段",
                user_id="tester",
                session_id="retry-backoff-session",
                dry_run=True,
                max_attempts=2,
                backoff_base_seconds=0.2,
                max_backoff_seconds=0.2,
            )
        )
        scheduled = await _wait_for_status(runtime, status.job_id, {"retry_scheduled"})

        assert scheduled.backoff_seconds == 0.2
        assert scheduled.next_retry_at is not None
        assert scheduled.retry_scheduled_at is not None

        payload = json.loads(
            (runtime.settings.resolved_data_dir / "runtime_background_jobs.json").read_text(encoding="utf-8")
        )
        persisted = next(row for row in payload["jobs"] if row["job_id"] == status.job_id)
        assert persisted["status"] == "retry_scheduled"
        assert persisted["backoff_seconds"] == 0.2
        assert persisted["next_retry_at"] is not None

        await runtime.request_background_interrupt(status.job_id)
        final = await _wait_for_status(runtime, status.job_id, {"interrupted"})
        assert final.status == "interrupted"

    asyncio.run(_run())


def test_background_retry_exhaustion_marks_terminal_state(tmp_path: Path) -> None:
    """Exhausted retries should end in an explicit retry_exhausted state."""

    runtime = _build_runtime(tmp_path)

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        raise RuntimeError("always fail")

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="连续失败直到耗尽",
                user_id="tester",
                session_id="retry-exhausted-session",
                dry_run=True,
                max_attempts=2,
                backoff_base_seconds=0.01,
            )
        )
        final = await _wait_for_status(runtime, status.job_id, {"retry_exhausted"})

        assert final.status == "retry_exhausted"
        assert final.attempt == 2
        assert final.retry_count == 1
        assert final.last_failure_at is not None
        assert any(event.kind == "job.retry_exhausted" for event in runtime._trace.events if event.job_id == status.job_id)

    asyncio.run(_run())


def test_background_capacity_admission_rejects_excess_jobs(tmp_path: Path) -> None:
    """Admission should reject excess queued jobs without peeking into semaphore internals."""

    runtime = _build_runtime(tmp_path)
    runtime._max_background_jobs = 1
    runtime._job_semaphore = asyncio.Semaphore(1)
    started = asyncio.Event()

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        started.set()
        await asyncio.sleep(0.2)
        return RunTaskResponse(
            session_id="capacity-session",
            user_id="tester",
            skill="test-skill",
            live_run=False,
            content="ok",
            usage=UsageMetrics(),
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        first = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="first-capacity-job",
                user_id="tester",
                session_id="capacity-session-1",
                dry_run=True,
            )
        )
        await started.wait()
        second = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="second-capacity-job",
                user_id="tester",
                session_id="capacity-session-2",
                dry_run=True,
            )
        )

        assert first.status == "queued"
        assert second.status == "failed"
        assert "Too many admitted background jobs" in (second.error or "")

        final = await _wait_for_status(runtime, first.job_id, {"completed"})
        assert final.status == "completed"

    asyncio.run(_run())
