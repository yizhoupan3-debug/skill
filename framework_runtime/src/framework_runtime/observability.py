"""Concrete runtime observability exporter and metrics helpers."""

from __future__ import annotations

from functools import lru_cache
import math
from dataclasses import dataclass
from typing import Any, Callable, TypeVar

from framework_runtime.paths import default_codex_home
from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.trace import (
    TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
    TRACE_EVENT_HANDOFF_SCHEMA_VERSION,
    TRACE_EVENT_SINK_SCHEMA_VERSION,
)

RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION = "runtime-observability-exporter-v1"
RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION = "runtime-observability-metric-record-v1"
RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION = "runtime-observability-metrics-v1"
RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION = "runtime-observability-dashboard-v1"
RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY = "shared-runtime-v1"
RUNTIME_OBSERVABILITY_RESOURCE_DIMENSIONS = (
    "service.name",
    "service.version",
    "runtime.instance.id",
    "route_engine_mode",
)
RUNTIME_OBSERVABILITY_BASE_DIMENSIONS = (
    "runtime.job_id",
    "runtime.session_id",
    "runtime.attempt",
    "runtime.worker_id",
    "runtime.generation",
)
RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS = (
    *RUNTIME_OBSERVABILITY_RESOURCE_DIMENSIONS,
    *RUNTIME_OBSERVABILITY_BASE_DIMENSIONS,
)
RUNTIME_METRIC_STAGE = "runtime.metric"
RUNTIME_METRIC_STATUS = "ok"
_R = TypeVar("_R")
RUNTIME_OBSERVABILITY_OWNERSHIP = {
    "ownership_lane": "rust-contract-lane",
    "producer_owner": "rust-control-plane",
    "producer_authority": "rust-runtime-control-plane",
    "exporter_owner": "rust-control-plane",
    "exporter_authority": "rust-runtime-control-plane",
}
__all__ = [
    "RUNTIME_OBSERVABILITY_BASE_DIMENSIONS",
    "RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS",
    "RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION",
    "RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_METRIC_SPECS",
    "RUNTIME_OBSERVABILITY_OWNERSHIP",
    "RUNTIME_OBSERVABILITY_RESOURCE_DIMENSIONS",
    "RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY",
    "RuntimeMetricSpec",
    "build_runtime_metric_record",
    "build_runtime_observability_exporter_descriptor",
    "build_runtime_observability_health_snapshot",
    "build_runtime_observability_resource_attributes",
    "runtime_observability_metric_catalog",
    "runtime_observability_dashboard_schema",
]
RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION = "runtime-observability-metric-catalog-v1"


@dataclass(frozen=True, slots=True)
class RuntimeMetricSpec:
    """One versioned runtime metric contract row."""

    intent: str
    metric_name: str
    metric_type: str
    unit: str
    base_dimensions: tuple[str, ...]
    dashboard_derivation: str


RUNTIME_OBSERVABILITY_METRIC_SPECS = (
    RuntimeMetricSpec(
        intent="route mismatch rate",
        metric_name="runtime.route_mismatch_total",
        metric_type="counter",
        unit="1",
        base_dimensions=RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS,
        dashboard_derivation="rate(route_mismatch_total) / rate(route_evaluation_total)",
    ),
    RuntimeMetricSpec(
        intent="replay resume success rate",
        metric_name="runtime.replay_resume_success_total",
        metric_type="counter",
        unit="1",
        base_dimensions=RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS,
        dashboard_derivation="rate(replay_resume_success_total) / rate(replay_resume_attempt_total)",
    ),
    RuntimeMetricSpec(
        intent="lease takeover latency",
        metric_name="runtime.lease_takeover_latency_ms",
        metric_type="histogram",
        unit="ms",
        base_dimensions=RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS,
        dashboard_derivation="p50 / p95 / p99",
    ),
    RuntimeMetricSpec(
        intent="interrupt completion latency",
        metric_name="runtime.interrupt_completion_latency_ms",
        metric_type="histogram",
        unit="ms",
        base_dimensions=RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS,
        dashboard_derivation="p50 / p95 / p99",
    ),
    RuntimeMetricSpec(
        intent="compression offload rate",
        metric_name="runtime.compression_offload_total",
        metric_type="counter",
        unit="1",
        base_dimensions=RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS,
        dashboard_derivation="rate(compression_offload_total) / rate(compression_candidate_total)",
    ),
    RuntimeMetricSpec(
        intent="sandbox timeout rate",
        metric_name="runtime.sandbox_timeout_total",
        metric_type="counter",
        unit="1",
        base_dimensions=RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS,
        dashboard_derivation="rate(sandbox_timeout_total) / rate(sandbox_execution_total)",
    ),
)
_RUNTIME_OBSERVABILITY_METRIC_INDEX = {
    spec.metric_name: spec for spec in RUNTIME_OBSERVABILITY_METRIC_SPECS
}
_RUNTIME_OBSERVABILITY_DASHBOARD_PANELS = (
    {
        "name": "Route mismatch rate",
        "metric": "runtime.route_mismatch_total",
        "visualization": "timeseries",
        "group_by": ["service.name", "service.version", "route_engine_mode"],
    },
    {
        "name": "Replay resume success rate",
        "metric": "runtime.replay_resume_success_total",
        "visualization": "timeseries",
        "group_by": ["service.name", "service.version", "runtime.session_id"],
    },
    {
        "name": "Lease takeover latency",
        "metric": "runtime.lease_takeover_latency_ms",
        "visualization": "histogram",
        "group_by": ["service.name", "service.version", "runtime.worker_id"],
    },
    {
        "name": "Interrupt completion latency",
        "metric": "runtime.interrupt_completion_latency_ms",
        "visualization": "histogram",
        "group_by": ["service.name", "service.version", "runtime.session_id"],
    },
    {
        "name": "Compression offload rate",
        "metric": "runtime.compression_offload_total",
        "visualization": "timeseries",
        "group_by": ["service.name", "service.version", "runtime.generation"],
    },
    {
        "name": "Sandbox timeout rate",
        "metric": "runtime.sandbox_timeout_total",
        "visualization": "timeseries",
        "group_by": ["service.name", "service.version", "runtime.worker_id"],
    },
)
_RUNTIME_OBSERVABILITY_DASHBOARD_ALERTS = (
    {
        "name": "route-mismatch-burst",
        "metric": "runtime.route_mismatch_total",
        "severity": "warning",
    },
    {
        "name": "lease-takeover-latency-regression",
        "metric": "runtime.lease_takeover_latency_ms",
        "severity": "critical",
    },
    {
        "name": "sandbox-timeout-spike",
        "metric": "runtime.sandbox_timeout_total",
        "severity": "warning",
    },
)


@lru_cache(maxsize=1)
def _observability_rust_adapter() -> RustRouteAdapter | None:
    """Return the repo-local Rust adapter when the observability lane is executable."""

    adapter = RustRouteAdapter(default_codex_home())
    if not adapter.health()["available"]:
        return None
    try:
        adapter.runtime_observability_exporter_descriptor()
    except Exception:
        return None
    return adapter


def _with_rust_observability_adapter(fallback_fn: Callable[[RustRouteAdapter], _R]) -> _R | None:
    adapter = _observability_rust_adapter()
    if adapter is None:
        return None
    try:
        return fallback_fn(adapter)
    except Exception:
        _observability_rust_adapter.cache_clear()
        return None


def _with_explicit_rust_observability_adapter(
    rust_adapter: RustRouteAdapter | None,
    resolver: Callable[[RustRouteAdapter], _R],
) -> _R | None:
    if rust_adapter is None:
        return None
    try:
        return resolver(rust_adapter)
    except Exception:
        return None


def _require_non_empty_string(value: str, *, field_name: str) -> str:
    """Reject empty observability dimensions instead of silently emitting them."""

    normalized = value.strip()
    if not normalized:
        raise ValueError(f"{field_name} must be a non-empty string")
    return normalized


def _resolve_metric_spec(metric_name: str) -> RuntimeMetricSpec:
    """Resolve one cataloged metric or fail closed with a stable error."""

    try:
        return _RUNTIME_OBSERVABILITY_METRIC_INDEX[metric_name]
    except KeyError as exc:
        raise ValueError(f"unsupported runtime metric: {metric_name}") from exc


def _normalize_metric_value(value: int | float) -> int | float:
    """Reject NaN and infinity so emitted metrics remain transport-safe."""

    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError("runtime metric record requires a numeric value")
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError("metric value must be finite")
    return value


def _normalize_attempt(attempt: int) -> int:
    """Keep the attempt dimension aligned with the Rust contract lane."""

    if isinstance(attempt, bool) or not isinstance(attempt, int):
        raise ValueError("runtime metric record requires integer field attempt")
    if attempt < 0:
        raise ValueError("runtime metric record requires non-negative integer field attempt")
    return attempt


def build_runtime_observability_resource_attributes(
    *,
    service_name: str,
    service_version: str,
    runtime_instance_id: str,
    route_engine_mode: str,
) -> dict[str, str]:
    """Build the stable resource envelope shared by logs, spans, and metrics."""

    return {
        "service.name": _require_non_empty_string(service_name, field_name="service_name"),
        "service.version": _require_non_empty_string(service_version, field_name="service_version"),
        "runtime.instance.id": _require_non_empty_string(runtime_instance_id, field_name="runtime_instance_id"),
        "route_engine_mode": _require_non_empty_string(route_engine_mode, field_name="route_engine_mode"),
    }


def build_runtime_observability_exporter_descriptor(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    """Describe the concrete exporter lane used by the runtime."""

    adapter = _with_explicit_rust_observability_adapter(
        rust_adapter,
        lambda adapter_obj: adapter_obj.runtime_observability_exporter_descriptor(),
    ) or _with_rust_observability_adapter(
        lambda adapter_obj: adapter_obj.runtime_observability_exporter_descriptor(),
    )
    if adapter is not None:
        return adapter
    return {
        "schema_version": RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
        "metric_catalog_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
        "dashboard_schema_version": RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
        "signal_vocabulary": RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY,
        "export_path": "jsonl-plus-otel",
        "jsonl_sink_schema_version": TRACE_EVENT_SINK_SCHEMA_VERSION,
        "trace_bridge_schema_version": TRACE_EVENT_BRIDGE_SCHEMA_VERSION,
        "trace_handoff_schema_version": TRACE_EVENT_HANDOFF_SCHEMA_VERSION,
        **RUNTIME_OBSERVABILITY_OWNERSHIP,
    }


def _build_runtime_observability_health_snapshot(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    exporter = build_runtime_observability_exporter_descriptor(rust_adapter=rust_adapter)
    dashboard = runtime_observability_dashboard_schema(rust_adapter=rust_adapter)
    metric_catalog = runtime_observability_metric_catalog(rust_adapter=rust_adapter)
    return {
        "ownership_lane": exporter["ownership_lane"],
        "metric_catalog_version": exporter["metric_catalog_version"],
        "dashboard_schema_version": dashboard["schema_version"],
        "resource_dimensions": list(dashboard["resource_dimensions"]),
        "metric_catalog_schema_version": metric_catalog["schema_version"],
        "metric_names": [metric["metric_name"] for metric in metric_catalog["metrics"]],
        "dashboard_panel_count": len(dashboard["panels"]),
        "dashboard_alert_count": len(dashboard["alerts"]),
        "exporter": exporter,
    }


@lru_cache(maxsize=1)
def _cached_runtime_observability_health_snapshot() -> dict[str, Any]:
    return _build_runtime_observability_health_snapshot()


def build_runtime_observability_health_snapshot(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    """Return one cached runtime-health projection of the observability contract."""

    if rust_adapter is not None:
        return _build_runtime_observability_health_snapshot(rust_adapter=rust_adapter)
    return _cached_runtime_observability_health_snapshot()


def runtime_observability_metric_catalog(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    """Return the machine-readable runtime metric catalog frozen by the contract."""

    catalog = _with_explicit_rust_observability_adapter(
        rust_adapter,
        lambda adapter_obj: adapter_obj.runtime_observability_metric_catalog(),
    ) or _with_rust_observability_adapter(
        lambda adapter_obj: adapter_obj.runtime_observability_metric_catalog(),
    )
    if catalog is not None:
        return catalog
    return {
        "schema_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
        "metric_catalog_version": RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
        "resource_dimensions": list(RUNTIME_OBSERVABILITY_RESOURCE_DIMENSIONS),
        "base_dimensions": list(RUNTIME_OBSERVABILITY_BASE_DIMENSIONS),
        "metrics": [
            {
                "intent": spec.intent,
                "metric_name": spec.metric_name,
                "metric_type": spec.metric_type,
                "unit": spec.unit,
                "dimensions": list(spec.base_dimensions),
                "dashboard_derivation": spec.dashboard_derivation,
            }
            for spec in RUNTIME_OBSERVABILITY_METRIC_SPECS
        ],
    }


def runtime_observability_dashboard_schema(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    """Return the canonical dashboard schema backing the contract doc."""

    schema = _with_explicit_rust_observability_adapter(
        rust_adapter,
        lambda adapter_obj: adapter_obj.runtime_observability_dashboard_schema(),
    ) or _with_rust_observability_adapter(
        lambda adapter_obj: adapter_obj.runtime_observability_dashboard_schema(),
    )
    if schema is not None:
        return schema
    return {
        "schema_version": RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
        "title": "Runtime Observability",
        "resource_dimensions": list(RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS),
        "panels": [dict(panel) for panel in _RUNTIME_OBSERVABILITY_DASHBOARD_PANELS],
        "alerts": [dict(alert) for alert in _RUNTIME_OBSERVABILITY_DASHBOARD_ALERTS],
    }


def build_runtime_metric_record(
    metric_name: str,
    *,
    value: int | float,
    service_name: str,
    service_version: str,
    runtime_instance_id: str,
    route_engine_mode: str,
    job_id: str,
    session_id: str,
    attempt: int,
    worker_id: str,
    generation: str,
) -> dict[str, Any]:
    """Build one concrete metrics payload locked to the catalog and base dimensions."""

    spec = _resolve_metric_spec(metric_name)
    normalized_value = _normalize_metric_value(value)
    resource_attributes = build_runtime_observability_resource_attributes(
        service_name=service_name,
        service_version=service_version,
        runtime_instance_id=runtime_instance_id,
        route_engine_mode=route_engine_mode,
    )
    dimensions = {
        "runtime.job_id": _require_non_empty_string(job_id, field_name="job_id"),
        "runtime.session_id": _require_non_empty_string(session_id, field_name="session_id"),
        "runtime.attempt": _normalize_attempt(attempt),
        "runtime.worker_id": _require_non_empty_string(worker_id, field_name="worker_id"),
        "runtime.generation": _require_non_empty_string(generation, field_name="generation"),
        "runtime.stage": RUNTIME_METRIC_STAGE,
        "runtime.status": RUNTIME_METRIC_STATUS,
    }
    record = _with_rust_observability_adapter(
        lambda adapter_obj: adapter_obj.runtime_metric_record(
            {
                "metric_name": spec.metric_name,
                "value": normalized_value,
                "service_name": resource_attributes["service.name"],
                "service_version": resource_attributes["service.version"],
                "runtime_instance_id": resource_attributes["runtime.instance.id"],
                "route_engine_mode": resource_attributes["route_engine_mode"],
                "job_id": dimensions["runtime.job_id"],
                "session_id": dimensions["runtime.session_id"],
                "attempt": dimensions["runtime.attempt"],
                "worker_id": dimensions["runtime.worker_id"],
                "generation": dimensions["runtime.generation"],
            }
        )
    )
    if record is not None:
        return record
    payload = {
        "schema_version": RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION,
        "metric_name": spec.metric_name,
        "metric_type": spec.metric_type,
        "unit": spec.unit,
        "value": normalized_value,
        "resource_attributes": resource_attributes,
        "dimensions": dimensions,
        "ownership": build_runtime_observability_exporter_descriptor(),
    }
    return payload
