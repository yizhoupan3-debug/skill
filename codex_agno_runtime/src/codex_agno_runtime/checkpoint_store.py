"""Unified checkpoint seam for runtime trace/state/resume artifacts."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Protocol

from pydantic import BaseModel

from codex_agno_runtime.trace import (
    JsonlTraceEventSink,
    RuntimeEventBridge,
    RuntimeEventTransport,
    RuntimeTraceRecorder,
    TraceReplayCursor,
    TraceResumeManifest,
)


RUNTIME_CHECKPOINT_CONTROL_PLANE_SCHEMA_VERSION = "runtime-checkpoint-control-plane-v1"
_DEFAULT_TRACE_SERVICE_DESCRIPTOR = {
    "authority": "rust-runtime-control-plane",
    "role": "trace-and-handoff",
    "projection": "python-thin-projection",
    "delegate_kind": "filesystem-trace-store",
}
_DEFAULT_STATE_SERVICE_DESCRIPTOR = {
    "authority": "rust-runtime-control-plane",
    "role": "durable-background-state",
    "projection": "python-thin-projection",
    "delegate_kind": "filesystem-state-store",
}


@dataclass(frozen=True)
class RuntimeStoreCapabilities:
    """Describe the active persistence backend family and forward-compat flags."""

    backend_family: str
    supports_atomic_replace: bool = True
    supports_compaction: bool = False
    supports_snapshot_delta: bool = False
    supports_remote_event_transport: bool = False


@dataclass(frozen=True)
class RuntimeCheckpointPaths:
    """Stable path descriptor shared across trace/state/resume artifacts."""

    trace_output_path: Path | None
    event_stream_path: Path | None
    resume_manifest_path: Path | None
    event_transport_dir: Path
    background_state_path: Path


class RuntimeCheckpointControlPlaneDescriptor(BaseModel):
    """Shared control-plane projection for checkpoint-backed trace/state artifacts."""

    schema_version: str = RUNTIME_CHECKPOINT_CONTROL_PLANE_SCHEMA_VERSION
    runtime_control_plane_schema_version: str | None = None
    runtime_control_plane_authority: str = _DEFAULT_TRACE_SERVICE_DESCRIPTOR["authority"]
    trace_service: dict[str, Any]
    state_service: dict[str, Any]
    backend_family: str
    supports_remote_event_transport: bool
    trace_output_path: str | None = None
    event_stream_path: str | None = None
    resume_manifest_path: str | None = None
    background_state_path: str
    event_transport_dir: str


def _build_service_projection(
    *,
    control_plane_descriptor: Mapping[str, Any] | None,
    service_name: str,
    defaults: Mapping[str, Any],
) -> dict[str, Any]:
    payload = dict(defaults)
    if isinstance(control_plane_descriptor, Mapping):
        services = control_plane_descriptor.get("services")
        if isinstance(services, Mapping):
            service = services.get(service_name)
            if isinstance(service, Mapping):
                for field in ("authority", "role", "projection", "delegate_kind"):
                    value = service.get(field)
                    if value is not None:
                        payload[field] = value
    return payload


def _build_checkpoint_control_plane_descriptor(
    *,
    control_plane_descriptor: Mapping[str, Any] | None,
    paths: RuntimeCheckpointPaths,
    capabilities: RuntimeStoreCapabilities,
) -> RuntimeCheckpointControlPlaneDescriptor:
    payload: dict[str, Any] = {
        "runtime_control_plane_schema_version": (
            control_plane_descriptor.get("schema_version")
            if isinstance(control_plane_descriptor, Mapping)
            else None
        ),
        "runtime_control_plane_authority": str(
            control_plane_descriptor.get("authority")
            if isinstance(control_plane_descriptor, Mapping) and control_plane_descriptor.get("authority") is not None
            else _DEFAULT_TRACE_SERVICE_DESCRIPTOR["authority"]
        ),
        "trace_service": _build_service_projection(
            control_plane_descriptor=control_plane_descriptor,
            service_name="trace",
            defaults=_DEFAULT_TRACE_SERVICE_DESCRIPTOR,
        ),
        "state_service": _build_service_projection(
            control_plane_descriptor=control_plane_descriptor,
            service_name="state",
            defaults=_DEFAULT_STATE_SERVICE_DESCRIPTOR,
        ),
        "backend_family": capabilities.backend_family,
        "supports_remote_event_transport": capabilities.supports_remote_event_transport,
        "trace_output_path": str(paths.trace_output_path) if paths.trace_output_path is not None else None,
        "event_stream_path": str(paths.event_stream_path) if paths.event_stream_path is not None else None,
        "resume_manifest_path": str(paths.resume_manifest_path) if paths.resume_manifest_path is not None else None,
        "background_state_path": str(paths.background_state_path),
        "event_transport_dir": str(paths.event_transport_dir),
    }
    return RuntimeCheckpointControlPlaneDescriptor.model_validate(payload)


class RuntimeStorageBackend(Protocol):
    """Low-level storage backend used by checkpoint and state services."""

    def capabilities(self) -> RuntimeStoreCapabilities:
        """Return the backend capability descriptor."""

    def exists(self, path: Path) -> bool:
        """Return whether a persisted object exists."""

    def read_text(self, path: Path) -> str:
        """Read one persisted UTF-8 payload."""

    def write_text(self, path: Path, payload: str) -> None:
        """Persist one UTF-8 payload atomically when supported."""


class FilesystemRuntimeStorageBackend:
    """Filesystem storage backend with atomic replace semantics."""

    def capabilities(self) -> RuntimeStoreCapabilities:
        return RuntimeStoreCapabilities(
            backend_family="filesystem",
            supports_atomic_replace=True,
            supports_compaction=False,
            supports_snapshot_delta=False,
            supports_remote_event_transport=True,
        )

    def exists(self, path: Path) -> bool:
        return path.exists()

    def read_text(self, path: Path) -> str:
        return path.read_text(encoding="utf-8")

    def write_text(self, path: Path, payload: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp_path = path.with_suffix(f"{path.suffix}.tmp")
        tmp_path.write_text(payload, encoding="utf-8")
        tmp_path.replace(path)


class RuntimeCheckpointer(Protocol):
    """Backend seam for checkpoint path discovery and resume manifest IO."""

    def describe_paths(self) -> RuntimeCheckpointPaths:
        """Return the current checkpoint path descriptor."""

    def storage_capabilities(self) -> RuntimeStoreCapabilities:
        """Return the active backend-family capability descriptor."""

    def checkpoint(
        self,
        *,
        session_id: str,
        job_id: str | None,
        status: str,
        generation: int,
        latest_cursor: TraceReplayCursor | None,
        event_transport_path: str | None,
        artifact_paths: list[str],
        supervisor_projection: dict[str, Any] | None = None,
    ) -> TraceResumeManifest | None:
        """Write the current runtime checkpoint when enabled."""

    def load_checkpoint(self) -> TraceResumeManifest | None:
        """Load the latest checkpoint manifest when available."""

    def transport_binding_path(self, *, session_id: str, job_id: str | None = None) -> Path | None:
        """Return the persisted transport-binding path when supported."""

    def write_transport_binding(self, transport: RuntimeEventTransport) -> Path | None:
        """Persist a transport binding artifact when supported."""


class FilesystemRuntimeCheckpointer:
    """Own filesystem-backed trace/state/resume artifact locations."""

    def __init__(
        self,
        *,
        data_dir: Path,
        trace_output_path: Path | None = None,
        background_state_path: Path | None = None,
        storage_backend: RuntimeStorageBackend | None = None,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.data_dir = data_dir
        self.storage_backend = storage_backend or FilesystemRuntimeStorageBackend()
        self._paths = RuntimeCheckpointPaths(
            trace_output_path=trace_output_path,
            event_stream_path=(trace_output_path.with_name("TRACE_EVENTS.jsonl") if trace_output_path else None),
            resume_manifest_path=(
                trace_output_path.with_name("TRACE_RESUME_MANIFEST.json") if trace_output_path else None
            ),
            event_transport_dir=data_dir / "runtime_event_transports",
            background_state_path=background_state_path or (data_dir / "runtime_background_jobs.json"),
        )
        self._control_plane = _build_checkpoint_control_plane_descriptor(
            control_plane_descriptor=control_plane_descriptor,
            paths=self._paths,
            capabilities=self.storage_backend.capabilities(),
        )

    def describe_paths(self) -> RuntimeCheckpointPaths:
        """Return the shared path descriptor."""

        return self._paths

    def storage_capabilities(self) -> RuntimeStoreCapabilities:
        """Return the active storage backend descriptor."""

        return self.storage_backend.capabilities()

    def build_trace_recorder(self, *, event_bridge: RuntimeEventBridge | None = None) -> RuntimeTraceRecorder:
        """Construct the recorder against the current backend paths."""

        paths = self.describe_paths()
        event_sink = (
            JsonlTraceEventSink(
                paths.event_stream_path,
                control_plane_descriptor=self._control_plane.trace_service,
            )
            if paths.event_stream_path is not None
            else None
        )
        return RuntimeTraceRecorder(
            output_path=paths.trace_output_path,
            event_sink=event_sink,
            event_bridge=event_bridge,
            control_plane_descriptor=self._control_plane.trace_service,
        )

    def checkpoint(
        self,
        *,
        session_id: str,
        job_id: str | None,
        status: str,
        generation: int,
        latest_cursor: TraceReplayCursor | None,
        event_transport_path: str | None,
        artifact_paths: list[str],
        supervisor_projection: dict[str, Any] | None = None,
    ) -> TraceResumeManifest | None:
        """Persist the current runtime resume manifest when enabled."""

        paths = self.describe_paths()
        if paths.resume_manifest_path is None:
            return None
        manifest = TraceResumeManifest(
            session_id=session_id,
            job_id=job_id,
            status=status,
            generation=generation,
            trace_output_path=str(paths.trace_output_path) if paths.trace_output_path is not None else None,
            trace_stream_path=str(paths.event_stream_path) if paths.event_stream_path is not None else None,
            event_transport_path=event_transport_path,
            background_state_path=str(paths.background_state_path),
            latest_cursor=latest_cursor,
            artifact_paths=artifact_paths,
            supervisor_projection=supervisor_projection,
            control_plane=self._control_plane.model_dump(mode="json"),
        )
        paths.resume_manifest_path.parent.mkdir(parents=True, exist_ok=True)
        self.storage_backend.write_text(
            paths.resume_manifest_path,
            manifest.model_dump_json(indent=2) + "\n",
        )
        return manifest

    def load_checkpoint(self) -> TraceResumeManifest | None:
        """Load the most recent resume manifest."""

        paths = self.describe_paths()
        if paths.resume_manifest_path is None or not self.storage_backend.exists(paths.resume_manifest_path):
            return None
        return TraceResumeManifest.model_validate_json(
            self.storage_backend.read_text(paths.resume_manifest_path)
        )

    def transport_binding_path(self, *, session_id: str, job_id: str | None = None) -> Path | None:
        """Return the stable transport-binding path for one stream."""

        if not self.storage_backend.capabilities().supports_remote_event_transport:
            return None
        stream_key = job_id or session_id
        return self.describe_paths().event_transport_dir / f"{session_id}__{stream_key}.json"

    def write_transport_binding(self, transport: RuntimeEventTransport) -> Path | None:
        """Persist one runtime event transport binding for host/remote handoff."""

        path = self.transport_binding_path(session_id=transport.session_id, job_id=transport.job_id)
        if path is None:
            return None
        projected = transport.model_copy(
            update={
                "binding_artifact_path": str(path),
                "control_plane_authority": self._control_plane.trace_service.get("authority"),
                "control_plane_role": self._control_plane.trace_service.get("role"),
                "control_plane_projection": self._control_plane.trace_service.get("projection"),
                "control_plane_delegate_kind": self._control_plane.trace_service.get("delegate_kind"),
                "transport_health": {
                    "backend_family": self._control_plane.backend_family,
                    "supports_remote_event_transport": self._control_plane.supports_remote_event_transport,
                },
            }
        )
        payload = projected.model_dump_json(indent=2) + "\n"
        self.storage_backend.write_text(path, payload)
        return path

    def control_plane_descriptor(self) -> RuntimeCheckpointControlPlaneDescriptor:
        """Return the shared control-plane descriptor for checkpoint-backed artifacts."""

        return self._control_plane.model_copy()

    def artifact_paths(
        self,
        *,
        codex_home: Path,
        extra_paths: Iterable[Path | None] = (),
    ) -> list[str]:
        """Return the canonical recovery artifact set for the current runtime."""

        paths = self.describe_paths()
        always_include = [
            paths.trace_output_path,
            paths.event_stream_path,
            paths.resume_manifest_path,
            paths.event_transport_dir,
            paths.background_state_path,
            *extra_paths,
        ]
        existing_only = [
            codex_home / ".supervisor_state.json",
            codex_home / "SESSION_SUMMARY.md",
            codex_home / "NEXT_ACTIONS.json",
            codex_home / "EVIDENCE_INDEX.json",
            codex_home / "TRACE_METADATA.json",
        ]
        seen: set[str] = set()
        paths: list[str] = []
        for candidate in always_include:
            if candidate is None:
                continue
            resolved = str(candidate.resolve())
            if resolved in seen:
                continue
            seen.add(resolved)
            paths.append(resolved)
        for candidate in existing_only:
            if not candidate.exists():
                continue
            resolved = str(candidate.resolve())
            if resolved in seen:
                continue
            seen.add(resolved)
            paths.append(resolved)
        return paths

    def health(self) -> dict[str, str | bool | None]:
        """Describe the active checkpoint backend paths."""

        paths = self.describe_paths()
        capabilities = self.storage_capabilities()
        return {
            "control_plane_authority": self._control_plane.runtime_control_plane_authority,
            "trace_control_plane_projection": self._control_plane.trace_service.get("projection"),
            "state_control_plane_projection": self._control_plane.state_service.get("projection"),
            "backend_family": capabilities.backend_family,
            "supports_atomic_replace": capabilities.supports_atomic_replace,
            "supports_compaction": capabilities.supports_compaction,
            "supports_snapshot_delta": capabilities.supports_snapshot_delta,
            "supports_remote_event_transport": capabilities.supports_remote_event_transport,
            "trace_output_path": str(paths.trace_output_path) if paths.trace_output_path is not None else None,
            "event_stream_path": str(paths.event_stream_path) if paths.event_stream_path is not None else None,
            "resume_manifest_path": str(paths.resume_manifest_path) if paths.resume_manifest_path is not None else None,
            "event_transport_dir": str(paths.event_transport_dir),
            "background_state_path": str(paths.background_state_path),
        }
