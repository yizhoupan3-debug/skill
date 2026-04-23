"""Regression tests for standard session artifact generation."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.checkpoint_store import FilesystemRuntimeCheckpointer


def write_artifacts(
    output_dir: Path,
    *,
    task: str,
    phase: str,
    status: str,
    summary: str,
    next_actions: list[str],
    evidence: list[dict[str, object]],
    task_id: str | None = None,
    mirror_output_dir: Path | None = None,
    repo_root: Path | None = None,
    focus: bool = False,
) -> dict[str, str]:
    payload = {
        "output_dir": str(output_dir),
        "task": task,
        "phase": phase,
        "status": status,
        "summary": summary,
        "next_actions": next_actions,
        "evidence": evidence,
        "task_id": task_id,
        "mirror_output_dir": str(mirror_output_dir) if mirror_output_dir is not None else None,
        "repo_root": str(repo_root) if repo_root is not None else None,
        "focus": focus,
    }
    debug_binary = PROJECT_ROOT / "scripts" / "router-rs" / "target" / "debug" / "router-rs"
    if not debug_binary.is_file():
        subprocess.run(
            [
                "cargo",
                "build",
                "--quiet",
                "--manifest-path",
                str(PROJECT_ROOT / "scripts" / "router-rs" / "Cargo.toml"),
            ],
            cwd=PROJECT_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
    completed = subprocess.run(
        [
            str(debug_binary),
            "--framework-session-artifact-write-json",
            "--framework-session-artifact-write-input-json",
            json.dumps(payload, ensure_ascii=False),
        ],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    resolved = json.loads(completed.stdout)
    assert isinstance(resolved, dict)
    return {
        "summary": resolved["summary"],
        "next_actions": resolved["next_actions"],
        "evidence": resolved["evidence"],
        "task_id": resolved["task_id"],
    }


def _read_task_registry(repo_root: Path) -> dict[str, object]:
    path = repo_root / "artifacts" / "current" / "task_registry.json"
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}


def test_write_artifacts_creates_all_phase1_contract_files(tmp_path: Path) -> None:
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


def test_write_artifacts_supports_task_scoped_output_and_focus_mirror(tmp_path: Path) -> None:
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
        focus=True,
    )

    assert Path(paths["summary"]).parent.name == "codex-first-convergence-20260418210000"
    assert (tmp_path / "artifacts" / "current" / "SESSION_SUMMARY.md").is_file()
    assert paths["task_id"] == "codex-first-convergence-20260418210000"


def test_write_artifacts_only_registers_background_tasks_without_focus_projection(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    output_dir = repo_root / "artifacts" / "current"

    paths = write_artifacts(
        output_dir,
        task="background rollout",
        phase="implementation",
        status="in_progress",
        summary="Keep this task out of shared mirrors.",
        next_actions=["stay scoped"],
        evidence=[],
        repo_root=repo_root,
    )

    registry = _read_task_registry(repo_root)

    assert Path(paths["summary"]).parent.name == paths["task_id"]
    assert registry["tasks"][0]["task_id"] == paths["task_id"]
    assert registry["focus_task_id"] is None
    assert not (repo_root / "artifacts" / "current" / "active_task.json").exists()
    assert not (repo_root / "artifacts" / "current" / "focus_task.json").exists()
    assert not (repo_root / "SESSION_SUMMARY.md").exists()


def test_write_artifacts_refreshes_focus_projection_when_requested(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    output_dir = repo_root / "artifacts" / "current"

    paths = write_artifacts(
        output_dir,
        task="pointer refresh rollout",
        phase="review",
        status="in_progress",
        summary="Refresh the active task pointer.",
        next_actions=["verify pointer"],
        evidence=[],
        repo_root=repo_root,
        focus=True,
    )

    pointer_path = repo_root / "artifacts" / "current" / "active_task.json"
    focus_path = repo_root / "artifacts" / "current" / "focus_task.json"
    assert pointer_path.exists()
    assert focus_path.exists()
    pointer = json.loads(pointer_path.read_text(encoding="utf-8"))
    focus_pointer = json.loads(focus_path.read_text(encoding="utf-8"))
    assert pointer["task"] == "pointer refresh rollout"
    assert pointer["task_id"] == paths["task_id"]
    assert focus_pointer["task_id"] == paths["task_id"]
    assert Path(paths["summary"]).parent.name == paths["task_id"]
    assert (repo_root / "SESSION_SUMMARY.md").exists()
    assert (repo_root / "NEXT_ACTIONS.json").exists()
    assert (repo_root / "EVIDENCE_INDEX.json").exists()

    checkpointer = FilesystemRuntimeCheckpointer(data_dir=repo_root / ".runtime")
    artifact_paths = checkpointer.artifact_paths(codex_home=repo_root)
    assert str((repo_root / "SESSION_SUMMARY.md").resolve()) in artifact_paths
    assert str((repo_root / "NEXT_ACTIONS.json").resolve()) in artifact_paths
    assert str((repo_root / "EVIDENCE_INDEX.json").resolve()) in artifact_paths
