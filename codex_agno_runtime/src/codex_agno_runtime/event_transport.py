"""Artifact-backed helpers for process-external runtime event transport."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from codex_agno_runtime.checkpoint_store import SQLiteRuntimeStorageBackend
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.trace import (
    InMemoryRuntimeEventBridge,
    JsonlTraceEventSink,
    RuntimeEventHandoff,
    RuntimeEventStreamChunk,
    RuntimeEventTransport,
    TraceResumeManifest,
)


class ExternalRuntimeEventTransportBridge:
    """Reattach to a runtime event stream from persisted transport artifacts."""

    def __init__(
        self,
        *,
        transport: RuntimeEventTransport,
        handoff: RuntimeEventHandoff | None,
        resume_manifest: TraceResumeManifest | None,
        binding_artifact_path: Path | None,
        handoff_path: Path | None,
        resume_manifest_path: Path | None,
        storage_backend: SQLiteRuntimeStorageBackend | None,
    ) -> None:
        self.transport = transport
        self.handoff = handoff
        self.resume_manifest = resume_manifest
        self.binding_artifact_path = binding_artifact_path
        self.handoff_path = handoff_path
        self.resume_manifest_path = resume_manifest_path
        self.storage_backend = storage_backend

    @classmethod
    def attach(
        cls,
        *,
        binding_artifact_path: str | None = None,
        handoff_path: str | None = None,
        resume_manifest_path: str | None = None,
    ) -> ExternalRuntimeEventTransportBridge:
        """Resolve a process-external attach bridge from persisted artifacts."""

        if binding_artifact_path is None and handoff_path is None and resume_manifest_path is None:
            raise ValueError(
                "External runtime event attach requires a binding artifact, handoff manifest, or resume manifest path."
            )

        binding_path = Path(binding_artifact_path).expanduser().resolve() if binding_artifact_path is not None else None
        handoff_file = Path(handoff_path).expanduser().resolve() if handoff_path is not None else None
        resume_file = Path(resume_manifest_path).expanduser().resolve() if resume_manifest_path is not None else None

        storage_backend = cls._resolve_storage_backend(binding_path, handoff_file, resume_file)
        handoff = cls._load_handoff(handoff_file, storage_backend=storage_backend)
        resume_manifest = cls._load_resume_manifest(resume_file, storage_backend=storage_backend)

        if resume_manifest is None and handoff is not None and handoff.resume_manifest_path is not None:
            inferred_resume = Path(handoff.resume_manifest_path).expanduser().resolve()
            if cls._artifact_exists(inferred_resume, storage_backend=storage_backend):
                resume_manifest = cls._load_resume_manifest(inferred_resume, storage_backend=storage_backend)
                resume_file = inferred_resume

        transport_path = binding_path
        if transport_path is None and resume_manifest is not None and resume_manifest.event_transport_path is not None:
            candidate = Path(resume_manifest.event_transport_path).expanduser().resolve()
            if cls._artifact_exists(candidate, storage_backend=storage_backend):
                transport_path = candidate
        if transport_path is None and handoff is not None and handoff.transport.binding_artifact_path is not None:
            candidate = Path(handoff.transport.binding_artifact_path).expanduser().resolve()
            if cls._artifact_exists(candidate, storage_backend=storage_backend):
                transport_path = candidate

        if transport_path is None and handoff is None:
            raise ValueError(
                "External runtime event attach could not resolve a transport binding artifact from the provided manifests."
            )

        transport = (
            cls._load_transport(transport_path, storage_backend=storage_backend)
            if transport_path is not None
            else handoff.transport
        )
        if transport is None:
            raise ValueError("External runtime event attach could not load a transport descriptor.")

        if resume_manifest is None and transport_path is not None:
            inferred_resume = cls._infer_resume_manifest_path(transport_path)
            if inferred_resume is not None and cls._artifact_exists(inferred_resume, storage_backend=storage_backend):
                resume_manifest = cls._load_resume_manifest(inferred_resume, storage_backend=storage_backend)
                resume_file = inferred_resume

        cls._validate_alignment(transport=transport, handoff=handoff, resume_manifest=resume_manifest)
        return cls(
            transport=transport,
            handoff=handoff,
            resume_manifest=resume_manifest,
            binding_artifact_path=transport_path,
            handoff_path=handoff_file,
            resume_manifest_path=resume_file,
            storage_backend=storage_backend,
        )

    def describe(self) -> dict[str, Any]:
        """Describe the resolved process-external attach bridge."""

        return {
            "attach_mode": "process_external_artifact_replay",
            "artifact_backend_family": self.transport.binding_backend_family,
            "transport": self.transport.model_dump(mode="json"),
            "handoff": self.handoff.model_dump(mode="json") if self.handoff is not None else None,
            "resume_manifest": (
                self.resume_manifest.model_dump(mode="json") if self.resume_manifest is not None else None
            ),
            "binding_artifact_path": str(self.binding_artifact_path) if self.binding_artifact_path is not None else None,
            "handoff_path": str(self.handoff_path) if self.handoff_path is not None else None,
            "resume_manifest_path": (
                str(self.resume_manifest_path) if self.resume_manifest_path is not None else None
            ),
            "trace_stream_path": self._resolved_trace_stream_path(),
            "replay_supported": True,
            "cleanup_semantics": "no_persisted_state",
            "cleanup_preserves_replay": True,
        }

    def subscribe(
        self,
        *,
        after_event_id: str | None = None,
        limit: int | None = 100,
        heartbeat: bool = False,
    ) -> RuntimeEventStreamChunk:
        """Replay a stream window from persisted artifacts in a new process."""

        trace_stream_path = self._required_trace_stream_path()
        bridge = InMemoryRuntimeEventBridge()
        bridge.seed(
            JsonlTraceEventSink(
                trace_stream_path,
                storage_backend=self.storage_backend,
            ).read_events()
        )
        return bridge.subscribe(
            session_id=self.transport.session_id,
            job_id=self.transport.job_id,
            after_event_id=after_event_id,
            limit=limit,
            heartbeat=heartbeat,
        )

    def cleanup(self) -> dict[str, Any]:
        """Report cleanup semantics for artifact-backed external attach."""

        return {
            "cleanup_semantics": "no_persisted_state",
            "cleanup_preserves_replay": True,
            "binding_artifact_path": str(self.binding_artifact_path) if self.binding_artifact_path is not None else None,
            "trace_stream_path": self._resolved_trace_stream_path(),
        }

    def _resolved_trace_stream_path(self) -> str | None:
        path = self._trace_stream_path()
        return str(path) if path is not None else None

    def _required_trace_stream_path(self) -> Path:
        path = self._trace_stream_path()
        if path is None:
            raise ValueError(
                "External runtime event replay requires a handoff or resume manifest with trace_stream_path, or a filesystem binding artifact adjacent to TRACE_EVENTS.jsonl."
            )
        if not self._artifact_exists(path, storage_backend=self.storage_backend):
            raise ValueError(f"External runtime event replay trace stream not found: {path}")
        return path

    def _trace_stream_path(self) -> Path | None:
        if self.handoff is not None and self.handoff.trace_stream_path is not None:
            return Path(self.handoff.trace_stream_path).expanduser().resolve()
        if self.resume_manifest is not None and self.resume_manifest.trace_stream_path is not None:
            return Path(self.resume_manifest.trace_stream_path).expanduser().resolve()
        if self.binding_artifact_path is not None:
            candidates = [
                self.binding_artifact_path.parent.parent / "TRACE_EVENTS.jsonl",
                self.binding_artifact_path.parent.parent.parent / "TRACE_EVENTS.jsonl",
            ]
            for candidate in candidates:
                resolved = candidate.resolve()
                if self._artifact_exists(resolved, storage_backend=self.storage_backend):
                    return resolved
            return candidates[0].resolve()
        return None

    @staticmethod
    def _load_transport(
        path: Path | None,
        storage_backend: SQLiteRuntimeStorageBackend | None,
    ) -> RuntimeEventTransport | None:
        if path is None or not ExternalRuntimeEventTransportBridge._artifact_exists(path, storage_backend=storage_backend):
            return None
        return RuntimeEventTransport.model_validate_json(
            ExternalRuntimeEventTransportBridge._read_text(path, storage_backend=storage_backend)
        )

    @staticmethod
    def _load_handoff(
        path: Path | None,
        storage_backend: SQLiteRuntimeStorageBackend | None,
    ) -> RuntimeEventHandoff | None:
        if path is None or not ExternalRuntimeEventTransportBridge._artifact_exists(path, storage_backend=storage_backend):
            return None
        return RuntimeEventHandoff.model_validate_json(
            ExternalRuntimeEventTransportBridge._read_text(path, storage_backend=storage_backend)
        )

    @staticmethod
    def _load_resume_manifest(
        path: Path | None,
        storage_backend: SQLiteRuntimeStorageBackend | None,
    ) -> TraceResumeManifest | None:
        if path is None or not ExternalRuntimeEventTransportBridge._artifact_exists(path, storage_backend=storage_backend):
            return None
        return TraceResumeManifest.model_validate_json(
            ExternalRuntimeEventTransportBridge._read_text(path, storage_backend=storage_backend)
        )

    @staticmethod
    def _infer_resume_manifest_path(binding_artifact_path: Path) -> Path | None:
        candidates = [
            binding_artifact_path.parent.parent / "TRACE_RESUME_MANIFEST.json",
            binding_artifact_path.parent.parent.parent / "TRACE_RESUME_MANIFEST.json",
        ]
        for candidate in candidates:
            resolved = candidate.resolve()
            if resolved.exists():
                return resolved
        return candidates[0].resolve()

    @staticmethod
    def _artifact_exists(path: Path, *, storage_backend: SQLiteRuntimeStorageBackend | None) -> bool:
        if path.exists():
            return True
        if storage_backend is not None:
            try:
                return storage_backend.exists(path)
            except ValueError:
                return False
        return False

    @staticmethod
    def _read_text(path: Path, *, storage_backend: SQLiteRuntimeStorageBackend | None) -> str:
        if path.exists():
            return path.read_text(encoding="utf-8")
        if storage_backend is None:
            raise FileNotFoundError(path)
        return storage_backend.read_text(path)

    @classmethod
    def _resolve_storage_backend(
        cls,
        *paths: Path | None,
    ) -> SQLiteRuntimeStorageBackend | None:
        concrete_paths = [path for path in paths if path is not None]
        if not concrete_paths:
            return None
        if any(path.exists() for path in concrete_paths):
            return None

        settings = RuntimeSettings()
        db_name_candidates = [settings.checkpoint_storage_db_file.name, "runtime_checkpoint_store.sqlite3"]
        seen_db_names: set[str] = set()
        ordered_db_names: list[str] = []
        for candidate in db_name_candidates:
            if candidate in seen_db_names:
                continue
            seen_db_names.add(candidate)
            ordered_db_names.append(candidate)

        roots: list[Path] = []
        seen_roots: set[Path] = set()
        for path in concrete_paths:
            candidates = []
            if path.parent.name == "runtime_event_transports":
                candidates.append(path.parent.parent)
            candidates.append(path.parent)
            for candidate_root in candidates:
                resolved_root = candidate_root.resolve()
                if resolved_root in seen_roots:
                    continue
                seen_roots.add(resolved_root)
                roots.append(resolved_root)

        absolute_db_path = (
            settings.checkpoint_storage_db_file.expanduser().resolve()
            if settings.checkpoint_storage_db_file.is_absolute()
            else None
        )
        if absolute_db_path is not None and absolute_db_path.exists():
            for root in roots:
                try:
                    backend = SQLiteRuntimeStorageBackend(db_path=absolute_db_path, storage_root=root)
                except ValueError:
                    continue
                if any(backend.exists(path) for path in concrete_paths):
                    return backend

        for root in roots:
            for db_name in ordered_db_names:
                db_path = (root / db_name).resolve()
                if not db_path.exists():
                    continue
                try:
                    backend = SQLiteRuntimeStorageBackend(db_path=db_path, storage_root=root)
                except ValueError:
                    continue
                if any(backend.exists(path) for path in concrete_paths):
                    return backend

        return None

    @staticmethod
    def _validate_alignment(
        *,
        transport: RuntimeEventTransport,
        handoff: RuntimeEventHandoff | None,
        resume_manifest: TraceResumeManifest | None,
    ) -> None:
        if handoff is not None:
            if handoff.stream_id != transport.stream_id:
                raise ValueError("External runtime event attach rejected mismatched transport/handoff stream ids.")
            if handoff.session_id != transport.session_id or handoff.job_id != transport.job_id:
                raise ValueError("External runtime event attach rejected mismatched transport/handoff stream scope.")
        if resume_manifest is not None:
            if resume_manifest.session_id != transport.session_id or resume_manifest.job_id != transport.job_id:
                raise ValueError("External runtime event attach rejected mismatched transport/resume stream scope.")
