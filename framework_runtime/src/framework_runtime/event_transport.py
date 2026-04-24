"""Rust-owned process-external runtime event transport surface."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping

from framework_runtime.config import RuntimeSettings
from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.trace import RuntimeEventStreamChunk


def _attach_request(
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


def _unwrap_attach_error(exc: RuntimeError) -> ValueError | None:
    prefix = "Rust attached runtime event transport failed: "
    message = str(exc)
    if not message.startswith(prefix):
        return None
    detail = message[len(prefix) :].strip()
    if detail.startswith('Error: "') and detail.endswith('"'):
        detail = detail[len('Error: "') : -1]
    return ValueError(detail.replace('\\"', "'").replace('"', "'"))


def resolve_external_runtime_event_transport(
    *,
    adapter: RustRouteAdapter,
    attach_descriptor: Mapping[str, Any] | None = None,
    binding_artifact_path: str | None = None,
    handoff_path: str | None = None,
    resume_manifest_path: str | None = None,
) -> dict[str, Any]:
    try:
        return adapter.attach_runtime_event_transport(
            _attach_request(
                attach_descriptor=attach_descriptor,
                binding_artifact_path=binding_artifact_path,
                handoff_path=handoff_path,
                resume_manifest_path=resume_manifest_path,
            )
        )
    except RuntimeError as exc:
        attach_error = _unwrap_attach_error(exc)
        if attach_error is not None:
            raise attach_error from exc
        raise


def subscribe_external_runtime_event_transport(
    *,
    adapter: RustRouteAdapter,
    attach_descriptor: Mapping[str, Any] | None = None,
    binding_artifact_path: str | None = None,
    handoff_path: str | None = None,
    resume_manifest_path: str | None = None,
    after_event_id: str | None = None,
    limit: int | None = 100,
    heartbeat: bool = False,
) -> RuntimeEventStreamChunk:
    request = _attach_request(
        attach_descriptor=attach_descriptor,
        binding_artifact_path=binding_artifact_path,
        handoff_path=handoff_path,
        resume_manifest_path=resume_manifest_path,
    )
    request["after_event_id"] = after_event_id
    request["limit"] = limit
    request["heartbeat"] = heartbeat
    try:
        return RuntimeEventStreamChunk.model_validate(
            adapter.subscribe_attached_runtime_events(request)
        )
    except RuntimeError as exc:
        attach_error = _unwrap_attach_error(exc)
        if attach_error is not None:
            raise attach_error from exc
        raise


def cleanup_external_runtime_event_transport(
    *,
    adapter: RustRouteAdapter,
    attach_descriptor: Mapping[str, Any] | None = None,
    binding_artifact_path: str | None = None,
    handoff_path: str | None = None,
    resume_manifest_path: str | None = None,
) -> dict[str, Any]:
    try:
        return adapter.cleanup_attached_runtime_event_transport(
            _attach_request(
                attach_descriptor=attach_descriptor,
                binding_artifact_path=binding_artifact_path,
                handoff_path=handoff_path,
                resume_manifest_path=resume_manifest_path,
            )
        )
    except RuntimeError as exc:
        attach_error = _unwrap_attach_error(exc)
        if attach_error is not None:
            raise attach_error from exc
        raise


class ExternalRuntimeEventTransportAttachment:
    """Thin Python handle over the Rust-owned external transport lane."""

    def __init__(self, *, adapter: RustRouteAdapter, payload: Mapping[str, Any]) -> None:
        self._adapter = adapter
        self._payload = dict(payload)
        self._attach_descriptor = dict(payload.get("attach_descriptor") or {})
        self.binding_artifact_path = _optional_path(payload.get("binding_artifact_path"))
        self.handoff_path = _optional_path(payload.get("handoff_path"))
        self.resume_manifest_path = _optional_path(payload.get("resume_manifest_path"))

    @classmethod
    def attach(
        cls,
        *,
        attach_descriptor: Mapping[str, Any] | None = None,
        binding_artifact_path: str | None = None,
        handoff_path: str | None = None,
        resume_manifest_path: str | None = None,
    ) -> ExternalRuntimeEventTransportAttachment:
        settings = RuntimeSettings()
        adapter = RustRouteAdapter(
            settings.codex_home,
            timeout_seconds=settings.rust_router_timeout_seconds,
        )
        return cls(
            adapter=adapter,
            payload=resolve_external_runtime_event_transport(
                adapter=adapter,
                attach_descriptor=attach_descriptor,
                binding_artifact_path=binding_artifact_path,
                handoff_path=handoff_path,
                resume_manifest_path=resume_manifest_path,
            ),
        )

    def describe(self) -> dict[str, Any]:
        return dict(self._payload)

    def attach_descriptor(self) -> dict[str, Any]:
        return dict(self._attach_descriptor)

    def subscribe(
        self,
        *,
        after_event_id: str | None = None,
        limit: int | None = 100,
        heartbeat: bool = False,
    ) -> RuntimeEventStreamChunk:
        return subscribe_external_runtime_event_transport(
            adapter=self._adapter,
            attach_descriptor=self._attach_descriptor,
            after_event_id=after_event_id,
            limit=limit,
            heartbeat=heartbeat,
        )

    def cleanup(self) -> dict[str, Any]:
        return cleanup_external_runtime_event_transport(
            adapter=self._adapter,
            attach_descriptor=self._attach_descriptor,
        )
