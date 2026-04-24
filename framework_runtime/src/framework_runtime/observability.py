"""Rust-owned runtime observability surface."""

from __future__ import annotations

from typing import Any

from framework_runtime.paths import default_codex_home
from framework_runtime.rust_router import RustRouteAdapter

RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION = "runtime-observability-exporter-v1"
RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION = "runtime-observability-metric-record-v1"
RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION = "runtime-observability-metric-catalog-v1"
RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION = "runtime-observability-metrics-v1"
RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION = "runtime-observability-dashboard-v1"
RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION = "runtime-observability-health-snapshot-v1"
RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY = "shared-runtime-v1"

__all__ = [
    "RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION",
    "RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION",
    "RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY",
    "build_runtime_metric_record",
    "build_runtime_observability_exporter_descriptor",
    "build_runtime_observability_health_snapshot",
    "runtime_observability_dashboard_schema",
    "runtime_observability_metric_catalog",
]


def _resolve_adapter(rust_adapter: RustRouteAdapter | None) -> RustRouteAdapter:
    adapter = rust_adapter or RustRouteAdapter(default_codex_home())
    if not adapter.health()["available"]:
        raise RuntimeError("Rust observability lane requires an available router-rs binary")
    return adapter


def build_runtime_observability_exporter_descriptor(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    return _resolve_adapter(rust_adapter).runtime_observability_exporter_descriptor()


def runtime_observability_metric_catalog(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    return _resolve_adapter(rust_adapter).runtime_observability_metric_catalog()


def runtime_observability_dashboard_schema(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    return _resolve_adapter(rust_adapter).runtime_observability_dashboard_schema()


def build_runtime_observability_health_snapshot(
    *,
    rust_adapter: RustRouteAdapter | None = None,
) -> dict[str, Any]:
    return _resolve_adapter(rust_adapter).runtime_observability_health_snapshot()


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
    return _resolve_adapter(None).runtime_metric_record(
        {
            "metric_name": metric_name,
            "value": value,
            "service_name": service_name,
            "service_version": service_version,
            "runtime_instance_id": runtime_instance_id,
            "route_engine_mode": route_engine_mode,
            "job_id": job_id,
            "session_id": session_id,
            "attempt": attempt,
            "worker_id": worker_id,
            "generation": generation,
        }
    )
