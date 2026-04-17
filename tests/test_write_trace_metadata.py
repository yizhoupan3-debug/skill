"""Regression tests for runtime trace metadata generation."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.write_trace_metadata import write_trace_metadata


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
        matched_skills=["execution-controller-coding", "execution-audit-codex"],
        owner="execution-controller-coding",
        gate="delegation",
        overlay="execution-audit-codex",
        reroute_count=1,
        retry_count=2,
        artifact_paths=["artifacts/current/SESSION_SUMMARY.md"],
        verification_status="passed",
    )

    data = json.loads(output.read_text(encoding="utf-8"))
    assert data["task"] == "full outline rollout"
    assert data["decision"]["owner"] == "execution-controller-coding"
    assert data["reroute_count"] == 1
    assert data["retry_count"] == 2
    assert data["artifact_paths"] == ["artifacts/current/SESSION_SUMMARY.md"]
    assert data["verification_status"] == "passed"
