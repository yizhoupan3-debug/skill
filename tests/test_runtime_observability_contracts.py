"""Regression tests for the runtime observability contract."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from unittest.mock import patch

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
RUST_ADAPTER_TIMEOUT_SECONDS = 120.0
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.observability import (
    RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_SPECS,
    build_runtime_metric_record,
    build_runtime_observability_exporter_descriptor,
    build_runtime_observability_health_snapshot,
    runtime_observability_metric_catalog,
    runtime_observability_dashboard_schema,
)
from framework_runtime.paths import default_codex_home
from framework_runtime.rust_router import RustRouteAdapter

CONTRACT_PATH = PROJECT_ROOT / "docs" / "runtime_observability_contract.md"
CONTRACT_TEXT = CONTRACT_PATH.read_text(encoding="utf-8")


def _section(title: str) -> str:
    pattern = rf"^## {re.escape(title)}\s*$"
    matches = list(re.finditer(pattern, CONTRACT_TEXT, flags=re.MULTILINE))
    assert matches, f"missing section: {title}"

    start = matches[0].end()
    tail = CONTRACT_TEXT[start:]
    next_heading = re.search(r"^##\s+", tail, flags=re.MULTILINE)
    end = start + next_heading.start() if next_heading else len(CONTRACT_TEXT)
    return CONTRACT_TEXT[start:end]


def _table_first_column(section_text: str) -> set[str]:
    values: set[str] = set()
    for line in section_text.splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if not cells:
            continue
        first = cells[0].strip("`")
        if first in {"name", "field", "JSONL token", "intent"}:
            continue
        if first and not first.startswith("---"):
            values.add(first)
    return values


def _table_mapping(section_text: str) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for line in section_text.splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) < 2:
            continue
        first = cells[0].strip("`")
        second = cells[1].strip("`")
        if first in {"JSONL token", "intent"} or first.startswith("---"):
            continue
        if first:
            mapping[first] = second
    return mapping


def _dashboard_schema() -> dict[str, object]:
    section = _section("Dashboard Schema")
    match = re.search(r"```json\s*(\{.*?\})\s*```", section, flags=re.DOTALL)
    assert match, "missing dashboard JSON schema block"
    return json.loads(match.group(1))


def test_contract_documents_required_resource_and_shared_fields() -> None:
    resource_section = _section("Runtime Resource Attributes")
    shared_section = _section("Shared Span / Metric / Log Fields")

    required_resource_fields = {
        "service.name",
        "service.version",
        "runtime.instance.id",
        "route_engine_mode",
    }
    required_shared_fields = {
        "runtime.job_id",
        "runtime.session_id",
        "runtime.attempt",
        "runtime.worker_id",
        "runtime.generation",
        "runtime.schema_version",
        "runtime.event_id",
        "runtime.stage",
        "runtime.kind",
        "runtime.status",
        "trace_id",
        "span_id",
    }

    resource_fields = _table_first_column(resource_section)
    shared_fields = _table_first_column(shared_section)

    assert required_resource_fields.issubset(resource_fields)
    assert required_shared_fields.issubset(shared_fields)

    for field in required_resource_fields | required_shared_fields:
        assert f"`{field}`" in CONTRACT_TEXT


def test_vocabulary_map_keeps_canonical_jsonl_to_otel_pairs() -> None:
    vocabulary_section = _section("JSONL <-> OTel Vocabulary Map")
    mapping = _table_mapping(vocabulary_section)

    expected_pairs = {
        "ts": "time_unix_nano",
        "event_id": "runtime.event.id",
        "seq": "runtime.event.seq",
        "cursor": "runtime.resume.cursor",
        "kind": "runtime.kind",
        "stage": "runtime.stage",
        "status": "runtime.status",
        "payload": "attributes",
        "service_name": "service.name",
        "service_version": "service.version",
        "runtime_instance_id": "runtime.instance.id",
        "route_engine_mode": "route_engine_mode",
        "job_id": "runtime.job_id",
        "session_id": "runtime.session_id",
        "attempt": "runtime.attempt",
        "worker_id": "runtime.worker_id",
        "generation": "runtime.generation",
        "schema_version": "runtime.schema_version",
    }

    for jsonl_token, otel_target in expected_pairs.items():
        assert mapping.get(jsonl_token) == otel_target

    assert len(set(mapping.values())) == len(mapping.values())
    assert "one canonical OTel target" in CONTRACT_TEXT
    assert "existing table stable" in CONTRACT_TEXT
    assert "in-memory bridge exists" in CONTRACT_TEXT
    assert "external SSE bridge not yet exposed" in CONTRACT_TEXT


def test_contract_records_rust_owned_producer_exporter_lane() -> None:
    ownership_section = _section("Producer / Exporter Ownership")

    assert "rust-contract-lane" in ownership_section
    assert 'producer_owner = "rust-control-plane"' in CONTRACT_TEXT
    assert 'exporter_owner = "rust-control-plane"' in CONTRACT_TEXT
    assert "producer_authority" in CONTRACT_TEXT
    assert "exporter_authority" in CONTRACT_TEXT
    assert "JSONL vocabulary" in ownership_section
    assert "OTel" in ownership_section
    assert "replay seam" in ownership_section
    assert "compaction seam" in ownership_section


def test_metrics_catalog_and_dashboard_schema_are_stable() -> None:
    metrics_section = _section("Runtime Metrics Catalog")
    schema = _dashboard_schema()

    expected_metrics = {
        "runtime.route_mismatch_total",
        "runtime.replay_resume_success_total",
        "runtime.lease_takeover_latency_ms",
        "runtime.interrupt_completion_latency_ms",
        "runtime.compression_offload_total",
        "runtime.sandbox_timeout_total",
    }
    for metric in expected_metrics:
        assert f"`{metric}`" in metrics_section

    assert schema["schema_version"] == "runtime-observability-dashboard-v1"
    assert schema["title"] == "Runtime Observability"

    resource_dimensions = set(schema["resource_dimensions"])
    required_dimensions = {
        "service.name",
        "service.version",
        "runtime.instance.id",
        "route_engine_mode",
        "runtime.job_id",
        "runtime.session_id",
        "runtime.attempt",
        "runtime.worker_id",
        "runtime.generation",
    }
    assert required_dimensions.issubset(resource_dimensions)

    panels = schema["panels"]
    panel_names = {panel["name"] for panel in panels}
    assert panel_names == {
        "Route mismatch rate",
        "Replay resume success rate",
        "Lease takeover latency",
        "Interrupt completion latency",
        "Compression offload rate",
        "Sandbox timeout rate",
    }

    panel_metrics = {panel["metric"] for panel in panels}
    assert panel_metrics == {
        "runtime.route_mismatch_total",
        "runtime.replay_resume_success_total",
        "runtime.lease_takeover_latency_ms",
        "runtime.interrupt_completion_latency_ms",
        "runtime.compression_offload_total",
        "runtime.sandbox_timeout_total",
    }

    for panel in panels:
        assert set(panel) >= {"name", "metric", "visualization", "group_by"}
        assert set(panel["group_by"]).issubset(resource_dimensions)

    alerts = schema["alerts"]
    alert_metrics = {alert["metric"] for alert in alerts}
    assert alert_metrics == {
        "runtime.route_mismatch_total",
        "runtime.lease_takeover_latency_ms",
        "runtime.sandbox_timeout_total",
    }


def test_concrete_observability_helpers_match_the_contract() -> None:
    adapter = RustRouteAdapter(default_codex_home(), timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS)
    with patch("framework_runtime.observability._observability_rust_adapter", return_value=adapter):
        exporter = build_runtime_observability_exporter_descriptor()
        assert exporter["ownership_lane"] == "rust-contract-lane"
        assert exporter["producer_owner"] == "rust-control-plane"
        assert exporter["exporter_owner"] == "rust-control-plane"
        assert exporter["export_path"] == "jsonl-plus-otel"

        schema = runtime_observability_dashboard_schema()
        assert schema["schema_version"] == "runtime-observability-dashboard-v1"
        assert tuple(schema["resource_dimensions"]) == RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS
        assert {panel["metric"] for panel in schema["panels"]} == {
            spec.metric_name for spec in RUNTIME_OBSERVABILITY_METRIC_SPECS
        }

        record = build_runtime_metric_record(
            "runtime.route_mismatch_total",
            value=3,
            service_name="codex-runtime",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=2,
            worker_id="worker-7",
            generation="gen-a",
        )
        assert record["metric_name"] == "runtime.route_mismatch_total"
        assert record["metric_type"] == "counter"
        assert record["unit"] == "1"
        assert record["resource_attributes"]["service.name"] == "codex-runtime"
        assert record["dimensions"] == {
            "runtime.job_id": "job-1",
            "runtime.session_id": "session-1",
            "runtime.attempt": 2,
            "runtime.worker_id": "worker-7",
            "runtime.generation": "gen-a",
            "runtime.stage": "runtime.metric",
            "runtime.status": "ok",
        }
        assert record["ownership"]["exporter_authority"] == "rust-runtime-control-plane"
        assert "build_runtime_metric_record()" in CONTRACT_TEXT


def test_metric_catalog_helper_freezes_machine_readable_metrics_path() -> None:
    catalog = runtime_observability_metric_catalog()

    assert catalog["schema_version"] == RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION
    assert catalog["metric_catalog_version"] == "runtime-observability-metrics-v1"
    assert catalog["resource_dimensions"] == [
        "service.name",
        "service.version",
        "runtime.instance.id",
        "route_engine_mode",
    ]
    assert catalog["base_dimensions"] == [
        "runtime.job_id",
        "runtime.session_id",
        "runtime.attempt",
        "runtime.worker_id",
        "runtime.generation",
    ]
    assert [metric["metric_name"] for metric in catalog["metrics"]] == [
        spec.metric_name for spec in RUNTIME_OBSERVABILITY_METRIC_SPECS
    ]
    assert [metric["dimensions"] for metric in catalog["metrics"]] == [
        list(spec.base_dimensions) for spec in RUNTIME_OBSERVABILITY_METRIC_SPECS
    ]
    assert "runtime_observability_metric_catalog()" in CONTRACT_TEXT

    health = build_runtime_observability_health_snapshot()
    assert health["metric_catalog_schema_version"] == RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION
    assert health["metric_names"] == [metric["metric_name"] for metric in catalog["metrics"]]


def test_observability_helpers_delegate_to_rust_contract_lane() -> None:
    adapter = RustRouteAdapter(default_codex_home(), timeout_seconds=RUST_ADAPTER_TIMEOUT_SECONDS)
    with patch("framework_runtime.observability._observability_rust_adapter", return_value=adapter):
        exporter = build_runtime_observability_exporter_descriptor()
        assert exporter == adapter.runtime_observability_exporter_descriptor()

        catalog = runtime_observability_metric_catalog()
        assert catalog == adapter.runtime_observability_metric_catalog()

        dashboard = runtime_observability_dashboard_schema()
        assert dashboard == adapter.runtime_observability_dashboard_schema()

        request = {
            "metric_name": "runtime.route_mismatch_total",
            "value": 3,
            "service_name": "codex-runtime",
            "service_version": "v1",
            "runtime_instance_id": "runtime-123",
            "route_engine_mode": "rust",
            "job_id": "job-1",
            "session_id": "session-1",
            "attempt": 2,
            "worker_id": "worker-7",
            "generation": "gen-a",
        }
        record = build_runtime_metric_record(
            "runtime.route_mismatch_total",
            value=3,
            service_name="codex-runtime",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=2,
            worker_id="worker-7",
            generation="gen-a",
        )
        assert record == adapter.runtime_metric_record(request)


def test_observability_helpers_fallback_to_python_when_rust_lane_is_unavailable() -> None:
    class _BrokenRustObservabilityAdapter:
        def runtime_observability_exporter_descriptor(self) -> dict[str, object]:
            raise RuntimeError("rust observability lane unavailable")

        def runtime_observability_metric_catalog(self) -> dict[str, object]:
            raise RuntimeError("rust observability lane unavailable")

        def runtime_observability_dashboard_schema(self) -> dict[str, object]:
            raise RuntimeError("rust observability lane unavailable")

        def runtime_metric_record(self, payload: dict[str, object]) -> dict[str, object]:
            raise RuntimeError("rust observability lane unavailable")

    with patch(
        "framework_runtime.observability._observability_rust_adapter",
        return_value=_BrokenRustObservabilityAdapter(),
    ):
        exporter = build_runtime_observability_exporter_descriptor()
        assert exporter["ownership_lane"] == "rust-contract-lane"
        assert exporter["producer_owner"] == "rust-control-plane"
        assert exporter["exporter_owner"] == "rust-control-plane"
        assert exporter["export_path"] == "jsonl-plus-otel"

        catalog = runtime_observability_metric_catalog()
        assert catalog["schema_version"] == "runtime-observability-metric-catalog-v1"
        assert catalog["metric_catalog_version"] == "runtime-observability-metrics-v1"
        assert tuple(catalog["resource_dimensions"]) == (
            "service.name",
            "service.version",
            "runtime.instance.id",
            "route_engine_mode",
        )

        schema = runtime_observability_dashboard_schema()
        assert schema["schema_version"] == "runtime-observability-dashboard-v1"
        assert tuple(schema["resource_dimensions"]) == RUNTIME_OBSERVABILITY_DASHBOARD_DIMENSIONS

        record = build_runtime_metric_record(
            "runtime.route_mismatch_total",
            value=3,
            service_name="codex-runtime",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=2,
            worker_id="worker-7",
            generation="gen-a",
        )
        assert record["dimensions"] == {
            "runtime.job_id": "job-1",
            "runtime.session_id": "session-1",
            "runtime.attempt": 2,
            "runtime.worker_id": "worker-7",
            "runtime.generation": "gen-a",
            "runtime.stage": "runtime.metric",
            "runtime.status": "ok",
        }


def test_observability_helpers_fail_closed_for_unknown_metrics_and_empty_dimensions() -> None:
    try:
        build_runtime_metric_record(
            "runtime.unknown_total",
            value=1,
            service_name="codex-runtime",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=1,
            worker_id="worker-7",
            generation="gen-a",
        )
    except ValueError as exc:
        assert str(exc) == "unsupported runtime metric: runtime.unknown_total"
    else:
        raise AssertionError("unknown metrics should fail closed")

    try:
        build_runtime_metric_record(
            "runtime.route_mismatch_total",
            value=1,
            service_name=" ",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=1,
            worker_id="worker-7",
            generation="gen-a",
        )
    except ValueError as exc:
        assert str(exc) == "service_name must be a non-empty string"
    else:
        raise AssertionError("empty resource dimensions should fail closed")

    try:
        build_runtime_metric_record(
            "runtime.route_mismatch_total",
            value=float("nan"),
            service_name="codex-runtime",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=1,
            worker_id="worker-7",
            generation="gen-a",
        )
    except ValueError as exc:
        assert str(exc) == "metric value must be finite"
    else:
        raise AssertionError("non-finite metric values should fail closed")

    try:
        build_runtime_metric_record(
            "runtime.route_mismatch_total",
            value=True,
            service_name="codex-runtime",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=1,
            worker_id="worker-7",
            generation="gen-a",
        )
    except ValueError as exc:
        assert str(exc) == "runtime metric record requires a numeric value"
    else:
        raise AssertionError("boolean metric values should fail closed")

    try:
        build_runtime_metric_record(
            "runtime.route_mismatch_total",
            value=1,
            service_name="codex-runtime",
            service_version="v1",
            runtime_instance_id="runtime-123",
            route_engine_mode="rust",
            job_id="job-1",
            session_id="session-1",
            attempt=-1,
            worker_id="worker-7",
            generation="gen-a",
        )
    except ValueError as exc:
        assert str(exc) == "runtime metric record requires non-negative integer field attempt"
    else:
        raise AssertionError("negative attempts should fail closed")
