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


def _run_bridge(repo_root: Path, event: str, payload: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(BRIDGE_SCRIPT),
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


def test_pre_tool_use_bridge_filters_claude_audit_metadata(tmp_path: Path) -> None:
    _write_text(
        tmp_path / ".claude" / "hooks" / "run.sh",
        "\n".join(
            [
                "#!/bin/sh",
                "shift",
                "cat >/dev/null",
                "printf '%s\\n' '{\"schema_version\":\"v1\",\"authority\":\"claude\",\"command\":\"pre-tool-use\",\"decision\":\"deny\",\"path\":\".claude/settings.json\",\"message\":\"blocked\",\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"blocked\"}}'",
            ]
        )
        + "\n",
    )

    result = _run_bridge(
        tmp_path,
        "pre-tool-use",
        '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    assert json.loads(result.stdout) == {
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "blocked",
        },
    }


def test_pre_tool_use_bridge_stays_silent_when_shared_hook_is_silent(tmp_path: Path) -> None:
    _write_text(
        tmp_path / ".claude" / "hooks" / "run.sh",
        "\n".join(
            [
                "#!/bin/sh",
                "shift",
                "cat >/dev/null",
            ]
        )
        + "\n",
    )

    result = _run_bridge(
        tmp_path,
        "pre-tool-use",
        '{"tool_name":"Bash","tool_input":{"command":"cat README.md"}}\n',
    )

    assert result.returncode == 0
    assert result.stdout == ""
    assert result.stderr == ""


def test_user_prompt_submit_bridge_filters_claude_audit_metadata(tmp_path: Path) -> None:
    _write_text(
        tmp_path / ".claude" / "hooks" / "run.sh",
        "\n".join(
            [
                "#!/bin/sh",
                "shift",
                "cat >/dev/null",
                "printf '%s\\n' '{\"schema_version\":\"v1\",\"authority\":\"claude\",\"command\":\"user-prompt-submit\",\"hookSpecificOutput\":{\"hookEventName\":\"UserPromptSubmit\",\"additionalContext\":\"热路径优先\"}}'",
            ]
        )
        + "\n",
    )

    result = _run_bridge(
        tmp_path,
        "user-prompt-submit",
        '{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime"}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    assert json.loads(result.stdout) == {
        "systemMessage": "热路径优先",
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": "热路径优先",
        }
    }


def test_user_prompt_submit_bridge_fails_open_with_visible_degraded_context_when_shared_hook_errors(tmp_path: Path) -> None:
    _write_text(
        tmp_path / ".claude" / "hooks" / "run.sh",
        "\n".join(
            [
                "#!/bin/sh",
                "shift",
                "cat >/dev/null",
                "exit 7",
            ]
        )
        + "\n",
    )

    result = _run_bridge(
        tmp_path,
        "user-prompt-submit",
        '{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime"}\n',
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["hookEventName"] == "UserPromptSubmit"
    assert "repo-local hook 注入本轮降级" in payload["hookSpecificOutput"]["additionalContext"]
    assert "status 7" in payload["hookSpecificOutput"]["additionalContext"]
    assert payload["systemMessage"] == payload["hookSpecificOutput"]["additionalContext"]


def test_user_prompt_submit_bridge_fails_open_with_visible_degraded_context_when_runner_is_missing(tmp_path: Path) -> None:
    result = _run_bridge(
        tmp_path,
        "user-prompt-submit",
        '{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime"}\n',
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["hookEventName"] == "UserPromptSubmit"
    assert "repo-local hook 注入本轮降级" in payload["hookSpecificOutput"]["additionalContext"]
    assert "missing shared hook script" in payload["hookSpecificOutput"]["additionalContext"]


def test_legacy_codex_hook_bridge_delegates_to_framework_bridge(tmp_path: Path) -> None:
    _write_text(
        tmp_path / ".claude" / "hooks" / "run.sh",
        "\n".join(
            [
                "#!/bin/sh",
                "shift",
                "cat >/dev/null",
                "printf '%s\\n' '{\"decision\":\"deny\",\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\"}}'",
            ]
        )
        + "\n",
    )

    result = subprocess.run(
        [
            sys.executable,
            str(LEGACY_BRIDGE_SCRIPT),
            "--repo-root",
            str(tmp_path),
            "--event",
            "pre-tool-use",
        ],
        input='{"tool_name":"Bash"}\n',
        text=True,
        capture_output=True,
        env=os.environ.copy(),
        check=False,
    )

    assert result.returncode == 0
    assert result.stderr == ""
    assert json.loads(result.stdout) == {
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
        },
    }
