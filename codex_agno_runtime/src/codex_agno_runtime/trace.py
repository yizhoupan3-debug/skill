"""Structured runtime trace helpers with resumable stream support."""

from __future__ import annotations

import hashlib
import json
import tempfile
import uuid
from contextlib import contextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any, Iterable, Mapping, Protocol

from pydantic import BaseModel, Field
from codex_agno_runtime.paths import default_codex_home
from codex_agno_runtime.rust_router import RustRouteAdapter

if TYPE_CHECKING:
    from codex_agno_runtime.checkpoint_store import RuntimeStorageBackend


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
TRACE_CONTROL_PLANE_SCHEMA_VERSION = "runtime-trace-control-plane-v1"
TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION = "runtime-trace-compaction-snapshot-v1"
TRACE_COMPACTION_DELTA_SCHEMA_VERSION = "runtime-trace-compaction-delta-v1"
TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION = "runtime-trace-artifact-ref-v1"
TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION = "runtime-trace-compaction-manifest-v1"
TRACE_COMPACTION_RESULT_SCHEMA_VERSION = "runtime-trace-compaction-result-v1"
TRACE_COMPACTION_RECOVERY_SCHEMA_VERSION = "runtime-trace-compaction-recovery-v1"
_TRACE_SERVICE_NAME = "trace"
_DEFAULT_TRACE_SERVICE_DESCRIPTOR = {
    "authority": "rust-runtime-control-plane",
    "role": "trace-and-handoff",
    "projection": "python-thin-projection",
    "delegate_kind": "filesystem-trace-store",
}
_DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR = {
    "ownership_lane": "rust-contract-lane",
    "producer_owner": "rust-control-plane",
    "producer_authority": "rust-runtime-control-plane",
    "exporter_owner": "rust-control-plane",
    "exporter_authority": "rust-runtime-control-plane",
}
_DEFAULT_TRACE_SCOPE_FIELDS = ("session_id", "job_id")


def _default_routing_runtime_path() -> Path:
    return default_codex_home() / "skills" / "SKILL_ROUTING_RUNTIME.json"


def _now_iso() -> str:
    """Return a canonical UTC timestamp."""

    return datetime.now(UTC).isoformat()


def _load_routing_runtime_version(runtime_path: Path | None = None) -> int:
    """Load the current routing runtime version for emitted trace metadata."""

    resolved_runtime_path = runtime_path or _default_routing_runtime_path()
    if not resolved_runtime_path.is_file():
        return 1
    try:
        payload = json.loads(resolved_runtime_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return 1
    value = payload.get("version")
    return value if isinstance(value, int) else 1


def _coerce_scope_fields(value: Any, *, default: tuple[str, ...]) -> list[str]:
    """Normalize a control-plane scope contract into an ordered field list."""

    if not isinstance(value, (list, tuple)):
        return list(default)
    normalized: list[str] = []
    seen: set[str] = set()
    for item in value:
        field = str(item).strip()
        if not field or field in seen:
            continue
        seen.add(field)
        normalized.append(field)
    return normalized or list(default)


def _stable_json_digest(payload: Any) -> str:
    """Return a stable digest for one JSON-serializable payload."""

    serialized = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(serialized.encode("utf-8")).hexdigest()


def _iter_file_lines(path: Path) -> Iterable[str]:
    """Yield UTF-8 lines from one file while closing the handle promptly."""

    with path.open(encoding="utf-8") as handle:
        yield from handle


def _build_compaction_stream_key(session_id: str, job_id: str | None) -> str:
    """Build a deterministic file-system-safe compaction key."""

    parts = [session_id, job_id or "session"]
    normalized = []
    for part in parts:
        normalized.append(
            "".join(char if char.isalnum() or char in {"-", "_", "."} else "_" for char in part) or "stream"
        )
    return "__".join(normalized)


def _event_matches_scope(
    event: "TraceEvent",
    *,
    session_id: str | None,
    job_id: str | None,
    scope_fields: Iterable[str],
) -> bool:
    """Project event filtering through the Rust-owned scope contract."""

    scope = set(scope_fields)
    if session_id is not None and "session_id" in scope and event.session_id != session_id:
        return False
    if job_id is not None and "job_id" in scope and event.job_id != job_id:
        return False
    return True


def _json_object(payload: Any) -> dict[str, Any] | None:
    """Return a JSON-like object copy when the payload is mapping-shaped."""

    if not isinstance(payload, Mapping):
        return None
    return dict(payload)


class TraceControlPlaneDescriptor(BaseModel):
    """Rust-owned trace descriptor consumed by the Python compatibility host."""

    schema_version: str = TRACE_CONTROL_PLANE_SCHEMA_VERSION
    runtime_control_plane_schema_version: str | None = None
    runtime_control_plane_authority: str = _DEFAULT_TRACE_SERVICE_DESCRIPTOR["authority"]
    service: str = _TRACE_SERVICE_NAME
    authority: str = _DEFAULT_TRACE_SERVICE_DESCRIPTOR["authority"]
    role: str = _DEFAULT_TRACE_SERVICE_DESCRIPTOR["role"]
    projection: str = _DEFAULT_TRACE_SERVICE_DESCRIPTOR["projection"]
    delegate_kind: str = _DEFAULT_TRACE_SERVICE_DESCRIPTOR["delegate_kind"]
    ownership_lane: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["ownership_lane"]
    producer_owner: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["producer_owner"]
    producer_authority: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["producer_authority"]
    exporter_owner: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["exporter_owner"]
    exporter_authority: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["exporter_authority"]
    transport_family: str = "artifact-handoff"
    resume_mode: str = "after_event_id"
    stream_scope_fields: list[str] = Field(default_factory=lambda: list(_DEFAULT_TRACE_SCOPE_FIELDS))
    cleanup_scope_fields: list[str] = Field(default_factory=lambda: list(_DEFAULT_TRACE_SCOPE_FIELDS))
    event_stream_path: str | None = None
    trace_output_path: str | None = None


def _build_trace_control_plane_descriptor(
    *,
    control_plane_descriptor: Mapping[str, Any] | None,
    event_stream_path: Path | None,
    trace_output_path: Path | None,
) -> TraceControlPlaneDescriptor:
    payload: dict[str, Any] = {
        "event_stream_path": str(event_stream_path) if event_stream_path is not None else None,
        "trace_output_path": str(trace_output_path) if trace_output_path is not None else None,
    }
    if isinstance(control_plane_descriptor, Mapping):
        payload["runtime_control_plane_schema_version"] = control_plane_descriptor.get("schema_version")
        payload["runtime_control_plane_authority"] = str(
            control_plane_descriptor.get("authority") or _DEFAULT_TRACE_SERVICE_DESCRIPTOR["authority"]
        )
        services = control_plane_descriptor.get("services")
        if isinstance(services, Mapping):
            service = services.get(_TRACE_SERVICE_NAME)
            if isinstance(service, Mapping):
                for field in ("authority", "role", "projection", "delegate_kind"):
                    value = service.get(field)
                    if value is not None:
                        payload[field] = value
                for field in (
                    "ownership_lane",
                    "producer_owner",
                    "producer_authority",
                    "exporter_owner",
                    "exporter_authority",
                ):
                    value = service.get(field)
                    if value is not None:
                        payload[field] = value
                if service.get("resume_mode") is not None:
                    payload["resume_mode"] = service.get("resume_mode")
                if service.get("stream_scope_fields") is not None:
                    payload["stream_scope_fields"] = _coerce_scope_fields(
                        service.get("stream_scope_fields"),
                        default=_DEFAULT_TRACE_SCOPE_FIELDS,
                    )
                if service.get("cleanup_scope_fields") is not None:
                    payload["cleanup_scope_fields"] = _coerce_scope_fields(
                        service.get("cleanup_scope_fields"),
                        default=_DEFAULT_TRACE_SCOPE_FIELDS,
                    )
    return TraceControlPlaneDescriptor.model_validate(payload)


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


class TraceArtifactRef(BaseModel):
    """Immutable reference to a recovery-critical artifact."""

    schema_version: str = TRACE_COMPACTION_ARTIFACT_REF_SCHEMA_VERSION
    artifact_id: str
    kind: str
    uri: str
    digest: str
    size_bytes: int
    created_at: str = Field(default_factory=_now_iso)
    producer: str = "runtime-trace-recorder"


class TraceCompactionSnapshot(BaseModel):
    """Stable snapshot for one compacted generation."""

    schema_version: str = TRACE_COMPACTION_SNAPSHOT_SCHEMA_VERSION
    generation: int
    snapshot_id: str
    parent_generation: int | None = None
    parent_snapshot_id: str | None = None
    session_id: str
    job_id: str | None = None
    created_at: str = Field(default_factory=_now_iso)
    watermark_event_id: str | None = None
    state_digest: str
    artifact_index_ref: TraceArtifactRef | None = None
    state_ref: TraceArtifactRef | None = None
    delta_cursor: str | None = None
    summary: dict[str, Any] = Field(default_factory=dict)


class TraceCompactionDelta(BaseModel):
    """Generation-local delta that replays on top of the latest stable snapshot."""

    schema_version: str = TRACE_COMPACTION_DELTA_SCHEMA_VERSION
    generation: int
    delta_id: str
    parent_snapshot_id: str
    seq: int
    ts: str
    kind: str
    payload: dict[str, Any] = Field(default_factory=dict)
    artifact_refs: list[TraceArtifactRef] = Field(default_factory=list)
    applies_to: dict[str, Any] = Field(default_factory=dict)


class TraceCompactionManifest(BaseModel):
    """Manifest joining the latest stable snapshot and active delta generation."""

    schema_version: str = TRACE_COMPACTION_MANIFEST_SCHEMA_VERSION
    session_id: str
    job_id: str | None = None
    backend_family: str
    compaction_supported: bool
    snapshot_delta_supported: bool
    latest_stable_snapshot: TraceCompactionSnapshot | None = None
    active_generation: int = 0
    active_parent_snapshot_id: str | None = None
    manifest_path: str | None = None
    snapshot_path: str | None = None
    delta_path: str | None = None
    artifact_index_path: str | None = None
    state_path: str | None = None
    updated_at: str = Field(default_factory=_now_iso)


class TraceCompactionResult(BaseModel):
    """Explicit result for compaction requests, including unsupported backends."""

    schema_version: str = TRACE_COMPACTION_RESULT_SCHEMA_VERSION
    applied: bool
    status: str
    reason: str | None = None
    session_id: str
    job_id: str | None = None
    backend_family: str | None = None
    current_generation: int = 0
    next_generation: int = 0
    latest_stable_snapshot: TraceCompactionSnapshot | None = None
    manifest_path: str | None = None


class TraceCompactionRecovery(BaseModel):
    """Resolved compaction view used for recovery and replay."""

    schema_version: str = TRACE_COMPACTION_RECOVERY_SCHEMA_VERSION
    session_id: str
    job_id: str | None = None
    latest_recoverable_generation: int
    snapshot: TraceCompactionSnapshot
    deltas: list[TraceCompactionDelta] = Field(default_factory=list)
    artifact_index: list[TraceArtifactRef] = Field(default_factory=list)
    state: dict[str, Any] = Field(default_factory=dict)
    latest_cursor: TraceReplayCursor | None = None


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


class TraceParallelGroupSummary(BaseModel):
    """Lightweight durable summary for one background parallel batch."""

    parallel_group_id: str
    job_ids: list[str] = Field(default_factory=list)
    session_ids: list[str] = Field(default_factory=list)
    lane_ids: list[str] = Field(default_factory=list)
    parent_job_ids: list[str] = Field(default_factory=list)
    status_counts: dict[str, int] = Field(default_factory=dict)
    active_job_count: int = 0
    terminal_job_count: int = 0
    total_job_count: int = 0
    latest_updated_at: str | None = None


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
    parallel_group: TraceParallelGroupSummary | None = None
    supervisor_projection: TraceSupervisorProjection | None = None
    control_plane: dict[str, Any] | None = None
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


class RuntimeEventAttachTarget(BaseModel):
    """Stable host-facing endpoint descriptor for one resumable stream."""

    endpoint_kind: str = "runtime_method"
    subscribe_method: str = "subscribe_runtime_events"
    describe_method: str = "describe_runtime_event_transport"
    cleanup_method: str = "cleanup_runtime_events"
    handoff_method: str | None = "describe_runtime_event_handoff"
    session_id: str
    job_id: str | None = None


class RuntimeEventReplayAnchor(BaseModel):
    """Replay anchor contract a remote host can use without Python bridge state."""

    anchor_kind: str = "trace_replay_cursor"
    cursor_schema_version: str = TRACE_REPLAY_CURSOR_SCHEMA_VERSION
    resume_mode: str = "after_event_id"
    latest_cursor: TraceReplayCursor | None = None
    replay_supported: bool = True


class RuntimeEventTransport(BaseModel):
    """Host-facing binding descriptor for one runtime event stream and export lane."""

    schema_version: str = TRACE_EVENT_TRANSPORT_SCHEMA_VERSION
    stream_id: str
    session_id: str
    job_id: str | None = None
    bridge_kind: str = "runtime_event_bridge"
    transport_family: str = "host-facing-bridge"
    transport_kind: str = "poll"
    endpoint_kind: str = "runtime_method"
    ownership_lane: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["ownership_lane"]
    producer_owner: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["producer_owner"]
    producer_authority: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["producer_authority"]
    exporter_owner: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["exporter_owner"]
    exporter_authority: str = _DEFAULT_TRACE_OWNERSHIP_DESCRIPTOR["exporter_authority"]
    remote_capable: bool = True
    remote_attach_supported: bool = True
    attach_mode: str = "process_external_artifact_replay"
    binding_artifact_role: str = "primary_attach_descriptor"
    recommended_remote_attach_method: str = "describe_runtime_event_handoff"
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
    attach_target: RuntimeEventAttachTarget | None = None
    replay_anchor: RuntimeEventReplayAnchor | None = None
    control_plane_authority: str | None = None
    control_plane_role: str | None = None
    control_plane_projection: str | None = None
    control_plane_delegate_kind: str | None = None
    transport_health: dict[str, Any] | None = None

    def model_post_init(self, __context: Any) -> None:
        """Backfill the remote attach contract from the transport descriptor itself."""

        if self.attach_target is None:
            self.attach_target = RuntimeEventAttachTarget(
                endpoint_kind=self.endpoint_kind,
                subscribe_method=self.subscribe_method,
                describe_method=self.describe_method,
                cleanup_method=self.cleanup_method,
                handoff_method=self.handoff_method,
                session_id=self.session_id,
                job_id=self.job_id,
            )
        if self.replay_anchor is None:
            self.replay_anchor = RuntimeEventReplayAnchor(
                resume_mode=self.resume_mode,
                latest_cursor=self.latest_cursor,
                replay_supported=self.replay_supported,
            )


class RuntimeEventHandoff(BaseModel):
    """Host/remote handoff descriptor for one resumable event stream."""

    schema_version: str = TRACE_EVENT_HANDOFF_SCHEMA_VERSION
    stream_id: str
    session_id: str
    job_id: str | None = None
    checkpoint_backend_family: str
    trace_stream_path: str | None = None
    resume_manifest_path: str | None = None
    attach_mode: str = "process_external_artifact_replay"
    resume_manifest_role: str = "checkpoint_recovery_anchor"
    remote_attach_strategy: str = "transport_descriptor_then_replay"
    cleanup_preserves_replay: bool = True
    attach_target: RuntimeEventAttachTarget | None = None
    replay_anchor: RuntimeEventReplayAnchor | None = None
    recovery_artifacts: list[str] = Field(default_factory=list)
    control_plane: dict[str, Any] | None = None
    transport: RuntimeEventTransport

    def model_post_init(self, __context: Any) -> None:
        """Project a remote-resume handoff that survives bridge-cache cleanup."""

        if self.attach_target is None:
            self.attach_target = self.transport.attach_target
        if self.replay_anchor is None:
            self.replay_anchor = self.transport.replay_anchor
        self.cleanup_preserves_replay = self.transport.cleanup_preserves_replay
        if not self.recovery_artifacts:
            ordered = [
                self.transport.binding_artifact_path,
                self.resume_manifest_path,
                self.trace_stream_path,
            ]
            self.recovery_artifacts = [path for path in ordered if path is not None]


class TraceEventSink(Protocol):
    """Versioned sink interface for runtime trace event export."""

    schema_version: str

    def write_event(self, event: TraceEvent) -> None:
        """Append one trace event to the sink backend."""

    def read_events(self) -> list[TraceEvent]:
        """Load persisted trace events from the sink backend."""


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

    def __init__(
        self,
        *,
        schema_version: str = TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.schema_version = schema_version
        self._events: list[TraceEvent] = []
        self._event_ids: set[str] = set()
        self._control_plane = _build_trace_control_plane_descriptor(
            control_plane_descriptor=control_plane_descriptor,
            event_stream_path=None,
            trace_output_path=None,
        )

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
            if not _event_matches_scope(
                event,
                session_id=session_id,
                job_id=job_id,
                scope_fields=self._control_plane.cleanup_scope_fields,
            )
        ]
        self._events = retained
        self._event_ids = {event.event_id for event in retained}

    def bind_control_plane(
        self,
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
        event_stream_path: Path | None = None,
        trace_output_path: Path | None = None,
    ) -> None:
        """Attach or refresh the Rust-owned trace projection used by the bridge."""

        self._control_plane = _build_trace_control_plane_descriptor(
            control_plane_descriptor=control_plane_descriptor,
            event_stream_path=event_stream_path,
            trace_output_path=trace_output_path,
        )

    def control_plane_descriptor(self) -> TraceControlPlaneDescriptor:
        """Return the bridge-facing control-plane descriptor."""

        return self._control_plane.model_copy()

    def health(self) -> dict[str, Any]:
        """Return bridge-local health derived from the shared trace contract."""

        descriptor = self.control_plane_descriptor()
        return {
            "control_plane_authority": descriptor.authority,
            "control_plane_role": descriptor.role,
            "control_plane_projection": descriptor.projection,
            "control_plane_delegate_kind": descriptor.delegate_kind,
            "ownership_lane": descriptor.ownership_lane,
            "producer_owner": descriptor.producer_owner,
            "producer_authority": descriptor.producer_authority,
            "exporter_owner": descriptor.exporter_owner,
            "exporter_authority": descriptor.exporter_authority,
            "cached_event_count": len(self._events),
            "resume_mode": descriptor.resume_mode,
            "transport_family": descriptor.transport_family,
            "stream_scope_fields": list(descriptor.stream_scope_fields),
            "cleanup_scope_fields": list(descriptor.cleanup_scope_fields),
        }

    def _filter_events(
        self,
        events: list[TraceEvent],
        *,
        session_id: str,
        job_id: str | None,
    ) -> list[TraceEvent]:
        return [
            event
            for event in events
            if _event_matches_scope(
                event,
                session_id=session_id,
                job_id=job_id,
                scope_fields=self._control_plane.stream_scope_fields,
            )
        ]


class JsonlTraceEventSink:
    """Append trace events to a structured JSONL stream."""

    def __init__(
        self,
        path: Path,
        *,
        schema_version: str = TRACE_EVENT_SINK_SCHEMA_VERSION,
        control_plane_descriptor: Mapping[str, Any] | None = None,
        storage_backend: "RuntimeStorageBackend | None" = None,
    ) -> None:
        self.path = path
        self.schema_version = schema_version
        self.storage_backend = storage_backend
        self._control_plane = _build_trace_control_plane_descriptor(
            control_plane_descriptor=control_plane_descriptor,
            event_stream_path=path,
            trace_output_path=None,
        )

    def write_event(self, event: TraceEvent) -> None:
        """Persist one event as one deterministic JSON line."""

        payload = {
            "sink_schema_version": self.schema_version,
            "event": event.model_dump(mode="json"),
        }
        serialized = json.dumps(payload, ensure_ascii=False, sort_keys=True) + "\n"
        if self.storage_backend is None:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("a", encoding="utf-8") as handle:
                handle.write(serialized)
            return

        self.storage_backend.append_text(self.path, serialized)

    def read_events(self) -> list[TraceEvent]:
        """Load persisted events from the JSONL sink with v1 fallback hydration."""

        if self.storage_backend is None:
            if not self.path.exists():
                return []
            raw_lines: Iterable[str] = _iter_file_lines(self.path)
        else:
            if not self.storage_backend.exists(self.path):
                return []
            raw_lines = self.storage_backend.iter_text_lines(self.path)

        events: list[TraceEvent] = []
        for line_number, raw_line in enumerate(raw_lines, start=1):
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

    def control_plane_descriptor(self) -> TraceControlPlaneDescriptor:
        """Return the sink-local control-plane projection."""

        return self._control_plane.model_copy()


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
        storage_backend: "RuntimeStorageBackend | None" = None,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.output_path = output_path
        self.event_schema_version = event_schema_version
        self.metadata_schema_version = metadata_schema_version
        self.framework_version = framework_version
        self.event_sink = event_sink
        self.event_bridge = event_bridge
        self.storage_backend = storage_backend
        self._rust_adapter = RustRouteAdapter(default_codex_home())
        self._control_plane = _build_trace_control_plane_descriptor(
            control_plane_descriptor=control_plane_descriptor,
            event_stream_path=event_stream_path,
            trace_output_path=output_path,
        )
        if self.event_sink is None and event_stream_path is not None:
            self.event_sink = JsonlTraceEventSink(
                event_stream_path,
                control_plane_descriptor=control_plane_descriptor,
                storage_backend=self.storage_backend,
            )
        elif isinstance(self.event_sink, JsonlTraceEventSink):
            if self.storage_backend is None:
                self.storage_backend = self.event_sink.storage_backend
            elif self.event_sink.storage_backend is None:
                self.event_sink.storage_backend = self.storage_backend
            self._control_plane = _build_trace_control_plane_descriptor(
                control_plane_descriptor=control_plane_descriptor,
                event_stream_path=self.event_sink.path,
                trace_output_path=output_path,
            )
        self._events: list[TraceEvent] = []
        self._generation = 0
        self._next_seq = 1
        if isinstance(self.event_bridge, InMemoryRuntimeEventBridge):
            self.event_bridge.bind_control_plane(
                control_plane_descriptor=control_plane_descriptor,
                event_stream_path=event_stream_path,
                trace_output_path=output_path,
            )

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

        self._activate_generation_for_stream(session_id=session_id, job_id=job_id)
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
        self._append_compaction_delta(event)
        return event

    def current_generation(self) -> int:
        """Return the active stream generation."""

        return self._generation

    def control_plane_descriptor(self) -> TraceControlPlaneDescriptor:
        """Return the Rust-owned trace control-plane descriptor for this recorder."""

        return self._control_plane.model_copy()

    def latest_cursor(self, *, session_id: str, job_id: str | None = None) -> TraceReplayCursor | None:
        """Return the latest replay cursor for one session/job filter."""

        rust_cursor = self._latest_cursor_via_rust(session_id=session_id, job_id=job_id)
        if rust_cursor is not None:
            return rust_cursor
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
        after_event_id: str | None = None,
        limit: int | None = None,
    ) -> TraceReplayChunk:
        """Replay persisted or in-memory trace events from a stable resume cursor."""

        rust_chunk = self._replay_via_rust(
            session_id=session_id,
            job_id=job_id,
            after_event_id=after_event_id or (after.event_id if after is not None else None),
            limit=limit,
        )
        if rust_chunk is not None:
            return rust_chunk

        recovery = (
            self.recover_compacted_state(session_id=session_id, job_id=job_id)
            if session_id is not None
            else None
        )
        if recovery is not None:
            source_events = [self._delta_to_event(delta) for delta in recovery.deltas]
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
            generation = next_cursor.generation if next_cursor is not None else recovery.latest_recoverable_generation
            return TraceReplayChunk(
                session_id=session_id,
                job_id=job_id,
                generation=generation,
                events=window,
                next_cursor=next_cursor,
                has_more=has_more,
            )

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

    def subscribe_chunk(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
        after_event_id: str | None = None,
        limit: int | None = None,
        heartbeat: bool = False,
    ) -> RuntimeEventStreamChunk:
        """Return one replay-backed subscription window without reseeding an in-memory cache."""

        replay = self.replay(
            session_id=session_id,
            job_id=job_id,
            after_event_id=after_event_id,
            limit=limit,
        )
        return RuntimeEventStreamChunk(
            session_id=session_id,
            job_id=job_id,
            generation=replay.generation,
            events=replay.events,
            next_cursor=replay.next_cursor,
            has_more=replay.has_more,
            after_event_id=after_event_id,
            heartbeat=RuntimeEventBridgeHeartbeat() if heartbeat and not replay.events else None,
        )

    def _supports_rust_trace_io(self) -> bool:
        if not isinstance(self.event_sink, JsonlTraceEventSink):
            return False
        capabilities = self._storage_capabilities()
        return capabilities is None or capabilities.backend_family in {"filesystem", "sqlite"}

    def _trace_stream_path(self) -> Path | None:
        return self.event_sink.path if isinstance(self.event_sink, JsonlTraceEventSink) else None

    def _compaction_manifest_path(self, *, session_id: str, job_id: str | None) -> Path | None:
        path = self._compaction_paths(session_id=session_id, job_id=job_id)["manifest"]
        return path if self._exists(path) else None

    @contextmanager
    def _rust_trace_request(
        self,
        *,
        session_id: str,
        job_id: str | None,
        after_event_id: str | None = None,
        limit: int | None = None,
    ) -> Iterable[dict[str, Any]]:
        trace_stream_path = self._trace_stream_path()
        manifest_path = self._compaction_manifest_path(session_id=session_id, job_id=job_id)
        payload: dict[str, Any] = {
            "path": str(trace_stream_path) if trace_stream_path is not None else None,
            "compaction_manifest_path": str(manifest_path) if manifest_path is not None else None,
            "session_id": session_id,
            "job_id": job_id,
            "stream_scope_fields": list(self._control_plane.stream_scope_fields),
        }
        if after_event_id is not None:
            payload["after_event_id"] = after_event_id
        if limit is not None:
            payload["limit"] = limit
        yield payload

    @staticmethod
    def _cursor_from_payload(payload: Mapping[str, Any] | None) -> TraceReplayCursor | None:
        if not isinstance(payload, Mapping):
            return None
        return TraceReplayCursor.model_validate(dict(payload))

    def _latest_cursor_via_rust(self, *, session_id: str, job_id: str | None) -> TraceReplayCursor | None:
        if not self._supports_rust_trace_io():
            return None
        try:
            with self._rust_trace_request(session_id=session_id, job_id=job_id) as payload:
                resolved = self._rust_adapter.trace_stream_inspect(payload)
        except RuntimeError:
            return None
        return self._cursor_from_payload(resolved.get("latest_cursor"))

    def _replay_via_rust(
        self,
        *,
        session_id: str | None,
        job_id: str | None,
        after_event_id: str | None,
        limit: int | None,
    ) -> TraceReplayChunk | None:
        if session_id is None or not self._supports_rust_trace_io():
            return None
        try:
            with self._rust_trace_request(
                    session_id=session_id,
                    job_id=job_id,
                    after_event_id=after_event_id,
                    limit=limit,
                ) as payload:
                resolved = self._rust_adapter.trace_stream_replay(payload)
        except RuntimeError:
            return None
        events = [TraceEvent.model_validate(payload) for payload in resolved.get("events", [])]
        next_cursor = None
        if next_cursor is None and events:
            tail = events[-1]
            next_cursor = TraceReplayCursor(
                session_id=tail.session_id,
                job_id=tail.job_id,
                generation=tail.generation,
                seq=tail.seq,
                event_id=tail.event_id,
                cursor=tail.cursor,
            )
        generation = next_cursor.generation if next_cursor is not None else 0
        latest_cursor = self._cursor_from_payload(resolved.get("latest_cursor"))
        if generation == 0 and latest_cursor is not None:
            generation = latest_cursor.generation
        return TraceReplayChunk(
            session_id=session_id,
            job_id=job_id,
            generation=generation,
            events=events,
            next_cursor=next_cursor,
            has_more=bool(resolved.get("has_more", False)),
        )

    def _stage_trace_stream(self, *, trace_stream_path: Path, temp_root: Path) -> Path:
        staged = temp_root / "TRACE_EVENTS.jsonl"
        staged.parent.mkdir(parents=True, exist_ok=True)
        with staged.open("w", encoding="utf-8") as handle:
            for line in self.storage_backend.iter_text_lines(trace_stream_path):
                handle.write(line)
        return staged

    def _stage_backend_file(self, *, source: Path, destination: Path) -> None:
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open("w", encoding="utf-8") as handle:
            for line in self.storage_backend.iter_text_lines(source):
                handle.write(line)

    def _stage_compaction_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        manifest_path: Path,
        temp_root: Path,
    ) -> Path:
        manifest = self.load_compaction_manifest(session_id=session_id, job_id=job_id)
        if manifest is None or manifest.latest_stable_snapshot is None:
            raise RuntimeError("Compaction manifest is missing required recovery artifact refs.")
        snapshot = manifest.latest_stable_snapshot
        if snapshot.state_ref is None or snapshot.artifact_index_ref is None:
            raise RuntimeError("Compaction manifest is missing required recovery artifact refs.")

        stream_key = _build_compaction_stream_key(session_id, job_id)
        trace_root = temp_root / "trace_compaction"
        artifacts_dir = trace_root / "artifacts"
        staged_manifest_path = trace_root / f"{stream_key}.manifest.json"
        staged_snapshot_path = trace_root / f"{stream_key}.snapshot.json"
        staged_delta_path = trace_root / f"{stream_key}.deltas.jsonl"
        staged_artifact_index_path = artifacts_dir / f"{stream_key}.artifacts.json"
        staged_state_path = artifacts_dir / f"{stream_key}.state.json"

        state_path = Path(snapshot.state_ref.uri)
        artifact_index_path = Path(snapshot.artifact_index_ref.uri)
        self._stage_backend_file(source=state_path, destination=staged_state_path)
        self._stage_backend_file(source=artifact_index_path, destination=staged_artifact_index_path)
        if manifest.delta_path is not None and self._exists(Path(manifest.delta_path)):
            self._stage_backend_file(source=Path(manifest.delta_path), destination=staged_delta_path)
        else:
            staged_delta_path.parent.mkdir(parents=True, exist_ok=True)
            staged_delta_path.write_text("", encoding="utf-8")

        staged_snapshot = snapshot.model_copy(
            update={
                "artifact_index_ref": snapshot.artifact_index_ref.model_copy(update={"uri": str(staged_artifact_index_path)}),
                "state_ref": snapshot.state_ref.model_copy(update={"uri": str(staged_state_path)}),
            }
        )
        staged_manifest = manifest.model_copy(
            update={
                "latest_stable_snapshot": staged_snapshot,
                "manifest_path": str(staged_manifest_path),
                "snapshot_path": str(staged_snapshot_path),
                "delta_path": str(staged_delta_path),
                "artifact_index_path": str(staged_artifact_index_path),
                "state_path": str(staged_state_path),
            }
        )
        staged_snapshot_path.parent.mkdir(parents=True, exist_ok=True)
        staged_snapshot_path.write_text(staged_snapshot.model_dump_json(indent=2) + "\n", encoding="utf-8")
        staged_manifest_path.write_text(staged_manifest.model_dump_json(indent=2) + "\n", encoding="utf-8")
        return staged_manifest_path

    def compact(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
        artifact_paths: Iterable[str] = (),
    ) -> TraceCompactionResult:
        """Persist a stable snapshot and roll the active stream into the next generation."""

        capabilities = self._storage_capabilities()
        if (
            capabilities is None
            or not capabilities.supports_compaction
            or not capabilities.supports_snapshot_delta
        ):
            backend_family = capabilities.backend_family if capabilities is not None else None
            return TraceCompactionResult(
                applied=False,
                status="unsupported",
                reason="storage backend does not advertise compaction + snapshot-delta support",
                session_id=session_id,
                job_id=job_id,
                backend_family=backend_family,
                current_generation=self._generation,
                next_generation=self._generation,
            )

        source_events = self._filter_events(self._load_stream_events(), session_id=session_id, job_id=job_id)
        active_events = [event for event in source_events if event.generation == source_events[-1].generation] if source_events else []
        if not active_events:
            return TraceCompactionResult(
                applied=False,
                status="no_events",
                reason="no matching events available for compaction",
                session_id=session_id,
                job_id=job_id,
                backend_family=capabilities.backend_family,
                current_generation=self._generation,
                next_generation=self._generation,
            )

        previous_manifest = self.load_compaction_manifest(session_id=session_id, job_id=job_id)
        parent_snapshot = previous_manifest.latest_stable_snapshot if previous_manifest is not None else None
        paths = self._compaction_paths(session_id=session_id, job_id=job_id)
        tail = active_events[-1]
        state_payload = {
            "session_id": session_id,
            "job_id": job_id,
            "generation": tail.generation,
            "watermark_event_id": tail.event_id,
            "delta_cursor": tail.cursor,
            "latest_cursor": TraceReplayCursor(
                session_id=tail.session_id,
                job_id=tail.job_id,
                generation=tail.generation,
                seq=tail.seq,
                event_id=tail.event_id,
                cursor=tail.cursor,
            ).model_dump(mode="json"),
            "event_count": len(active_events),
            "latest_event": tail.model_dump(mode="json"),
            "control_plane": self._control_plane.model_dump(mode="json"),
            "continuity_artifacts": list(dict.fromkeys(str(path) for path in artifact_paths)),
        }
        state_serialized = json.dumps(state_payload, ensure_ascii=False, indent=2) + "\n"
        self._write_text(paths["state"], state_serialized)
        state_ref = self._build_artifact_ref(kind="state_ref", path=paths["state"], payload=state_serialized)

        artifact_index_refs = [
            state_ref,
            self._build_artifact_ref(
                kind="trace_output",
                path=self.output_path,
            )
            if self.output_path is not None
            else None,
            self._build_artifact_ref(
                kind="trace_stream",
                path=self.event_sink.path,
            )
            if isinstance(self.event_sink, JsonlTraceEventSink)
            else None,
        ]
        artifact_index_refs.extend(
            self._build_external_artifact_ref(path=artifact_path)
            for artifact_path in list(dict.fromkeys(str(path) for path in artifact_paths))
        )
        artifact_index_payload = [ref.model_dump(mode="json") for ref in artifact_index_refs if ref is not None]
        artifact_index_serialized = json.dumps(artifact_index_payload, ensure_ascii=False, indent=2) + "\n"
        self._write_text(paths["artifact_index"], artifact_index_serialized)
        artifact_index_ref = self._build_artifact_ref(
            kind="artifact_index_ref",
            path=paths["artifact_index"],
            payload=artifact_index_serialized,
        )

        snapshot_summary = {
            "latest_event_id": tail.event_id,
            "latest_seq": tail.seq,
            "event_count": len(active_events),
            "latest_cursor": state_payload["latest_cursor"],
            "kind": tail.kind,
            "stage": tail.stage,
            "status": tail.status,
        }
        snapshot = TraceCompactionSnapshot(
            generation=tail.generation,
            snapshot_id=f"snap_{uuid.uuid4().hex[:12]}",
            parent_generation=parent_snapshot.generation if parent_snapshot is not None else None,
            parent_snapshot_id=parent_snapshot.snapshot_id if parent_snapshot is not None else None,
            session_id=session_id,
            job_id=job_id,
            watermark_event_id=tail.event_id,
            state_digest=_stable_json_digest(state_payload),
            artifact_index_ref=artifact_index_ref,
            state_ref=state_ref,
            delta_cursor=tail.cursor,
            summary=snapshot_summary,
        )
        self._write_text(paths["snapshot"], snapshot.model_dump_json(indent=2) + "\n")
        self._write_text(paths["deltas"], "")

        next_generation = tail.generation + 1
        manifest = TraceCompactionManifest(
            session_id=session_id,
            job_id=job_id,
            backend_family=capabilities.backend_family,
            compaction_supported=True,
            snapshot_delta_supported=True,
            latest_stable_snapshot=snapshot,
            active_generation=next_generation,
            active_parent_snapshot_id=snapshot.snapshot_id,
            manifest_path=str(paths["manifest"]),
            snapshot_path=str(paths["snapshot"]),
            delta_path=str(paths["deltas"]),
            artifact_index_path=str(paths["artifact_index"]),
            state_path=str(paths["state"]),
        )
        self._write_text(paths["manifest"], manifest.model_dump_json(indent=2) + "\n")
        self._generation = next_generation
        self._next_seq = 1
        return TraceCompactionResult(
            applied=True,
            status="compacted",
            session_id=session_id,
            job_id=job_id,
            backend_family=capabilities.backend_family,
            current_generation=snapshot.generation,
            next_generation=next_generation,
            latest_stable_snapshot=snapshot,
            manifest_path=str(paths["manifest"]),
        )

    def load_compaction_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None = None,
    ) -> TraceCompactionManifest | None:
        """Load the compaction manifest for one session/job stream when present."""

        path = self._compaction_paths(session_id=session_id, job_id=job_id)["manifest"]
        if not self._exists(path):
            return None
        return TraceCompactionManifest.model_validate_json(self._read_text(path))

    def recover_compacted_state(
        self,
        *,
        session_id: str | None,
        job_id: str | None = None,
    ) -> TraceCompactionRecovery | None:
        """Resolve the latest snapshot plus active-generation deltas without scanning old history."""

        if session_id is None:
            return None
        if self._supports_rust_trace_io():
            manifest_path = self._compaction_manifest_path(session_id=session_id, job_id=job_id)
            if manifest_path is not None:
                try:
                    with self._rust_trace_request(session_id=session_id, job_id=job_id) as payload:
                        resolved = self._rust_adapter.trace_stream_inspect(payload)
                except RuntimeError:
                    pass
                else:
                    recovery_payload = resolved.get("recovery")
                    if isinstance(recovery_payload, Mapping):
                        return TraceCompactionRecovery.model_validate(dict(recovery_payload))
        manifest = self.load_compaction_manifest(session_id=session_id, job_id=job_id)
        if manifest is None or manifest.latest_stable_snapshot is None:
            return None

        snapshot = manifest.latest_stable_snapshot
        if snapshot.state_ref is None or snapshot.artifact_index_ref is None:
            raise RuntimeError("Compaction manifest is missing required recovery artifact refs.")

        state_path = Path(snapshot.state_ref.uri)
        artifact_index_path = Path(snapshot.artifact_index_ref.uri)
        if not self._exists(state_path) or not self._exists(artifact_index_path):
            raise RuntimeError("Compaction recovery failed closed because a referenced artifact is missing.")

        state_payload = json.loads(self._read_text(state_path))
        artifact_index = [
            TraceArtifactRef.model_validate(payload)
            for payload in json.loads(self._read_text(artifact_index_path) or "[]")
        ]
        delta_path = Path(manifest.delta_path) if manifest.delta_path is not None else None
        deltas = self._load_compaction_deltas(delta_path)
        latest_cursor = None
        if deltas:
            tail = deltas[-1]
            latest_cursor = self._cursor_from_delta(tail)
        elif isinstance(state_payload.get("latest_cursor"), Mapping):
            latest_cursor = TraceReplayCursor.model_validate(state_payload["latest_cursor"])
        latest_generation = deltas[-1].generation if deltas else manifest.active_generation
        return TraceCompactionRecovery(
            session_id=session_id,
            job_id=job_id,
            latest_recoverable_generation=latest_generation,
            snapshot=snapshot,
            deltas=deltas,
            artifact_index=artifact_index,
            state=state_payload,
            latest_cursor=latest_cursor,
        )

    @staticmethod
    def _count_reroutes_in(events: list[TraceEvent]) -> int:
        """Return reroute count for one scoped event list."""

        route_select_count = sum(1 for event in events if event.kind == "route.selected")
        return max(0, route_select_count - 1)

    @staticmethod
    def _count_retries_in(events: list[TraceEvent]) -> int:
        """Return retry count for one scoped event list."""

        return sum(1 for event in events if event.kind == "run.failed")

    def _scoped_stream_events(
        self,
        *,
        session_id: str | None = None,
        job_id: str | None = None,
    ) -> list[TraceEvent]:
        """Return persisted stream events narrowed to the requested scope."""

        events = self._load_stream_events()
        if session_id is None and job_id is None:
            return events
        return self._filter_events(events, session_id=session_id, job_id=job_id)

    def count_reroutes(self, session_id: str, job_id: str | None = None) -> int:
        """Return reroute count for a session based on route selection events."""

        events = self._scoped_stream_events(session_id=session_id, job_id=job_id)
        return self._count_reroutes_in(events)

    def count_retries(self, session_id: str, job_id: str | None = None) -> int:
        """Return retry count for a session based on prior run failures."""

        events = self._scoped_stream_events(session_id=session_id, job_id=job_id)
        return self._count_retries_in(events)

    def latest_route_selection(self, *, session_id: str) -> dict[str, Any] | None:
        """Return the latest route-selected payload for one session."""

        events = self._scoped_stream_events(session_id=session_id, job_id=None)
        for event in reversed(events):
            if event.kind != "route.selected":
                continue
            payload = _json_object(event.payload)
            if payload is not None:
                return payload
        return None

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
        session_id: str | None = None,
        job_id: str | None = None,
        parallel_group: dict[str, Any] | None = None,
        supervisor_projection: dict[str, Any] | None = None,
        reroute_count: int | None = None,
        retry_count: int | None = None,
    ) -> None:
        """Write a canonical trace metadata artifact when configured."""

        if self.output_path is None:
            return

        stream_state = self.describe_stream()
        events = self._scoped_stream_events(session_id=session_id, job_id=job_id)
        resolved_reroute_count = (
            reroute_count if reroute_count is not None else self._count_reroutes_in(events)
        )
        resolved_retry_count = (
            retry_count if retry_count is not None else self._count_retries_in(events)
        )

        payload = {
            "version": 1,
            "schema_version": self.metadata_schema_version,
            "metadata_schema_version": self.metadata_schema_version,
            "ts": _now_iso(),
            "task": task,
            "framework_version": self.framework_version,
            "trace_event_schema_version": self.event_schema_version,
            "trace_event_sink_schema_version": self.event_sink.schema_version if self.event_sink is not None else None,
            "routing_runtime_version": _load_routing_runtime_version(),
            "matched_skills": matched_skills,
            "decision": {
                "owner": owner,
                "gate": gate,
                "overlay": overlay,
            },
            "reroute_count": resolved_reroute_count,
            "retry_count": resolved_retry_count,
            "artifact_paths": artifact_paths,
            "verification_status": verification_status,
            "parallel_group": parallel_group,
            "supervisor_projection": supervisor_projection,
            "control_plane": self._control_plane.model_dump(mode="json"),
            "stream": stream_state,
            "events": [event.model_dump() for event in events],
        }
        serialized = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
        if self.storage_backend is None:
            self.output_path.parent.mkdir(parents=True, exist_ok=True)
            self.output_path.write_text(serialized, encoding="utf-8")
        else:
            self.storage_backend.write_text(self.output_path, serialized)

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
            "control_plane_authority": self._control_plane.authority,
            "control_plane_role": self._control_plane.role,
            "control_plane_projection": self._control_plane.projection,
            "control_plane_delegate_kind": self._control_plane.delegate_kind,
            "ownership_lane": self._control_plane.ownership_lane,
            "producer_owner": self._control_plane.producer_owner,
            "producer_authority": self._control_plane.producer_authority,
            "exporter_owner": self._control_plane.exporter_owner,
            "exporter_authority": self._control_plane.exporter_authority,
            "transport_family": self._control_plane.transport_family,
            "resume_mode": self._control_plane.resume_mode,
            "stream_scope_fields": list(self._control_plane.stream_scope_fields),
            "cleanup_scope_fields": list(self._control_plane.cleanup_scope_fields),
            "event_stream_path": (
                str(self.event_sink.path)
                if isinstance(self.event_sink, JsonlTraceEventSink)
                else None
            ),
            "compaction_manifest_path": self._latest_compaction_manifest_path(),
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

        if self.event_sink is not None:
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

        return [
            event
            for event in events
            if _event_matches_scope(
                event,
                session_id=session_id,
                job_id=job_id,
                scope_fields=self._control_plane.stream_scope_fields,
            )
        ]

    def _storage_capabilities(self) -> Any:
        if self.storage_backend is None:
            return None
        return self.storage_backend.capabilities()

    def _compaction_paths(self, *, session_id: str, job_id: str | None) -> dict[str, Path]:
        root_candidate = (
            self.event_sink.path.parent
            if isinstance(self.event_sink, JsonlTraceEventSink)
            else (self.output_path.parent if self.output_path is not None else Path.cwd())
        )
        root = root_candidate / "trace_compaction"
        artifacts_dir = root / "artifacts"
        stream_key = _build_compaction_stream_key(session_id, job_id)
        return {
            "manifest": root / f"{stream_key}.manifest.json",
            "snapshot": root / f"{stream_key}.snapshot.json",
            "deltas": root / f"{stream_key}.deltas.jsonl",
            "artifact_index": artifacts_dir / f"{stream_key}.artifacts.json",
            "state": artifacts_dir / f"{stream_key}.state.json",
        }

    def _write_text(self, path: Path, payload: str) -> None:
        if self.storage_backend is not None:
            self.storage_backend.write_text(path, payload)
            return
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(payload, encoding="utf-8")

    def _append_text(self, path: Path, payload: str) -> None:
        if self.storage_backend is not None:
            self.storage_backend.append_text(path, payload)
            return
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(payload)

    def _read_text(self, path: Path) -> str:
        if self.storage_backend is not None:
            return self.storage_backend.read_text(path)
        return path.read_text(encoding="utf-8")

    def _iter_text_lines(self, path: Path) -> Iterable[str]:
        if self.storage_backend is not None:
            yield from self.storage_backend.iter_text_lines(path)
            return
        yield from _iter_file_lines(path)

    def _exists(self, path: Path) -> bool:
        if self.storage_backend is not None:
            return self.storage_backend.exists(path)
        return path.exists()

    def _build_artifact_ref(
        self,
        *,
        kind: str,
        path: Path | None,
        payload: str | None = None,
    ) -> TraceArtifactRef | None:
        if path is None:
            return None
        if payload is None:
            if not self._exists(path):
                return None
            payload = self._read_text(path)
        return TraceArtifactRef(
            artifact_id=f"art_{uuid.uuid4().hex[:12]}",
            kind=kind,
            uri=str(path),
            digest=hashlib.sha256(payload.encode("utf-8")).hexdigest(),
            size_bytes=len(payload.encode("utf-8")),
        )

    def _build_external_artifact_ref(self, *, path: str) -> TraceArtifactRef:
        encoded = path.encode("utf-8")
        return TraceArtifactRef(
            artifact_id=f"art_{uuid.uuid4().hex[:12]}",
            kind="continuity_artifact",
            uri=path,
            digest=hashlib.sha256(encoded).hexdigest(),
            size_bytes=len(encoded),
            producer="runtime-trace-recorder-external",
        )

    def _append_compaction_delta(self, event: TraceEvent) -> None:
        manifest = self.load_compaction_manifest(session_id=event.session_id, job_id=event.job_id)
        if manifest is None or manifest.delta_path is None or manifest.active_parent_snapshot_id is None:
            return
        if event.generation != manifest.active_generation:
            return
        delta = TraceCompactionDelta(
            generation=event.generation,
            delta_id=f"delta_{uuid.uuid4().hex[:12]}",
            parent_snapshot_id=manifest.active_parent_snapshot_id,
            seq=event.seq,
            ts=event.ts,
            kind=event.kind,
            payload={
                "event_id": event.event_id,
                "cursor": event.cursor,
                "stage": event.stage,
                "status": event.status,
                "payload": event.payload,
            },
            artifact_refs=[],
            applies_to={
                "session_id": event.session_id,
                "job_id": event.job_id,
            },
        )
        path = Path(manifest.delta_path)
        if self._supports_rust_trace_io():
            try:
                self._append_compaction_delta_via_rust(path=path, delta=delta)
                return
            except RuntimeError:
                pass
        serialized = delta.model_dump_json() + "\n"
        self._append_text(path, serialized)

    def _append_compaction_delta_via_rust(self, *, path: Path, delta: TraceCompactionDelta) -> None:
        payload = {"path": str(path), "delta": delta.model_dump(mode="json")}
        capabilities = self._storage_capabilities()
        if capabilities is None or capabilities.backend_family == "filesystem" or self.storage_backend is None:
            self._rust_adapter.write_trace_compaction_delta(payload)
            return
        with tempfile.TemporaryDirectory(prefix="runtime-trace-delta-") as temp_dir:
            staged_path = Path(temp_dir) / path.name
            self._rust_adapter.write_trace_compaction_delta(
                {
                    "path": str(staged_path),
                    "delta": delta.model_dump(mode="json"),
                }
            )
            self.storage_backend.append_text(path, staged_path.read_text(encoding="utf-8"))

    def _load_compaction_deltas(self, path: Path | None) -> list[TraceCompactionDelta]:
        if path is None or not self._exists(path):
            return []
        return [
            TraceCompactionDelta.model_validate_json(line)
            for line in self._iter_text_lines(path)
            if line.strip()
        ]

    def _delta_to_event(self, delta: TraceCompactionDelta) -> TraceEvent:
        payload = delta.payload
        return TraceEvent(
            event_id=str(payload["event_id"]),
            seq=delta.seq,
            generation=delta.generation,
            cursor=str(payload["cursor"]),
            ts=delta.ts,
            session_id=str(delta.applies_to["session_id"]),
            job_id=delta.applies_to.get("job_id"),
            kind=delta.kind,
            stage=str(payload["stage"]),
            status=str(payload.get("status", "ok")),
            payload=dict(payload.get("payload", {})),
        )

    def _cursor_from_delta(self, delta: TraceCompactionDelta) -> TraceReplayCursor:
        payload = delta.payload
        return TraceReplayCursor(
            session_id=str(delta.applies_to["session_id"]),
            job_id=delta.applies_to.get("job_id"),
            generation=delta.generation,
            seq=delta.seq,
            event_id=str(payload["event_id"]),
            cursor=str(payload["cursor"]),
        )

    def _activate_generation_for_stream(self, *, session_id: str, job_id: str | None) -> None:
        manifest = self.load_compaction_manifest(session_id=session_id, job_id=job_id)
        if manifest is None:
            return
        if manifest.active_generation < self._generation:
            return
        self._generation = manifest.active_generation
        active_events = [
            event
            for event in self._filter_events(self._load_stream_events(), session_id=session_id, job_id=job_id)
            if event.generation == self._generation
        ]
        active_deltas = self._load_compaction_deltas(Path(manifest.delta_path) if manifest.delta_path is not None else None)
        last_seq = 0
        if active_events:
            last_seq = max(last_seq, active_events[-1].seq)
        if active_deltas:
            last_seq = max(last_seq, active_deltas[-1].seq)
        self._next_seq = last_seq + 1 if last_seq else 1

    def _latest_compaction_manifest_path(self) -> str | None:
        root_candidate = (
            self.event_sink.path.parent
            if isinstance(self.event_sink, JsonlTraceEventSink)
            else (self.output_path.parent if self.output_path is not None else None)
        )
        if root_candidate is None:
            return None
        candidate = root_candidate / "trace_compaction"
        return str(candidate) if self._exists(candidate) else str(candidate)
