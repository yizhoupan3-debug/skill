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
    assert "phase1 rollout" in summary_path.read_text(encoding="utf-8")
    next_actions_payload = json.loads(next_actions_path.read_text(encoding="utf-8"))
    evidence_payload = json.loads(evidence_path.read_text(encoding="utf-8"))
    assert next_actions_payload["schema_version"] == "next-actions-v2"
    assert next_actions_payload["next_actions"] == [
        "add loadouts",
        "wire approval middleware",
    ]
    assert evidence_payload["schema_version"] == "evidence-index-v2"
    assert evidence_payload["artifacts"][0]["kind"] == "report"


def test_write_artifacts_supports_task_scoped_output_and_mirror(tmp_path: Path) -> None:
    paths = write_artifacts(
        tmp_path / "artifacts" / "current",
        task="codex-first convergence",
        phase="implementation",
        status="in_progress",
        summary="Task-scoped continuity is now the source of truth.",
        next_actions=["run sync", "refresh mirrors"],
        evidence=[],
        task_id="codex-first-convergence-20260418210000",
        mirror_output_dir=tmp_path / "artifacts" / "current",
    )

    assert Path(paths["summary"]).parent.name == "codex-first-convergence-20260418210000"
    assert (tmp_path / "artifacts" / "current" / "SESSION_SUMMARY.md").is_file()
    assert paths["task_id"] == "codex-first-convergence-20260418210000"
