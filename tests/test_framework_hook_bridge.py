from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
BRIDGE_SCRIPT = PROJECT_ROOT / "scripts" / "framework_hook_bridge.py"
LEGACY_BRIDGE_SCRIPT = PROJECT_ROOT / "scripts" / "codex_hook_bridge.py"


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _run_bridge(script: Path, repo_root: Path, event: str, payload: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(script),
            "--repo-root",
            str(repo_root),
            "--event",
            event,
        ],
        input=payload,
        text=True,
        capture_output=True,
        env=os.environ.copy(),
        check=False,
    )


def test_pre_tool_use_bridge_blocks_generated_host_edits(tmp_path: Path) -> None:
    result = _run_bridge(
        BRIDGE_SCRIPT,
        tmp_path,
        "pre-tool-use",
        '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    payload = json.loads(result.stdout)
    assert payload["decision"] == "block"
    assert payload["hookSpecificOutput"]["hookEventName"] == "PreToolUse"
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".claude/settings.json" in payload["hookSpecificOutput"]["permissionDecisionReason"]
    assert "materialize_cli_host_entrypoints.py" in payload["hookSpecificOutput"]["permissionDecisionReason"]


def test_pre_tool_use_bridge_stays_silent_when_allowed(tmp_path: Path) -> None:
    result = _run_bridge(
        BRIDGE_SCRIPT,
        tmp_path,
        "pre-tool-use",
        '{"tool_name":"Bash","tool_input":{"command":"cat README.md"}}\n',
    )

    assert result.returncode == 0
    assert result.stdout == ""
    assert result.stderr == ""


def test_pre_tool_use_bridge_allows_read_only_generated_surface_access(tmp_path: Path) -> None:
    result = _run_bridge(
        BRIDGE_SCRIPT,
        tmp_path,
        "pre-tool-use",
        '{"tool_name":"Bash","tool_input":{"command":"cat .claude/settings.json"}}\n',
    )

    assert result.returncode == 0
    assert result.stdout == ""
    assert result.stderr == ""


def test_user_prompt_submit_bridge_surfaces_codex_context(tmp_path: Path) -> None:
    result = _run_bridge(
        BRIDGE_SCRIPT,
        tmp_path,
        "user-prompt-submit",
        '{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime，去掉补丁式保底并顺手看内存和速度"}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["hookEventName"] == "UserPromptSubmit"
    assert payload["systemMessage"] == payload["hookSpecificOutput"]["additionalContext"]
    assert "repo-local shared memory" in payload["systemMessage"]
    assert "当前状态：" in payload["systemMessage"]


def test_permission_request_bridge_maps_deny_to_codex_permission_shape(tmp_path: Path) -> None:
    result = _run_bridge(
        BRIDGE_SCRIPT,
        tmp_path,
        "permission-request",
        '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["hookEventName"] == "PermissionRequest"
    assert payload["hookSpecificOutput"]["decision"]["behavior"] == "deny"
    assert ".claude/settings.json" in payload["hookSpecificOutput"]["decision"]["message"]
    assert "materialize_cli_host_entrypoints.py" in payload["hookSpecificOutput"]["decision"]["message"]


def test_permission_request_bridge_stays_silent_when_not_targeting_generated_surface(
    tmp_path: Path,
) -> None:
    result = _run_bridge(
        BRIDGE_SCRIPT,
        tmp_path,
        "permission-request",
        '{"tool_name":"Bash","tool_input":{"command":"cp tmp notes.txt"}}\n',
    )

    assert result.returncode == 0
    assert result.stdout == ""
    assert result.stderr == ""


def test_legacy_codex_hook_bridge_delegates_to_rust_bridge(tmp_path: Path) -> None:
    result = _run_bridge(
        LEGACY_BRIDGE_SCRIPT,
        tmp_path,
        "pre-tool-use",
        '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    payload = json.loads(result.stdout)
    assert payload["decision"] == "block"
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
