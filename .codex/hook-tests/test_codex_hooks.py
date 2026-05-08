#!/usr/bin/env python3
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import json
import hashlib
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


def state_file(session_id: str) -> Path:
    key = hashlib.sha256(session_id.encode("utf-8")).hexdigest()[:32]
    return ROOT / ".codex" / "hook-state" / f"review-subagent-{key}.json"


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


def test_plain_prompt_requires_subagent_by_default() -> None:
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
    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "default policy should require subagent")


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


def test_autopilot_goal_mode_blocks_without_goal_evidence() -> None:
    cleanup_state()
    session_id = f"codex-goal-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "/autopilot 修复回归",
        }
    )
    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "explorer", "message": "independent lane"},
        }
    )
    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    reason = parsed_stdout(blocked).get("reason", "")
    assert_true(
        parsed_stdout(blocked).get("decision") == "block"
        and "Autopilot goal mode requires a goal contract" in reason,
        "autopilot goal mode should block without goal evidence",
    )


def test_autopilot_goal_mode_passes_with_goal_evidence() -> None:
    cleanup_state()
    session_id = f"codex-goal-ok-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "/autopilot Goal: fix bug Done when: tests pass Validation commands: pytest",
        }
    )
    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "explorer", "message": "independent lane"},
        }
    )
    allowed = run_hook(
        {
            "hook_event_name": "Stop",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "checkpoint progress next step; verification passed",
        }
    )
    assert_true(parsed_stdout(allowed).get("decision") != "block", "autopilot goal mode should pass with evidence")


def test_autopilot_goal_mode_escalates_after_repeated_no_progress() -> None:
    cleanup_state()
    session_id = f"codex-goal-stall-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "/autopilot Goal: fix bug Done when: tests pass Validation commands: pytest",
        }
    )
    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.spawn_agent",
            "tool_input": {"agent_type": "explorer", "message": "independent lane"},
        }
    )
    first = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT), "prompt": "checkpoint progress next step"})
    second = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT), "prompt": "checkpoint progress next step"})
    assert_true("No-progress loops: 1" in parsed_stdout(first).get("reason", ""), "first no-progress block should be counted")
    assert_true(
        "No-progress loops: 2" in parsed_stdout(second).get("reason", "")
        and "No-progress threshold exceeded" in parsed_stdout(second).get("reason", "")
        and "Next action template" in parsed_stdout(second).get("reason", ""),
        "second no-progress block should escalate",
    )


def test_reject_reason_allows_completion_without_subagent() -> None:
    cleanup_state()
    session_id = f"codex-reject-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，但 reject_reason: shared_context_heavy",
        }
    )
    allowed = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(allowed).get("decision") != "block", "explicit reject reason should satisfy gate")


def test_non_subagent_tool_does_not_clear_gate() -> None:
    cleanup_state()
    session_id = f"codex-non-tool-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review这个仓库，找 bug 并按严重程度给 findings。",
        }
    )
    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.shell",
            "tool_input": {"message": "security plan task"},
        }
    )
    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "non-subagent tool must not satisfy gate")


def test_typed_unrelated_subagent_clears_gate() -> None:
    cleanup_state()
    session_id = f"codex-unrelated-typed-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review这个仓库，找 bug 并按严重程度给 findings。",
        }
    )
    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.spawn_agent",
            "tool_input": {
                "agent_type": "generalPurpose",
                "description": "implement feature X",
            },
        }
    )
    allowed = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(allowed).get("decision") != "block", "typed subagent should satisfy gate without lane keywords")


def test_subagent_tool_evidence_without_lane_keywords_clears_gate() -> None:
    cleanup_state()
    session_id = f"codex-no-lane-kw-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review这个仓库，找 bug 并按严重程度给 findings。",
        }
    )
    run_hook(
        {
            "hook_event_name": "PostToolUse",
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.subagent",
            "tool_input": {
                "subagent_type": "explore",
                "description": "collect data",
            },
        }
    )
    allowed = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(allowed).get("decision") != "block", "valid subagent evidence should satisfy gate without lane keywords")


def test_corrupted_state_file_fails_closed() -> None:
    cleanup_state()
    session_id = f"codex-corrupt-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "并行检查 API、数据库和 UI 三个模块的回归风险",
        }
    )
    bogus = state_file(session_id)
    bogus.parent.mkdir(parents=True, exist_ok=True)
    bogus.write_text("{ invalid json", encoding="utf-8")
    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "corrupted state should fail closed")


def test_state_survives_cwd_drift() -> None:
    cleanup_state()
    session_id = f"codex-cwd-{uuid.uuid4()}"
    private_tmp = Path("/private/tmp")
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT / "scripts"),
            "prompt": "并行检查 API、数据库和 UI 三个模块的回归风险",
        }
    )
    stop_before = subprocess.run(
        [sys.executable, str(HOOK)],
        input=json.dumps({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT), "loop_count": "2"}),
        text=True,
        capture_output=True,
        cwd=private_tmp,
        timeout=10,
    )
    assert_true(parsed_stdout(stop_before).get("decision") == "block", "state should resolve despite cwd drift")


def test_missing_state_file_fails_closed() -> None:
    cleanup_state()
    session_id = f"codex-missing-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review这个仓库，找 bug 并按严重程度给 findings。",
        }
    )
    state_file(session_id).unlink(missing_ok=True)
    blocked = run_hook(
        {
            "hook_event_name": "Stop",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review这个仓库，找 bug 并按严重程度给 findings。",
        }
    )
    assert_true(parsed_stdout(blocked).get("decision") == "block", "missing state should fail closed")


def test_missing_state_without_review_prompt_blocks_conservatively() -> None:
    cleanup_state()
    blocked = run_hook({"hook_event_name": "Stop", "session_id": f"codex-missing-quiet-{uuid.uuid4()}", "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "missing state should block even without review stop payload")


def test_reject_reason_substring_does_not_bypass_gate() -> None:
    cleanup_state()
    session_id = f"codex-reject-substr-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，备注 my_shared_context_heavy_note",
        }
    )
    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "substring match must not satisfy reject reason")


def test_override_synonym_disables_gate() -> None:
    cleanup_state()
    session_id = f"codex-override-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，但不要分路，直接处理",
        }
    )
    allowed = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(allowed).get("decision") != "block", "override synonym should disable gate")


def test_state_read_failure_fails_closed() -> None:
    cleanup_state()
    session_id = f"codex-read-fail-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "并行检查 API、数据库和 UI 三个模块的回归风险",
        }
    )
    path = state_file(session_id)
    path.unlink(missing_ok=True)
    path.mkdir(parents=True, exist_ok=True)
    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT)})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "state read failure should fail closed")


def test_english_pr_review_triggers_gate_without_this_keyword() -> None:
    cleanup_state()
    session_id = f"codex-pr-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "please review pull request 456 for regressions",
        }
    )
    blocked = run_hook({"hook_event_name": "Stop", "session_id": session_id, "cwd": str(ROOT), "prompt": "please review pull request 456 for regressions"})
    assert_true(parsed_stdout(blocked).get("decision") == "block", "english PR review should trigger gate")


def test_english_deep_repo_review_with_severity_triggers_gate() -> None:
    cleanup_state()
    session_id = f"codex-repo-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "do a deep review of this repository and rank findings by severity",
        }
    )
    blocked = run_hook(
        {
            "hook_event_name": "Stop",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "do a deep review of this repository and rank findings by severity",
        }
    )
    assert_true(parsed_stdout(blocked).get("decision") == "block", "deep repo severity review should trigger gate")


def test_parallel_lane_only_english_triggers_delegation_gate() -> None:
    cleanup_state()
    session_id = f"codex-lanes-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "split into independent lanes and run in parallel for backend and database checks",
        }
    )
    blocked = run_hook(
        {
            "hook_event_name": "Stop",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "split into independent lanes and run in parallel for backend and database checks",
        }
    )
    assert_true(parsed_stdout(blocked).get("decision") == "block", "lane-only english should trigger delegation gate")


def test_generic_prompt_without_lanes_still_requires_subagent() -> None:
    cleanup_state()
    session_id = f"codex-generic-parallel-{uuid.uuid4()}"
    run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "run checks in parallel and return quickly",
        }
    )
    blocked = run_hook(
        {
            "hook_event_name": "Stop",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "run checks in parallel and return quickly",
        }
    )
    assert_true(parsed_stdout(blocked).get("decision") == "block", "default policy should still require subagent")


def test_review_and_parallel_both_emit_prompt_context() -> None:
    cleanup_state()
    session_id = f"codex-dual-followup-{uuid.uuid4()}"
    prompt = run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review并行分路检查 API、数据库、UI 的回归风险",
        }
    )
    context = parsed_stdout(prompt).get("hookSpecificOutput", {}).get("additionalContext", "")
    assert_true("Default subagent policy is active" in context, "dual trigger should include default guidance")
    assert_true("Parallel lane request detected" in context, "dual trigger should include delegation guidance")


def test_concurrent_writes_keep_state_json_valid() -> None:
    cleanup_state()
    session_id = f"codex-concurrent-{uuid.uuid4()}"
    base = {"session_id": session_id, "cwd": str(ROOT)}

    armed = run_hook(
        {
            "hook_event_name": "UserPromptSubmit",
            **base,
            "prompt": "全面review并行分路检查 API、数据库、UI 的回归风险",
        }
    )
    assert_true(armed.returncode == 0, "arming prompt should succeed")

    events: list[dict] = []
    for i in range(16):
        events.append(
            {
                "hook_event_name": "PostToolUse",
                **base,
                "tool_name": "functions.spawn_agent",
                "tool_input": {
                    "agent_type": "explore",
                    "message": f"independent review lane {i}",
                },
            }
        )
    for _ in range(10):
        events.append({"hook_event_name": "Stop", **base, "prompt": "全面review并行分路检查 API、数据库、UI 的回归风险"})

    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(run_hook, events))

    assert_true(all(r.returncode == 0 for r in results), "all concurrent hook invocations should exit 0")

    path = state_file(session_id)
    assert_true(path.exists(), "state file should exist after concurrent writes")
    content = path.read_text(encoding="utf-8")
    parsed = json.loads(content)
    assert_true(isinstance(parsed, dict), "state file should stay valid JSON object")
    assert_true(
        parsed == {"seq": 0} or ("review_required" in parsed and "review_subagent_seen" in parsed),
        "state should either reset cleanly or retain core gate keys",
    )


def main() -> int:
    test_broad_review_requires_subagent_until_seen()
    test_plain_prompt_requires_subagent_by_default()
    test_parallel_lanes_require_subagent_until_seen()
    test_autopilot_goal_mode_blocks_without_goal_evidence()
    test_autopilot_goal_mode_passes_with_goal_evidence()
    test_autopilot_goal_mode_escalates_after_repeated_no_progress()
    test_reject_reason_allows_completion_without_subagent()
    test_non_subagent_tool_does_not_clear_gate()
    test_typed_unrelated_subagent_clears_gate()
    test_subagent_tool_evidence_without_lane_keywords_clears_gate()
    test_corrupted_state_file_fails_closed()
    test_state_survives_cwd_drift()
    test_missing_state_file_fails_closed()
    test_missing_state_without_review_prompt_blocks_conservatively()
    test_reject_reason_substring_does_not_bypass_gate()
    test_override_synonym_disables_gate()
    test_state_read_failure_fails_closed()
    test_english_pr_review_triggers_gate_without_this_keyword()
    test_english_deep_repo_review_with_severity_triggers_gate()
    test_parallel_lane_only_english_triggers_delegation_gate()
    test_generic_prompt_without_lanes_still_requires_subagent()
    test_review_and_parallel_both_emit_prompt_context()
    test_concurrent_writes_keep_state_json_valid()
    cleanup_state()
    print("codex hook tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
