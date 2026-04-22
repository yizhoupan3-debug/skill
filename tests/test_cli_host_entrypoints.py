from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.claude_hook_audit import run_config_change, run_stop_failure
from scripts.claude_statusline import render_statusline
from scripts.materialize_cli_host_entrypoints import (
    CLAUDE_BACKGROUND_BATCH_COMMAND,
    CLAUDE_REFRESH_COMMAND,
    materialize_repo_host_entrypoints,
    sync_repo_host_entrypoints,
)
from scripts.sync_skills import write_generated_files


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _seed_runtime_artifacts(repo_root: Path) -> None:
    _write_text(
        repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md",
        "\n".join(
            [
                "- task: Validate Claude hooks",
                "- phase: integration",
                "- status: in_progress",
            ]
        )
        + "\n",
    )
    _write_json(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json", {"next_actions": ["Wire hooks"]})
    _write_json(repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(repo_root / "artifacts" / "current" / "TRACE_METADATA.json", {"matched_skills": ["checklist-fixer"]})
    _write_json(
        repo_root / ".supervisor_state.json",
        {
            "task_summary": "Validate Claude hooks",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "execution_contract": {
                "scope": ["Claude hooks"],
                "acceptance_criteria": ["Hooks refresh projection"],
            },
        },
    )


def _seed_shared_memory(repo_root: Path) -> None:
    _write_text(
        repo_root / ".codex" / "memory" / "MEMORY.md",
        "\n".join(
            [
                "# 项目长期记忆",
                "",
                "## Active Patterns",
                "",
                "- AP-1: Externalize complex task state into artifacts",
                "",
                "## 稳定决策",
                "",
                "- SD-1: Generated host files should not drift from source",
                "",
            ]
        )
        + "\n",
    )


def _init_git_repo(repo_root: Path) -> None:
    subprocess.run(["git", "init"], cwd=repo_root, check=True)
    subprocess.run(["git", "config", "user.name", "Codex Test"], cwd=repo_root, check=True)
    subprocess.run(["git", "config", "user.email", "codex@example.com"], cwd=repo_root, check=True)
    (repo_root / "README.md").write_text("seed\n", encoding="utf-8")
    subprocess.run(["git", "add", "README.md"], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", "init"], cwd=repo_root, check=True)


def test_materialize_repo_host_entrypoints_creates_shared_policy_and_host_proxies(
    tmp_path: Path,
) -> None:
    result = materialize_repo_host_entrypoints(tmp_path)

    assert "AGENT.md" in result["written"]
    assert "AGENTS.md" in result["written"]
    assert "CLAUDE.md" in result["written"]
    assert "GEMINI.md" in result["written"]
    assert ".claude/settings.json" in result["written"]
    assert ".gemini/settings.json" in result["written"]

    assert (tmp_path / "AGENT.md").is_file()
    agent_policy = (tmp_path / "AGENT.md").read_text(encoding="utf-8")
    assert "Shared Agent Policy" in agent_policy
    assert "RTK.md" in agent_policy
    assert "## Communication Style" in agent_policy
    assert "## Task Closeout" in agent_policy
    assert "changed-file inventories" in agent_policy
    assert "what now works or what" in agent_policy
    assert "effect was achieved" in agent_policy
    assert "Avoid internal runtime, routing, framework, or tool jargon" in agent_policy
    assert "AGENT.md" in (tmp_path / "AGENTS.md").read_text(encoding="utf-8")
    assert not (tmp_path / ".codex" / "model_instructions.md").exists()
    assert not (tmp_path / ".mcp.json").exists()
    assert "@.claude/CLAUDE.md" in (tmp_path / "CLAUDE.md").read_text(encoding="utf-8")
    assert "AGENT.md" in (tmp_path / "GEMINI.md").read_text(encoding="utf-8")
    assert "@../AGENT.md" in (tmp_path / ".claude" / "CLAUDE.md").read_text(encoding="utf-8")
    assert "@../.codex/memory/CLAUDE_MEMORY.md" in (
        tmp_path / ".claude" / "CLAUDE.md"
    ).read_text(encoding="utf-8")
    assert "scripts/materialize_cli_host_entrypoints.py" in (
        tmp_path / ".claude" / "CLAUDE.md"
    ).read_text(encoding="utf-8")
    settings = json.loads((tmp_path / ".claude" / "settings.json").read_text(encoding="utf-8"))
    assert settings["$schema"] == "https://json.schemastore.org/claude-code-settings.json"
    assert settings["permissions"]["allow"] == [
        "Bash(ls)",
        "Bash(pwd)",
        "Bash(git status)",
        "Bash(git diff)",
        "Bash(python3 scripts/check_skills.py --verify-sync)",
        "Bash(python3 scripts/check_skills.py --verify-codex-link)",
        "Bash(python3 scripts/session_lifecycle_hook.py *)",
        "Bash(python3 scripts/claude_memory_bridge.py *)",
        "Bash(python3 scripts/runtime_background_cli.py *)",
        "Bash(python3 scripts/claude_statusline.py --repo-root *)",
        "Bash(cmp -s TRACE_METADATA.json artifacts/current/TRACE_METADATA.json)",
        "Bash(./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
        "Bash(bash ./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
    ]
    assert settings["allowedMcpServers"] == [{"serverName": "browser-mcp"}]
    assert settings["statusLine"] == {
        "type": "command",
        "command": 'python3 "$CLAUDE_PROJECT_DIR"/scripts/claude_statusline.py --repo-root "$CLAUDE_PROJECT_DIR"',
        "padding": 1,
        "refreshInterval": 30,
    }
    assert set(settings["hooks"]) == {
        "SessionStart",
        "Stop",
        "PreCompact",
        "SubagentStop",
        "SessionEnd",
        "ConfigChange",
        "StopFailure",
    }
    assert json.loads((tmp_path / ".gemini" / "settings.json").read_text(encoding="utf-8")) == {}
    assert (tmp_path / ".claude" / "agents" / "README.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "refresh.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "background_batch.md").is_file()
    refresh_command = (tmp_path / ".claude" / "commands" / "refresh.md").read_text(encoding="utf-8")
    background_batch_command = (
        tmp_path / ".claude" / "commands" / "background_batch.md"
    ).read_text(encoding="utf-8")
    assert refresh_command == CLAUDE_REFRESH_COMMAND
    assert background_batch_command == CLAUDE_BACKGROUND_BATCH_COMMAND
    assert "claude_memory_bridge.py refresh-workflow --json" in refresh_command
    assert "one fixed sentence" in refresh_command
    assert "下一轮执行 prompt 已准备好，并且已经复制到剪贴板。" in refresh_command
    assert "summary" not in refresh_command.lower()
    assert "clear" not in refresh_command.lower()
    assert "CLAUDE_PROJECT_DIR" not in refresh_command
    assert "allowed-tools: Bash(python3 scripts/claude_memory_bridge.py *)" in refresh_command
    assert "runtime_background_cli.py" in background_batch_command
    assert "enqueue-batch" in background_batch_command
    assert "group-summary" in background_batch_command
    assert "list-groups" in background_batch_command
    assert "allowed-tools: Bash(python3 scripts/runtime_background_cli.py *)" in background_batch_command
    assert (tmp_path / ".claude" / "hooks" / "README.md").is_file()
    hooks_readme = (tmp_path / ".claude" / "hooks" / "README.md").read_text(encoding="utf-8")
    assert "Generated-first maintenance" in hooks_readme
    assert "Event-level lifecycle decisions live in `.claude/hooks/README.md`." in (
        tmp_path / ".claude" / "CLAUDE.md"
    ).read_text(encoding="utf-8")
    for marker in (
        "`StopFailure` | enabled",
        "`ConfigChange` | enabled",
        "audit-only stderr guidance",
        "host-private failure classification hint",
        "`InstructionsLoaded` | document-disable",
        "`PostToolUse` | document-disable",
        "UserPromptSubmit",
        "Notification",
    ):
        assert marker in hooks_readme
    assert "UserPromptSubmit" in hooks_readme
    assert (tmp_path / ".claude" / "hooks" / "session_start.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "stop.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "pre_compact.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "subagent_stop.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "session_end.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "config_change.sh").is_file()
    assert (tmp_path / ".claude" / "hooks" / "stop_failure.sh").is_file()
    assert (tmp_path / "configs" / "claude" / "CLAUDE.md").is_file()
    assert (tmp_path / "configs" / "gemini" / "GEMINI.md").is_file()


def test_materialize_repo_host_entrypoints_is_idempotent(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    result = materialize_repo_host_entrypoints(tmp_path)

    assert result["written"] == []
    assert "AGENT.md" in result["unchanged"]
    assert ".claude/settings.json" in result["unchanged"]


def test_sync_repo_host_entrypoints_reports_drift_without_writing(tmp_path: Path) -> None:
    result = sync_repo_host_entrypoints(tmp_path, apply=False)

    assert "AGENT.md" in result["written"]
    assert not (tmp_path / "AGENT.md").exists()


def test_materialize_repo_host_entrypoints_syncs_matching_worktrees(tmp_path: Path) -> None:
    _init_git_repo(tmp_path)
    peer_worktree = tmp_path / ".claude" / "worktrees" / "agent-peer"
    peer_worktree.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["git", "worktree", "add", str(peer_worktree), "--detach"], cwd=tmp_path, check=True)

    result = materialize_repo_host_entrypoints(tmp_path)

    assert str(peer_worktree.resolve()) in result["synced_worktrees"]
    assert (peer_worktree / ".claude" / "commands" / "refresh.md").is_file()
    assert (peer_worktree / ".claude" / "commands" / "background_batch.md").is_file()
    assert (
        peer_worktree / ".claude" / "commands" / "refresh.md"
    ).read_text(encoding="utf-8") == CLAUDE_REFRESH_COMMAND
    assert (
        peer_worktree / ".claude" / "commands" / "background_batch.md"
    ).read_text(encoding="utf-8") == CLAUDE_BACKGROUND_BATCH_COMMAND


def test_write_generated_files_includes_shared_cli_entrypoints_when_repo_is_dirty(tmp_path: Path) -> None:
    root = PROJECT_ROOT
    managed_paths = [
        root / "AGENT.md",
        root / "AGENTS.md",
        root / "CLAUDE.md",
        root / "GEMINI.md",
        root / ".claude" / "CLAUDE.md",
        root / ".claude" / "settings.json",
        root / ".claude" / "agents" / "README.md",
        root / ".claude" / "commands" / "refresh.md",
        root / ".claude" / "commands" / "background_batch.md",
        root / ".claude" / "hooks" / "README.md",
        root / ".claude" / "hooks" / "session_start.sh",
        root / ".claude" / "hooks" / "stop.sh",
        root / ".claude" / "hooks" / "pre_compact.sh",
        root / ".claude" / "hooks" / "subagent_stop.sh",
        root / ".claude" / "hooks" / "session_end.sh",
        root / ".claude" / "hooks" / "config_change.sh",
        root / ".claude" / "hooks" / "stop_failure.sh",
        root / ".gemini" / "settings.json",
        root / "configs" / "claude" / "CLAUDE.md",
        root / "configs" / "codex" / "AGENTS.md",
        root / "configs" / "gemini" / "GEMINI.md",
    ]
    backups = {
        path: path.read_text(encoding="utf-8") if path.exists() else None
        for path in managed_paths
    }

    target = root / "CLAUDE.md"
    original = target.read_text(encoding="utf-8")
    target.write_text(original + "\nDRIFT\n", encoding="utf-8")
    try:
        changed = write_generated_files(apply=False)
        assert "CLAUDE.md" in changed
    finally:
        for path, content in backups.items():
            if content is None:
                if path.exists():
                    path.unlink()
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")


def test_materialized_claude_hooks_execute_without_error(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    (tmp_path / "scripts").symlink_to(PROJECT_ROOT / "scripts", target_is_directory=True)
    _seed_runtime_artifacts(tmp_path)
    _seed_shared_memory(tmp_path)

    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(tmp_path)

    for script_name in (
        "session_start.sh",
        "stop.sh",
        "pre_compact.sh",
        "subagent_stop.sh",
        "config_change.sh",
        "stop_failure.sh",
    ):
        payload = None
        if script_name == "config_change.sh":
            payload = '{"hook_event_name":"ConfigChange","scope":"project_settings","changed_path":".claude/settings.json"}\n'
        elif script_name == "stop_failure.sh":
            payload = '{"hook_event_name":"StopFailure","failure_type":"server_error"}\n'
        subprocess.run(
            ["sh", str(tmp_path / ".claude" / "hooks" / script_name)],
            cwd=tmp_path,
            check=True,
            env=env,
            input=payload,
            text=True,
        )
        assert (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").is_file()
        assert not (tmp_path / ".codex" / "memory" / "MEMORY_AUTO.md").exists()

    subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "session_end.sh")],
        cwd=tmp_path,
        check=True,
        env=env,
    )
    assert (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").is_file()
    assert (tmp_path / ".codex" / "memory" / "state.json").is_file()
    assert not (tmp_path / ".codex" / "memory" / "MEMORY_AUTO.md").exists()


def test_claude_statusline_renders_runtime_summary(tmp_path: Path) -> None:
    _write_text(
        tmp_path / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Validate status line",
            "- phase: integration",
            "- status: in_progress",
        ])
        + "\n",
    )
    _write_json(
        tmp_path / "TRACE_METADATA.json",
        {
            "matched_skills": ["execution-controller-coding", "checklist-fixer"],
            "verification_status": "completed",
        },
    )
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_summary": "Fallback task",
            "active_phase": "finalized",
            "verification": {"verification_status": "completed"},
        },
    )

    statusline = render_statusline(tmp_path)

    assert "task=Validate status line" in statusline
    assert "integration/in_progress" in statusline
    assert "route=execution-controller-coding+1" in statusline
    assert "git=nogit" in statusline


def test_claude_statusline_prefers_task_scoped_runtime_over_stale_root_mirrors(tmp_path: Path) -> None:
    task_root = tmp_path / "artifacts" / "current" / "fresh-task-20260419013000"
    _write_json(
        tmp_path / "artifacts" / "current" / "active_task.json",
        {"task_id": "fresh-task-20260419013000", "task": "Fresh current task"},
    )
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Fresh current task",
            "- phase: integration",
            "- status: in_progress",
        ])
        + "\n",
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": ["Ship the fix"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        task_root / "TRACE_METADATA.json",
        {
            "matched_skills": ["execution-controller-coding", "agent-memory"],
            "verification_status": "in_progress",
        },
    )
    _write_text(
        tmp_path / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Stale root task",
            "- phase: finalized",
            "- status: completed",
        ])
        + "\n",
    )
    _write_json(
        tmp_path / "TRACE_METADATA.json",
        {
            "matched_skills": ["checklist-fixer"],
            "verification_status": "completed",
        },
    )
    _write_json(tmp_path / "NEXT_ACTIONS.json", {"next_actions": ["Ignore me"]})
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_summary": "Fresh current task",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
        },
    )

    statusline = render_statusline(tmp_path)

    assert "task=Fresh current task" in statusline
    assert "integration/in_progress" in statusline
    assert "route=execution-controller-coding+1" in statusline
    assert "Stale root task" not in statusline


def test_claude_statusline_prefers_supervisor_owned_actions_over_stale_sidecars(tmp_path: Path) -> None:
    task_root = tmp_path / "artifacts" / "current" / "fresh-task-20260419013000"
    _write_json(
        tmp_path / "artifacts" / "current" / "active_task.json",
        {"task_id": "fresh-task-20260419013000", "task": "Fresh current task"},
    )
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Fresh current task",
            "- phase: verification",
            "- status: completed",
        ])
        + "\n",
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": ["stale sidecar action"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        task_root / "TRACE_METADATA.json",
        {
            "task": "Fresh current task",
            "matched_skills": ["legacy-skill"],
            "verification_status": "completed",
            "routing_runtime_version": 0,
        },
    )
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_id": "fresh-task-20260419013000",
            "task_summary": "Fresh current task",
            "active_phase": "verification",
            "verification": {"verification_status": "completed"},
            "continuity": {"story_state": "completed", "resume_allowed": False},
            "next_actions": ["Ship the real follow-up"],
            "controller": {
                "primary_owner": "execution-controller-coding",
                "gate": "subagent-delegation",
            },
        },
    )

    statusline = render_statusline(tmp_path)

    assert "next=Ship the real follow-up" in statusline
    assert "route=subagent-delegation+1" in statusline
    assert "legacy-skill" not in statusline


def test_claude_hook_audit_reports_generated_surface_drift(tmp_path: Path, capsys) -> None:
    result = run_config_change(
        tmp_path,
        {
            "scope": "project_settings",
            "changed_path": str(tmp_path / ".claude" / "settings.json"),
        },
    )

    captured = capsys.readouterr()
    assert result == 0
    assert "generated Claude host surfaces" in captured.err
    assert "scripts/materialize_cli_host_entrypoints.py" in captured.err


def test_claude_hook_audit_reports_stop_failure_without_mutation(tmp_path: Path, capsys) -> None:
    result = run_stop_failure(
        tmp_path,
        {
            "failure_type": "server_error",
            "context": "host projection",
        },
    )

    captured = capsys.readouterr()
    assert result == 0
    assert "server_error" in captured.err
    assert "host-private projection drift" in captured.err
