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
RUNTIME_EVENT_ATTACH_MODE = "process_external_artifact_replay"
RUNTIME_EVENT_ATTACH_SOURCE_HANDOFF_METHOD = "describe_runtime_event_handoff"
RUNTIME_EVENT_ATTACH_SOURCE_TRANSPORT_METHOD = "describe_runtime_event_transport"
RUNTIME_EVENT_ATTACH_METHOD = "attach_runtime_event_transport"
RUNTIME_EVENT_ATTACH_SUBSCRIBE_METHOD = "subscribe_attached_runtime_events"
RUNTIME_EVENT_ATTACH_CLEANUP_METHOD = "cleanup_attached_runtime_event_transport"
RUNTIME_EVENT_ATTACH_RESUME_MODE = "after_event_id"
_DESCRIPTOR_PATH_FIELDS = (
    ("requested_artifacts", "binding_artifact_path"),
    ("requested_artifacts", "handoff_path"),
    ("requested_artifacts", "resume_manifest_path"),
    ("resolved_artifacts", "binding_artifact_path"),
    ("resolved_artifacts", "handoff_path"),
    ("resolved_artifacts", "resume_manifest_path"),
    ("resolved_artifacts", "trace_stream_path"),
)
_DESCRIPTOR_SCALAR_FIELDS = (
    ("attach_mode",),
    ("artifact_backend_family",),
    ("source_transport_method",),
    ("source_handoff_method",),
    ("attach_method",),
    ("subscribe_method",),
    ("cleanup_method",),
    ("resume_mode",),
    ("cleanup_semantics",),
    ("recommended_entrypoint",),
    ("attach_capabilities", "artifact_replay"),
    ("attach_capabilities", "live_remote_stream"),
    ("attach_capabilities", "cleanup_preserves_replay"),
    ("resolution", "binding_artifact_path"),
    ("resolution", "handoff_path"),
    ("resolution", "resume_manifest_path"),
    ("resolution", "trace_stream_path"),
)


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

    def _assert_contract(self) -> None:
        """Fail closed when callers mutate descriptor vocabulary away from the Rust-owned contract."""

        expected_scalars = {
            "attach_mode": RUNTIME_EVENT_ATTACH_MODE,
            "source_transport_method": RUNTIME_EVENT_ATTACH_SOURCE_TRANSPORT_METHOD,
            "source_handoff_method": RUNTIME_EVENT_ATTACH_SOURCE_HANDOFF_METHOD,
            "attach_method": RUNTIME_EVENT_ATTACH_METHOD,
            "subscribe_method": RUNTIME_EVENT_ATTACH_SUBSCRIBE_METHOD,
            "cleanup_method": RUNTIME_EVENT_ATTACH_CLEANUP_METHOD,
            "resume_mode": RUNTIME_EVENT_ATTACH_RESUME_MODE,
        }
        for field_name, expected_value in expected_scalars.items():
            value = getattr(self, field_name)
            if value is not None and value != expected_value:
                raise ValueError(
                    "External runtime event attach descriptor must use "
                    f"{field_name}={expected_value!r}."
                )
        if self.attach_capabilities is None:
            return
        if (
            self.attach_capabilities.artifact_replay is not None
            and self.attach_capabilities.artifact_replay is not True
        ):
            raise ValueError(
                "External runtime event attach descriptor must advertise "
                "attach_capabilities.artifact_replay=True."
            )
        if (
            self.attach_capabilities.cleanup_preserves_replay is not None
            and self.attach_capabilities.cleanup_preserves_replay is not True
        ):
            raise ValueError(
                "External runtime event attach descriptor must advertise "
                "attach_capabilities.cleanup_preserves_replay=True."
            )
        if (
            self.attach_capabilities.live_remote_stream is not None
            and self.attach_capabilities.live_remote_stream is not False
        ):
            raise ValueError(
                "External runtime event attach descriptor must advertise "
                "attach_capabilities.live_remote_stream=False."
            )

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeEventAttachDescriptor":
        """Validate and normalize a raw attach descriptor payload."""

        try:
            descriptor = cls.model_validate(payload)
        except ValidationError as exc:
            errors = exc.errors()
            if any(err.get("loc") == ("schema_version",) for err in errors):
                raise ValueError(
                    "External runtime event attach payload returned an unknown attach_descriptor schema_version."
                ) from exc
            raise ValueError("External runtime event attach payload returned an invalid attach_descriptor.") from exc
        descriptor._assert_contract()
        return descriptor


class ExternalRuntimeEventTransportAttachment(BaseModel):
    """Validated attach payload emitted by the Rust-owned external attach seam."""

    model_config = ConfigDict(extra="allow")

    attach_mode: Literal["process_external_artifact_replay"]
    authority: str
    transport: RuntimeEventTransport
    handoff: RuntimeEventHandoff | None = None
    resume_manifest: TraceResumeManifest | None = None
    binding_artifact_path: str | None = None
    handoff_path: str | None = None
    resume_manifest_path: str | None = None
    trace_stream_path: str
    replay_supported: bool = True
    cleanup_semantics: str | None = None
    cleanup_preserves_replay: bool = True
    artifact_backend_family: str | None = None
    source_handoff_method: str | None = None
    source_transport_method: str | None = None
    attach_method: str | None = None
    subscribe_method: str | None = None
    cleanup_method: str | None = None
    resume_mode: str | None = None
    attach_descriptor: RuntimeEventAttachDescriptor


class ExternalRuntimeEventTransportCleanupResult(BaseModel):
    """Validated cleanup payload emitted by the Rust-owned external attach seam."""

    model_config = ConfigDict(extra="allow")

    authority: str
    cleanup_semantics: str | None = None
    cleanup_preserves_replay: bool = True
    cleanup_method: Literal[RUNTIME_EVENT_ATTACH_CLEANUP_METHOD]
    binding_artifact_path: str | None = None
    trace_stream_path: str | None = None


def _clone_json(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in payload.items()}


def _descriptor_leaf(payload: Mapping[str, Any], path: tuple[str, ...]) -> Any:
    current: Any = payload
    for key in path:
        if not isinstance(current, Mapping):
            return None
        current = current.get(key)
    return current


def _normalize_compare_path(value: Any) -> str | None:
    path = _optional_path(value)
    return str(path) if path is not None else None


def _assert_matching_value(*, field_name: str, requested: Any, canonical: Any, path_like: bool) -> None:
    if requested is None:
        return
    requested_value = _normalize_compare_path(requested) if path_like else requested
    canonical_value = _normalize_compare_path(canonical) if path_like else canonical
    if canonical_value is None:
        raise ValueError(f"External runtime event attach descriptor must include canonical {field_name!r}.")
    if requested_value != canonical_value:
        raise ValueError(
            f"External runtime event attach descriptor must already match canonical {field_name!r}."
        )


def _assert_descriptor_matches_canonical(
    *,
    requested_descriptor: Mapping[str, Any],
    canonical_descriptor: Mapping[str, Any],
) -> None:
    for path in _DESCRIPTOR_PATH_FIELDS:
        _assert_matching_value(
            field_name=".".join(path),
            requested=_descriptor_leaf(requested_descriptor, path),
            canonical=_descriptor_leaf(canonical_descriptor, path),
            path_like=True,
        )
    for path in _DESCRIPTOR_SCALAR_FIELDS:
        _assert_matching_value(
            field_name=".".join(path),
            requested=_descriptor_leaf(requested_descriptor, path),
            canonical=_descriptor_leaf(canonical_descriptor, path),
            path_like=False,
        )


def _build_external_runtime_attach_request(
    *,
    attach_descriptor: Mapping[str, Any] | None = None,
    binding_artifact_path: str | None = None,
    handoff_path: str | None = None,
    resume_manifest_path: str | None = None,
) -> dict[str, Any]:
    request: dict[str, Any] = {}
    if attach_descriptor is not None:
        request["attach_descriptor"] = RuntimeEventAttachDescriptor.from_payload(attach_descriptor).model_dump(mode="json")
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


def _validate_attachment_payload(payload: Mapping[str, Any]) -> ExternalRuntimeEventTransportAttachment:
    try:
        attachment = ExternalRuntimeEventTransportAttachment.model_validate(payload)
    except ValidationError as exc:
        raise ValueError("External runtime event attach payload returned an invalid attachment payload.") from exc
    descriptor = attachment.attach_descriptor.model_dump(mode="json")
    _assert_matching_value(
        field_name="attach_mode",
        requested=descriptor.get("attach_mode"),
        canonical=attachment.attach_mode,
        path_like=False,
    )
    _assert_matching_value(
        field_name="artifact_backend_family",
        requested=descriptor.get("artifact_backend_family"),
        canonical=attachment.artifact_backend_family,
        path_like=False,
    )
    _assert_matching_value(
        field_name="resume_mode",
        requested=descriptor.get("resume_mode"),
        canonical=attachment.resume_mode,
        path_like=False,
    )
    _assert_matching_value(
        field_name="cleanup_semantics",
        requested=descriptor.get("cleanup_semantics"),
        canonical=attachment.cleanup_semantics,
        path_like=False,
    )
    resolved_artifacts = descriptor.get("resolved_artifacts")
    if isinstance(resolved_artifacts, Mapping):
        _assert_matching_value(
            field_name="resolved_artifacts.binding_artifact_path",
            requested=resolved_artifacts.get("binding_artifact_path"),
            canonical=attachment.binding_artifact_path,
            path_like=True,
        )
        _assert_matching_value(
            field_name="resolved_artifacts.handoff_path",
            requested=resolved_artifacts.get("handoff_path"),
            canonical=attachment.handoff_path,
            path_like=True,
        )
        _assert_matching_value(
            field_name="resolved_artifacts.resume_manifest_path",
            requested=resolved_artifacts.get("resume_manifest_path"),
            canonical=attachment.resume_manifest_path,
            path_like=True,
        )
        _assert_matching_value(
            field_name="resolved_artifacts.trace_stream_path",
            requested=resolved_artifacts.get("trace_stream_path"),
            canonical=attachment.trace_stream_path,
            path_like=True,
        )
    return attachment


def _validate_cleanup_payload(payload: Mapping[str, Any]) -> ExternalRuntimeEventTransportCleanupResult:
    try:
        return ExternalRuntimeEventTransportCleanupResult.model_validate(payload)
    except ValidationError as exc:
        raise ValueError("External runtime event cleanup payload returned an invalid cleanup payload.") from exc


def resolve_external_runtime_event_transport(
    *,
    adapter: RustRouteAdapter,
    attach_descriptor: Mapping[str, Any] | None = None,
    binding_artifact_path: str | None = None,
    handoff_path: str | None = None,
    resume_manifest_path: str | None = None,
) -> ExternalRuntimeEventTransportAttachment:
    """Resolve and validate the canonical attach payload for external runtime replay."""

    requested_descriptor = (
        RuntimeEventAttachDescriptor.from_payload(attach_descriptor).model_dump(mode="json")
        if attach_descriptor is not None
        else None
    )
    request = _build_external_runtime_attach_request(
        attach_descriptor=requested_descriptor,
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
    attachment = _validate_attachment_payload(payload)
    if requested_descriptor is not None:
        _assert_descriptor_matches_canonical(
            requested_descriptor=requested_descriptor,
            canonical_descriptor=attachment.attach_descriptor.model_dump(mode="json"),
        )
    return attachment


def cleanup_external_runtime_event_transport(
    *,
    adapter: RustRouteAdapter,
    attach_descriptor: Mapping[str, Any] | None = None,
    binding_artifact_path: str | None = None,
    handoff_path: str | None = None,
    resume_manifest_path: str | None = None,
) -> ExternalRuntimeEventTransportCleanupResult:
    """Cleanup the external runtime replay bridge through the canonical descriptor only."""

    attachment = resolve_external_runtime_event_transport(
        adapter=adapter,
        attach_descriptor=attach_descriptor,
        binding_artifact_path=binding_artifact_path,
        handoff_path=handoff_path,
        resume_manifest_path=resume_manifest_path,
    )
    try:
        payload = adapter.cleanup_attached_runtime_event_transport(
            {
                "attach_descriptor": attachment.attach_descriptor.model_dump(mode="json"),
            }
        )
    except RuntimeError as exc:
        attach_error = _unwrap_rust_attach_error(exc)
        if attach_error is not None:
            raise attach_error from exc
        raise
    cleanup = _validate_cleanup_payload(payload)
    _assert_matching_value(
        field_name="binding_artifact_path",
        requested=cleanup.binding_artifact_path,
        canonical=attachment.binding_artifact_path,
        path_like=True,
    )
    _assert_matching_value(
        field_name="trace_stream_path",
        requested=cleanup.trace_stream_path,
        canonical=attachment.trace_stream_path,
        path_like=True,
    )
    return cleanup


class ExternalRuntimeEventTransportBridge:
    """Thin Python projection over the Rust-owned attached runtime transport lane."""

    def __init__(
        self,
        *,
        adapter: RustRouteAdapter,
        payload: Mapping[str, Any],
    ) -> None:
        self._adapter = adapter
        attachment = _validate_attachment_payload(payload)
        self._payload = attachment.model_dump(mode="json")
        self._attach_descriptor = attachment.attach_descriptor.model_dump(mode="json")
        self.transport = attachment.transport
        self.handoff = attachment.handoff
        self.resume_manifest = attachment.resume_manifest
        self.binding_artifact_path = _optional_path(attachment.binding_artifact_path)
        self.handoff_path = _optional_path(attachment.handoff_path)
        self.resume_manifest_path = _optional_path(attachment.resume_manifest_path)
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
        payload = resolve_external_runtime_event_transport(
            adapter=adapter,
            attach_descriptor=attach_descriptor,
            binding_artifact_path=binding_artifact_path,
            handoff_path=handoff_path,
            resume_manifest_path=resume_manifest_path,
        ).model_dump(mode="json")
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

        return cleanup_external_runtime_event_transport(
            adapter=self._adapter,
            attach_descriptor=self._attach_descriptor,
        ).model_dump(mode="json")
