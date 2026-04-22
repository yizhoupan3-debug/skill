"""Unified checkpoint seam for runtime trace/state/resume artifacts."""

from __future__ import annotations

from dataclasses import dataclass
import sqlite3
from pathlib import Path
from typing import Any, Callable, Iterable, Mapping, Protocol

from pydantic import BaseModel

from codex_agno_runtime.rust_router import RustRouteAdapter
from codex_agno_runtime.trace import (
    JsonlTraceEventSink,
    RuntimeEventBridge,
    RuntimeEventHandoff,
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


def _default_service_delegate_kind(*, service_name: str, backend_family: str) -> str:
    """Return the backend-aware default delegate kind for one service lane."""

    normalized_backend = backend_family.strip().lower().replace("_", "-")
    return f"{normalized_backend}-{service_name}-store"


def _coerce_legacy_service_delegate_kind(
    *,
    delegate_kind: str,
    service_name: str,
    backend_family: str,
) -> str:
    """Rewrite stale filesystem delegate labels when the active backend is not filesystem."""

    legacy_delegate = f"filesystem-{service_name}-store"
    if backend_family == "filesystem" or delegate_kind != legacy_delegate:
        return delegate_kind
    return _default_service_delegate_kind(service_name=service_name, backend_family=backend_family)


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
    supports_atomic_replace: bool
    supports_compaction: bool
    supports_snapshot_delta: bool
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
    capabilities: RuntimeStoreCapabilities,
) -> dict[str, Any]:
    payload = dict(defaults)
    payload["delegate_kind"] = _default_service_delegate_kind(
        service_name=service_name,
        backend_family=capabilities.backend_family,
    )
    if isinstance(control_plane_descriptor, Mapping):
        services = control_plane_descriptor.get("services")
        if isinstance(services, Mapping):
            service = services.get(service_name)
            if isinstance(service, Mapping):
                for field in ("authority", "role", "projection", "delegate_kind"):
                    value = service.get(field)
                    if value is not None:
                        payload[field] = value
    delegate_kind = payload.get("delegate_kind")
    if isinstance(delegate_kind, str):
        payload["delegate_kind"] = _coerce_legacy_service_delegate_kind(
            delegate_kind=delegate_kind,
            service_name=service_name,
            backend_family=capabilities.backend_family,
        )
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
            capabilities=capabilities,
        ),
        "state_service": _build_service_projection(
            control_plane_descriptor=control_plane_descriptor,
            service_name="state",
            defaults=_DEFAULT_STATE_SERVICE_DESCRIPTOR,
            capabilities=capabilities,
        ),
        "backend_family": capabilities.backend_family,
        "supports_atomic_replace": capabilities.supports_atomic_replace,
        "supports_compaction": capabilities.supports_compaction,
        "supports_snapshot_delta": capabilities.supports_snapshot_delta,
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


class InMemoryRuntimeStorageBackend:
    """In-process backend used to validate backend-family-neutral runtime persistence."""

    def __init__(self) -> None:
        self._payloads: dict[str, str] = {}

    def capabilities(self) -> RuntimeStoreCapabilities:
        return RuntimeStoreCapabilities(
            backend_family="memory",
            supports_atomic_replace=False,
            supports_compaction=False,
            supports_snapshot_delta=False,
            supports_remote_event_transport=True,
        )

    def exists(self, path: Path) -> bool:
        return self._key(path) in self._payloads

    def read_text(self, path: Path) -> str:
        return self._payloads[self._key(path)]

    def write_text(self, path: Path, payload: str) -> None:
        self._payloads[self._key(path)] = payload

    @staticmethod
    def _key(path: Path) -> str:
        return str(path)


class SQLiteRuntimeStorageBackend:
    """SQLite-backed storage backend for runtime-usable non-filesystem persistence."""

    _TABLE_NAME = "runtime_storage_payloads"

    def __init__(self, *, db_path: Path, storage_root: Path | None = None) -> None:
        self._db_path = db_path.expanduser().resolve()
        self._storage_root = (
            storage_root.expanduser().resolve() if storage_root is not None else self._db_path.parent
        )
        self._ensure_schema()

    def capabilities(self) -> RuntimeStoreCapabilities:
        return RuntimeStoreCapabilities(
            backend_family="sqlite",
            supports_atomic_replace=True,
            supports_compaction=True,
            supports_snapshot_delta=True,
            supports_remote_event_transport=True,
        )

    def exists(self, path: Path) -> bool:
        keys = self._lookup_keys(path)
        with self._connect() as conn:
            for key in keys:
                row = conn.execute(
                    f"SELECT 1 FROM {self._TABLE_NAME} WHERE payload_key = ? LIMIT 1",
                    (key,),
                ).fetchone()
                if row is not None:
                    return True
        return False

    def read_text(self, path: Path) -> str:
        keys = self._lookup_keys(path)
        with self._connect() as conn:
            for key in keys:
                row = conn.execute(
                    f"SELECT payload_text FROM {self._TABLE_NAME} WHERE payload_key = ?",
                    (key,),
                ).fetchone()
                if row is not None:
                    return row[0]
        raise KeyError(f"No payload stored for path {path!s}")

    def write_text(self, path: Path, payload: str) -> None:
        with self._connect() as conn:
            conn.execute(
                f"""
                INSERT INTO {self._TABLE_NAME} (payload_key, payload_text)
                VALUES (?, ?)
                ON CONFLICT(payload_key) DO UPDATE SET payload_text = excluded.payload_text
                """,
                (self._stable_key(path), payload),
            )
            conn.commit()

    def _connect(self) -> sqlite3.Connection:
        self._db_path.parent.mkdir(parents=True, exist_ok=True)
        conn = sqlite3.connect(self._db_path, timeout=30.0)
        conn.execute("PRAGMA journal_mode=WAL")
        conn.execute("PRAGMA synchronous=NORMAL")
        self._ensure_schema(conn)
        return conn

    def _ensure_schema(self, conn: sqlite3.Connection | None = None) -> None:
        owns_connection = conn is None
        if conn is None:
            self._db_path.parent.mkdir(parents=True, exist_ok=True)
            connection = sqlite3.connect(self._db_path, timeout=30.0)
        else:
            connection = conn
        try:
            connection.execute(
                f"""
                CREATE TABLE IF NOT EXISTS {self._TABLE_NAME} (
                    payload_key TEXT PRIMARY KEY,
                    payload_text TEXT NOT NULL
                )
                """
            )
            connection.commit()
        finally:
            if owns_connection:
                connection.close()

    def _lookup_keys(self, path: Path) -> tuple[str, str]:
        return (self._stable_key(path), self._legacy_key(path))

    def _stable_key(self, path: Path) -> str:
        resolved = path.expanduser().resolve()
        try:
            relative_path = resolved.relative_to(self._storage_root)
        except ValueError as exc:
            raise ValueError(
                f"SQLite runtime storage path {resolved!s} must stay under storage root {self._storage_root!s}"
            ) from exc
        return relative_path.as_posix()

    @staticmethod
    def _legacy_key(path: Path) -> str:
        return str(path.expanduser().resolve())


def _runtime_settings() -> "RuntimeSettings":
    from codex_agno_runtime.config import RuntimeSettings

    return RuntimeSettings()


def _normalized_backend_family(value: str) -> str:
    return value.strip().lower().replace("-", "_")


def select_runtime_storage_backend(
    *,
    backend_family: str | None = None,
    storage_root: Path | None = None,
    sqlite_db_path: Path | None = None,
) -> RuntimeStorageBackend:
    """Select a concrete backend from config or an explicit family override."""

    settings = None
    resolved_family = backend_family
    if resolved_family is None:
        settings = _runtime_settings()
        resolved_family = settings.checkpoint_storage_backend_family
    normalized_family = _normalized_backend_family(resolved_family)

    if normalized_family in {"filesystem", "file"}:
        return FilesystemRuntimeStorageBackend()
    if normalized_family in {"memory", "in_memory", "regression", "regression_double"}:
        return InMemoryRuntimeStorageBackend()
    if normalized_family in {"sqlite", "sqlite3"}:
        if sqlite_db_path is None:
            if settings is None:
                settings = _runtime_settings()
            db_file = settings.checkpoint_storage_db_file
            if storage_root is not None and not db_file.is_absolute():
                sqlite_db_path = (storage_root / db_file).resolve()
            else:
                sqlite_db_path = settings.resolved_checkpoint_storage_db_file
        return SQLiteRuntimeStorageBackend(db_path=sqlite_db_path, storage_root=storage_root)
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
    ) -> None:
        self.data_dir = data_dir
        self.storage_backend = storage_backend or select_runtime_storage_backend(storage_root=data_dir)
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
        self._rust_adapter = RustRouteAdapter(_runtime_settings().codex_home)

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
                storage_backend=self.storage_backend,
            )
            if paths.event_stream_path is not None
            else None
        )
        return RuntimeTraceRecorder(
            output_path=paths.trace_output_path,
            event_sink=event_sink,
            event_bridge=event_bridge,
            storage_backend=self.storage_backend,
            control_plane_descriptor=self._control_plane.trace_service,
        )

    def resolve_transport_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        latest_cursor: TraceReplayCursor | None,
    ) -> RuntimeEventTransport:
        return self._resolve_rust_manifest_payload(
            resolver=self._rust_adapter.describe_transport,
            payload={
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
            },
            model_factory=RuntimeEventTransport.model_validate,
            fallback=lambda: self._fallback_transport_manifest(
                session_id=session_id,
                job_id=job_id,
                latest_cursor=latest_cursor,
            ),
        )

    def resolve_handoff_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        transport: RuntimeEventTransport,
    ) -> RuntimeEventHandoff:
        paths = self.describe_paths()
        return self._resolve_rust_manifest_payload(
            resolver=self._rust_adapter.describe_handoff,
            payload={
                "session_id": session_id,
                "job_id": job_id,
                "transport": transport.model_dump(mode="json"),
                "checkpoint_backend_family": self.storage_capabilities().backend_family,
                "trace_stream_path": str(paths.event_stream_path) if paths.event_stream_path is not None else None,
                "resume_manifest_path": str(paths.resume_manifest_path) if paths.resume_manifest_path is not None else None,
                "control_plane": self._control_plane.model_dump(mode="json"),
            },
            model_factory=RuntimeEventHandoff.model_validate,
            fallback=lambda: self._fallback_handoff_manifest(
                session_id=session_id,
                job_id=job_id,
                transport=transport,
            ),
        )

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
        return self._resolve_rust_manifest_payload(
            resolver=self._rust_adapter.checkpoint_resume_manifest,
            payload={
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
            },
            model_factory=TraceResumeManifest.model_validate,
            fallback=lambda: self._fallback_resume_manifest(
                session_id=session_id,
                job_id=job_id,
                status=status,
                generation=generation,
                latest_cursor=latest_cursor,
                event_transport_path=event_transport_path,
                artifact_paths=artifact_paths,
                parallel_group=parallel_group,
                supervisor_projection=supervisor_projection,
            ),
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
        if self._write_resume_manifest_via_rust(paths.resume_manifest_path, manifest):
            return manifest
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
        if self._write_transport_binding_via_rust(path, projected):
            return path
        payload = projected.model_dump_json(indent=2) + "\n"
        self.storage_backend.write_text(path, payload)
        return path

    def _transport_health_payload(self) -> dict[str, Any]:
        return {
            "backend_family": self._control_plane.backend_family,
            "supports_atomic_replace": self._control_plane.supports_atomic_replace,
            "supports_compaction": self._control_plane.supports_compaction,
            "supports_snapshot_delta": self._control_plane.supports_snapshot_delta,
            "supports_remote_event_transport": self._control_plane.supports_remote_event_transport,
        }

    def _is_filesystem_storage_backend(self) -> bool:
        return isinstance(self.storage_backend, FilesystemRuntimeStorageBackend)

    def _build_transport_binding_write_payload(
        self,
        *,
        path: Path,
        transport: RuntimeEventTransport,
    ) -> dict[str, Any]:
        payload = transport.model_dump(mode="json")
        payload["path"] = str(path)
        return payload

    def _build_resume_manifest_write_payload(
        self,
        *,
        path: Path,
        manifest: TraceResumeManifest,
    ) -> dict[str, Any]:
        payload = manifest.model_dump(mode="json")
        payload["path"] = str(path)
        return payload

    def _write_transport_binding_via_rust(self, path: Path, transport: RuntimeEventTransport) -> bool:
        if not self._is_filesystem_storage_backend():
            return False
        try:
            resolved = self._rust_adapter.write_transport_binding(
                self._build_transport_binding_write_payload(path=path, transport=transport)
            )
        except RuntimeError:
            return False
        return resolved.get("path") == str(path)

    def _write_resume_manifest_via_rust(self, path: Path, manifest: TraceResumeManifest) -> bool:
        if not self._is_filesystem_storage_backend():
            return False
        try:
            resolved = self._rust_adapter.write_checkpoint_resume_manifest(
                self._build_resume_manifest_write_payload(path=path, manifest=manifest)
            )
        except RuntimeError:
            return False
        return resolved.get("path") == str(path)

    def _fallback_transport_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        latest_cursor: TraceReplayCursor | None,
    ) -> RuntimeEventTransport:
        stream_key = job_id or session_id
        return RuntimeEventTransport(
            stream_id=f"stream::{stream_key}",
            session_id=session_id,
            job_id=job_id,
            binding_backend_family=self.storage_capabilities().backend_family,
            binding_artifact_path=(
                str(path)
                if (path := self.transport_binding_path(session_id=session_id, job_id=job_id)) is not None
                else None
            ),
            latest_cursor=latest_cursor,
            control_plane_authority=self._control_plane.trace_service.get("authority"),
            control_plane_role=self._control_plane.trace_service.get("role"),
            control_plane_projection=self._control_plane.trace_service.get("projection"),
            control_plane_delegate_kind=self._control_plane.trace_service.get("delegate_kind"),
            transport_health=self._transport_health_payload(),
        )

    def _fallback_handoff_manifest(
        self,
        *,
        session_id: str,
        job_id: str | None,
        transport: RuntimeEventTransport,
    ) -> RuntimeEventHandoff:
        paths = self.describe_paths()
        return RuntimeEventHandoff(
            stream_id=transport.stream_id,
            session_id=session_id,
            job_id=job_id,
            checkpoint_backend_family=self.storage_capabilities().backend_family,
            trace_stream_path=str(paths.event_stream_path) if paths.event_stream_path is not None else None,
            resume_manifest_path=str(paths.resume_manifest_path) if paths.resume_manifest_path is not None else None,
            control_plane=self._control_plane.model_dump(mode="json"),
            transport=transport,
        )

    def _fallback_resume_manifest(
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
        return TraceResumeManifest(
            session_id=session_id,
            job_id=job_id,
            status=status,
            generation=generation,
            trace_output_path=str(paths.trace_output_path) if paths.trace_output_path is not None else None,
            trace_stream_path=str(paths.event_stream_path) if paths.event_stream_path is not None else None,
            event_transport_path=event_transport_path,
            background_state_path=str(paths.background_state_path),
            latest_cursor=latest_cursor,
            artifact_paths=list(artifact_paths),
            parallel_group=parallel_group,
            supervisor_projection=supervisor_projection,
            control_plane=self._control_plane.model_dump(mode="json"),
        )

    def _resolve_rust_manifest_payload(
        self,
        *,
        resolver: Callable[[dict[str, Any]], dict[str, Any]],
        payload: dict[str, Any],
        model_factory: Callable[[Any], Any],
        fallback: Callable[[], Any],
    ) -> Any:
        try:
            resolved_payload = resolver(payload)
            return model_factory(resolved_payload)
        except (RuntimeError, ValueError):
            return fallback()

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
        `artifacts/current/*` remains the bridge-facing mirror and should stay in
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
