"""Rust-owned helpers for process-external runtime event transport."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping

from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.rust_router import RustRouteAdapter
from codex_agno_runtime.trace import (
    RuntimeEventHandoff,
    RuntimeEventStreamChunk,
    RuntimeEventTransport,
    TraceResumeManifest,
)

RUNTIME_EVENT_ATTACH_DESCRIPTOR_SCHEMA_VERSION = "runtime-event-attach-descriptor-v1"
RUNTIME_EVENT_ATTACH_SOURCE_HANDOFF_METHOD = "describe_runtime_event_handoff"
RUNTIME_EVENT_ATTACH_SOURCE_TRANSPORT_METHOD = "describe_runtime_event_transport"
RUNTIME_EVENT_ATTACH_METHOD = "attach_runtime_event_transport"
RUNTIME_EVENT_ATTACH_SUBSCRIBE_METHOD = "subscribe_attached_runtime_events"
RUNTIME_EVENT_ATTACH_CLEANUP_METHOD = "cleanup_attached_runtime_event_transport"


def _clone_json(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in payload.items()}


def _optional_path(value: Any) -> Path | None:
    if not isinstance(value, str) or not value:
        return None
    return Path(value).expanduser().resolve()


def _unwrap_rust_attach_error(exc: RuntimeError) -> ValueError | None:
    prefix = "Rust attached runtime event transport failed: "
    message = str(exc)
    if not message.startswith(prefix):
        return None
    detail = message[len(prefix) :].strip()
    if detail.startswith('Error: "') and detail.endswith('"'):
        detail = detail[len('Error: "') : -1]
    detail = detail.replace('\\"', "'")
    return ValueError(detail)


class ExternalRuntimeEventTransportBridge:
    """Thin Python projection over the Rust-owned attached runtime transport lane."""

    def __init__(
        self,
        *,
        adapter: RustRouteAdapter,
        payload: Mapping[str, Any],
    ) -> None:
        self._adapter = adapter
        self._payload = _clone_json(payload)
        self.transport = RuntimeEventTransport.model_validate(self._payload["transport"])
        self.handoff = (
            RuntimeEventHandoff.model_validate(self._payload["handoff"])
            if self._payload.get("handoff") is not None
            else None
        )
        self.resume_manifest = (
            TraceResumeManifest.model_validate(self._payload["resume_manifest"])
            if self._payload.get("resume_manifest") is not None
            else None
        )
        self.binding_artifact_path = _optional_path(self._payload.get("binding_artifact_path"))
        self.handoff_path = _optional_path(self._payload.get("handoff_path"))
        self.resume_manifest_path = _optional_path(self._payload.get("resume_manifest_path"))
        self.binding_artifact_source = self.attach_descriptor().get("resolution", {}).get("binding_artifact_path")
        self.handoff_source = self.attach_descriptor().get("resolution", {}).get("handoff_path")
        self.resume_manifest_source = self.attach_descriptor().get("resolution", {}).get("resume_manifest_path")
        self.storage_backend = None

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

        settings = RuntimeSettings()
        adapter = RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        request: dict[str, Any] = {}
        if attach_descriptor is not None:
            request["attach_descriptor"] = dict(attach_descriptor)
        if binding_artifact_path is not None:
            request["binding_artifact_path"] = binding_artifact_path
        if handoff_path is not None:
            request["handoff_path"] = handoff_path
        if resume_manifest_path is not None:
            request["resume_manifest_path"] = resume_manifest_path
        try:
            payload = adapter.attach_runtime_event_transport(request)
        except RuntimeError as exc:
            attach_error = _unwrap_rust_attach_error(exc)
            if attach_error is not None:
                raise attach_error from exc
            raise
        return cls(adapter=adapter, payload=payload)

    def describe(self) -> dict[str, Any]:
        """Describe the resolved process-external attach bridge."""

        return _clone_json(self._payload)

    def attach_descriptor(self) -> dict[str, Any]:
        """Return a stable attach descriptor that external consumers can persist and replay."""

        payload = self._payload.get("attach_descriptor")
        if not isinstance(payload, Mapping):
            raise ValueError("External runtime event attach payload is missing attach_descriptor.")
        return dict(payload)

    def subscribe(
        self,
        *,
        after_event_id: str | None = None,
        limit: int | None = 100,
        heartbeat: bool = False,
    ) -> RuntimeEventStreamChunk:
        """Replay a stream window from persisted artifacts in a new process."""

        payload = self._adapter.subscribe_attached_runtime_events(
            {
                "attach_descriptor": self.attach_descriptor(),
                "after_event_id": after_event_id,
                "limit": limit,
                "heartbeat": heartbeat,
            }
        )
        return RuntimeEventStreamChunk.model_validate(payload)

    def cleanup(self) -> dict[str, Any]:
        """Report cleanup semantics for artifact-backed external attach."""

        return {
            "cleanup_semantics": "no_persisted_state",
            "cleanup_preserves_replay": True,
            "cleanup_method": RUNTIME_EVENT_ATTACH_CLEANUP_METHOD,
            "binding_artifact_path": str(self.binding_artifact_path) if self.binding_artifact_path is not None else None,
            "trace_stream_path": self._payload.get("trace_stream_path"),
        }
