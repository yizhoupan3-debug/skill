from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.claude_memory_bridge import run_bridge, sync_claude_memory_projection


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _seed_runtime_artifacts(repo_root: Path, *, mode: str = "active") -> None:
    if mode == "active":
        summary_lines = [
            "- task: Validate Claude bridge",
            "- phase: integration",
            "- status: in_progress",
        ]
        next_actions = {"next_actions": ["Wire hooks", "Verify sync"]}
        trace_metadata = {"matched_skills": ["skill-developer-codex", "agent-memory"]}
        supervisor_state = {
            "task_summary": "Validate Claude bridge",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
            "blockers": {"open_blockers": ["Need lifecycle hook wiring"]},
            "execution_contract": {
                "scope": ["Claude lifecycle bridge", "shared memory projection"],
                "acceptance_criteria": ["Claude imports shared memory", "hooks refresh projection"],
            },
        }
    elif mode == "completed":
        summary_lines = [
            "- task: checklist-series final closeout",
            "- phase: finalized",
            "- status: completed",
        ]
        next_actions = {"next_actions": ["Open a new task before reviving related work"]}
        trace_metadata = {"task": "checklist-series final closeout", "matched_skills": ["checklist-fixer"]}
        supervisor_state = {
            "task_summary": "checklist-series final closeout",
            "active_phase": "finalized",
            "verification": {"verification_status": "completed"},
            "continuity": {"story_state": "completed", "resume_allowed": False},
        }
    elif mode == "stale":
        summary_lines = []
        next_actions = {"next_actions": ["Do not inject stale continuity"]}
        trace_metadata = {"matched_skills": ["execution-controller-coding"]}
        supervisor_state = {
            "task_summary": "stale bridge lane",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "continuity": {
                "story_state": "active",
                "resume_allowed": False,
                "state_reason": "superseded by a newer continuity bundle",
            },
        }
    else:
        raise ValueError(f"Unsupported mode: {mode}")

    _write_text(
        repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md",
        "\n".join(summary_lines) + ("\n" if summary_lines else ""),
    )
    _write_json(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json", next_actions)
    _write_json(
        repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json",
        {"artifacts": [{"kind": "doc", "path": "CLAUDE.md", "status": "ok"}]},
    )
    _write_json(repo_root / "artifacts" / "current" / "TRACE_METADATA.json", trace_metadata)
    _write_json(repo_root / ".supervisor_state.json", supervisor_state)


def _seed_shared_memory(repo_root: Path) -> None:
    _write_text(
        repo_root / ".codex" / "memory" / "MEMORY.md",
        "\n".join(
            [
                "# 项目长期记忆",
                "",
                "## Active Patterns",
                "",
                "- AP-1: Sync skills after skill edits",
                "- AP-2: Externalize complex task state into artifacts",
                "",
                "## 稳定决策",
                "",
                "- SD-1: Shared CLI memory root lives under `./.codex/memory/`",
                "",
                "## Lessons",
                "",
                "- L-1: Do not let generated host files drift from runtime truth",
                "",
            ]
        )
        + "\n",
    )


def test_sync_claude_memory_projection_renders_shared_runtime_state(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path)
    _seed_shared_memory(tmp_path)

    result = sync_claude_memory_projection(tmp_path)

    target = tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md"
    content = target.read_text(encoding="utf-8")
    assert result["changed"] is True
    assert "Validate Claude bridge" in content
    assert "AP-1: Sync skills after skill edits" in content
    assert "SD-1: Shared CLI memory root lives under `./.codex/memory/`" in content
    assert "`artifacts/current/SESSION_SUMMARY.md`" in content
    assert "logical->physical memory mapping" in content
    assert "sync rule:" in content


def test_run_bridge_session_end_consolidates_then_refreshes_projection(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, mode="completed")
    _seed_shared_memory(tmp_path)

    result = run_bridge("session-end", tmp_path)

    assert result["canonical_command"] == "session-end"
    assert result["contract"]["consolidates_shared_memory"] is True
    assert "consolidation" in result
    assert (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").is_file()
    assert (tmp_path / ".codex" / "memory" / "state.json").is_file()
    assert not (tmp_path / ".codex" / "memory" / "MEMORY_AUTO.md").exists()
    projection = (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").read_text(encoding="utf-8")
    assert "## Recent Completed Task" in projection
    assert "Current Execution State" not in projection


def test_run_bridge_projection_only_commands_refresh_without_consolidation(tmp_path: Path) -> None:
    for command in (
        "sync",
        "refresh-projection",
        "session-start",
        "session-stop",
        "pre-compact",
        "subagent-stop",
    ):
        case_root = tmp_path / command
        _seed_runtime_artifacts(case_root)
        _seed_shared_memory(case_root)

        result = run_bridge(command, case_root)

        expected_canonical = "refresh-projection" if command in {"sync", "refresh-projection"} else command
        assert result["canonical_command"] == expected_canonical
        assert result["contract"]["consolidates_shared_memory"] is False
        assert "consolidation" not in result
        assert (case_root / ".codex" / "memory" / "CLAUDE_MEMORY.md").is_file()
        assert not (case_root / ".codex" / "memory" / "MEMORY_AUTO.md").exists()


def test_sync_claude_memory_projection_marks_completed_tasks_as_recent_completed(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, mode="completed")
    _seed_shared_memory(tmp_path)

    sync_claude_memory_projection(tmp_path)

    content = (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").read_text(encoding="utf-8")
    assert "## Recent Completed Task" in content
    assert "checklist-series final closeout" in content
    assert "current_execution_injection: blocked" in content
    assert "## Current Execution State" not in content


def test_sync_claude_memory_projection_blocks_stale_continuity(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, mode="stale")
    _seed_shared_memory(tmp_path)

    sync_claude_memory_projection(tmp_path)

    content = (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").read_text(encoding="utf-8")
    assert "## Stale Continuity Warning" in content
    assert "current_execution_injection: blocked" in content
    assert "stale bridge lane" in content


@pytest.mark.parametrize(
    "command",
    ("stop-failure", "config-change", "instructions-loaded", "post-tool-use"),
)
def test_run_bridge_rejects_unwired_candidate_commands(tmp_path: Path, command: str) -> None:
    with pytest.raises(ValueError, match="Unsupported bridge command"):
        run_bridge(command, tmp_path)
