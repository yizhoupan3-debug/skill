"""Artifact-backed helpers for process-external runtime event transport."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping

from codex_agno_runtime.checkpoint_store import SQLiteRuntimeStorageBackend
from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.trace import (
    RuntimeEventHandoff,
    RuntimeEventStreamChunk,
    RuntimeEventTransport,
    RuntimeTraceRecorder,
    TraceResumeManifest,
)

RUNTIME_EVENT_ATTACH_DESCRIPTOR_SCHEMA_VERSION = "runtime-event-attach-descriptor-v1"
RUNTIME_EVENT_ATTACH_SOURCE_HANDOFF_METHOD = "describe_runtime_event_handoff"
RUNTIME_EVENT_ATTACH_SOURCE_TRANSPORT_METHOD = "describe_runtime_event_transport"
RUNTIME_EVENT_ATTACH_METHOD = "attach_runtime_event_transport"
RUNTIME_EVENT_ATTACH_SUBSCRIBE_METHOD = "subscribe_attached_runtime_events"
RUNTIME_EVENT_ATTACH_CLEANUP_METHOD = "cleanup_attached_runtime_event_transport"


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
        binding_artifact_source: str | None = None,
        handoff_source: str | None = None,
        resume_manifest_source: str | None = None,
    ) -> None:
        self.transport = transport
        self.handoff = handoff
        self.resume_manifest = resume_manifest
        self.binding_artifact_path = binding_artifact_path
        self.handoff_path = handoff_path
        self.resume_manifest_path = resume_manifest_path
        self.storage_backend = storage_backend
        self.binding_artifact_source = binding_artifact_source
        self.handoff_source = handoff_source
        self.resume_manifest_source = resume_manifest_source

    @classmethod
    def attach(
        cls,
        *,
        attach_descriptor: Mapping[str, Any] | None = None,
        binding_artifact_path: str | None = None,
        handoff_path: str | None = None,
        resume_manifest_path: str | None = None,
    ) -> ExternalRuntimeEventTransportBridge:
        """Resolve a process-external attach bridge from persisted artifacts."""

        binding_artifact_path, handoff_path, resume_manifest_path = cls._normalize_attach_request(
            attach_descriptor=attach_descriptor,
            binding_artifact_path=binding_artifact_path,
            handoff_path=handoff_path,
            resume_manifest_path=resume_manifest_path,
        )
        if binding_artifact_path is None and handoff_path is None and resume_manifest_path is None:
            raise ValueError(
                "External runtime event attach requires a binding artifact, handoff manifest, or resume manifest path."
            )

        binding_path = Path(binding_artifact_path).expanduser().resolve() if binding_artifact_path is not None else None
        handoff_file = Path(handoff_path).expanduser().resolve() if handoff_path is not None else None
        resume_file = Path(resume_manifest_path).expanduser().resolve() if resume_manifest_path is not None else None
        binding_source = "explicit_request" if binding_path is not None else None
        handoff_source = "explicit_request" if handoff_file is not None else None
        resume_source = "explicit_request" if resume_file is not None else None

        storage_backend = cls._resolve_storage_backend(binding_path, handoff_file, resume_file)
        cls._require_requested_artifact(
            binding_path,
            storage_backend=storage_backend,
            field_name="binding_artifact_path",
        )
        cls._require_requested_artifact(
            handoff_file,
            storage_backend=storage_backend,
            field_name="handoff_path",
        )
        cls._require_requested_artifact(
            resume_file,
            storage_backend=storage_backend,
            field_name="resume_manifest_path",
        )
        handoff = cls._load_handoff(handoff_file, storage_backend=storage_backend)
        resume_manifest = cls._load_resume_manifest(resume_file, storage_backend=storage_backend)

        if resume_manifest is None and handoff is not None and handoff.resume_manifest_path is not None:
            inferred_resume = Path(handoff.resume_manifest_path).expanduser().resolve()
            if cls._artifact_exists(inferred_resume, storage_backend=storage_backend):
                resume_manifest = cls._load_resume_manifest(inferred_resume, storage_backend=storage_backend)
                resume_file = inferred_resume
                resume_source = "handoff_manifest"

        transport_path = binding_path
        if transport_path is None and resume_manifest is not None and resume_manifest.event_transport_path is not None:
            candidate = Path(resume_manifest.event_transport_path).expanduser().resolve()
            if cls._artifact_exists(candidate, storage_backend=storage_backend):
                transport_path = candidate
                binding_source = "resume_manifest"
        if transport_path is None and handoff is not None and handoff.transport.binding_artifact_path is not None:
            candidate = Path(handoff.transport.binding_artifact_path).expanduser().resolve()
            if cls._artifact_exists(candidate, storage_backend=storage_backend):
                transport_path = candidate
                binding_source = "handoff_transport"

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

        cls._validate_alignment(
            transport=transport,
            handoff=handoff,
            resume_manifest=resume_manifest,
            binding_artifact_path=transport_path,
            resume_manifest_path=resume_file,
            storage_backend=storage_backend,
        )
        bridge = cls(
            transport=transport,
            handoff=handoff,
            resume_manifest=resume_manifest,
            binding_artifact_path=transport_path,
            handoff_path=handoff_file,
            resume_manifest_path=resume_file,
            storage_backend=storage_backend,
            binding_artifact_source=binding_source,
            handoff_source=handoff_source,
            resume_manifest_source=resume_source,
        )
        bridge._required_trace_stream_path()
        return bridge

    def describe(self) -> dict[str, Any]:
        """Describe the resolved process-external attach bridge."""

        return {
            "attach_mode": "process_external_artifact_replay",
            "artifact_backend_family": self.transport.binding_backend_family,
            "source_handoff_method": RUNTIME_EVENT_ATTACH_SOURCE_HANDOFF_METHOD,
            "source_transport_method": RUNTIME_EVENT_ATTACH_SOURCE_TRANSPORT_METHOD,
            "attach_method": RUNTIME_EVENT_ATTACH_METHOD,
            "subscribe_method": RUNTIME_EVENT_ATTACH_SUBSCRIBE_METHOD,
            "cleanup_method": RUNTIME_EVENT_ATTACH_CLEANUP_METHOD,
            "resume_mode": self.transport.resume_mode,
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
            "attach_descriptor": self.attach_descriptor(),
        }

    def attach_descriptor(self) -> dict[str, Any]:
        """Return a stable attach descriptor that external consumers can persist and replay."""

        trace_stream_path, trace_stream_source = self._trace_stream_resolution()
        return {
            "schema_version": RUNTIME_EVENT_ATTACH_DESCRIPTOR_SCHEMA_VERSION,
            "attach_mode": "process_external_artifact_replay",
            "artifact_backend_family": self.transport.binding_backend_family,
            "source_transport_method": RUNTIME_EVENT_ATTACH_SOURCE_TRANSPORT_METHOD,
            "source_handoff_method": RUNTIME_EVENT_ATTACH_SOURCE_HANDOFF_METHOD,
            "attach_method": RUNTIME_EVENT_ATTACH_METHOD,
            "subscribe_method": RUNTIME_EVENT_ATTACH_SUBSCRIBE_METHOD,
            "cleanup_method": RUNTIME_EVENT_ATTACH_CLEANUP_METHOD,
            "resume_mode": self.transport.resume_mode,
            "cleanup_semantics": "no_persisted_state",
            "attach_capabilities": {
                "artifact_replay": True,
                "live_remote_stream": False,
                "cleanup_preserves_replay": True,
            },
            "recommended_entrypoint": RUNTIME_EVENT_ATTACH_SOURCE_HANDOFF_METHOD,
            "requested_artifacts": {
                "binding_artifact_path": str(self.binding_artifact_path) if self.binding_artifact_path is not None else None,
                "handoff_path": str(self.handoff_path) if self.handoff_path is not None else None,
                "resume_manifest_path": (
                    str(self.resume_manifest_path) if self.resume_manifest_path is not None else None
                ),
            },
            "resolved_artifacts": {
                "binding_artifact_path": str(self.binding_artifact_path) if self.binding_artifact_path is not None else None,
                "handoff_path": str(self.handoff_path) if self.handoff_path is not None else None,
                "resume_manifest_path": (
                    str(self.resume_manifest_path) if self.resume_manifest_path is not None else None
                ),
                "trace_stream_path": str(trace_stream_path) if trace_stream_path is not None else None,
            },
            "resolution": {
                "binding_artifact_path": self.binding_artifact_source,
                "handoff_path": self.handoff_source,
                "resume_manifest_path": self.resume_manifest_source,
                "trace_stream_path": trace_stream_source,
            },
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
        recorder = RuntimeTraceRecorder(
            event_stream_path=trace_stream_path,
            storage_backend=self.storage_backend,
        )
        return recorder.subscribe_chunk(
            session_id=self.transport.session_id,
            job_id=self.transport.job_id,
            after_event_id=after_event_id,
            limit=limit,
            heartbeat=heartbeat,
        )

    def cleanup(self) -> dict[str, Any]:
        """Report cleanup semantics for artifact-backed external attach."""

        trace_stream_path, _ = self._trace_stream_resolution()
        return {
            "cleanup_semantics": "no_persisted_state",
            "cleanup_preserves_replay": True,
            "cleanup_method": RUNTIME_EVENT_ATTACH_CLEANUP_METHOD,
            "binding_artifact_path": str(self.binding_artifact_path) if self.binding_artifact_path is not None else None,
            "trace_stream_path": str(trace_stream_path) if trace_stream_path is not None else None,
        }

    def _resolved_trace_stream_path(self) -> str | None:
        path, _ = self._trace_stream_resolution()
        return str(path) if path is not None else None

    def _required_trace_stream_path(self) -> Path:
        path, _ = self._trace_stream_resolution()
        if path is None:
            raise ValueError(
                "External runtime event replay requires a handoff or resume manifest with trace_stream_path, or a filesystem binding artifact adjacent to TRACE_EVENTS.jsonl."
            )
        if not self._artifact_exists(path, storage_backend=self.storage_backend):
            raise ValueError(f"External runtime event replay trace stream not found: {path}")
        return path

    def _trace_stream_resolution(self) -> tuple[Path | None, str | None]:
        if self.handoff is not None and self.handoff.trace_stream_path is not None:
            return Path(self.handoff.trace_stream_path).expanduser().resolve(), "handoff_manifest"
        if self.resume_manifest is not None and self.resume_manifest.trace_stream_path is not None:
            return Path(self.resume_manifest.trace_stream_path).expanduser().resolve(), "resume_manifest"
        if self.binding_artifact_path is not None:
            candidates = [
                self.binding_artifact_path.parent.parent / "TRACE_EVENTS.jsonl",
                self.binding_artifact_path.parent.parent.parent / "TRACE_EVENTS.jsonl",
            ]
            for candidate in candidates:
                resolved = candidate.resolve()
                if self._artifact_exists(resolved, storage_backend=self.storage_backend):
                    return resolved, "binding_artifact_adjacency"
        return None, None

    @staticmethod
    def _infer_trace_stream_from_binding_artifact(
        binding_artifact_path: Path | None,
        *,
        storage_backend: SQLiteRuntimeStorageBackend | None,
    ) -> Path | None:
        if binding_artifact_path is None:
            return None
        candidates = [
            binding_artifact_path.parent.parent / "TRACE_EVENTS.jsonl",
            binding_artifact_path.parent.parent.parent / "TRACE_EVENTS.jsonl",
        ]
        for candidate in candidates:
            resolved = candidate.resolve()
            if ExternalRuntimeEventTransportBridge._artifact_exists(resolved, storage_backend=storage_backend):
                return resolved
        return None

    @classmethod
    def _normalize_attach_request(
        cls,
        *,
        attach_descriptor: Mapping[str, Any] | None,
        binding_artifact_path: str | None,
        handoff_path: str | None,
        resume_manifest_path: str | None,
    ) -> tuple[str | None, str | None, str | None]:
        if attach_descriptor is None:
            return binding_artifact_path, handoff_path, resume_manifest_path
        if not isinstance(attach_descriptor, Mapping):
            raise ValueError("External runtime event attach descriptor must be a mapping.")
        schema_version = attach_descriptor.get("schema_version")
        if schema_version is not None and schema_version != RUNTIME_EVENT_ATTACH_DESCRIPTOR_SCHEMA_VERSION:
            raise ValueError(f"Unsupported runtime event attach descriptor schema: {schema_version!r}")
        attach_mode = attach_descriptor.get("attach_mode")
        if attach_mode is not None and attach_mode != "process_external_artifact_replay":
            raise ValueError(f"Unsupported runtime event attach mode: {attach_mode!r}")

        attach_capabilities = cls._descriptor_mapping(attach_descriptor, "attach_capabilities")
        if attach_capabilities is not None:
            if attach_capabilities.get("artifact_replay") is not True:
                raise ValueError(
                    "External runtime event attach descriptor must advertise attach_capabilities.artifact_replay=True."
                )
            if attach_capabilities.get("live_remote_stream") not in (None, False):
                raise ValueError(
                    "External runtime event attach descriptor must advertise attach_capabilities.live_remote_stream=False."
                )
            if attach_capabilities.get("cleanup_preserves_replay") not in (None, True):
                raise ValueError(
                    "External runtime event attach descriptor must advertise attach_capabilities.cleanup_preserves_replay=True."
                )

        cls._descriptor_mapping(attach_descriptor, "requested_artifacts")
        cls._descriptor_mapping(attach_descriptor, "resolution")
        resolved_mapping = cls._descriptor_mapping(attach_descriptor, "resolved_artifacts") or attach_descriptor
        descriptor_binding = cls._mapping_string(resolved_mapping, "binding_artifact_path")
        descriptor_handoff = cls._mapping_string(resolved_mapping, "handoff_path")
        descriptor_resume = cls._mapping_string(resolved_mapping, "resume_manifest_path")

        return (
            cls._merge_attach_path(
                explicit_value=binding_artifact_path,
                descriptor_value=descriptor_binding,
                field_name="binding_artifact_path",
            ),
            cls._merge_attach_path(
                explicit_value=handoff_path,
                descriptor_value=descriptor_handoff,
                field_name="handoff_path",
            ),
            cls._merge_attach_path(
                explicit_value=resume_manifest_path,
                descriptor_value=descriptor_resume,
                field_name="resume_manifest_path",
            ),
        )

    @staticmethod
    def _descriptor_mapping(
        attach_descriptor: Mapping[str, Any],
        field_name: str,
    ) -> Mapping[str, Any] | None:
        value = attach_descriptor.get(field_name)
        if value is None:
            return None
        if isinstance(value, Mapping):
            return value
        raise ValueError(f"External runtime event attach descriptor field {field_name!r} must be a mapping.")

    @staticmethod
    def _mapping_string(mapping: Mapping[str, Any], field_name: str) -> str | None:
        value = mapping.get(field_name)
        if value is None:
            return None
        if isinstance(value, str):
            return value
        raise ValueError(f"External runtime event attach descriptor field {field_name!r} must be a string.")

    @staticmethod
    def _merge_attach_path(
        *,
        explicit_value: str | None,
        descriptor_value: str | None,
        field_name: str,
    ) -> str | None:
        if explicit_value is None:
            return descriptor_value
        if descriptor_value is None or descriptor_value == explicit_value:
            return explicit_value
        raise ValueError(
            f"External runtime event attach received conflicting {field_name!r} values between direct args and attach_descriptor."
        )

    @staticmethod
    def _require_requested_artifact(
        path: Path | None,
        *,
        storage_backend: SQLiteRuntimeStorageBackend | None,
        field_name: str,
    ) -> None:
        if path is None:
            return
        if ExternalRuntimeEventTransportBridge._artifact_exists(path, storage_backend=storage_backend):
            return
        raise ValueError(f"External runtime event attach requested {field_name!r} that does not exist: {path}")

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
        binding_artifact_path: Path | None,
        resume_manifest_path: Path | None,
        storage_backend: SQLiteRuntimeStorageBackend | None,
    ) -> None:
        if handoff is not None:
            if handoff.stream_id != transport.stream_id:
                raise ValueError("External runtime event attach rejected mismatched transport/handoff stream ids.")
            if handoff.session_id != transport.session_id or handoff.job_id != transport.job_id:
                raise ValueError("External runtime event attach rejected mismatched transport/handoff stream scope.")
            if (
                binding_artifact_path is not None
                and handoff.transport.binding_artifact_path is not None
                and Path(handoff.transport.binding_artifact_path).expanduser().resolve() != binding_artifact_path
            ):
                raise ValueError(
                    "External runtime event attach rejected mismatched transport/handoff binding artifact paths."
                )
            if (
                resume_manifest_path is not None
                and handoff.resume_manifest_path is not None
                and Path(handoff.resume_manifest_path).expanduser().resolve() != resume_manifest_path
            ):
                raise ValueError(
                    "External runtime event attach rejected mismatched handoff/resume manifest paths."
                )
        if resume_manifest is not None:
            if resume_manifest.session_id != transport.session_id or resume_manifest.job_id != transport.job_id:
                raise ValueError("External runtime event attach rejected mismatched transport/resume stream scope.")
            if (
                binding_artifact_path is not None
                and resume_manifest.event_transport_path is not None
                and Path(resume_manifest.event_transport_path).expanduser().resolve() != binding_artifact_path
            ):
                raise ValueError(
                    "External runtime event attach rejected mismatched transport/resume binding artifact paths."
                )
            if (
                handoff is not None
                and handoff.trace_stream_path is not None
                and resume_manifest.trace_stream_path is not None
                and Path(handoff.trace_stream_path).expanduser().resolve()
                != Path(resume_manifest.trace_stream_path).expanduser().resolve()
            ):
                raise ValueError("External runtime event attach rejected mismatched handoff/resume trace stream paths.")
        binding_trace_stream_path = ExternalRuntimeEventTransportBridge._infer_trace_stream_from_binding_artifact(
            binding_artifact_path,
            storage_backend=storage_backend,
        )
        if (
            binding_trace_stream_path is not None
            and handoff is not None
            and handoff.trace_stream_path is not None
            and Path(handoff.trace_stream_path).expanduser().resolve() != binding_trace_stream_path
        ):
            raise ValueError(
                "External runtime event attach rejected mismatched binding/handoff trace stream paths."
            )
        if (
            binding_trace_stream_path is not None
            and resume_manifest is not None
            and resume_manifest.trace_stream_path is not None
            and Path(resume_manifest.trace_stream_path).expanduser().resolve() != binding_trace_stream_path
        ):
            raise ValueError(
                "External runtime event attach rejected mismatched binding/resume trace stream paths."
            )
