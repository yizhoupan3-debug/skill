#!/usr/bin/env python3
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
HOOK = ROOT / ".codex" / "hooks" / "review_subagent_gate.py"


def run_hook(payload: dict) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(HOOK)],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        cwd=ROOT,
        timeout=10,
    )


def parsed_stdout(result: subprocess.CompletedProcess[str]) -> dict:
    return json.loads(result.stdout) if result.stdout.strip() else {}


def assert_true(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def cleanup_state() -> None:
    shutil.rmtree(ROOT / ".codex" / "hook-state", ignore_errors=True)


def test_broad_review_requires_subagent_until_seen() -> None:
    cleanup_state()
    session_id = f"codex-review-{uuid.uuid4()}"
    prompt = run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review这个仓库，找 bug 并按严重程度给 findings。",
        }
    )
    assert_true(prompt.returncode == 0, "prompt hook exits cleanly")
    assert_true("additionalContext" in prompt.stdout, "prompt hook adds review guidance")

    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "stop blocks before subagent")

    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "explorer", "message": "Review one independent lane."},
        }
    )
    allowed = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(allowed).get("decision") != "block", "stop passes after subagent")


def test_plain_review_sentence_does_not_block() -> None:
    cleanup_state()
    session_id = f"codex-review-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "I will review my notes later.",
        }
    )
    allowed = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(allowed).get("decision") != "block", "generic review sentence does not block")


def test_parallel_lanes_require_subagent_until_seen() -> None:
    cleanup_state()
    session_id = f"codex-delegation-{uuid.uuid4()}"
    prompt = run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "并行检查 API、数据库和 UI 三个模块的回归风险",
        }
    )
    assert_true(prompt.returncode == 0, "prompt hook exits cleanly")
    assert_true("additionalContext" in prompt.stdout, "prompt hook adds delegation guidance")

    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "stop blocks before sidecar")

    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "explorer", "message": "Review API regression lane."},
        }
    )
    allowed = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(allowed).get("decision") != "block", "stop passes after sidecar")


def main() -> int:
    test_broad_review_requires_subagent_until_seen()
    test_plain_review_sentence_does_not_block()
    test_parallel_lanes_require_subagent_until_seen()
    cleanup_state()
    print("codex hook tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
