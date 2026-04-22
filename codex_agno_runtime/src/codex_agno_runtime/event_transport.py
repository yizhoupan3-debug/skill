"""Rust-owned helpers for process-external runtime event transport."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping
from typing import Literal

from pydantic import BaseModel, ConfigDict, ValidationError

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


class _RuntimeEventAttachDescriptorArtifacts(BaseModel):
    """Optional artifact path bundle carried by attach descriptors."""

    model_config = ConfigDict(extra="allow")

    binding_artifact_path: str | None = None
    handoff_path: str | None = None
    resume_manifest_path: str | None = None
    trace_stream_path: str | None = None


class _RuntimeEventAttachDescriptorCapabilities(BaseModel):
    """Optional capability bits carried by attach descriptors."""

    model_config = ConfigDict(extra="allow")

    artifact_replay: bool | None = None
    live_remote_stream: bool | None = None
    cleanup_preserves_replay: bool | None = None


class RuntimeEventAttachDescriptor(BaseModel):
    """Single source-of-truth schema for process-external runtime attach descriptors."""

    model_config = ConfigDict(extra="allow")

    schema_version: Literal[RUNTIME_EVENT_ATTACH_DESCRIPTOR_SCHEMA_VERSION]
    attach_mode: str | None = None
    artifact_backend_family: str | None = None
    source_transport_method: str | None = None
    source_handoff_method: str | None = None
    attach_method: str | None = None
    subscribe_method: str | None = None
    cleanup_method: str | None = None
    resume_mode: str | None = None
    cleanup_semantics: str | None = None
    recommended_entrypoint: str | None = None
    attach_capabilities: _RuntimeEventAttachDescriptorCapabilities | None = None
    requested_artifacts: _RuntimeEventAttachDescriptorArtifacts | None = None
    resolved_artifacts: _RuntimeEventAttachDescriptorArtifacts | None = None
    resolution: _RuntimeEventAttachDescriptorArtifacts | None = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeEventAttachDescriptor":
        """Validate and normalize a raw attach descriptor payload."""

        try:
            return cls.model_validate(payload)
        except ValidationError as exc:
            errors = exc.errors()
            if any(err.get("loc") == ("schema_version",) for err in errors):
                raise ValueError(
                    "External runtime event attach payload returned an unknown attach_descriptor schema_version."
                ) from exc
            raise ValueError("External runtime event attach payload returned an invalid attach_descriptor.") from exc


def _clone_json(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in payload.items()}


def _build_external_runtime_attach_request(
    *,
    attach_descriptor: Mapping[str, Any] | None = None,
    binding_artifact_path: str | None = None,
    handoff_path: str | None = None,
    resume_manifest_path: str | None = None,
) -> dict[str, Any]:
    request: dict[str, Any] = {}
    if attach_descriptor is not None:
        request["attach_descriptor"] = dict(attach_descriptor)
    if binding_artifact_path is not None:
        request["binding_artifact_path"] = binding_artifact_path
    if handoff_path is not None:
        request["handoff_path"] = handoff_path
    if resume_manifest_path is not None:
        request["resume_manifest_path"] = resume_manifest_path
    return request


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
    detail = detail.replace('"', "'")
    return ValueError(detail)


def _validated_attach_descriptor(payload: Mapping[str, Any]) -> dict[str, Any]:
    attach_descriptor = payload.get("attach_descriptor")
    if not isinstance(attach_descriptor, Mapping):
        raise ValueError("External runtime event attach payload is missing attach_descriptor.")
    return RuntimeEventAttachDescriptor.from_payload(attach_descriptor).model_dump(mode="json")


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
        self._attach_descriptor = _validated_attach_descriptor(self._payload)
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
        self.binding_artifact_source = self._attach_descriptor.get("resolution", {}).get("binding_artifact_path")
        self.handoff_source = self._attach_descriptor.get("resolution", {}).get("handoff_path")
        self.resume_manifest_source = self._attach_descriptor.get("resolution", {}).get("resume_manifest_path")
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
        request = _build_external_runtime_attach_request(
            attach_descriptor=attach_descriptor,
            binding_artifact_path=binding_artifact_path,
            handoff_path=handoff_path,
            resume_manifest_path=resume_manifest_path,
        )
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

        return dict(self._attach_descriptor)

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
                **_build_external_runtime_attach_request(attach_descriptor=self._attach_descriptor),
                "after_event_id": after_event_id,
                "limit": limit,
                "heartbeat": heartbeat,
            }
        )
        return RuntimeEventStreamChunk.model_validate(payload)

    def cleanup(self) -> dict[str, Any]:
        """Report cleanup semantics for artifact-backed external attach."""

        try:
            return self._adapter.cleanup_attached_runtime_event_transport(
                _build_external_runtime_attach_request(attach_descriptor=self._attach_descriptor)
            )
        except RuntimeError as exc:
            attach_error = _unwrap_rust_attach_error(exc)
            if attach_error is not None:
                raise attach_error from exc
            raise
