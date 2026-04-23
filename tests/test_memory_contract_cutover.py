from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.consolidate_memory import archive_legacy_memory_bundle, persist_memory_bundle
from scripts.default_bootstrap import run_default_bootstrap
from scripts.memory_store import MemoryItem, MemoryStore
from framework_runtime.rust_router import RustRouteAdapter
from scripts.memory_support import build_memory_state, load_runtime_snapshot
from scripts.run_memory_automation import (
    migrate_current_artifact_clutter,
    migrate_legacy_artifact_roots,
    run_pipeline,
)


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _rust_adapter() -> RustRouteAdapter:
    return RustRouteAdapter(PROJECT_ROOT)


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


def test_framework_memory_recall_respects_artifact_source_dir_and_task_id(tmp_path: Path) -> None:
    repo_artifacts = tmp_path / "artifacts"
    isolated_artifacts = tmp_path / "isolated-artifacts"
    _seed_runtime(tmp_path, repo_artifacts, task_id="repo-task-20260418210000", task="repo default task")
    _seed_runtime(tmp_path, isolated_artifacts, task_id="isolated-task-20260418220000", task="isolated active task")
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    snapshot = load_runtime_snapshot(tmp_path, artifact_root=isolated_artifacts)
    _write_json(tmp_path / ".codex" / "memory" / "state.json", build_memory_state(snapshot))
    payload = _rust_adapter().framework_memory_recall(
        repo_root=tmp_path,
        query="isolated active task",
        artifact_source_dir=isolated_artifacts,
        task_id="isolated-task-20260418220000",
        top=8,
        mode="active",
    )

    assert payload["active_task"]["task_id"] == "isolated-task-20260418220000"
    assert payload["continuity"]["task"] == "isolated active task"
    assert payload["retrieval"]["active_task_included"] is True
    assert payload["continuity"]["paths"]["supervisor_state"] == str(
        tmp_path / ".supervisor_state.json"
    )
    assert (
        payload["source_artifacts"]["artifact_lanes"]["bootstrap"]
        == str(isolated_artifacts / "bootstrap" / "<task_id>")
    )
    assert (
        payload["source_artifacts"]["artifact_lanes"]["ops_memory_automation"]
        == str(isolated_artifacts / "ops" / "memory_automation" / "<run_id>")
    )
    assert (
        payload["source_artifacts"]["artifact_lanes"]["evidence"]
        == str(isolated_artifacts / "evidence" / "<task_id>")
    )
    assert (
        payload["source_artifacts"]["artifact_lanes"]["scratch"]
        == str(isolated_artifacts / "scratch" / "<run_id>")
    )


def test_framework_memory_recall_uses_rust_authority_on_default_path(tmp_path: Path) -> None:
    _seed_runtime(
        tmp_path,
        tmp_path / "artifacts",
        task_id="bootstrap-task-20260418220000",
        task="bootstrap task",
    )
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    payload = _rust_adapter().framework_memory_recall(
        repo_root=tmp_path,
        query="bootstrap task",
        top=8,
        mode="active",
    )

    assert payload["continuity"]["state"] == "active"
    assert payload["continuity"]["task"] == "bootstrap task"
    assert payload["continuity_decision"]["source_task"] == "bootstrap task"
    assert payload["continuity_decision"]["task_id"] == "bootstrap-task-20260418220000"


def test_framework_memory_recall_does_not_reuse_completed_task_identity(tmp_path: Path) -> None:
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

    payload = _rust_adapter().framework_memory_recall(
        repo_root=tmp_path,
        query="fresh unrelated repair",
        top=8,
        mode="active",
    )

    assert payload["continuity"]["state"] == "completed"
    assert payload["continuity_decision"]["source_task"] is None
    assert payload["continuity_decision"]["task_id"] != "completed-task-20260418220000"


def test_framework_memory_recall_bootstraps_empty_memory_through_rust(
    tmp_path: Path,
) -> None:
    _seed_runtime(
        tmp_path,
        tmp_path / "artifacts",
        task_id="bootstrap-task-20260418220000",
        task="bootstrap task",
    )
    memory_root = tmp_path / ".codex" / "memory"
    _write_text(memory_root / "MEMORY_AUTO.md", "# legacy auto memory\n")
    _write_text(memory_root / "sessions" / "2026-04-18.md", "old session\n")

    payload = _rust_adapter().framework_memory_recall(
        repo_root=tmp_path,
        query="bootstrap task",
        top=8,
        mode="active",
    )

    assert payload["consolidation_note"] == "memory_workspace was empty; bridge ran one-shot consolidation"
    assert (memory_root / "MEMORY.md").is_file()
    assert any(path.endswith("MEMORY.md") for path in payload["changed_files"])
    assert not (memory_root / "MEMORY_AUTO.md").exists()
    assert not (memory_root / "sessions").exists()
    assert list((memory_root / "archive").glob("pre-cutover-*/MEMORY_AUTO.md"))
    assert list((memory_root / "archive").glob("pre-cutover-*/sessions"))


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


def test_run_pipeline_defaults_to_planning_migrations_without_mutating_live_artifacts(
    tmp_path: Path,
    monkeypatch,
) -> None:
    _seed_runtime(
        tmp_path,
        tmp_path / "artifacts",
        task_id="keep-task-20260418220000",
        task="keep task",
    )
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    stray_dir = tmp_path / "artifacts" / "current" / "older-task"
    stray_dir.mkdir(parents=True)
    _write_text(stray_dir / "SESSION_SUMMARY.md", "- task: older\n")
    monkeypatch.setattr(
        "scripts.run_memory_automation.collect_storage_report",
        lambda *_args, **_kwargs: {"root": str(tmp_path / ".codex"), "total_mib": 1.0, "top_entries": []},
    )

    result = run_pipeline(workspace=tmp_path.name, source_root=tmp_path)

    assert result["apply_artifact_migrations"] is False
    assert result["planned_current_artifact_migrations"]
    assert result["moved_current_artifacts"] == []
    assert stray_dir.is_dir()


def test_archive_legacy_memory_bundle_exports_and_clears_non_authoritative_memory_rows(
    tmp_path: Path,
) -> None:
    memory_root = tmp_path / ".codex" / "memory"
    memory_root.mkdir(parents=True, exist_ok=True)
    _write_text(memory_root / "MEMORY.md", "# 项目长期记忆\n\n- stable row\n")
    store = MemoryStore.for_workspace(tmp_path.name, resolved_dir=memory_root)
    store.upsert_memory_item(
        MemoryItem(
            item_id=f"{tmp_path.name}:task_state:1",
            category="task_state",
            source="current_state_extractor",
            summary="stale task state",
        )
    )

    result = archive_legacy_memory_bundle(tmp_path.name, memory_root)

    assert result["legacy_memory_item_count"] == 1
    dump = json.loads((Path(result["archive_root"]) / "sqlite_legacy_dump.json").read_text(encoding="utf-8"))
    assert dump["memory_items"][0]["source"] == "current_state_extractor"
    remaining_sources = {
        row["source"] for row in store.list_memory_items(limit=100, active=False)
    }
    assert "current_state_extractor" not in remaining_sources


def test_persist_memory_bundle_prunes_non_authoritative_memory_sources(tmp_path: Path) -> None:
    memory_root = tmp_path / ".codex" / "memory"
    memory_root.mkdir(parents=True, exist_ok=True)
    store = MemoryStore.for_workspace(tmp_path.name, resolved_dir=memory_root)
    store.upsert_memory_item(
        MemoryItem(
            item_id=f"{tmp_path.name}:task_state:1",
            category="task_state",
            source="current_state_extractor",
            summary="stale task state",
        )
    )

    persist_memory_bundle(
        tmp_path.name,
        {"MEMORY.md": "# 项目长期记忆\n\n- stable row\n"},
        resolved_dir=memory_root,
    )

    rows = store.list_memory_items(limit=100, active=False)
    assert {row["source"] for row in rows} == {"MEMORY.md"}


def test_persist_memory_bundle_stores_heading_scoped_segments(tmp_path: Path) -> None:
    memory_root = tmp_path / ".codex" / "memory"
    memory_root.mkdir(parents=True, exist_ok=True)
    store = MemoryStore.for_workspace(tmp_path.name, resolved_dir=memory_root)
    document = "\n".join(
        [
            "# 项目长期记忆",
            "",
            "## 稳定决策",
            "",
            "### 执行编排",
            "",
            "- framework bootstrap 只 propose 上下文",
            "",
        ]
    )

    persist_memory_bundle(
        tmp_path.name,
        {
            "MEMORY.md": document + "\n",
            "preferences.md": "# preferences\n",
            "decisions.md": "# decisions\n",
            "lessons.md": "# lessons\n",
            "runbooks.md": "# runbooks\n",
        },
        resolved_dir=memory_root,
    )

    rows = store.list_memory_items(limit=100, active=False)
    memory_rows = [row for row in rows if row["source"] == "MEMORY.md"]
    assert len(memory_rows) == 1
    row = memory_rows[0]
    assert row["summary"] == "framework bootstrap 只 propose 上下文"
    assert row["notes"] == "稳定决策 / 执行编排"
    metadata = json.loads(row["metadata_json"])
    assert metadata["headings"] == ["稳定决策", "执行编排"]


def test_default_bootstrap_compacts_evolution_payload(tmp_path: Path) -> None:
    _seed_runtime(
        tmp_path,
        tmp_path / "artifacts",
        task_id="bootstrap-task-20260418220000",
        task="bootstrap task",
    )
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")

    result = run_default_bootstrap(repo_root=tmp_path, output_dir=tmp_path / "out", workspace=tmp_path.name)

    payload = result["payload"]["evolution-proposals"]
    assert set(payload) == {"proposal_count", "proposals"}


def test_default_bootstrap_uses_prompt_safe_memory_payload(tmp_path: Path) -> None:
    _seed_runtime(
        tmp_path,
        tmp_path / "artifacts",
        task_id="bootstrap-task-20260418220000",
        task="bootstrap task",
    )
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")

    result = run_default_bootstrap(
        repo_root=tmp_path,
        output_dir=tmp_path / "out",
        workspace=tmp_path.name,
        query="artifact anchors",
    )

    payload = result["payload"]["memory-bootstrap"]
    assert "retrieval" in payload
    assert "memory_root" not in payload
    assert "source_artifacts" not in payload
