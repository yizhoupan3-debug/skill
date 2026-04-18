from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.framework_bridge import build_framework_memory_bootstrap
from scripts.memory_support import build_memory_state, load_runtime_snapshot
from scripts.run_memory_automation import migrate_current_artifact_clutter, migrate_legacy_artifact_roots


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _seed_runtime(repo_root: Path, artifact_base: Path, *, task_id: str, task: str) -> None:
    task_root = artifact_base / "current" / task_id
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join([f"- task: {task}", "- phase: implementation", "- status: in_progress"]) + "\n",
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": [f"Continue {task}"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(task_root / "TRACE_METADATA.json", {"task": task, "matched_skills": ["execution-controller-coding"]})
    _write_text(artifact_base / "current" / "SESSION_SUMMARY.md", (task_root / "SESSION_SUMMARY.md").read_text(encoding="utf-8"))
    _write_json(artifact_base / "current" / "NEXT_ACTIONS.json", {"next_actions": [f"Continue {task}"]})
    _write_json(artifact_base / "current" / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(artifact_base / "current" / "TRACE_METADATA.json", {"task": task, "matched_skills": ["execution-controller-coding"]})
    _write_json(artifact_base / "current" / "active_task.json", {"task_id": task_id, "task": task})
    _write_json(
        repo_root / ".supervisor_state.json",
        {
            "task_id": task_id,
            "task_summary": task,
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True, "last_updated_at": "2026-04-18T22:49:57+08:00"},
        },
    )


def test_build_framework_memory_bootstrap_respects_artifact_source_dir(tmp_path: Path) -> None:
    repo_artifacts = tmp_path / "artifacts"
    isolated_artifacts = tmp_path / "isolated-artifacts"
    _seed_runtime(tmp_path, repo_artifacts, task_id="repo-task-20260418210000", task="repo default task")
    _seed_runtime(tmp_path, isolated_artifacts, task_id="isolated-task-20260418220000", task="isolated active task")
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    snapshot = load_runtime_snapshot(tmp_path, artifact_root=isolated_artifacts)
    _write_json(tmp_path / ".codex" / "memory" / "state.json", build_memory_state(snapshot))

    payload = build_framework_memory_bootstrap(
        workspace=tmp_path.name,
        query="isolated active task",
        source_root=tmp_path,
        artifact_source_dir=isolated_artifacts,
        mode="active",
    )

    assert payload["active_task"]["task_id"] == "isolated-task-20260418220000"
    assert payload["continuity"]["task"] == "isolated active task"
    assert payload["retrieval"]["active_task_included"] is True


def test_build_framework_memory_bootstrap_does_not_reuse_completed_task_identity(tmp_path: Path) -> None:
    repo_artifacts = tmp_path / "artifacts"
    _seed_runtime(
        tmp_path,
        repo_artifacts,
        task_id="completed-task-20260418220000",
        task="finished repair lane",
    )
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_id": "completed-task-20260418220000",
            "task_summary": "finished repair lane",
            "active_phase": "completed",
            "verification": {"verification_status": "completed"},
            "continuity": {"story_state": "completed", "resume_allowed": False},
        },
    )
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")

    payload = build_framework_memory_bootstrap(
        workspace=tmp_path.name,
        query="fresh unrelated repair",
        source_root=tmp_path,
        mode="active",
    )

    assert payload["continuity"]["state"] == "completed"
    assert payload["continuity_decision"]["source_task"] is None
    assert payload["continuity_decision"]["task_id"] != "completed-task-20260418220000"


def test_migrate_current_artifact_clutter_clears_non_continuity_entries(tmp_path: Path) -> None:
    current_root = tmp_path / "artifacts" / "current"
    current_root.mkdir(parents=True, exist_ok=True)
    _write_text(current_root / "SESSION_SUMMARY.md", "- task: keep\n")
    _write_json(current_root / "NEXT_ACTIONS.json", {"next_actions": []})
    _write_json(current_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(current_root / "TRACE_METADATA.json", {"matched_skills": []})
    _write_json(current_root / "active_task.json", {"task_id": "keep-task", "task": "keep"})
    task_root = current_root / "keep-task"
    task_root.mkdir()
    _write_text(task_root / "SESSION_SUMMARY.md", "- task: keep\n")
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": []})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(task_root / "TRACE_METADATA.json", {"matched_skills": []})
    _write_json(task_root / ".supervisor_state.json", {"task_id": "keep-task"})
    _write_json(task_root / "framework_default_bootstrap.json", {"bootstrap": {"task": "keep"}})
    _write_json(current_root / "framework_default_bootstrap.json", {"bootstrap": {}})
    _write_json(current_root / "run_summary.json", {"ok": True})
    _write_text(current_root / "vibeproxy-management.log", "log\n")
    (current_root / "tmp-demo").mkdir()

    moved = migrate_current_artifact_clutter(tmp_path, "keep-task")

    assert moved
    assert (tmp_path / "artifacts" / "bootstrap" / "legacy-current" / "framework_default_bootstrap.json").is_file()
    assert (
        tmp_path
        / "artifacts"
        / "bootstrap"
        / "legacy-current"
        / "keep-task"
        / "framework_default_bootstrap.json"
    ).is_file()
    assert (tmp_path / "artifacts" / "ops" / "memory_automation" / "legacy-current" / "run_summary.json").is_file()
    assert (tmp_path / "artifacts" / "evidence" / "legacy-current" / "vibeproxy-management.log").is_file()
    assert (tmp_path / "artifacts" / "scratch" / "tmp-demo").is_dir()
    assert sorted(path.name for path in current_root.iterdir()) == [
        "EVIDENCE_INDEX.json",
        "NEXT_ACTIONS.json",
        "SESSION_SUMMARY.md",
        "TRACE_METADATA.json",
        "active_task.json",
        "keep-task",
    ]
    assert sorted(path.name for path in task_root.iterdir()) == [
        ".supervisor_state.json",
        "EVIDENCE_INDEX.json",
        "NEXT_ACTIONS.json",
        "SESSION_SUMMARY.md",
        "TRACE_METADATA.json",
    ]


def test_migrate_legacy_artifact_roots_moves_old_memory_automation_and_tmp_dirs(tmp_path: Path) -> None:
    legacy_memory_root = tmp_path / "artifacts" / "memory_automation" / "current"
    _write_json(legacy_memory_root / "run_summary.json", {"ok": True})
    _write_json(legacy_memory_root / "storage_audit.json", {"total_mib": 1})
    _write_json(tmp_path / "artifacts" / "tmp-demo" / "runtime_background_jobs.json", {"jobs": []})

    moved = migrate_legacy_artifact_roots(tmp_path)

    assert moved
    assert (
        tmp_path
        / "artifacts"
        / "ops"
        / "memory_automation"
        / "legacy-root"
        / "current"
        / "run_summary.json"
    ).is_file()
    assert (
        tmp_path / "artifacts" / "scratch" / "tmp-demo" / "runtime_background_jobs.json"
    ).is_file()
    assert not (tmp_path / "artifacts" / "memory_automation").exists()
