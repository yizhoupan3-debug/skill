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
from codex_agno_runtime.execution_kernel import ExecutionKernelRequest
from codex_agno_runtime.runtime import CodexAgnoRuntime
from codex_agno_runtime.schemas import (
    BackgroundRunRequest,
    RoutingResult,
    RunTaskResponse,
    SkillMetadata,
    UsageMetrics,
)


async def _wait_for_status(
    runtime: CodexAgnoRuntime,
    job_id: str,
    expected: set[str],
    *,
    timeout: float = 5.0,
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
    seen_operations: list[str] = []
    original = runtime.rust_adapter.background_control

    def wrapped(payload):
        seen_operations.append(str(payload["operation"]))
        resolved = original(payload)
        if payload["operation"] == "interrupt":
            resolved = dict(resolved)
            resolved["effect_plan"] = dict(resolved["effect_plan"])
            resolved["effect_plan"]["cancel_running_task"] = False
        return resolved

    runtime.rust_adapter.background_control = wrapped  # type: ignore[method-assign]

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
        interrupt_requested = next(
            event for event in runtime._trace.events if event.job_id == status.job_id and event.kind == "job.interrupt_requested"
        )
        interrupted = next(
            event for event in runtime._trace.events if event.job_id == status.job_id and event.kind == "job.interrupted"
        )
        assert interrupt_requested.payload["background_policy_authority"] == "rust-background-control"
        assert interrupted.payload["background_policy_authority"] == "rust-background-control"
        assert "interrupt" in seen_operations
        assert "interrupt-finalize" in seen_operations
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


def test_background_completion_race_is_finalized_by_rust_adapter(tmp_path: Path) -> None:
    """A late interrupt should force the successful completion path through the Rust race reducer."""

    runtime = _build_runtime(tmp_path)
    seen_operations: list[str] = []
    original = runtime.rust_adapter.background_control

    def wrapped(payload):
        seen_operations.append(str(payload["operation"]))
        resolved = original(payload)
        if payload["operation"] == "interrupt":
            resolved = dict(resolved)
            resolved["effect_plan"] = dict(resolved["effect_plan"])
            resolved["effect_plan"]["cancel_running_task"] = False
        return resolved

    runtime.rust_adapter.background_control = wrapped  # type: ignore[method-assign]
    started = asyncio.Event()

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        started.set()
        await asyncio.sleep(1.0)
        return RunTaskResponse(
            session_id="completion-race-session",
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
                task="完成与中断赛跑",
                user_id="tester",
                session_id="completion-race-session",
                dry_run=True,
            )
        )
        await started.wait()
        interrupt_status = await runtime.request_background_interrupt(status.job_id)
        assert interrupt_status is not None
        final = await _wait_for_status(runtime, status.job_id, {"interrupted"})

        assert final.status == "interrupted"
        assert "interrupt" in seen_operations
        assert "completion-race" in seen_operations
        assert "interrupt-finalize" in seen_operations

    asyncio.run(_run())


def test_background_retry_after_failure(tmp_path: Path) -> None:
    """The runtime should schedule and claim a retry before succeeding."""

    runtime = _build_runtime(tmp_path)
    attempts: list[int] = []
    seen_operations: list[str] = []
    original = runtime.rust_adapter.background_control

    def wrapped(payload):
        seen_operations.append(str(payload["operation"]))
        return original(payload)

    runtime.rust_adapter.background_control = wrapped  # type: ignore[method-assign]

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
        assert "retry" in seen_operations
        assert "retry-claim" in seen_operations
        assert "completion-race" in seen_operations
        assert any(event.kind == "job.retry_scheduled" for event in runtime._trace.events if event.job_id == status.job_id)
        assert any(event.kind == "job.retry_claimed" for event in runtime._trace.events if event.job_id == status.job_id)

    asyncio.run(_run())


def test_background_retry_backoff_routes_through_state_service_host(tmp_path: Path) -> None:
    """Retry backoff waits should be delegated through the state-service host lane."""

    runtime = _build_runtime(tmp_path)
    waits: list[float] = []
    original_wait = runtime.state_service.wait_for_retry_backoff
    attempts = 0

    async def wrapped_wait(*, backoff_seconds: float) -> None:
        waits.append(backoff_seconds)
        await original_wait(backoff_seconds=backoff_seconds)

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        nonlocal attempts
        attempts += 1
        if attempts == 1:
            raise RuntimeError("retry once through state service")
        return RunTaskResponse(
            session_id="retry-host-session",
            user_id="tester",
            skill="test-skill",
            live_run=False,
            content="ok",
            usage=UsageMetrics(),
        )

    runtime.state_service.wait_for_retry_backoff = wrapped_wait  # type: ignore[method-assign]
    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="retry host lane",
                user_id="tester",
                session_id="retry-host-session",
                dry_run=True,
                max_attempts=2,
                backoff_base_seconds=0.01,
                max_backoff_seconds=0.01,
            )
        )
        final = await _wait_for_status(runtime, status.job_id, {"completed"})

        assert final.status == "completed"
        assert final.attempt == 2
        assert waits == [0.01]

    asyncio.run(_run())


def test_background_runtime_sandbox_events_keep_background_job_id(tmp_path: Path) -> None:
    """Sandbox event logs should preserve the real background job id through cleanup."""

    runtime = _build_runtime(tmp_path)
    selected_skill = SkillMetadata(
        name="test-skill",
        routing_layer="L2",
        routing_owner="owner",
        routing_gate="none",
    )

    async def fake_kernel_execute(request) -> RunTaskResponse:
        return RunTaskResponse(
            session_id=request.session_id,
            user_id=request.user_id,
            skill=request.routing_result.selected_skill.name,
            overlay=request.routing_result.overlay_skill.name if request.routing_result.overlay_skill else None,
            live_run=False,
            content="background-ok",
            prompt_preview="Rust-owned dry-run prompt",
            usage=UsageMetrics(input_tokens=3, output_tokens=2, total_tokens=5, mode="dry_run"),
            metadata={
                **runtime.execution_service.kernel_payload(dry_run=True),
            },
        )

    async def fake_run_task(request: BackgroundRunRequest) -> RunTaskResponse:
        session_id = request.session_id or "background-sandbox-session"
        user_id = request.user_id or "tester"
        routing_result = RoutingResult(
            task=request.task,
            session_id=session_id,
            selected_skill=selected_skill,
            layer=selected_skill.routing_layer,
        )
        return await runtime.execution_service.execute_request(
            ExecutionKernelRequest(
                task=request.task,
                session_id=session_id,
                job_id=request.job_id,
                user_id=user_id,
                routing_result=routing_result,
                dry_run=True,
            ),
            executor=fake_kernel_execute,
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="验证 background sandbox 日志 job_id",
                user_id="tester",
                session_id="background-sandbox-session",
                dry_run=True,
            )
        )
        await asyncio.wait_for(runtime._background_tasks[status.job_id], timeout=5.0)
        final = runtime.get_background_status(status.job_id)

        assert final is not None
        assert final.status == "completed"
        events_path = tmp_path / "runtime-data" / "runtime_sandbox_events.jsonl"
        events = [
            json.loads(line)
            for line in events_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        scoped = [event for event in events if event["session_id"] == status.session_id]
        assert scoped
        assert {event["job_id"] for event in scoped} == {status.job_id}
        cleanup_completed = next(event for event in scoped if event["kind"] == "sandbox.cleanup_completed")
        assert cleanup_completed["session_id"] == status.session_id
        assert cleanup_completed["job_id"] == status.job_id

    asyncio.run(_run())


def test_background_takeover_waits_through_rust_session_release_plan(tmp_path: Path) -> None:
    """A takeover should use the Rust session-release plan before claiming the slot."""

    runtime = _build_runtime(tmp_path)
    seen_operations: list[str] = []
    original = runtime.rust_adapter.background_control

    def wrapped(payload):
        seen_operations.append(str(payload["operation"]))
        resolved = original(payload)
        if payload["operation"] == "interrupt":
            resolved = dict(resolved)
            resolved["effect_plan"] = dict(resolved["effect_plan"])
            resolved["effect_plan"]["cancel_running_task"] = True
        return resolved

    runtime.rust_adapter.background_control = wrapped  # type: ignore[method-assign]
    first_started = asyncio.Event()

    async def fake_run_task(request: BackgroundRunRequest) -> RunTaskResponse:
        if request.task == "primary takeover job":
            first_started.set()
            try:
                await asyncio.sleep(10)
            except asyncio.CancelledError:
                raise
        return RunTaskResponse(
            session_id=request.session_id or "takeover-session",
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
                task="primary takeover job",
                user_id="tester",
                session_id="takeover-session",
                dry_run=True,
            )
        )
        await first_started.wait()
        second = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="takeover follow-up",
                user_id="tester",
                session_id="takeover-session",
                dry_run=True,
                multitask_strategy="interrupt",
            )
        )

        first_final = await _wait_for_status(runtime, first.job_id, {"interrupted"})
        second_final = await _wait_for_status(runtime, second.job_id, {"completed"})

        assert first_final.status == "interrupted"
        assert second_final.status == "completed"
        assert "interrupt" in seen_operations
        assert "session-release" in seen_operations

    asyncio.run(_run())


def test_background_session_release_wait_routes_through_state_service_host(tmp_path: Path) -> None:
    """Session release waits should be delegated through the state-service host lane."""

    runtime = _build_runtime(tmp_path)
    waits: list[tuple[str, float, float]] = []
    original_wait = runtime.state_service.wait_for_session_release
    first_started = asyncio.Event()

    async def wrapped_wait(*, session_id: str, timeout_seconds: float, poll_interval_seconds: float) -> None:
        waits.append((session_id, timeout_seconds, poll_interval_seconds))
        await original_wait(
            session_id=session_id,
            timeout_seconds=timeout_seconds,
            poll_interval_seconds=poll_interval_seconds,
        )

    async def fake_run_task(request: BackgroundRunRequest) -> RunTaskResponse:
        if request.task == "primary takeover job":
            first_started.set()
            try:
                await asyncio.sleep(10)
            except asyncio.CancelledError:
                raise
        return RunTaskResponse(
            session_id=request.session_id or "takeover-host-session",
            user_id="tester",
            skill="test-skill",
            live_run=False,
            content="ok",
            usage=UsageMetrics(),
        )

    runtime.state_service.wait_for_session_release = wrapped_wait  # type: ignore[method-assign]
    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        first = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="primary takeover job",
                user_id="tester",
                session_id="takeover-host-session",
                dry_run=True,
            )
        )
        await first_started.wait()
        second = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="takeover follow-up",
                user_id="tester",
                session_id="takeover-host-session",
                dry_run=True,
                multitask_strategy="interrupt",
            )
        )

        first_final = await _wait_for_status(runtime, first.job_id, {"interrupted"})
        second_final = await _wait_for_status(runtime, second.job_id, {"completed"})

        assert first_final.status == "interrupted"
        assert second_final.status == "completed"
        assert waits == [("takeover-host-session", 5.0, 0.01)]

    asyncio.run(_run())


def test_background_claim_records_rust_kernel_authority(tmp_path: Path) -> None:
    """Background control should surface the Rust-owned kernel contract during admission/execution."""

    runtime = _build_runtime(tmp_path)
    assert runtime.control_plane_descriptor["services"]["background"]["delegate_kind"] == (
        "rust-background-control-policy"
    )

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


def test_background_claim_step_is_resolved_through_rust_control(tmp_path: Path) -> None:
    """The runner claim step should go through the Rust background-control reducer."""

    runtime = _build_runtime(tmp_path)
    seen_operations: list[str] = []
    original = runtime.rust_adapter.background_control

    def wrapped(payload):
        seen_operations.append(str(payload["operation"]))
        return original(payload)

    runtime.rust_adapter.background_control = wrapped  # type: ignore[method-assign]

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        return RunTaskResponse(
            session_id="background-claim-session",
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
                task="background claim rust control",
                user_id="tester",
                session_id="background-claim-session",
                dry_run=True,
            )
        )
        final = await _wait_for_status(runtime, status.job_id, {"completed"})
        assert final.status == "completed"
        assert "claim" in seen_operations

        claimed = next(
            event for event in runtime._trace.events if event.job_id == status.job_id and event.kind == "job.claimed"
        )
        assert claimed.payload["status"] == "running"
        assert claimed.payload["background_policy_authority"] == "rust-background-control"

    asyncio.run(_run())


def test_background_interrupt_finalize_routes_mutations_through_state_service_host(tmp_path: Path) -> None:
    """Interrupt request/finalize mutations should go through the state-service host lane."""

    runtime = _build_runtime(tmp_path)
    seen_statuses: list[str] = []
    original_apply = runtime.state_service.apply_mutation

    def wrapped_apply(job_id: str, mutation) -> object:
        seen_statuses.append(mutation.status)
        return original_apply(job_id, mutation)

    runtime.state_service.apply_mutation = wrapped_apply  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="interrupt through state service",
                user_id="tester",
                session_id="interrupt-host-session",
                dry_run=True,
            )
        )
        interrupted = await runtime.request_background_interrupt(status.job_id)
        final = await _wait_for_status(runtime, status.job_id, {"interrupted"})

        assert interrupted is not None
        assert final.status == "interrupted"
        assert seen_statuses[:3] == ["queued", "interrupt_requested", "interrupted"]

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
        retry_event = next(
            event for event in runtime._trace.events if event.job_id == status.job_id and event.kind == "job.retry_scheduled"
        )
        assert retry_event.payload["background_policy_authority"] == "rust-background-control"

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


def test_background_control_policy_routes_through_rust_adapter(tmp_path: Path) -> None:
    """Admission and retry policy should be resolved through the Rust adapter seam."""

    runtime = _build_runtime(tmp_path)
    seen_operations: list[str] = []
    original = runtime.rust_adapter.background_control

    def wrapped(payload):
        seen_operations.append(str(payload["operation"]))
        return original(payload)

    runtime.rust_adapter.background_control = wrapped  # type: ignore[method-assign]

    async def fake_run_task(_request: BackgroundRunRequest) -> RunTaskResponse:
        raise RuntimeError("force retry scheduling")

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        status = await runtime.enqueue_background_run(
            BackgroundRunRequest(
                task="rust background policy seam",
                user_id="tester",
                session_id="rust-background-policy-session",
                dry_run=True,
                max_attempts=2,
                backoff_base_seconds=0.01,
            )
        )
        final = await _wait_for_status(runtime, status.job_id, {"retry_scheduled"})
        assert final.status == "retry_scheduled"
        assert seen_operations.count("enqueue") >= 2
        assert "retry" in seen_operations
        await runtime.request_background_interrupt(status.job_id)
        interrupted = await _wait_for_status(runtime, status.job_id, {"interrupted"})
        assert interrupted.status == "interrupted"
        assert "interrupt" in seen_operations

    asyncio.run(_run())


def test_background_batch_plan_routes_through_rust_adapter(tmp_path: Path) -> None:
    """Parallel batch group/lane planning should be resolved through the Rust adapter seam."""

    runtime = _build_runtime(tmp_path)
    seen_operations: list[str] = []
    original = runtime.rust_adapter.background_control

    def wrapped(payload):
        seen_operations.append(str(payload["operation"]))
        return original(payload)

    runtime.rust_adapter.background_control = wrapped  # type: ignore[method-assign]

    async def fake_run_task(request: BackgroundRunRequest) -> RunTaskResponse:
        return RunTaskResponse(
            session_id=request.session_id or "batch-plan-session",
            user_id=request.user_id or "tester",
            skill="test-skill",
            live_run=False,
            content=request.task,
            usage=UsageMetrics(),
        )

    runtime.run_task = fake_run_task  # type: ignore[method-assign]

    async def _run() -> None:
        batch = await runtime.enqueue_background_batch(
            [
                BackgroundRunRequest(
                    task="lane-a",
                    user_id="tester",
                    session_id="batch-plan-a",
                    parallel_group_id="pgroup-contract",
                    lane_id="lane-a",
                    dry_run=True,
                ),
                BackgroundRunRequest(
                    task="lane-b",
                    user_id="tester",
                    session_id="batch-plan-b",
                    parallel_group_id="pgroup-contract",
                    dry_run=True,
                ),
            ],
            parallel_group_id="pgroup-contract",
        )

        assert batch.parallel_group_id == "pgroup-contract"
        assert [status.lane_id for status in batch.statuses] == ["lane-a", "lane-2"]
        assert "batch-plan" in seen_operations

        for status in batch.statuses:
            final = await _wait_for_status(runtime, status.job_id, {"completed"})
            assert final.status == "completed"

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
