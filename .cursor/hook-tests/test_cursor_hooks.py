#!/usr/bin/env python3
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
HOOK = ROOT / ".cursor" / "hooks" / "review_subagent_gate.py"


def run_hook(payload: dict, *, event: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(HOOK), "--event", event],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        cwd=cwd,
        timeout=10,
    )


def parsed_stdout(result: subprocess.CompletedProcess[str]) -> dict:
    return json.loads(result.stdout) if result.stdout.strip() else {}


def assert_true(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def cleanup_state() -> None:
    shutil.rmtree(ROOT / ".cursor" / "hook-state", ignore_errors=True)


def test_review_prompt_followup_until_subagent_seen() -> None:
    cleanup_state()
    session_id = f"cursor-review-{uuid.uuid4()}"
    prompt = run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，按严重级别给出问题",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    assert_true(prompt.returncode == 0, "beforeSubmitPrompt exits cleanly")
    assert_true(parsed_stdout(prompt).get("continue") is True, "beforeSubmitPrompt should continue")

    stop_before = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": 0},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop_before),
        "stop should emit followup before subagent",
    )

    mark_seen = run_hook(
        {"session_id": session_id, "cwd": str(ROOT)},
        event="subagentStart",
        cwd=ROOT,
    )
    assert_true(mark_seen.returncode == 0, "subagentStart exits cleanly")

    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" not in parsed_stdout(stop_after),
        "stop should clear followup after subagent",
    )


def test_state_survives_cwd_drift() -> None:
    cleanup_state()
    session_id = f"cursor-cwd-{uuid.uuid4()}"
    private_tmp = Path("/private/tmp")

    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT / "scripts"),
            "prompt": "并行审查 API、数据库和 UI 风险",
        },
        event="beforeSubmitPrompt",
        cwd=private_tmp,
    )

    # Stop event uses a different cwd, but should still resolve to repo .cursor/hook-state.
    stop_before = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "2"},
        event="stop",
        cwd=private_tmp,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop_before),
        "state should be found even when shell cwd drifts",
    )


def main() -> int:
    test_review_prompt_followup_until_subagent_seen()
    test_state_survives_cwd_drift()
    cleanup_state()
    print("cursor hook tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
