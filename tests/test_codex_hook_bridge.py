from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
BRIDGE_SCRIPT = PROJECT_ROOT / "scripts" / "codex_hook_bridge.py"


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


def test_codex_hook_bridge_blocks_generated_host_edits(tmp_path: Path) -> None:
    result = _run_bridge(
        tmp_path,
        "pre-tool-use",
        '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    payload = json.loads(result.stdout)
    assert payload["decision"] == "block"
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".claude/settings.json" in payload["hookSpecificOutput"]["permissionDecisionReason"]


def test_codex_hook_bridge_surfaces_user_prompt_context(tmp_path: Path) -> None:
    result = _run_bridge(
        tmp_path,
        "user-prompt-submit",
        '{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime"}\n',
    )

    assert result.returncode == 0
    assert result.stderr == ""
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["hookEventName"] == "UserPromptSubmit"
    assert payload["systemMessage"] == payload["hookSpecificOutput"]["additionalContext"]
    assert "repo-local shared memory" in payload["systemMessage"]
