"""Regression tests for the runtime observability contract."""

from __future__ import annotations

import json
import re
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
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
