"""Regression tests for runtime trace metadata generation."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.write_trace_metadata import load_routing_runtime_version, write_trace_metadata


def test_write_trace_metadata_emits_required_fields(tmp_path: Path) -> None:
    """Verify trace writer emits the expected required keys.

    Parameters:
        tmp_path: Temporary pytest directory fixture.

    Returns:
        None.
    """

    output = tmp_path / "TRACE_METADATA.json"
    write_trace_metadata(
        output,
        task="full outline rollout",
        matched_skills=["execution-controller-coding", "execution-audit"],
        owner="execution-controller-coding",
        gate="delegation",
        overlay="execution-audit",
        reroute_count=1,
        retry_count=2,
        artifact_paths=["artifacts/current/SESSION_SUMMARY.md"],
        verification_status="passed",
    )

    data = json.loads(output.read_text(encoding="utf-8"))
    assert data["schema_version"] == "trace-metadata-v2"
    assert data["task"] == "full outline rollout"
    assert data["matched_skills"] == [
        "execution-controller-coding",
        "execution-audit",
    ]
    assert data["decision"]["owner"] == "execution-controller-coding"
    assert data["reroute_count"] == 1
    assert data["retry_count"] == 2
    assert data["artifact_paths"] == ["artifacts/current/SESSION_SUMMARY.md"]
    assert data["verification_status"] == "passed"


def test_write_trace_metadata_mirror_outputs_are_byte_identical(tmp_path: Path) -> None:
    """Verify one materialization keeps root and mirror perfectly aligned."""

    output = tmp_path / "TRACE_METADATA.json"
    mirror = tmp_path / "artifacts" / "current" / "TRACE_METADATA.json"
    write_trace_metadata(
        output,
        task="trace drift closure",
        matched_skills=["execution-controller-coding", "checklist-fixer"],
        owner="checklist-fixer",
        gate="subagent-delegation",
        overlay=None,
        reroute_count=0,
        retry_count=0,
        artifact_paths=["TRACE_METADATA.json", "artifacts/current/TRACE_METADATA.json"],
        verification_status="completed",
        mirror_paths=[mirror],
    )

    assert output.read_text(encoding="utf-8") == mirror.read_text(encoding="utf-8")


def test_write_trace_metadata_loads_runtime_version_when_omitted(tmp_path: Path) -> None:
    output = tmp_path / "TRACE_METADATA.json"

    write_trace_metadata(
        output,
        task="trace drift closure",
        matched_skills=["execution-controller-coding"],
        owner="execution-controller-coding",
        gate="delegation",
        overlay=None,
        reroute_count=0,
        retry_count=0,
        artifact_paths=[],
        verification_status="completed",
    )

    payload = json.loads(output.read_text(encoding="utf-8"))
    assert payload["routing_runtime_version"] == load_routing_runtime_version()
