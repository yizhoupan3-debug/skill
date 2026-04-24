"""Unified checkpoint seam for runtime trace/state/resume artifacts."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Iterator, Mapping, Protocol

from pydantic import BaseModel

from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.trace import (
    JsonlTraceEventSink,
    RuntimeEventStream,
    RuntimeEventHandoff,
    RuntimeEventTransport,
    RuntimeTraceRecorder,
    TraceReplayCursor,
    TraceResumeManifest,
)


RUNTIME_CHECKPOINT_CONTROL_PLANE_SCHEMA_VERSION = "runtime-checkpoint-control-plane-v1"
_DEFAULT_RUNTIME_CONTROL_PLANE_AUTHORITY = "rust-runtime-control-plane"
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
    runtime_control_plane_authority: str = _DEFAULT_RUNTIME_CONTROL_PLANE_AUTHORITY
    trace_service: dict[str, Any]
    state_service: dict[str, Any]
    backend_family: str
    supports_atomic_replace: bool
    supports_compaction: bool
    supports_snapshot_delta: bool
    supports_remote_event_transport: bool
    trace_output_path: str | None = None
    event_stream_path: str | None = None
    resume_manifest_path: str | None = None
    background_state_path: str
    event_transport_dir: str
class RuntimeStorageBackend(Protocol):
    """Low-level storage backend used by checkpoint and state services."""

    def capabilities(self) -> RuntimeStoreCapabilities:
        """Return the backend capability descriptor."""

    def exists(self, path: Path) -> bool:
        """Return whether a persisted object exists."""

    def read_text(self, path: Path) -> str:
        """Read one persisted UTF-8 payload."""

    def iter_text_lines(self, path: Path) -> Iterator[str]:
        """Stream persisted UTF-8 payload line-by-line."""

    def write_text(self, path: Path, payload: str) -> None:
        """Persist one UTF-8 payload atomically when supported."""

    def append_text(self, path: Path, payload: str) -> None:
        """Append UTF-8 payload text without re-reading the full object."""


class FilesystemRuntimeStorageBackend:
    """Filesystem storage backend with atomic replace semantics."""

    def __init__(self, *, rust_adapter: RustRouteAdapter | None = None) -> None:
        self._rust_adapter = rust_adapter or RustRouteAdapter(_runtime_settings().codex_home)

    def capabilities(self) -> RuntimeStoreCapabilities:
        return RuntimeStoreCapabilities(
            backend_family="filesystem",
            supports_atomic_replace=True,
            supports_compaction=False,
            supports_snapshot_delta=False,
            supports_remote_event_transport=True,
        )

    def exists(self, path: Path) -> bool:
        return self._rust_adapter.runtime_storage_exists(path=path, backend_family="filesystem")

    def read_text(self, path: Path) -> str:
        if not self.exists(path):
            raise KeyError(f"No payload stored for path {path!s}")
        return self._rust_adapter.runtime_storage_read_text(path=path, backend_family="filesystem")

    def iter_text_lines(self, path: Path) -> Iterator[str]:
        yield from self.read_text(path).splitlines(keepends=True)

    def write_text(self, path: Path, payload: str) -> None:
        self._rust_adapter.runtime_storage_write_text(
            path=path,
            backend_family="filesystem",
            payload_text=payload,
        )

    def append_text(self, path: Path, payload: str) -> None:
        self._rust_adapter.runtime_storage_append_text(
            path=path,
            backend_family="filesystem",
            payload_text=payload,
        )


class InMemoryRuntimeStorageBackend:
    """In-process backend used to validate backend-family-neutral runtime persistence."""

    def __init__(self, *, rust_adapter: RustRouteAdapter | None = None) -> None:
        self._rust_adapter = rust_adapter or RustRouteAdapter(_runtime_settings().codex_home)

    def capabilities(self) -> RuntimeStoreCapabilities:
        return RuntimeStoreCapabilities(
            backend_family="memory",
            supports_atomic_replace=False,
            supports_compaction=False,
            supports_snapshot_delta=False,
            supports_remote_event_transport=True,
        )

    def exists(self, path: Path) -> bool:
        return self._rust_adapter.runtime_storage_exists(path=path, backend_family="memory")

    def read_text(self, path: Path) -> str:
        if not self.exists(path):
            raise KeyError(f"No payload stored for path {path!s}")
        return self._rust_adapter.runtime_storage_read_text(path=path, backend_family="memory")

    def iter_text_lines(self, path: Path) -> Iterator[str]:
        yield from self.read_text(path).splitlines(keepends=True)

    def write_text(self, path: Path, payload: str) -> None:
        self._rust_adapter.runtime_storage_write_text(
            path=path,
            backend_family="memory",
            payload_text=payload,
        )

    def append_text(self, path: Path, payload: str) -> None:
        self._rust_adapter.runtime_storage_append_text(
            path=path,
            backend_family="memory",
            payload_text=payload,
        )


class SQLiteRuntimeStorageBackend:
    """SQLite-backed storage backend for runtime-usable non-filesystem persistence."""

    def __init__(
        self,
        *,
        db_path: Path,
        storage_root: Path | None = None,
        rust_adapter: RustRouteAdapter | None = None,
    ) -> None:
        self._db_path = db_path.expanduser().resolve()
        self._storage_root = (
            storage_root.expanduser().resolve() if storage_root is not None else self._db_path.parent
        )
        self._rust_adapter = rust_adapter or RustRouteAdapter(_runtime_settings().codex_home)

    def capabilities(self) -> RuntimeStoreCapabilities:
        return RuntimeStoreCapabilities(
            backend_family="sqlite",
            supports_atomic_replace=True,
            supports_compaction=True,
            supports_snapshot_delta=True,
            supports_remote_event_transport=True,
        )

    def exists(self, path: Path) -> bool:
        return self._rust_adapter.runtime_storage_exists(
            path=path,
            backend_family="sqlite",
            sqlite_db_path=self._db_path,
            storage_root=self._storage_root,
        )

    def read_text(self, path: Path) -> str:
        if not self.exists(path):
            raise KeyError(f"No payload stored for path {path!s}")
        return self._rust_adapter.runtime_storage_read_text(
            path=path,
            backend_family="sqlite",
            sqlite_db_path=self._db_path,
            storage_root=self._storage_root,
        )

    def iter_text_lines(self, path: Path) -> Iterator[str]:
        yield from self.read_text(path).splitlines(keepends=True)

    def write_text(self, path: Path, payload: str) -> None:
        resolved = path.expanduser().resolve()
        self._rust_adapter.runtime_storage_write_text(
            path=resolved,
            backend_family="sqlite",
            sqlite_db_path=self._db_path,
            storage_root=self._storage_root,
            payload_text=payload,
        )

    def append_text(self, path: Path, payload: str) -> None:
        resolved = path.expanduser().resolve()
        self._rust_adapter.runtime_storage_append_text(
            path=resolved,
            backend_family="sqlite",
            sqlite_db_path=self._db_path,
            storage_root=self._storage_root,
            payload_text=payload,
        )

def _runtime_settings() -> "RuntimeSettings":
    from framework_runtime.config import RuntimeSettings

    return RuntimeSettings()


def _normalized_backend_family(value: str) -> str:
    return value.strip().lower().replace("-", "_")


def select_runtime_storage_backend(
    *,
    backend_family: str | None = None,
    storage_root: Path | None = None,
    sqlite_db_path: Path | None = None,
    rust_adapter: RustRouteAdapter | None = None,
) -> RuntimeStorageBackend:
    """Select a concrete backend from config or an explicit family override."""

    settings = None
    resolved_family = backend_family
    if resolved_family is None:
        settings = _runtime_settings()
        resolved_family = settings.checkpoint_storage_backend_family
    normalized_family = _normalized_backend_family(resolved_family)

    if normalized_family in {"filesystem", "file"}:
        return FilesystemRuntimeStorageBackend(rust_adapter=rust_adapter)
    if normalized_family in {"memory", "in_memory", "regression", "regression_double"}:
        return InMemoryRuntimeStorageBackend(rust_adapter=rust_adapter)
    if normalized_family in {"sqlite", "sqlite3"}:
        if sqlite_db_path is None:
            if settings is None:
                settings = _runtime_settings()
            db_file = settings.checkpoint_storage_db_file
            if storage_root is not None and not db_file.is_absolute():
                sqlite_db_path = (storage_root / db_file).resolve()
            else:
                sqlite_db_path = settings.resolved_checkpoint_storage_db_file
        return SQLiteRuntimeStorageBackend(
            db_path=sqlite_db_path,
            storage_root=storage_root,
            rust_adapter=rust_adapter,
        )
    raise ValueError(f"Unsupported runtime storage backend family: {resolved_family!r}")


class RuntimeCheckpointer(Protocol):
    """Backend seam for checkpoint path discovery and resume manifest IO."""

    def describe_paths(self) -> RuntimeCheckpointPaths:
        """Return the current checkpoint path descriptor."""

    def storage_capabilities(self) -> RuntimeStoreCapabilities:
        """Return the active backend-family capability descriptor."""

    def resolve_transport_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        latest_cursor: TraceReplayCursor | None,
    ) -> RuntimeEventTransport:
        """Resolve the host-facing transport manifest for one stream."""

    def resolve_handoff_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        transport: RuntimeEventTransport,
    ) -> RuntimeEventHandoff:
        """Resolve the durable handoff manifest for one stream."""

    def resolve_resume_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        status: str,
        generation: int,
        latest_cursor: TraceReplayCursor | None,
        event_transport_path: str | None,
        artifact_paths: list[str],
        parallel_group: dict[str, Any] | None = None,
        supervisor_projection: dict[str, Any] | None = None,
    ) -> TraceResumeManifest:
        """Resolve the runtime resume manifest before backend persistence."""

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
        parallel_group: dict[str, Any] | None = None,
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
        rust_adapter: RustRouteAdapter | None = None,
    ) -> None:
        self.data_dir = data_dir
        self._rust_adapter = rust_adapter or RustRouteAdapter(_runtime_settings().codex_home)
        self.storage_backend = storage_backend or select_runtime_storage_backend(
            storage_root=data_dir,
            rust_adapter=self._rust_adapter,
        )
        self._paths = RuntimeCheckpointPaths(
            trace_output_path=trace_output_path,
            event_stream_path=(trace_output_path.with_name("TRACE_EVENTS.jsonl") if trace_output_path else None),
            resume_manifest_path=(
                trace_output_path.with_name("TRACE_RESUME_MANIFEST.json") if trace_output_path else None
            ),
                event_transport_dir=data_dir / "runtime_event_transports",
            background_state_path=background_state_path or (data_dir / "runtime_background_jobs.json"),
        )
        self._control_plane = RuntimeCheckpointControlPlaneDescriptor.model_validate(
            self._rust_adapter.runtime_checkpoint_control_plane(
                {
                    "control_plane_descriptor": dict(control_plane_descriptor)
                    if isinstance(control_plane_descriptor, Mapping)
                    else None,
                    "paths": {
                        "trace_output_path": (
                            str(self._paths.trace_output_path)
                            if self._paths.trace_output_path is not None
                            else None
                        ),
                        "event_stream_path": (
                            str(self._paths.event_stream_path)
                            if self._paths.event_stream_path is not None
                            else None
                        ),
                        "resume_manifest_path": (
                            str(self._paths.resume_manifest_path)
                            if self._paths.resume_manifest_path is not None
                            else None
                        ),
                        "background_state_path": str(self._paths.background_state_path),
                        "event_transport_dir": str(self._paths.event_transport_dir),
                    },
                    "capabilities": self.storage_backend.capabilities().__dict__,
                }
            )
        )

    def describe_paths(self) -> RuntimeCheckpointPaths:
        """Return the shared path descriptor."""

        return self._paths

    def storage_capabilities(self) -> RuntimeStoreCapabilities:
        """Return the active storage backend descriptor."""

        return self.storage_backend.capabilities()

    def build_trace_recorder(self, *, event_stream: RuntimeEventStream | None = None) -> RuntimeTraceRecorder:
        """Construct the recorder against the current backend paths."""

        paths = self.describe_paths()
        event_sink = (
            JsonlTraceEventSink(
                paths.event_stream_path,
                control_plane_descriptor=self._control_plane.trace_service,
                storage_backend=self.storage_backend,
            )
            if paths.event_stream_path is not None
            else None
        )
        return RuntimeTraceRecorder(
            output_path=paths.trace_output_path,
            event_sink=event_sink,
            event_stream=event_stream,
            storage_backend=self.storage_backend,
            control_plane_descriptor=self._control_plane.trace_service,
            rust_adapter=self._rust_adapter,
        )

    def resolve_transport_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        latest_cursor: TraceReplayCursor | None,
    ) -> RuntimeEventTransport:
        payload = self._rust_adapter.describe_transport(
            {
                "session_id": session_id,
                "job_id": job_id,
                "latest_cursor": latest_cursor.model_dump(mode="json") if latest_cursor is not None else None,
                "binding_backend_family": self.storage_capabilities().backend_family,
                "binding_artifact_path": (
                    str(path)
                    if (path := self.transport_binding_path(session_id=session_id, job_id=job_id)) is not None
                    else None
                ),
                "control_plane": self._control_plane.model_dump(mode="json"),
            }
        )
        return RuntimeEventTransport.model_validate(payload)

    def resolve_handoff_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        transport: RuntimeEventTransport,
    ) -> RuntimeEventHandoff:
        paths = self.describe_paths()
        payload = self._rust_adapter.describe_handoff(
            {
                "session_id": session_id,
                "job_id": job_id,
                "transport": transport.model_dump(mode="json"),
                "checkpoint_backend_family": self.storage_capabilities().backend_family,
                "trace_stream_path": str(paths.event_stream_path) if paths.event_stream_path is not None else None,
                "resume_manifest_path": str(paths.resume_manifest_path) if paths.resume_manifest_path is not None else None,
                "control_plane": self._control_plane.model_dump(mode="json"),
            }
        )
        return RuntimeEventHandoff.model_validate(payload)

    def resolve_resume_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        status: str,
        generation: int,
        latest_cursor: TraceReplayCursor | None,
        event_transport_path: str | None,
        artifact_paths: list[str],
        parallel_group: dict[str, Any] | None = None,
        supervisor_projection: dict[str, Any] | None = None,
    ) -> TraceResumeManifest:
        paths = self.describe_paths()
        payload = self._rust_adapter.checkpoint_resume_manifest(
            {
                "session_id": session_id,
                "job_id": job_id,
                "status": status,
                "generation": generation,
                "trace_output_path": str(paths.trace_output_path) if paths.trace_output_path is not None else None,
                "trace_stream_path": str(paths.event_stream_path) if paths.event_stream_path is not None else None,
                "event_transport_path": event_transport_path,
                "background_state_path": str(paths.background_state_path),
                "latest_cursor": latest_cursor.model_dump(mode="json") if latest_cursor is not None else None,
                "artifact_paths": artifact_paths,
                "parallel_group": parallel_group,
                "supervisor_projection": supervisor_projection,
                "control_plane": self._control_plane.model_dump(mode="json"),
            }
        )
        return TraceResumeManifest.model_validate(payload)

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
        parallel_group: dict[str, Any] | None = None,
        supervisor_projection: dict[str, Any] | None = None,
    ) -> TraceResumeManifest | None:
        """Persist the current runtime resume manifest when enabled."""

        paths = self.describe_paths()
        if paths.resume_manifest_path is None:
            return None
        manifest = self.resolve_resume_manifest(
            session_id=session_id,
            job_id=job_id,
            status=status,
            generation=generation,
            latest_cursor=latest_cursor,
            event_transport_path=event_transport_path,
            artifact_paths=artifact_paths,
            parallel_group=parallel_group,
            supervisor_projection=supervisor_projection,
        )
        self.storage_backend.write_text(paths.resume_manifest_path, manifest.model_dump_json(indent=2) + "\n")
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
                "binding_backend_family": transport.binding_backend_family or self._control_plane.backend_family,
                "control_plane_authority": self._control_plane.trace_service.get("authority"),
                "control_plane_role": self._control_plane.trace_service.get("role"),
                "control_plane_projection": self._control_plane.trace_service.get("projection"),
                "control_plane_delegate_kind": self._control_plane.trace_service.get("delegate_kind"),
                "transport_health": {
                    "backend_family": self._control_plane.backend_family,
                    "supports_atomic_replace": self._control_plane.supports_atomic_replace,
                    "supports_compaction": self._control_plane.supports_compaction,
                    "supports_snapshot_delta": self._control_plane.supports_snapshot_delta,
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
        """Return the canonical recovery artifact set for the current runtime.

        This recovery surface intentionally lists the root continuity artifacts.
        `artifacts/current/*` remains the stream-facing mirror and should stay in
        sync, but it is not the recovery anchor returned here.
        """

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
