from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.consolidate_memory import persist_memory_bundle
from scripts.memory_store import MemoryItem, MemoryStore
from scripts.memory_support import build_memory_state, load_runtime_snapshot
from scripts.retrieve_memory import render_context


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _seed_runtime(repo_root: Path, *, task: str = "active bootstrap repair") -> None:
    task_id = "active-bootstrap-repair-20260418210000"
    task_root = repo_root / "artifacts" / "current" / task_id
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join(
            [
                f"- task: {task}",
                "- phase: implementation",
                "- status: in_progress",
            ]
        )
        + "\n",
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": ["Patch classifier", "Run pytest"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(task_root / "TRACE_METADATA.json", {"task": task, "matched_skills": ["execution-controller-coding"]})
    _write_text(repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md", (task_root / "SESSION_SUMMARY.md").read_text(encoding="utf-8"))
    _write_json(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json", {"next_actions": ["Patch classifier", "Run pytest"]})
    _write_json(repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(repo_root / "artifacts" / "current" / "TRACE_METADATA.json", {"task": task, "matched_skills": ["execution-controller-coding"]})
    _write_json(repo_root / "artifacts" / "current" / "active_task.json", {"task_id": task_id, "task": task})
    _write_json(
        repo_root / ".supervisor_state.json",
        {
            "task_id": task_id,
            "task_summary": task,
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True, "last_updated_at": "2026-04-18T22:49:57+08:00"},
            "blockers": {"open_blockers": ["Need regression coverage"]},
        },
    )


def _seed_stable_memory(repo_root: Path) -> None:
    memory_root = repo_root / ".codex" / "memory"
    _write_text(memory_root / "MEMORY.md", "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: Stable only by default\n")
    _write_text(memory_root / "preferences.md", "# preferences\n\n- prefer compact recall\n")
    snapshot = load_runtime_snapshot(repo_root)
    _write_json(memory_root / "state.json", build_memory_state(snapshot))


def _seed_sqlite_memory(repo_root: Path) -> None:
    memory_root = repo_root / ".codex" / "memory"
    store = MemoryStore.for_workspace(repo_root.name, resolved_dir=memory_root)
    store.upsert_memory_item(
        MemoryItem(
            item_id="sqlite-item-1",
            category="general",
            source="sqlite",
            summary="sqlite-only row",
            notes="diagnostic row",
        )
    )


def test_render_context_stable_mode_excludes_active_task_and_archive(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _write_text(tmp_path / ".codex" / "memory" / "archive" / "pre-cutover-2026-04-18" / "sessions" / "2026-04-18.md", "task=old\n")

    result = render_context(
        workspace=tmp_path.name,
        topic="active bootstrap repair",
        repo_root=tmp_path,
        mode="stable",
    )

    assert result["mode"] == "stable"
    assert result["active_task_included"] is False
    assert all(item["path"] != "runtime/current_task.md" for item in result["items"])
    assert all("archive/" not in item["path"] for item in result["items"])


def test_render_context_active_mode_includes_matching_current_task_when_fresh(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)

    result = render_context(
        workspace=tmp_path.name,
        topic="active bootstrap repair",
        repo_root=tmp_path,
        mode="active",
    )

    assert result["active_task_included"] is True
    assert result["freshness"]["state"] == "fresh"
    assert any(item["path"] == "runtime/current_task.md" for item in result["items"])


def test_render_context_active_mode_refreshes_stale_memory_state(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _write_json(
        tmp_path / ".codex" / "memory" / "state.json",
        {
            "schema_version": "memory-state-v1",
            "source_task_id": "older-task",
            "content_hash": "stale",
            "source_updated_at": "2026-04-18T20:00:00+08:00",
        },
    )

    result = render_context(
        workspace=tmp_path.name,
        topic="active bootstrap repair",
        repo_root=tmp_path,
        mode="active",
    )

    assert result["active_task_included"] is True
    assert result["freshness"]["state"] == "fresh"
    state = json.loads((tmp_path / ".codex" / "memory" / "state.json").read_text(encoding="utf-8"))
    assert state["source_task_id"] == "active-bootstrap-repair-20260418210000"


def test_render_context_active_mode_self_heals_missing_memory_state(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    memory_root = tmp_path / ".codex" / "memory"
    _write_text(memory_root / "MEMORY.md", "# 项目长期记忆\n")
    _write_text(memory_root / "preferences.md", "# preferences\n")

    result = render_context(
        workspace=tmp_path.name,
        topic="active bootstrap repair",
        repo_root=tmp_path,
        mode="active",
    )

    assert result["active_task_included"] is True
    assert (memory_root / "state.json").is_file()
    assert result["freshness"]["state"] == "fresh"


def test_render_context_history_mode_can_read_archive(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _write_text(
        tmp_path / ".codex" / "memory" / "archive" / "pre-cutover-2026-04-18" / "sessions" / "2026-04-18.md",
        "task=old closeout\n",
    )

    result = render_context(
        workspace=tmp_path.name,
        topic="old closeout",
        repo_root=tmp_path,
        mode="history",
    )

    assert any("archive/" in item["path"] for item in result["items"])


def test_default_modes_do_not_include_sqlite_sections(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _seed_sqlite_memory(tmp_path)

    for mode in ("stable", "active", "history"):
        result = render_context(
            workspace=tmp_path.name,
            topic="sqlite",
            repo_root=tmp_path,
            mode=mode,
        )
        assert all(not item["path"].startswith("sqlite/") for item in result["items"])


def test_debug_mode_exposes_sqlite_sections(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    _seed_stable_memory(tmp_path)
    _seed_sqlite_memory(tmp_path)

    result = render_context(
        workspace=tmp_path.name,
        topic="sqlite",
        repo_root=tmp_path,
        mode="debug",
    )

    assert any(item["path"] == "sqlite/memory_items.md" for item in result["items"])


def test_stable_recall_does_not_fallback_on_partial_sqlite_token_match(tmp_path: Path) -> None:
    _seed_runtime(tmp_path)
    memory_root = tmp_path / ".codex" / "memory"
    memory_root.mkdir(parents=True, exist_ok=True)
    document = "\n".join(
        [
            "# 项目长期记忆",
            "",
            "## 稳定决策",
            "",
            "### 执行编排",
            "",
            "- runtime contract only",
            "",
        ]
    )
    _write_text(memory_root / "MEMORY.md", document + "\n")
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

    result = render_context(
        workspace=tmp_path.name,
        topic="runtime observability",
        repo_root=tmp_path,
        mode="stable",
    )

    assert result["items"] == []
