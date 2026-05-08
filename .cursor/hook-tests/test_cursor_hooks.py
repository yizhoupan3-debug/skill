#!/usr/bin/env python3
from __future__ import annotations

import json
import hashlib
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


def state_file(session_id: str) -> Path:
    key = hashlib.sha256(session_id.encode("utf-8")).hexdigest()[:32]
    return ROOT / ".cursor" / "hook-state" / f"review-subagent-{key}.json"


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
    assert_true("followup_message" in parsed_stdout(prompt), "beforeSubmitPrompt should proactively request subagent")

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
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "description": "independent review lane",
            "subagent_type": "generalPurpose",
        },
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


def test_unrelated_subagent_does_not_clear_gate() -> None:
    cleanup_state()
    session_id = f"cursor-unrelated-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，按严重级别给出问题",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "description": "implement feature X"},
        event="subagentStart",
        cwd=ROOT,
    )
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop_after),
        "unrelated subagent should not clear review gate",
    )


def test_empty_subagent_payload_does_not_clear_gate() -> None:
    cleanup_state()
    session_id = f"cursor-empty-subagent-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，按严重级别给出问题",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    run_hook(
        {"session_id": session_id, "cwd": str(ROOT)},
        event="subagentStart",
        cwd=ROOT,
    )
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop_after),
        "empty subagent event should not clear review gate",
    )


def test_reject_reason_allows_completion_without_subagent() -> None:
    cleanup_state()
    session_id = f"cursor-reject-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，但 reject_reason: shared_context_heavy",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" not in parsed_stdout(stop_after),
        "reject reason should satisfy gate without subagent",
    )


def test_reject_reason_substring_does_not_bypass_gate() -> None:
    cleanup_state()
    session_id = f"cursor-reject-substr-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，备注 my_small_task_note",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop_after),
        "substring match must not satisfy reject reason",
    )


def test_override_synonym_disables_gate() -> None:
    cleanup_state()
    session_id = f"cursor-override-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，但不要分路，直接处理",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" not in parsed_stdout(stop_after),
        "override synonym should disable gate",
    )


def test_narrow_file_review_does_not_followup() -> None:
    cleanup_state()
    session_id = f"cursor-narrow-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "review ./README.md",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" not in parsed_stdout(stop_after),
        "narrow file review should not trigger gate",
    )


def test_english_pr_review_triggers_gate_without_this_keyword() -> None:
    cleanup_state()
    session_id = f"cursor-pr-{uuid.uuid4()}"
    prompt = run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "please review pull request 456 for regressions",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    assert_true("followup_message" in parsed_stdout(prompt), "english PR review should trigger proactive followup")
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true("followup_message" in parsed_stdout(stop_after), "english PR review should enforce gate at stop")


def test_english_deep_repo_review_with_severity_triggers_gate() -> None:
    cleanup_state()
    session_id = f"cursor-repo-{uuid.uuid4()}"
    prompt = run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "do a deep review of this repository and rank findings by severity",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    assert_true("followup_message" in parsed_stdout(prompt), "deep repo severity review should trigger proactive followup")


def test_parallel_lane_only_english_triggers_delegation_gate() -> None:
    cleanup_state()
    session_id = f"cursor-lanes-{uuid.uuid4()}"
    prompt = run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "split into independent lanes and run in parallel",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    assert_true("followup_message" in parsed_stdout(prompt), "lane-only english should trigger delegation gate")


def test_corrupted_state_file_emits_warning() -> None:
    cleanup_state()
    session_id = f"cursor-corrupt-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "并行审查 API、数据库和 UI 风险",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )

    bogus = state_file(session_id)
    bogus.parent.mkdir(parents=True, exist_ok=True)
    bogus.write_text("{ invalid json", encoding="utf-8")

    stop = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "0"},
        event="stop",
        cwd=ROOT,
    )
    out = parsed_stdout(stop)
    assert_true(
        "followup_message" in out and "state is unreadable or unavailable" in out["followup_message"],
        "corrupted state should emit enforcement warning",
    )


def test_missing_state_file_emits_warning() -> None:
    cleanup_state()
    session_id = f"cursor-missing-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "并行审查 API、数据库和 UI 风险",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    state_file(session_id).unlink(missing_ok=True)
    stop = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1", "prompt": "并行审查 API、数据库和 UI 风险"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop) and "state is missing" in parsed_stdout(stop)["followup_message"],
        "missing state should emit enforcement warning",
    )


def test_missing_state_without_review_prompt_stays_quiet() -> None:
    cleanup_state()
    stop = run_hook(
        {"session_id": f"cursor-missing-quiet-{uuid.uuid4()}", "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(parsed_stdout(stop) == {}, "missing state should not warn for non-review stop payload")


def test_state_read_failure_emits_warning() -> None:
    cleanup_state()
    session_id = f"cursor-read-fail-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "并行审查 API、数据库和 UI 风险",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    path = state_file(session_id)
    path.unlink(missing_ok=True)
    path.mkdir(parents=True, exist_ok=True)
    stop = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop) and "state is unreadable or unavailable" in parsed_stdout(stop)["followup_message"],
        "state read failure should emit warning",
    )


def test_invalid_agent_type_does_not_clear_gate() -> None:
    cleanup_state()
    session_id = f"cursor-invalid-agent-{uuid.uuid4()}"
    run_hook(
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，按严重级别给出问题",
        },
        event="beforeSubmitPrompt",
        cwd=ROOT,
    )
    run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "agent_type": "not-a-real-type"},
        event="subagentStart",
        cwd=ROOT,
    )
    stop_after = run_hook(
        {"session_id": session_id, "cwd": str(ROOT), "loop_count": "1"},
        event="stop",
        cwd=ROOT,
    )
    assert_true(
        "followup_message" in parsed_stdout(stop_after),
        "invalid agent type should not clear review gate",
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
    test_unrelated_subagent_does_not_clear_gate()
    test_empty_subagent_payload_does_not_clear_gate()
    test_reject_reason_allows_completion_without_subagent()
    test_reject_reason_substring_does_not_bypass_gate()
    test_override_synonym_disables_gate()
    test_narrow_file_review_does_not_followup()
    test_english_pr_review_triggers_gate_without_this_keyword()
    test_english_deep_repo_review_with_severity_triggers_gate()
    test_parallel_lane_only_english_triggers_delegation_gate()
    test_corrupted_state_file_emits_warning()
    test_missing_state_file_emits_warning()
    test_missing_state_without_review_prompt_stays_quiet()
    test_state_read_failure_emits_warning()
    test_invalid_agent_type_does_not_clear_gate()
    test_state_survives_cwd_drift()
    cleanup_state()
    print("cursor hook tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
