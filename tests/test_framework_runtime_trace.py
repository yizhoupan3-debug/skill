"""Focused trace and middleware observability regression tests."""

from __future__ import annotations

import asyncio
import json
import sys
from dataclasses import dataclass
from pathlib import Path
import tempfile

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.checkpoint_store import FilesystemRuntimeCheckpointer, SQLiteRuntimeStorageBackend
from framework_runtime.config import RuntimeSettings
from framework_runtime.event_transport import ExternalRuntimeEventTransportBridge
from framework_runtime.middleware import (
    MemoryMiddleware,
    Middleware,
    MiddlewareChain,
    MiddlewareContext,
    SkillInjectionMiddleware,
)
from framework_runtime.schemas import (
    RoutingResult,
    RunTaskResponse,
    SkillMetadata,
    UsageMetrics,
)
from framework_runtime.trace import (
    TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION,
    TRACE_COMPACTION_DELTA_SCHEMA_VERSION,
    TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION,
    TRACE_COMPACTION_RECOVERY_SCHEMA_VERSION,
    TRACE_COMPACTION_RESULT_SCHEMA_VERSION,
    TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION,
    TRACE_EVENT_SINK_SCHEMA_VERSION,
    TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
    TRACE_EVENT_HANDOFF_SCHEMA_VERSION,
    InMemoryRuntimeEventBridge,
    JsonlTraceEventSink,
    RuntimeEventHandoff,
    RuntimeEventTransport,
    RuntimeTraceRecorder,
    _load_routing_runtime_version,
    TRACE_EVENT_SCHEMA_VERSION,
    TRACE_METADATA_SCHEMA_VERSION,
    TRACE_REPLAY_CHUNK_SCHEMA_VERSION,
    TRACE_REPLAY_CURSOR_SCHEMA_VERSION,
)
from framework_runtime.services import TraceService


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

    def build_prompt(self, routing_result: RoutingResult, *, prompt_preview: str | None = None) -> str:
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


@dataclass(frozen=True)
class _MemoryStorageCapabilities:
    backend_family: str = "memory"
    supports_atomic_replace: bool = False
    supports_compaction: bool = False
    supports_snapshot_delta: bool = False
    supports_remote_event_transport: bool = True


class _InMemoryStorageBackend:
    """Backend double that keeps trace artifacts in process memory."""

    def __init__(self) -> None:
        self._payloads: dict[Path, str] = {}

    def capabilities(self) -> _MemoryStorageCapabilities:
        return _MemoryStorageCapabilities()

    def exists(self, path: Path) -> bool:
        return path in self._payloads

    def read_text(self, path: Path) -> str:
        return self._payloads[path]

    def iter_text_lines(self, path: Path):
        yield from self.read_text(path).splitlines(keepends=True)

    def write_text(self, path: Path, payload: str) -> None:
        self._payloads[path] = payload

    def append_text(self, path: Path, payload: str) -> None:
        self._payloads[path] = self._payloads.get(path, "") + payload

    def delete_text(self, path: Path) -> None:
        self._payloads.pop(path, None)


def test_trace_routing_runtime_version_follows_codex_home_env(monkeypatch) -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        codex_home = Path(tmpdir)
        runtime_path = codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"
        runtime_path.parent.mkdir(parents=True)
        runtime_path.write_text(json.dumps({"version": 7}), encoding="utf-8")
        monkeypatch.setenv("CODEX_HOME", str(codex_home))

        assert _load_routing_runtime_version() == 7


@dataclass(frozen=True)
class _CompactionStorageCapabilities:
    backend_family: str = "memory-compaction"
    supports_atomic_replace: bool = False
    supports_compaction: bool = True
    supports_snapshot_delta: bool = True
    supports_remote_event_transport: bool = True


class _CompactionStorageBackend(_InMemoryStorageBackend):
    """Backend double that advertises compaction and snapshot-delta support."""

    def capabilities(self) -> _CompactionStorageCapabilities:
        return _CompactionStorageCapabilities()


class _FilesystemCompactionStorageBackend:
    """Filesystem-backed test double that keeps compaction enabled for Rust hot-path coverage."""

    def capabilities(self) -> _CompactionStorageCapabilities:
        return _CompactionStorageCapabilities(backend_family="filesystem")

    def exists(self, path: Path) -> bool:
        return path.exists()

    def read_text(self, path: Path) -> str:
        return path.read_text(encoding="utf-8")

    def iter_text_lines(self, path: Path):
        with path.open(encoding="utf-8") as handle:
            yield from handle

    def write_text(self, path: Path, payload: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(payload, encoding="utf-8")

    def append_text(self, path: Path, payload: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(payload)

    def delete_text(self, path: Path) -> None:
        path.unlink(missing_ok=True)


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


def _build_rust_first_trace_control_plane_descriptor(
    *,
    stream_scope_fields: list[str],
    cleanup_scope_fields: list[str],
) -> dict[str, object]:
    return {
        "schema_version": "router-rs-runtime-control-plane-v1",
        "authority": "rust-runtime-control-plane",
        "python_host_role": "thin-projection",
        "rustification_status": {
            "hot_path_projection_mode": "descriptor-driven",
            "python_runtime_role": "compatibility-host",
            "runtime_primary_owner": "rust-control-plane",
            "runtime_primary_owner_authority": "rust-runtime-control-plane",
            "steady_state_python_allowed": False,
        },
        "services": {
            "trace": {
                "authority": "rust-runtime-control-plane",
                "role": "trace-and-handoff",
                "projection": "rust-first-trace-projection",
                "delegate_kind": "rust-trace-store",
                "ownership_lane": "rust-contract-lane",
                "producer_owner": "rust-control-plane",
                "producer_authority": "rust-runtime-control-plane",
                "exporter_owner": "rust-control-plane",
                "exporter_authority": "rust-runtime-control-plane",
                "resume_mode": "after_event_id",
                "stream_scope_fields": stream_scope_fields,
                "cleanup_scope_fields": cleanup_scope_fields,
            }
        },
    }


def test_trace_service_health_exposes_background_effect_host_contract(tmp_path: Path) -> None:
    """Trace health should expose the current rustification owner and residual Python role."""

    settings = RuntimeSettings(
        codex_home=PROJECT_ROOT,
        data_dir=tmp_path / "runtime-data",
        trace_output_path=tmp_path / "TRACE_METADATA.json",
        live_model_override=False,
    )
    checkpointer = FilesystemRuntimeCheckpointer(
        data_dir=settings.resolved_data_dir,
        trace_output_path=settings.resolved_trace_output_path,
    )
    control_plane_descriptor = _build_rust_first_trace_control_plane_descriptor(
        stream_scope_fields=["session_id"],
        cleanup_scope_fields=["session_id"],
    )
    trace_service = TraceService(
        checkpointer,
        control_plane_descriptor=control_plane_descriptor,
    )

    trace_service.startup()
    health = trace_service.health()
    contract = health["background_effect_host_contract"]

    assert health["control_plane_authority"] == "rust-runtime-control-plane"
    assert health["control_plane_role"] == "trace-and-handoff"
    assert contract["service"] == "trace"
    assert contract["control_plane_authority"] == "rust-runtime-control-plane"
    assert contract["control_plane_role"] == "trace-and-handoff"
    assert contract["control_plane_projection"] == "rust-first-trace-projection"
    assert contract["control_plane_delegate_kind"] == "rust-trace-store"
    assert contract["runtime_control_plane_authority"] == "rust-runtime-control-plane"
    assert contract["python_host_role"] == "thin-projection"
    assert contract["steady_state_owner"] == "rust-control-plane"
    assert contract["remaining_python_role"] == "compatibility-host"
    assert contract["progression"]["runtime_primary_owner"] == "rust-control-plane"
    assert contract["progression"]["runtime_primary_owner_authority"] == "rust-runtime-control-plane"
    assert contract["progression"]["python_runtime_role"] == "compatibility-host"
    assert contract["progression"]["steady_state_python_allowed"] is False
    recorder_contract = health["control_plane_contract"]["recorder"]
    bridge_contract = health["control_plane_contract"]["bridge"]
    assert recorder_contract["ownership_lane"] == "rust-contract-lane"
    assert recorder_contract["producer_owner"] == "rust-control-plane"
    assert recorder_contract["producer_authority"] == "rust-runtime-control-plane"
    assert recorder_contract["exporter_owner"] == "rust-control-plane"
    assert recorder_contract["exporter_authority"] == "rust-runtime-control-plane"
    assert bridge_contract["ownership_lane"] == "rust-contract-lane"
    assert bridge_contract["producer_owner"] == "rust-control-plane"
    assert bridge_contract["exporter_owner"] == "rust-control-plane"
    assert health["control_plane_contract"]["aligned"] is True

    trace_service.shutdown()


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
    runtime = json.loads(
        (PROJECT_ROOT / "skills" / "SKILL_ROUTING_RUNTIME.json").read_text(encoding="utf-8")
    )
    assert data["routing_runtime_version"] == runtime["version"]
    assert data["reroute_count"] == 1
    assert data["retry_count"] == 1
    assert data["control_plane"]["authority"] == "rust-runtime-control-plane"
    assert data["control_plane"]["projection"] == "rust-native-projection"
    assert data["control_plane"]["ownership_lane"] == "rust-contract-lane"
    assert data["control_plane"]["producer_owner"] == "rust-control-plane"
    assert data["control_plane"]["producer_authority"] == "rust-runtime-control-plane"
    assert data["control_plane"]["exporter_owner"] == "rust-control-plane"
    assert data["control_plane"]["exporter_authority"] == "rust-runtime-control-plane"
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
    assert data["stream"]["control_plane_projection"] == "rust-native-projection"
    assert data["stream"]["ownership_lane"] == "rust-contract-lane"
    assert data["stream"]["producer_owner"] == "rust-control-plane"
    assert data["stream"]["producer_authority"] == "rust-runtime-control-plane"
    assert data["stream"]["exporter_owner"] == "rust-control-plane"
    assert data["stream"]["exporter_authority"] == "rust-runtime-control-plane"
    assert data["stream"]["latest_seq"] == 4
    assert data["stream"]["latest_cursor"]["schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION


def test_trace_recorder_uses_storage_backend_for_events_metadata_and_replay(tmp_path: Path) -> None:
    """Backend-backed trace recording should persist events, metadata, and replay from memory."""

    output = tmp_path / "TRACE_METADATA.json"
    stream = tmp_path / "TRACE_EVENTS.jsonl"
    backend = _InMemoryStorageBackend()
    recorder = RuntimeTraceRecorder(
        output_path=output,
        event_stream_path=stream,
        storage_backend=backend,
    )
    recorder.record(
        session_id="session-backend",
        job_id="job-backend",
        kind="route.selected",
        stage="routing",
        payload={"skill": "trace-observability"},
    )
    recorder.record(
        session_id="session-backend",
        job_id="job-backend",
        kind="run.failed",
        stage="execution",
        payload={"error": "backend failure"},
    )
    recorder.flush_metadata(
        task="backend trace observability",
        matched_skills=["trace-observability"],
        owner="trace-observability",
        gate="none",
        overlay=None,
        artifact_paths=["artifacts/current/TRACE_METADATA.json"],
        verification_status="dry_run",
    )

    assert backend.capabilities().backend_family == "memory"
    assert not output.exists()
    assert not stream.exists()
    assert backend.exists(output)
    assert backend.exists(stream)

    metadata = json.loads(backend.read_text(output))
    assert metadata["metadata_schema_version"] == TRACE_METADATA_SCHEMA_VERSION
    assert metadata["trace_event_sink_schema_version"] == TRACE_EVENT_SINK_SCHEMA_VERSION
    runtime = json.loads(
        (PROJECT_ROOT / "skills" / "SKILL_ROUTING_RUNTIME.json").read_text(encoding="utf-8")
    )
    assert metadata["routing_runtime_version"] == runtime["version"]
    assert metadata["events"][0]["schema_version"] == TRACE_EVENT_SCHEMA_VERSION

    replayed = RuntimeTraceRecorder(
        output_path=output,
        event_stream_path=stream,
        storage_backend=backend,
    ).replay(session_id="session-backend", job_id="job-backend")
    assert [event.kind for event in replayed.events] == ["route.selected", "run.failed"]
    assert replayed.next_cursor is not None
    assert replayed.next_cursor.seq == 2


def test_trace_recorder_flush_metadata_reloads_persisted_stream_after_restart(tmp_path: Path) -> None:
    """Trace metadata should rebuild counts and events from the persisted stream."""

    output = tmp_path / "TRACE_METADATA.json"
    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(output_path=output, event_stream_path=stream)
    recorder.record(
        session_id="session-restart",
        kind="route.selected",
        stage="routing",
        payload={"skill": "agent-memory"},
    )
    recorder.record(
        session_id="session-restart",
        kind="route.selected",
        stage="routing",
        payload={"skill": "execution-controller-coding"},
    )
    recorder.record(
        session_id="session-restart",
        kind="run.failed",
        stage="execution",
        payload={"error": "retry me"},
    )

    restarted = RuntimeTraceRecorder(output_path=output, event_stream_path=stream)
    assert restarted.count_reroutes("session-restart") == 1
    assert restarted.count_retries("session-restart") == 1

    restarted.flush_metadata(
        task="restart trace recovery",
        matched_skills=["execution-controller-coding", "agent-memory"],
        owner="agent-memory",
        gate="subagent-delegation",
        overlay=None,
        artifact_paths=["TRACE_METADATA.json"],
        verification_status="completed",
        session_id="session-restart",
    )

    payload = json.loads(output.read_text(encoding="utf-8"))
    assert payload["reroute_count"] == 1
    assert payload["retry_count"] == 1
    assert [event["kind"] for event in payload["events"]] == [
        "route.selected",
        "route.selected",
        "run.failed",
    ]


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
        execution_kernel_delegate="router-rs",
        execution_kernel_delegate_authority="rust-execution-cli",
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
    assert trace.events[0].payload["execution_kernel_delegate"] == "router-rs"
    assert trace.events[0].payload["execution_kernel_delegate_authority"] == "rust-execution-cli"
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


def test_trace_recorder_latest_cursor_prefers_rust_trace_io_on_filesystem(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Filesystem latest-cursor lookups should not fall back to Python full-stream reads."""

    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(event_stream_path=stream)
    recorder.record(
        session_id="session-rust-cursor",
        job_id="job-rust-cursor",
        kind="job.started",
        stage="background",
    )
    recorder.record(
        session_id="session-rust-cursor",
        job_id="job-rust-cursor",
        kind="job.completed",
        stage="background",
    )

    monkeypatch.setattr(
        JsonlTraceEventSink,
        "read_events",
        lambda self: (_ for _ in ()).throw(AssertionError("python read_events hot path should stay unused")),
    )

    latest_cursor = recorder.latest_cursor(session_id="session-rust-cursor", job_id="job-rust-cursor")
    assert latest_cursor is not None
    assert latest_cursor.event_id.startswith("evt_")
    assert latest_cursor.seq == 2


def test_trace_recorder_replay_prefers_rust_trace_io_on_filesystem(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Filesystem replay should go through router-rs instead of Python stream hydration."""

    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(event_stream_path=stream)
    for index in range(3):
        recorder.record(
            session_id="session-rust-replay",
            job_id="job-rust-replay",
            kind=f"job.event.{index}",
            stage="background",
        )

    monkeypatch.setattr(
        JsonlTraceEventSink,
        "read_events",
        lambda self: (_ for _ in ()).throw(AssertionError("python read_events hot path should stay unused")),
    )

    replay = recorder.replay(session_id="session-rust-replay", job_id="job-rust-replay", limit=2)
    assert [event.seq for event in replay.events] == [1, 2]
    assert replay.has_more is True
    assert replay.next_cursor is not None
    assert replay.next_cursor.seq == 2


def test_trace_recorder_replay_prefers_rust_trace_io_on_sqlite_backend(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """SQLite-backed replay should still route through router-rs via compatibility staging."""

    backend = SQLiteRuntimeStorageBackend(
        db_path=tmp_path / "runtime_checkpoint_store.sqlite3",
        storage_root=tmp_path,
    )
    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(event_stream_path=stream, storage_backend=backend)
    for index in range(3):
        recorder.record(
            session_id="session-sqlite-replay",
            job_id="job-sqlite-replay",
            kind=f"job.event.{index}",
            stage="background",
        )

    monkeypatch.setattr(
        JsonlTraceEventSink,
        "read_events",
        lambda self: (_ for _ in ()).throw(AssertionError("python read_events hot path should stay unused")),
    )

    latest_cursor = recorder.latest_cursor(session_id="session-sqlite-replay", job_id="job-sqlite-replay")
    assert latest_cursor is not None
    assert latest_cursor.seq == 3

    replay = recorder.replay(session_id="session-sqlite-replay", job_id="job-sqlite-replay", limit=2)
    assert [event.seq for event in replay.events] == [1, 2]
    assert replay.has_more is True
    assert replay.next_cursor is not None
    assert replay.next_cursor.seq == 2


def test_trace_compaction_rolls_generation_and_recovers_from_snapshot_plus_deltas(tmp_path: Path) -> None:
    """Compaction should freeze one stable snapshot and replay later deltas from the next generation."""

    backend = _CompactionStorageBackend()
    recorder = RuntimeTraceRecorder(
        output_path=tmp_path / "TRACE_METADATA.json",
        event_stream_path=tmp_path / "TRACE_EVENTS.jsonl",
        storage_backend=backend,
    )
    recorder.record(
        session_id="session-compact",
        job_id="job-compact",
        kind="job.started",
        stage="background",
        payload={"step": 1},
    )
    recorder.record(
        session_id="session-compact",
        job_id="job-compact",
        kind="job.progress",
        stage="background",
        payload={"step": 2},
    )

    compaction = recorder.compact(
        session_id="session-compact",
        job_id="job-compact",
        artifact_paths=["/tmp/.supervisor_state.json"],
    )
    assert compaction.schema_version == TRACE_COMPACTION_RESULT_SCHEMA_VERSION
    assert compaction.applied is True
    assert compaction.status == "compacted"
    assert compaction.backend_family == "memory-compaction"
    assert compaction.current_generation == 0
    assert compaction.next_generation == 1
    assert compaction.latest_stable_snapshot is not None
    assert compaction.latest_stable_snapshot.schema_version == TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION
    assert compaction.latest_stable_snapshot.generation == 0

    first_new = recorder.record(
        session_id="session-compact",
        job_id="job-compact",
        kind="job.resumed",
        stage="background",
        payload={"step": 3},
    )
    second_new = recorder.record(
        session_id="session-compact",
        job_id="job-compact",
        kind="job.completed",
        stage="background",
        payload={"step": 4},
    )
    assert first_new.generation == 1
    assert first_new.seq == 1
    assert second_new.generation == 1
    assert second_new.seq == 2

    manifest = recorder.load_compaction_manifest(session_id="session-compact", job_id="job-compact")
    assert manifest is not None
    assert manifest.schema_version == TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION
    assert manifest.active_generation == 1
    assert manifest.compaction_supported is True
    assert manifest.snapshot_delta_supported is True

    recovery = recorder.recover_compacted_state(session_id="session-compact", job_id="job-compact")
    assert recovery is not None
    assert recovery.schema_version == TRACE_COMPACTION_RECOVERY_SCHEMA_VERSION
    assert recovery.snapshot.generation == 0
    assert recovery.latest_recoverable_generation == 1
    assert recovery.latest_cursor is not None
    assert recovery.latest_cursor.generation == 1
    assert recovery.latest_cursor.seq == 2
    assert recovery.state["continuity_artifacts"] == ["/tmp/.supervisor_state.json"]
    assert len(recovery.deltas) == 2
    assert all(delta.schema_version == TRACE_COMPACTION_DELTA_SCHEMA_VERSION for delta in recovery.deltas)
    assert [delta.kind for delta in recovery.deltas] == ["job.resumed", "job.completed"]
    assert any(ref.kind == "continuity_artifact" for ref in recovery.artifact_index)
    assert all(ref.schema_version == TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION for ref in recovery.artifact_index)

    replay = recorder.replay(session_id="session-compact", job_id="job-compact")
    assert replay.generation == 1
    assert [event.kind for event in replay.events] == ["job.resumed", "job.completed"]
    assert replay.next_cursor is not None
    assert replay.next_cursor.generation == 1
    assert replay.next_cursor.seq == 2
    assert recorder.describe_stream()["compaction_manifest_path"].endswith("trace_compaction")


def test_trace_compaction_delta_append_prefers_rust_trace_io_on_filesystem(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Filesystem delta append should not fall back to the Python append helper."""

    recorder = RuntimeTraceRecorder(
        output_path=tmp_path / "TRACE_METADATA.json",
        event_stream_path=tmp_path / "TRACE_EVENTS.jsonl",
        storage_backend=_FilesystemCompactionStorageBackend(),
    )
    recorder.record(
        session_id="session-rust-delta",
        job_id="job-rust-delta",
        kind="job.started",
        stage="background",
    )
    compaction = recorder.compact(session_id="session-rust-delta", job_id="job-rust-delta")
    assert compaction.applied is True

    monkeypatch.setattr(
        RuntimeTraceRecorder,
        "_append_text",
        lambda self, path, payload: (_ for _ in ()).throw(AssertionError("python delta append should stay unused")),
    )

    recorder.record(
        session_id="session-rust-delta",
        job_id="job-rust-delta",
        kind="job.completed",
        stage="background",
    )

    manifest = recorder.load_compaction_manifest(session_id="session-rust-delta", job_id="job-rust-delta")
    assert manifest is not None
    assert manifest.delta_path is not None
    delta_payload = Path(manifest.delta_path).read_text(encoding="utf-8")
    assert '"kind":"job.completed"' in delta_payload


def test_trace_compaction_recovery_prefers_rust_trace_io_on_filesystem(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Filesystem compaction recovery should use router-rs instead of Python delta loading."""

    recorder = RuntimeTraceRecorder(
        output_path=tmp_path / "TRACE_METADATA.json",
        event_stream_path=tmp_path / "TRACE_EVENTS.jsonl",
        storage_backend=_FilesystemCompactionStorageBackend(),
    )
    recorder.record(
        session_id="session-rust-recovery",
        job_id="job-rust-recovery",
        kind="job.started",
        stage="background",
    )
    recorder.record(
        session_id="session-rust-recovery",
        job_id="job-rust-recovery",
        kind="job.progress",
        stage="background",
    )
    compaction = recorder.compact(session_id="session-rust-recovery", job_id="job-rust-recovery")
    assert compaction.applied is True
    recorder.record(
        session_id="session-rust-recovery",
        job_id="job-rust-recovery",
        kind="job.completed",
        stage="background",
    )

    monkeypatch.setattr(
        RuntimeTraceRecorder,
        "_load_compaction_deltas",
        lambda self, path: (_ for _ in ()).throw(AssertionError("python delta recovery should stay unused")),
    )

    recovery = recorder.recover_compacted_state(session_id="session-rust-recovery", job_id="job-rust-recovery")
    assert recovery is not None
    assert [delta.kind for delta in recovery.deltas] == ["job.completed"]
    assert recovery.latest_cursor is not None
    assert recovery.latest_cursor.generation == 1


def test_trace_compaction_recovery_prefers_rust_trace_io_on_sqlite_backend(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """SQLite compaction recovery should use the staged Rust contract instead of Python delta parsing."""

    backend = SQLiteRuntimeStorageBackend(
        db_path=tmp_path / "runtime_checkpoint_store.sqlite3",
        storage_root=tmp_path,
    )
    recorder = RuntimeTraceRecorder(
        output_path=tmp_path / "TRACE_METADATA.json",
        event_stream_path=tmp_path / "TRACE_EVENTS.jsonl",
        storage_backend=backend,
    )
    recorder.record(
        session_id="session-sqlite-recovery",
        job_id="job-sqlite-recovery",
        kind="job.started",
        stage="background",
    )
    recorder.record(
        session_id="session-sqlite-recovery",
        job_id="job-sqlite-recovery",
        kind="job.progress",
        stage="background",
    )
    compaction = recorder.compact(session_id="session-sqlite-recovery", job_id="job-sqlite-recovery")
    assert compaction.applied is True
    recorder.record(
        session_id="session-sqlite-recovery",
        job_id="job-sqlite-recovery",
        kind="job.completed",
        stage="background",
    )

    monkeypatch.setattr(
        RuntimeTraceRecorder,
        "_load_compaction_deltas",
        lambda self, path: (_ for _ in ()).throw(AssertionError("python delta recovery should stay unused")),
    )

    recovery = recorder.recover_compacted_state(session_id="session-sqlite-recovery", job_id="job-sqlite-recovery")
    assert recovery is not None
    assert [delta.kind for delta in recovery.deltas] == ["job.completed"]
    assert recovery.latest_cursor is not None
    assert recovery.latest_cursor.generation == 1


def test_trace_compaction_fail_closed_when_referenced_artifact_is_missing(tmp_path: Path) -> None:
    """Recovery must fail closed when compaction points at a missing required artifact."""

    backend = _CompactionStorageBackend()
    recorder = RuntimeTraceRecorder(
        output_path=tmp_path / "TRACE_METADATA.json",
        event_stream_path=tmp_path / "TRACE_EVENTS.jsonl",
        storage_backend=backend,
    )
    recorder.record(
        session_id="session-artifact",
        job_id="job-artifact",
        kind="job.started",
        stage="background",
    )
    compaction = recorder.compact(session_id="session-artifact", job_id="job-artifact")
    assert compaction.applied is True
    assert compaction.latest_stable_snapshot is not None

    backend.delete_text(Path(compaction.latest_stable_snapshot.state_ref.uri))
    with pytest.raises(RuntimeError, match="failed closed"):
        recorder.recover_compacted_state(session_id="session-artifact", job_id="job-artifact")


def test_trace_compaction_returns_explicit_unsupported_result_when_backend_lacks_capabilities(tmp_path: Path) -> None:
    """Backends without compaction capability should return an explicit non-applying result."""

    backend = _InMemoryStorageBackend()
    recorder = RuntimeTraceRecorder(
        output_path=tmp_path / "TRACE_METADATA.json",
        event_stream_path=tmp_path / "TRACE_EVENTS.jsonl",
        storage_backend=backend,
    )
    recorder.record(
        session_id="session-unsupported",
        job_id="job-unsupported",
        kind="job.started",
        stage="background",
    )

    compaction = recorder.compact(session_id="session-unsupported", job_id="job-unsupported")
    assert compaction.schema_version == TRACE_COMPACTION_RESULT_SCHEMA_VERSION
    assert compaction.applied is False
    assert compaction.status == "unsupported"
    assert compaction.reason == "storage backend does not advertise compaction + snapshot-delta support"
    assert recorder.load_compaction_manifest(session_id="session-unsupported", job_id="job-unsupported") is None


def test_trace_contract_scoping_uses_rust_first_scope_fields(tmp_path: Path) -> None:
    """Trace replay and cleanup should follow the Rust-owned scope contract, not hardcoded Python scope."""

    control_plane_descriptor = _build_rust_first_trace_control_plane_descriptor(
        stream_scope_fields=["session_id"],
        cleanup_scope_fields=["session_id"],
    )
    bridge = InMemoryRuntimeEventBridge(control_plane_descriptor=control_plane_descriptor)
    recorder = RuntimeTraceRecorder(
        event_stream_path=tmp_path / "TRACE_EVENTS.jsonl",
        event_bridge=bridge,
        control_plane_descriptor=control_plane_descriptor,
    )
    recorder.record(
        session_id="session-contract",
        job_id="job-a",
        kind="job.started",
        stage="background",
    )
    recorder.record(
        session_id="session-contract",
        job_id="job-b",
        kind="job.completed",
        stage="background",
    )

    replay = recorder.replay(session_id="session-contract", job_id="job-a")
    assert [event.job_id for event in replay.events] == ["job-a", "job-b"]
    assert recorder.describe_stream()["stream_scope_fields"] == ["session_id"]
    assert recorder.describe_stream()["cleanup_scope_fields"] == ["session_id"]
    assert recorder.describe_stream()["ownership_lane"] == "rust-contract-lane"
    assert recorder.describe_stream()["producer_owner"] == "rust-control-plane"
    assert recorder.control_plane_descriptor().stream_scope_fields == ["session_id"]
    assert recorder.control_plane_descriptor().ownership_lane == "rust-contract-lane"

    window = bridge.subscribe(session_id="session-contract", job_id="job-a")
    assert [event.job_id for event in window.events] == ["job-a", "job-b"]

    bridge.cleanup(session_id="session-contract", job_id="job-a")
    cleaned = bridge.subscribe(session_id="session-contract", heartbeat=True)
    assert cleaned.events == []
    assert cleaned.heartbeat is not None
    assert bridge.health()["stream_scope_fields"] == ["session_id"]
    assert bridge.health()["cleanup_scope_fields"] == ["session_id"]
    assert bridge.health()["ownership_lane"] == "rust-contract-lane"
    assert bridge.health()["producer_owner"] == "rust-control-plane"


def test_runtime_event_handoff_serializes_transport_and_replay_refs() -> None:
    """Handoff descriptor should carry transport details plus replay anchors."""

    transport = RuntimeEventTransport(
        stream_id="stream::session-4",
        session_id="session-4",
        job_id="job-4",
        binding_backend_family="filesystem",
        binding_artifact_path="/tmp/runtime_event_transports/session-4__session-4.json",
    )
    handoff = RuntimeEventHandoff(
        stream_id=transport.stream_id,
        session_id="session-4",
        job_id="job-4",
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
    assert payload["remote_attach_strategy"] == "transport_descriptor_then_replay"
    assert payload["cleanup_preserves_replay"] is True
    assert payload["attach_target"]["endpoint_kind"] == "runtime_method"
    assert payload["attach_target"]["subscribe_method"] == "subscribe_runtime_events"
    assert payload["attach_target"]["session_id"] == "session-4"
    assert payload["attach_target"]["job_id"] == "job-4"
    assert payload["replay_anchor"]["anchor_kind"] == "trace_replay_cursor"
    assert payload["replay_anchor"]["cursor_schema_version"] == TRACE_REPLAY_CURSOR_SCHEMA_VERSION
    assert payload["replay_anchor"]["resume_mode"] == "after_event_id"
    assert payload["transport"]["handoff_supported"] is True
    assert payload["transport"]["remote_attach_supported"] is True
    assert payload["transport"]["ownership_lane"] == "rust-contract-lane"
    assert payload["transport"]["producer_owner"] == "rust-control-plane"
    assert payload["transport"]["producer_authority"] == "rust-runtime-control-plane"
    assert payload["transport"]["exporter_owner"] == "rust-control-plane"
    assert payload["transport"]["exporter_authority"] == "rust-runtime-control-plane"
    assert payload["transport"]["attach_target"]["session_id"] == "session-4"
    assert payload["transport"]["replay_anchor"]["anchor_kind"] == "trace_replay_cursor"
    assert payload["transport"]["binding_artifact_path"].endswith("session-4__session-4.json")
    assert payload["recovery_artifacts"] == [
        "/tmp/runtime_event_transports/session-4__session-4.json",
        "/tmp/TRACE_RESUME_MANIFEST.json",
        "/tmp/TRACE_EVENTS.jsonl",
    ]


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
    assert bridge.health()["control_plane_projection"] == "rust-native-projection"
    assert bridge.health()["transport_family"] == "artifact-handoff"
    other_job = bridge.subscribe(session_id="session-4", job_id="job-5")
    assert [event.job_id for event in other_job.events] == ["job-5"]

    with pytest.raises(ValueError, match="Unknown event id"):
        bridge.subscribe(session_id="session-4", after_event_id="evt_missing")


def test_external_runtime_transport_bridge_subscribe_prefers_rust_trace_io_on_filesystem(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """External attach replay should route through router-rs instead of Python replay hydration."""

    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(event_stream_path=stream)
    recorder.record(
        session_id="session-attach",
        job_id="job-attach",
        kind="job.started",
        stage="background",
    )
    recorder.record(
        session_id="session-attach",
        job_id="job-attach",
        kind="job.completed",
        stage="background",
    )

    handoff_path = tmp_path / "ATTACHED_RUNTIME_EVENT_HANDOFF.json"
    handoff = RuntimeEventHandoff(
        stream_id="stream::session-attach",
        session_id="session-attach",
        job_id="job-attach",
        checkpoint_backend_family="filesystem",
        trace_stream_path=str(stream),
        transport=RuntimeEventTransport(
            stream_id="stream::session-attach",
            session_id="session-attach",
            job_id="job-attach",
            binding_backend_family="filesystem",
            binding_artifact_path=str(tmp_path / "runtime_event_transports" / "session-attach__job-attach.json"),
        ),
    )
    handoff_path.write_text(handoff.model_dump_json(indent=2) + "\n", encoding="utf-8")

    monkeypatch.setattr(
        JsonlTraceEventSink,
        "read_events",
        lambda self: (_ for _ in ()).throw(AssertionError("python read_events hot path should stay unused")),
    )

    bridge = ExternalRuntimeEventTransportBridge.attach(handoff_path=str(handoff_path))
    replay = bridge.subscribe(limit=1)
    assert replay.events[0].kind == "job.started"
    resumed = bridge.subscribe(after_event_id=replay.events[0].event_id, limit=5)
    assert [event.kind for event in resumed.events] == ["job.completed"]


def test_external_runtime_transport_bridge_subscribe_prefers_rust_trace_io_on_sqlite_backend(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """SQLite-backed external attach replay should still route through the Rust trace contract."""

    backend = SQLiteRuntimeStorageBackend(
        db_path=tmp_path / "runtime_checkpoint_store.sqlite3",
        storage_root=tmp_path,
    )
    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(event_stream_path=stream, storage_backend=backend)
    recorder.record(
        session_id="session-attach-sqlite",
        job_id="job-attach-sqlite",
        kind="job.started",
        stage="background",
    )
    recorder.record(
        session_id="session-attach-sqlite",
        job_id="job-attach-sqlite",
        kind="job.completed",
        stage="background",
    )

    handoff_path = tmp_path / "ATTACHED_RUNTIME_EVENT_HANDOFF.json"
    handoff = RuntimeEventHandoff(
        stream_id="stream::session-attach-sqlite",
        session_id="session-attach-sqlite",
        job_id="job-attach-sqlite",
        checkpoint_backend_family="sqlite",
        trace_stream_path=str(stream),
        transport=RuntimeEventTransport(
            stream_id="stream::session-attach-sqlite",
            session_id="session-attach-sqlite",
            job_id="job-attach-sqlite",
            binding_backend_family="sqlite",
            binding_artifact_path=str(tmp_path / "runtime_event_transports" / "session-attach-sqlite__job-attach-sqlite.json"),
        ),
    )
    backend.write_text(handoff_path, handoff.model_dump_json(indent=2) + "\n")

    monkeypatch.setattr(
        JsonlTraceEventSink,
        "read_events",
        lambda self: (_ for _ in ()).throw(AssertionError("python read_events hot path should stay unused")),
    )

    bridge = ExternalRuntimeEventTransportBridge.attach(handoff_path=str(handoff_path))
    replay = bridge.subscribe(limit=1)
    assert replay.events[0].kind == "job.started"
    resumed = bridge.subscribe(after_event_id=replay.events[0].event_id, limit=5)
    assert [event.kind for event in resumed.events] == ["job.completed"]


def test_external_runtime_transport_bridge_cleanup_uses_canonical_attach_descriptor_contract(
    tmp_path: Path,
) -> None:
    """Cleanup should round-trip through the Rust attach contract, not a Python-only shim."""

    stream = tmp_path / "TRACE_EVENTS.jsonl"
    recorder = RuntimeTraceRecorder(event_stream_path=stream)
    recorder.record(
        session_id="session-cleanup",
        job_id="job-cleanup",
        kind="job.started",
        stage="background",
    )

    handoff_path = tmp_path / "ATTACHED_RUNTIME_EVENT_HANDOFF.json"
    binding_path = tmp_path / "runtime_event_transports" / "session-cleanup__job-cleanup.json"
    binding_path.parent.mkdir(parents=True, exist_ok=True)
    handoff = RuntimeEventHandoff(
        stream_id="stream::session-cleanup",
        session_id="session-cleanup",
        job_id="job-cleanup",
        checkpoint_backend_family="filesystem",
        trace_stream_path=str(stream),
        transport=RuntimeEventTransport(
            stream_id="stream::session-cleanup",
            session_id="session-cleanup",
            job_id="job-cleanup",
            binding_backend_family="filesystem",
            binding_artifact_path=str(binding_path),
        ),
    )
    handoff_path.write_text(handoff.model_dump_json(indent=2) + "\n", encoding="utf-8")

    bridge = ExternalRuntimeEventTransportBridge.attach(handoff_path=str(handoff_path))
    cleanup = bridge.cleanup()

    assert cleanup["authority"] == bridge._adapter.attached_runtime_event_transport_authority
    assert cleanup["cleanup_method"] == "cleanup_attached_runtime_event_transport"
    assert cleanup["cleanup_semantics"] == "no_persisted_state"
    assert cleanup["cleanup_preserves_replay"] is True
    assert cleanup["binding_artifact_path"] == bridge.describe().get("binding_artifact_path")
    assert cleanup["trace_stream_path"] == bridge.describe().get("trace_stream_path")


def test_external_runtime_transport_bridge_rejects_missing_trace_stream_on_binding_only_attach(
    tmp_path: Path,
) -> None:
    """Binding-only attach should fail closed when no replayable trace stream can be resolved."""

    binding_path = tmp_path / "runtime_event_transports" / "session-missing-trace__job-missing-trace.json"
    binding_path.parent.mkdir(parents=True, exist_ok=True)
    binding_path.write_text(
        RuntimeEventTransport(
            stream_id="stream::session-missing-trace",
            session_id="session-missing-trace",
            job_id="job-missing-trace",
            binding_backend_family="filesystem",
            binding_artifact_path=str(binding_path),
        ).model_dump_json(indent=2)
        + "\n",
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="External runtime event replay requires"):
        ExternalRuntimeEventTransportBridge.attach(binding_artifact_path=str(binding_path))
