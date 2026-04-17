"""Structured runtime trace helpers with resumable stream support."""

from __future__ import annotations

import json
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable, Protocol

from pydantic import BaseModel, Field


TRACE_EVENT_SCHEMA_VERSION = "runtime-trace-v2"
TRACE_METADATA_SCHEMA_VERSION = "runtime-trace-metadata-v2"
TRACE_FRAMEWORK_VERSION = "runtime-v1"
TRACE_EVENT_SINK_SCHEMA_VERSION = "runtime-trace-sink-v2"
TRACE_REPLAY_CURSOR_SCHEMA_VERSION = "runtime-trace-cursor-v1"
TRACE_REPLAY_CHUNK_SCHEMA_VERSION = "runtime-trace-replay-v1"
TRACE_RESUME_MANIFEST_SCHEMA_VERSION = "runtime-resume-manifest-v1"
TRACE_EVENT_BRIDGE_SCHEMA_VERSION = "runtime-event-bridge-v1"
TRACE_EVENT_TRANSPORT_SCHEMA_VERSION = "runtime-event-transport-v1"
TRACE_EVENT_HANDOFF_SCHEMA_VERSION = "runtime-event-handoff-v1"


def _now_iso() -> str:
    """Return a canonical UTC timestamp."""

    return datetime.now(UTC).isoformat()


class TraceEvent(BaseModel):
    """Structured runtime trace event."""

    event_id: str = Field(default_factory=lambda: f"evt_{uuid.uuid4().hex[:12]}")
    seq: int
    generation: int = 0
    cursor: str
    ts: str = Field(default_factory=_now_iso)
    session_id: str
    job_id: str | None = None
    kind: str
    stage: str
    status: str = "ok"
    payload: dict[str, Any] = Field(default_factory=dict)
    schema_version: str = TRACE_EVENT_SCHEMA_VERSION


class TraceReplayCursor(BaseModel):
    """Stable resume cursor for trace replay."""

    schema_version: str = TRACE_REPLAY_CURSOR_SCHEMA_VERSION
    session_id: str
    job_id: str | None = None
    generation: int = 0
    seq: int
    event_id: str
    cursor: str


class TraceReplayChunk(BaseModel):
    """One replay window plus the next resume cursor."""

    schema_version: str = TRACE_REPLAY_CHUNK_SCHEMA_VERSION
    session_id: str | None = None
    job_id: str | None = None
    generation: int = 0
    events: list[TraceEvent] = Field(default_factory=list)
    next_cursor: TraceReplayCursor | None = None
    has_more: bool = False


class TraceDelegationSummary(BaseModel):
    """Lightweight delegation summary projected from supervisor state."""

    plan_created: bool | None = None
    spawn_attempted: bool | None = None
    fallback_mode: str | None = None
    sidecar_count: int = 0


class TraceSupervisorProjection(BaseModel):
    """Thin supervisor/control-plane descriptor surfaced in trace/resume artifacts."""

    supervisor_state_path: str | None = None
    active_phase: str | None = None
    verification_status: str | None = None
    delegation: TraceDelegationSummary | None = None


class TraceResumeManifest(BaseModel):
    """Versioned runtime-owned resume handle joining trace, state, and artifacts."""

    schema_version: str = TRACE_RESUME_MANIFEST_SCHEMA_VERSION
    session_id: str
    job_id: str | None = None
    status: str
    generation: int = 0
    trace_output_path: str | None = None
    trace_stream_path: str | None = None
    event_transport_path: str | None = None
    background_state_path: str | None = None
    latest_cursor: TraceReplayCursor | None = None
    artifact_paths: list[str] = Field(default_factory=list)
    supervisor_projection: TraceSupervisorProjection | None = None
    updated_at: str = Field(default_factory=_now_iso)


class RuntimeEventBridgeHeartbeat(BaseModel):
    """Ephemeral heartbeat payload for quiet stream windows."""

    ts: str = Field(default_factory=_now_iso)
    kind: str = "bridge.heartbeat"
    status: str = "idle"


class RuntimeEventStreamChunk(BaseModel):
    """Live bridge delivery window for host adapters and local subscribers."""

    schema_version: str = TRACE_EVENT_BRIDGE_SCHEMA_VERSION
    session_id: str
    job_id: str | None = None
    generation: int = 0
    events: list[TraceEvent] = Field(default_factory=list)
    next_cursor: TraceReplayCursor | None = None
    has_more: bool = False
    after_event_id: str | None = None
    heartbeat: RuntimeEventBridgeHeartbeat | None = None


class RuntimeEventTransport(BaseModel):
    """Host-facing binding descriptor for one runtime event stream."""

    schema_version: str = TRACE_EVENT_TRANSPORT_SCHEMA_VERSION
    stream_id: str
    session_id: str
    job_id: str | None = None
    bridge_kind: str = "runtime_event_bridge"
    transport_family: str = "host-facing-bridge"
    transport_kind: str = "poll"
    endpoint_kind: str = "runtime_method"
    remote_capable: bool = True
    handoff_supported: bool = True
    handoff_method: str | None = "describe_runtime_event_handoff"
    subscribe_method: str = "subscribe_runtime_events"
    cleanup_method: str = "cleanup_runtime_events"
    describe_method: str = "describe_runtime_event_transport"
    handoff_kind: str = "artifact_handoff"
    binding_refresh_mode: str = "describe_or_checkpoint"
    binding_artifact_format: str = "json"
    binding_backend_family: str | None = None
    binding_artifact_path: str | None = None
    resume_mode: str = "after_event_id"
    heartbeat_supported: bool = True
    cleanup_semantics: str = "bridge_cache_only"
    cleanup_preserves_replay: bool = True
    replay_reseed_supported: bool = True
    chunk_schema_version: str = TRACE_EVENT_BRIDGE_SCHEMA_VERSION
    cursor_schema_version: str = TRACE_REPLAY_CURSOR_SCHEMA_VERSION
    latest_cursor: TraceReplayCursor | None = None
    replay_supported: bool = True


class RuntimeEventHandoff(BaseModel):
    """Host/remote handoff descriptor for one resumable event stream."""

    schema_version: str = TRACE_EVENT_HANDOFF_SCHEMA_VERSION
    stream_id: str
    session_id: str
    job_id: str | None = None
    checkpoint_backend_family: str
    trace_stream_path: str | None = None
    resume_manifest_path: str | None = None
    transport: RuntimeEventTransport


class TraceEventSink(Protocol):
    """Versioned sink interface for runtime trace event export."""

    schema_version: str

    def write_event(self, event: TraceEvent) -> None:
        """Append one trace event to the sink backend."""


class RuntimeEventBridge(Protocol):
    """Live event-bridge seam decoupling producers from consumers."""

    schema_version: str

    def seed(self, events: Iterable[TraceEvent]) -> None:
        """Load existing events into the bridge cache."""

    def publish(self, event: TraceEvent) -> None:
        """Publish one new live event."""

    def subscribe(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
        after_event_id: str | None = None,
        limit: int | None = None,
        heartbeat: bool = False,
    ) -> RuntimeEventStreamChunk:
        """Return the next live delivery window for a subscriber."""

    def cleanup(self, *, session_id: str | None = None, job_id: str | None = None) -> None:
        """Release cached events globally or for one filtered stream."""


class InMemoryRuntimeEventBridge:
    """In-memory event bridge with Last-Event-ID-style resume semantics."""

    def __init__(self, *, schema_version: str = TRACE_EVENT_BRIDGE_SCHEMA_VERSION) -> None:
        self.schema_version = schema_version
        self._events: list[TraceEvent] = []
        self._event_ids: set[str] = set()

    def seed(self, events: Iterable[TraceEvent]) -> None:
        """Seed the bridge with persisted events without duplicating event ids."""

        for event in events:
            if event.event_id in self._event_ids:
                continue
            self._event_ids.add(event.event_id)
            self._events.append(event)

    def publish(self, event: TraceEvent) -> None:
        """Publish one new live event to the bridge."""

        self.seed([event])

    def subscribe(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
        after_event_id: str | None = None,
        limit: int | None = None,
        heartbeat: bool = False,
    ) -> RuntimeEventStreamChunk:
        """Return one filtered stream window after an optional event id cursor."""

        source_events = self._filter_events(self._events, session_id=session_id, job_id=job_id)
        if after_event_id is not None:
            anchor = next((event for event in source_events if event.event_id == after_event_id), None)
            if anchor is None:
                raise ValueError(f"Unknown event id for stream resume: {after_event_id}")
            source_events = [
                event
                for event in source_events
                if (event.generation, event.seq) > (anchor.generation, anchor.seq)
            ]
        has_more = limit is not None and len(source_events) > limit
        window = source_events[:limit] if limit is not None else source_events
        next_cursor = None
        generation = 0
        if window:
            tail = window[-1]
            generation = tail.generation
            next_cursor = TraceReplayCursor(
                session_id=tail.session_id,
                job_id=tail.job_id,
                generation=tail.generation,
                seq=tail.seq,
                event_id=tail.event_id,
                cursor=tail.cursor,
            )
        heartbeat_payload = RuntimeEventBridgeHeartbeat() if heartbeat and not window else None
        return RuntimeEventStreamChunk(
            session_id=session_id,
            job_id=job_id,
            generation=generation,
            events=window,
            next_cursor=next_cursor,
            has_more=has_more,
            after_event_id=after_event_id,
            heartbeat=heartbeat_payload,
        )

    def cleanup(self, *, session_id: str | None = None, job_id: str | None = None) -> None:
        """Release cached bridge events for one stream or clear the full cache."""

        if session_id is None and job_id is None:
            self._events = []
            self._event_ids = set()
            return
        retained = [
            event
            for event in self._events
            if not (
                (session_id is None or event.session_id == session_id)
                and (job_id is None or event.job_id == job_id)
            )
        ]
        self._events = retained
        self._event_ids = {event.event_id for event in retained}

    def _filter_events(
        self,
        events: list[TraceEvent],
        *,
        session_id: str,
        job_id: str | None,
    ) -> list[TraceEvent]:
        filtered = [event for event in events if event.session_id == session_id]
        if job_id is not None:
            filtered = [event for event in filtered if event.job_id == job_id]
        return filtered


class JsonlTraceEventSink:
    """Append trace events to a structured JSONL stream."""

    def __init__(self, path: Path, *, schema_version: str = TRACE_EVENT_SINK_SCHEMA_VERSION) -> None:
        self.path = path
        self.schema_version = schema_version

    def write_event(self, event: TraceEvent) -> None:
        """Persist one event as one deterministic JSON line."""

        self.path.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "sink_schema_version": self.schema_version,
            "event": event.model_dump(mode="json"),
        }
        with self.path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(payload, ensure_ascii=False, sort_keys=True) + "\n")

    def read_events(self) -> list[TraceEvent]:
        """Load persisted events from the JSONL sink with v1 fallback hydration."""

        if not self.path.exists():
            return []

        events: list[TraceEvent] = []
        for line_number, raw_line in enumerate(self.path.read_text(encoding="utf-8").splitlines(), start=1):
            if not raw_line.strip():
                continue
            payload = json.loads(raw_line)
            event_payload = payload.get("event", payload)
            if "seq" not in event_payload:
                event_payload["seq"] = line_number
            generation = int(event_payload.get("generation", 0))
            event_id = str(event_payload.get("event_id", f"evt_replay_{line_number:06d}"))
            event_payload.setdefault("generation", generation)
            event_payload.setdefault("cursor", _build_cursor(generation, int(event_payload["seq"]), event_id))
            event_payload.setdefault("status", "ok")
            event_payload.setdefault("schema_version", TRACE_EVENT_SCHEMA_VERSION)
            events.append(TraceEvent.model_validate(event_payload))
        return events


def _build_cursor(generation: int, seq: int, event_id: str) -> str:
    """Build a deterministic replay cursor from generation, sequence, and event id."""

    return f"g{generation}:s{seq}:{event_id}"


class RuntimeTraceRecorder:
    """Collect and optionally flush runtime trace events."""

    def __init__(
        self,
        output_path: Path | None = None,
        *,
        event_schema_version: str = TRACE_EVENT_SCHEMA_VERSION,
        metadata_schema_version: str = TRACE_METADATA_SCHEMA_VERSION,
        framework_version: str = TRACE_FRAMEWORK_VERSION,
        event_sink: TraceEventSink | None = None,
        event_bridge: RuntimeEventBridge | None = None,
        event_stream_path: Path | None = None,
    ) -> None:
        self.output_path = output_path
        self.event_schema_version = event_schema_version
        self.metadata_schema_version = metadata_schema_version
        self.framework_version = framework_version
        self.event_sink = event_sink
        self.event_bridge = event_bridge
        if self.event_sink is None and event_stream_path is not None:
            self.event_sink = JsonlTraceEventSink(event_stream_path)
        self._events: list[TraceEvent] = []
        self._generation = 0
        self._next_seq = 1

    @property
    def events(self) -> list[TraceEvent]:
        """Return a snapshot of collected events."""

        return list(self._events)

    def record(
        self,
        *,
        session_id: str,
        kind: str,
        stage: str,
        status: str = "ok",
        payload: dict[str, Any] | None = None,
        job_id: str | None = None,
    ) -> TraceEvent:
        """Append one trace event."""

        seq = self._next_seq
        event_id = f"evt_{uuid.uuid4().hex[:12]}"
        event = TraceEvent(
            event_id=event_id,
            seq=seq,
            generation=self._generation,
            cursor=_build_cursor(self._generation, seq, event_id),
            session_id=session_id,
            job_id=job_id,
            kind=kind,
            stage=stage,
            status=status,
            payload=payload or {},
            schema_version=self.event_schema_version,
        )
        self._events.append(event)
        self._next_seq += 1
        if self.event_sink is not None:
            self.event_sink.write_event(event)
        if self.event_bridge is not None:
            self.event_bridge.publish(event)
        return event

    def current_generation(self) -> int:
        """Return the active stream generation."""

        return self._generation

    def latest_cursor(self, *, session_id: str, job_id: str | None = None) -> TraceReplayCursor | None:
        """Return the latest replay cursor for one session/job filter."""

        matched = self._filter_events(self._load_stream_events(), session_id=session_id, job_id=job_id)
        if not matched:
            return None
        tail = matched[-1]
        return TraceReplayCursor(
            session_id=tail.session_id,
            job_id=tail.job_id,
            generation=tail.generation,
            seq=tail.seq,
            event_id=tail.event_id,
            cursor=tail.cursor,
        )

    def replay(
        self,
        *,
        session_id: str | None = None,
        job_id: str | None = None,
        after: TraceReplayCursor | None = None,
        limit: int | None = None,
    ) -> TraceReplayChunk:
        """Replay persisted or in-memory trace events from a stable resume cursor."""

        source_events = self._filter_events(self._load_stream_events(), session_id=session_id, job_id=job_id)
        if after is not None:
            source_events = [
                event
                for event in source_events
                if (event.generation, event.seq) > (after.generation, after.seq)
            ]
        has_more = limit is not None and len(source_events) > limit
        window = source_events[:limit] if limit is not None else source_events
        next_cursor = None
        if window:
            tail = window[-1]
            next_cursor = TraceReplayCursor(
                session_id=tail.session_id,
                job_id=tail.job_id,
                generation=tail.generation,
                seq=tail.seq,
                event_id=tail.event_id,
                cursor=tail.cursor,
            )
        generation = next_cursor.generation if next_cursor is not None else (after.generation if after is not None else 0)
        return TraceReplayChunk(
            session_id=session_id,
            job_id=job_id,
            generation=generation,
            events=window,
            next_cursor=next_cursor,
            has_more=has_more,
        )

    def stream_events(self, *, session_id: str | None = None, job_id: str | None = None) -> list[TraceEvent]:
        """Return replayable events with optional filtering for bridge seeding."""

        return self._filter_events(self._load_stream_events(), session_id=session_id, job_id=job_id)

    def count_reroutes(self, session_id: str) -> int:
        """Return reroute count for a session based on route selection events."""

        route_select_count = sum(
            1 for event in self._events if event.session_id == session_id and event.kind == "route.selected"
        )
        return max(0, route_select_count - 1)

    def count_retries(self, session_id: str) -> int:
        """Return retry count for a session based on prior run failures."""

        return sum(1 for event in self._events if event.session_id == session_id and event.kind == "run.failed")

    def flush_metadata(
        self,
        *,
        task: str,
        matched_skills: list[str],
        owner: str,
        gate: str,
        overlay: str | None,
        artifact_paths: list[str],
        verification_status: str,
        supervisor_projection: dict[str, Any] | None = None,
        reroute_count: int = 0,
        retry_count: int = 0,
    ) -> None:
        """Write a canonical trace metadata artifact when configured."""

        if self.output_path is None:
            return

        stream_state = self.describe_stream()

        payload = {
            "version": 1,
            "metadata_schema_version": self.metadata_schema_version,
            "ts": _now_iso(),
            "task": task,
            "framework_version": self.framework_version,
            "trace_event_schema_version": self.event_schema_version,
            "trace_event_sink_schema_version": self.event_sink.schema_version if self.event_sink is not None else None,
            "routing_runtime_version": 1,
            "matched_skills": matched_skills,
            "decision": {
                "owner": owner,
                "gate": gate,
                "overlay": overlay,
            },
            "reroute_count": reroute_count,
            "retry_count": retry_count,
            "artifact_paths": artifact_paths,
            "verification_status": verification_status,
            "supervisor_projection": supervisor_projection,
            "stream": stream_state,
            "events": [event.model_dump() for event in self._events],
        }
        self.output_path.parent.mkdir(parents=True, exist_ok=True)
        self.output_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    def describe_stream(self) -> dict[str, Any]:
        """Describe the resumable event stream seam exposed by the recorder."""

        source_events = self._load_stream_events()
        latest = source_events[-1] if source_events else None
        return {
            "generation": self._generation,
            "replay_supported": True,
            "event_bridge_supported": self.event_bridge is not None,
            "event_bridge_schema_version": (
                self.event_bridge.schema_version if self.event_bridge is not None else None
            ),
            "event_stream_path": (
                str(self.event_sink.path)
                if isinstance(self.event_sink, JsonlTraceEventSink)
                else None
            ),
            "event_count": len(source_events),
            "latest_seq": latest.seq if latest is not None else 0,
            "latest_event_id": latest.event_id if latest is not None else None,
            "latest_cursor": (
                TraceReplayCursor(
                    session_id=latest.session_id,
                    job_id=latest.job_id,
                    generation=latest.generation,
                    seq=latest.seq,
                    event_id=latest.event_id,
                    cursor=latest.cursor,
                ).model_dump(mode="json")
                if latest is not None
                else None
            ),
        }

    def _load_stream_events(self) -> list[TraceEvent]:
        """Return the best available source of replayable events."""

        if isinstance(self.event_sink, JsonlTraceEventSink):
            persisted = self.event_sink.read_events()
            if persisted:
                return persisted
        return list(self._events)

    def _filter_events(
        self,
        events: list[TraceEvent],
        *,
        session_id: str | None,
        job_id: str | None,
    ) -> list[TraceEvent]:
        """Apply optional session/job filters to a replay source."""

        filtered = events
        if session_id is not None:
            filtered = [event for event in filtered if event.session_id == session_id]
        if job_id is not None:
            filtered = [event for event in filtered if event.job_id == job_id]
        return filtered
