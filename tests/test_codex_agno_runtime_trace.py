"""Focused trace and middleware observability regression tests."""

from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.middleware import (
    MemoryMiddleware,
    Middleware,
    MiddlewareChain,
    MiddlewareContext,
    SkillInjectionMiddleware,
)
from codex_agno_runtime.schemas import (
    RoutingResult,
    RunTaskResponse,
    SkillMetadata,
    UsageMetrics,
)
from codex_agno_runtime.trace import (
    TRACE_EVENT_SINK_SCHEMA_VERSION,
    TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
    TRACE_EVENT_HANDOFF_SCHEMA_VERSION,
    InMemoryRuntimeEventBridge,
    RuntimeEventHandoff,
    RuntimeEventTransport,
    RuntimeTraceRecorder,
    TRACE_EVENT_SCHEMA_VERSION,
    TRACE_METADATA_SCHEMA_VERSION,
    TRACE_REPLAY_CHUNK_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
)


class _RecordingMemoryStore:
    """Minimal memory store used to assert dry-run safety."""

    def __init__(self) -> None:
        self.saved: list[tuple[str, list[str]]] = []
        self.extracted: list[str] = []

    def load_facts(self, user_id: str) -> list[str]:
        return [f"known fact for {user_id}"]

    def extract_facts_sync(self, conversation: str) -> list[str]:
        self.extracted.append(conversation)
        return ["new fact"]

    def save_facts(self, user_id: str, facts: list[str]) -> None:
        self.saved.append((user_id, facts))


class _RecordingPromptBuilder:
    """Prompt builder spy used to assert live middleware skips Python shaping."""

    def __init__(self) -> None:
        self.calls = 0

    def build_prompt(self, routing_result: RoutingResult) -> str:
        self.calls += 1
        return f"prompt for {routing_result.selected_skill.name}"


class _NoOpMiddleware(Middleware):
    """Simple middleware used to validate enter/exit trace emission."""

    async def before_agent(self, ctx: MiddlewareContext) -> MiddlewareContext:
        ctx.metadata["no_op_before"] = True
        return ctx

    async def after_agent(self, ctx: MiddlewareContext, result: RunTaskResponse) -> RunTaskResponse:
        ctx.metadata["no_op_after"] = True
        return result


def _build_routing_result(session_id: str) -> RoutingResult:
    skill = SkillMetadata(
        name="trace-observability",
        routing_owner="runtime",
        routing_gate="none",
        routing_layer="L2",
    )
    return RoutingResult(
        task="dry-run trace coverage",
        session_id=session_id,
        selected_skill=skill,
        layer=skill.routing_layer,
    )


def test_trace_recorder_writes_versioned_metadata(tmp_path: Path) -> None:
    """Verify the trace artifact advertises both schema versions."""

    output = tmp_path / "TRACE_METADATA.json"
    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(output_path=output, event_stream_path=stream)
    recorder.record(
        session_id="session-1",
        kind="route.selected",
        stage="routing",
        payload={"skill": "trace-observability"},
    )
    recorder.record(
        session_id="session-1",
        kind="route.selected",
        stage="routing",
        payload={"skill": "trace-observability"},
    )
    recorder.record(
        session_id="session-1",
        kind="run.failed",
        stage="execution",
        payload={"error": "mock failure"},
    )
    recorder.record(
        session_id="session-1",
        kind="middleware.enter",
        stage="middleware",
        payload={"middleware": "MemoryMiddleware"},
    )
    recorder.flush_metadata(
        task="trace observability",
        matched_skills=["trace-observability"],
        owner="trace-observability",
        gate="none",
        overlay=None,
        artifact_paths=["artifacts/current/TRACE_METADATA.json"],
        verification_status="dry_run",
        supervisor_projection={
            "supervisor_state_path": "/tmp/.supervisor_state.json",
            "active_phase": "validated",
            "verification_status": "completed",
            "delegation": {
                "plan_created": True,
                "spawn_attempted": False,
                "fallback_mode": "local-supervisor",
                "sidecar_count": 1,
            },
        },
        reroute_count=recorder.count_reroutes("session-1"),
        retry_count=recorder.count_retries("session-1"),
    )

    data = json.loads(output.read_text(encoding="utf-8"))
    assert data["version"] == 1
    assert data["metadata_schema_version"] == TRACE_METADATA_SCHEMA_VERSION
    assert data["trace_event_schema_version"] == TRACE_EVENT_SCHEMA_VERSION
    assert data["trace_event_sink_schema_version"] == TRACE_EVENT_SINK_SCHEMA_VERSION
    assert data["reroute_count"] == 1
    assert data["retry_count"] == 1
    assert data["control_plane"]["authority"] == "rust-runtime-control-plane"
    assert data["control_plane"]["projection"] == "python-thin-projection"
    assert data["supervisor_projection"] == {
        "supervisor_state_path": "/tmp/.supervisor_state.json",
        "active_phase": "validated",
        "verification_status": "completed",
        "delegation": {
            "plan_created": True,
            "spawn_attempted": False,
            "fallback_mode": "local-supervisor",
            "sidecar_count": 1,
        },
    }
    assert data["events"][0]["schema_version"] == TRACE_EVENT_SCHEMA_VERSION
    lines = [json.loads(line) for line in stream.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert len(lines) == 4
    assert lines[0]["sink_schema_version"] == TRACE_EVENT_SINK_SCHEMA_VERSION
    assert lines[0]["event"]["schema_version"] == TRACE_EVENT_SCHEMA_VERSION
    assert lines[0]["event"]["seq"] == 1
    assert lines[0]["event"]["cursor"].startswith("g0:s1:")
    assert data["stream"]["replay_supported"] is True
    assert data["stream"]["control_plane_authority"] == "rust-runtime-control-plane"
    assert data["stream"]["control_plane_projection"] == "python-thin-projection"
    assert data["stream"]["latest_seq"] == 4
    assert data["stream"]["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION


def test_middleware_chain_emits_trace_events_and_skips_memory_write_on_dry_run() -> None:
    """Dry-run middleware execution should be observable but side-effect free."""

    trace = RuntimeTraceRecorder()
    memory_store = _RecordingMemoryStore()
    chain = MiddlewareChain(
        [MemoryMiddleware(memory_store), _NoOpMiddleware()],
        trace_recorder=trace,
    )
    ctx = MiddlewareContext(
        task="dry-run trace coverage",
        session_id="session-2",
        user_id="user-2",
        routing_result=_build_routing_result("session-2"),
        execution_kernel="rust-execution-kernel-slice",
        execution_kernel_authority="rust-execution-kernel-authority",
        execution_kernel_delegate="python-agno",
        execution_kernel_delegate_authority="python-agno-kernel-adapter",
    )
    ctx.metadata["dry_run"] = True

    async def _agent(mw_ctx: MiddlewareContext) -> RunTaskResponse:
        return RunTaskResponse(
            session_id=mw_ctx.session_id,
            user_id=mw_ctx.user_id,
            skill=mw_ctx.routing_result.selected_skill.name,
            live_run=False,
            content="ok",
            usage=UsageMetrics(),
            prompt_preview=mw_ctx.prompt,
        )

    result = asyncio.run(chain.execute(ctx, _agent))

    assert result.live_run is False
    assert memory_store.saved == []
    assert memory_store.extracted == []
    assert [event.kind for event in trace.events] == [
        "middleware.enter",
        "middleware.enter",
        "middleware.exit",
        "middleware.exit",
    ]
    assert all(event.schema_version == TRACE_EVENT_SCHEMA_VERSION for event in trace.events)
    assert trace.events[0].payload["middleware"] == "MemoryMiddleware"
    assert trace.events[0].payload["execution_kernel"] == "rust-execution-kernel-slice"
    assert trace.events[0].payload["execution_kernel_authority"] == "rust-execution-kernel-authority"
    assert trace.events[0].payload["execution_kernel_delegate"] == "python-agno"
    assert trace.events[0].payload["execution_kernel_delegate_authority"] == "python-agno-kernel-adapter"
    assert trace.events[-1].payload["middleware"] == "MemoryMiddleware"


def test_middleware_chain_skips_python_prompt_mutation_on_live_runs() -> None:
    """Live middleware should not eagerly build or rewrite Python prompt text."""

    trace = RuntimeTraceRecorder()
    memory_store = _RecordingMemoryStore()
    prompt_builder = _RecordingPromptBuilder()
    chain = MiddlewareChain(
        [SkillInjectionMiddleware(prompt_builder), MemoryMiddleware(memory_store), _NoOpMiddleware()],
        trace_recorder=trace,
    )
    ctx = MiddlewareContext(
        task="live trace coverage",
        session_id="session-live",
        user_id="user-live",
        routing_result=_build_routing_result("session-live"),
        execution_kernel="rust-execution-kernel-slice",
        execution_kernel_authority="rust-execution-kernel-authority",
        execution_kernel_delegate="router-rs",
        execution_kernel_delegate_authority="rust-execution-cli",
    )
    ctx.metadata["dry_run"] = False

    async def _agent(mw_ctx: MiddlewareContext) -> RunTaskResponse:
        return RunTaskResponse(
            session_id=mw_ctx.session_id,
            user_id=mw_ctx.user_id,
            skill=mw_ctx.routing_result.selected_skill.name,
            live_run=True,
            content="live result",
            usage=UsageMetrics(input_tokens=5, output_tokens=3, total_tokens=8, mode="live"),
            prompt_preview=None,
            model_id="gpt-5.4",
        )

    result = asyncio.run(chain.execute(ctx, _agent))

    assert result.live_run is True
    assert result.prompt_preview is None
    assert prompt_builder.calls == 0
    assert ctx.prompt == ""
    assert memory_store.extracted == ["User: live trace coverage\nAssistant: live result"]
    assert memory_store.saved == [("user-live", ["new fact"])]
    assert [event.kind for event in trace.events] == [
        "middleware.enter",
        "middleware.enter",
        "middleware.enter",
        "middleware.exit",
        "middleware.exit",
        "middleware.exit",
    ]
    assert trace.events[0].payload["middleware"] == "SkillInjectionMiddleware"
    assert trace.events[1].payload["middleware"] == "MemoryMiddleware"
    assert trace.events[-1].payload["middleware"] == "SkillInjectionMiddleware"


def test_trace_recorder_supports_resumable_replay_windows(tmp_path: Path) -> None:
    """Replay windows should expose a stable next cursor for later resume."""

    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(event_stream_path=stream)
    for index in range(3):
        recorder.record(
            session_id="session-3",
            job_id="job-3",
            kind=f"job.event.{index}",
            stage="background",
            payload={"index": index},
        )

    first_window = recorder.replay(session_id="session-3", job_id="job-3", limit=2)
    assert first_window.schema_version == TRACE_REPLAY_CHUNK_SCHEMA_VERSION
    assert len(first_window.events) == 2
    assert first_window.has_more is True
    assert first_window.next_cursor is not None
    assert first_window.next_cursor.schema_version == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
    assert [event.seq for event in first_window.events] == [1, 2]

    second_window = recorder.replay(
        session_id="session-3",
        job_id="job-3",
        after=first_window.next_cursor,
        limit=2,
    )
    assert second_window.has_more is False
    assert [event.seq for event in second_window.events] == [3]
    assert second_window.next_cursor is not None
    assert second_window.next_cursor.seq == 3


def test_runtime_event_handoff_serializes_transport_and_replay_refs() -> None:
    """Handoff descriptor should carry transport details plus replay anchors."""

    transport = RuntimeEventTransport(
        stream_id="stream::session-4",
        session_id="session-4",
        binding_backend_family="filesystem",
        binding_artifact_path="/tmp/runtime_event_transports/session-4__session-4.json",
    )
    handoff = RuntimeEventHandoff(
        stream_id=transport.stream_id,
        session_id="session-4",
        checkpoint_backend_family="filesystem",
        trace_stream_path="/tmp/TRACE_EVENTS.jsonl",
        resume_manifest_path="/tmp/TRACE_RESUME_MANIFEST.json",
        transport=transport,
    )

    payload = handoff.model_dump(mode="json")
    assert payload["schema_version"] == TRACE_EVENT_HANDOFF_SCHEMA_VERSION
    assert payload["checkpoint_backend_family"] == "filesystem"
    assert payload["trace_stream_path"].endswith("TRACE_EVENTS.jsonl")
    assert payload["resume_manifest_path"].endswith("TRACE_RESUME_MANIFEST.json")
    assert payload["transport"]["handoff_supported"] is True
    assert payload["transport"]["binding_artifact_path"].endswith("session-4__session-4.json")


def test_in_memory_event_bridge_supports_last_event_id_heartbeat_and_cleanup() -> None:
    """The bridge should support live subscribe, idle heartbeat, and explicit cleanup."""

    bridge = InMemoryRuntimeEventBridge()
    recorder = RuntimeTraceRecorder(event_bridge=bridge)
    first = recorder.record(
        session_id="session-4",
        job_id="job-4",
        kind="job.queued",
        stage="background",
    )
    recorder.record(
        session_id="session-4",
        job_id="job-4",
        kind="job.completed",
        stage="background",
    )
    recorder.record(
        session_id="session-4",
        job_id="job-5",
        kind="job.completed",
        stage="background",
    )

    first_window = bridge.subscribe(session_id="session-4", job_id="job-4", limit=1)
    assert first_window.schema_version == TRACE_EVENT_BRIDGE_SCHEMA_VERSION
    assert len(first_window.events) == 1
    assert first_window.events[0].event_id == first.event_id
    assert first_window.has_more is True
    assert first_window.next_cursor is not None

    second_window = bridge.subscribe(
        session_id="session-4",
        job_id="job-4",
        after_event_id=first.event_id,
        limit=5,
    )
    assert [event.kind for event in second_window.events] == ["job.completed"]
    assert second_window.has_more is False

    idle_window = bridge.subscribe(
        session_id="session-4",
        job_id="job-4",
        after_event_id=second_window.events[0].event_id,
        heartbeat=True,
    )
    assert idle_window.events == []
    assert idle_window.heartbeat is not None
    assert idle_window.heartbeat.kind == "bridge.heartbeat"
    assert idle_window.heartbeat.status == "idle"

    bridge.seed(recorder.stream_events())
    bridge.seed(recorder.stream_events())
    reseeded = bridge.subscribe(session_id="session-4", limit=10)
    event_ids = [event.event_id for event in reseeded.events]
    assert len(event_ids) == len(set(event_ids))

    bridge.cleanup(session_id="session-4", job_id="job-4")
    cleaned = bridge.subscribe(session_id="session-4", job_id="job-4", heartbeat=True)
    assert cleaned.events == []
    assert cleaned.heartbeat is not None
    assert bridge.health()["control_plane_projection"] == "python-thin-projection"
    assert bridge.health()["transport_family"] == "artifact-handoff"
    other_job = bridge.subscribe(session_id="session-4", job_id="job-5")
    assert [event.job_id for event in other_job.events] == ["job-5"]

    with pytest.raises(ValueError, match="Unknown event id"):
        bridge.subscribe(session_id="session-4", after_event_id="evt_missing")
