"""Regression tests for standard session artifact generation."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.write_session_artifacts import write_artifacts


def test_write_artifacts_creates_all_phase1_contract_files(tmp_path: Path) -> None:
    """Verify artifact writer creates the three standard files.

    Parameters:
        tmp_path: Temporary pytest directory fixture.

    Returns:
        None.
    """

    paths = write_artifacts(
        tmp_path,
        task="phase1 rollout",
        phase="implementation",
        status="completed",
        summary="Implemented source precedence and artifact contract.",
        next_actions=["add loadouts", "wire approval middleware"],
        evidence=[{"kind": "report", "path": "artifacts/report.md"}],
    )

    summary_path = Path(paths["summary"])
    next_actions_path = Path(paths["next_actions"])
    evidence_path = Path(paths["evidence"])

    assert summary_path.exists()
    assert next_actions_path.exists()
    assert evidence_path.exists()
    assert paths["task_id"] == "phase1-rollout"
    assert "phase1 rollout" in summary_path.read_text(encoding="utf-8")
    assert json.loads(next_actions_path.read_text(encoding="utf-8"))["next_actions"] == [
        "add loadouts",
        "wire approval middleware",
    ]
    assert json.loads(evidence_path.read_text(encoding="utf-8"))["artifacts"][0]["kind"] == "report"


def test_write_artifacts_refreshes_active_task_pointer_when_repo_root_is_present(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    output_dir = repo_root / "artifacts" / "contracts" / "demo-task"

    paths = write_artifacts(
        output_dir,
        task="demo task",
        phase="implementation",
        status="completed",
        summary="Closed the continuity pointer gap.",
        next_actions=["verify resume loader"],
        evidence=[{"kind": "test", "path": "tests/test_write_session_artifacts.py"}],
        repo_root=repo_root,
    )

    pointer_path = repo_root / "artifacts" / "current" / "active_task.json"
    assert pointer_path.exists()
    assert json.loads(pointer_path.read_text(encoding="utf-8"))["task_id"] == paths["task_id"]
