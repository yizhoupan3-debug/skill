#!/usr/bin/env python3
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import shutil
import subprocess
import sys
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
HOOK = ROOT / ".cursor" / "hooks" / "review_subagent_gate.py"


def run_hook(event: str, payload: dict) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(HOOK), "--event", event],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        cwd=ROOT,
        timeout=10,
    )


def parsed_stdout(result: subprocess.CompletedProcess[str]) -> dict:
    return json.loads(result.stdout) if result.stdout.strip() else {}


def assert_true(cond: bool, msg: str) -> None:
    if not cond:
        raise AssertionError(msg)


def cleanup_state() -> None:
    shutil.rmtree(ROOT / ".cursor" / "hook-state", ignore_errors=True)


def state_file(session_id: str) -> Path:
    key = hashlib.sha256(session_id.encode()).hexdigest()[:32]
    return ROOT / ".cursor" / "hook-state" / f"review-subagent-{key}.json"


def load_state_for(session_id: str) -> dict:
    path = state_file(session_id)
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def test_broad_review_arms_state() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "全面review这个仓库，找bug并按严重程度给findings",
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 1, "phase should arm to 1")
    assert_true(state.get("review_required") is True, "review_required should be true")


def test_parallel_lanes_arms_delegation() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "并行检查 API、数据库和 UI 三个模块的回归风险",
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 1, "phase should arm to 1")
    assert_true(state.get("delegation_required") is True, "delegation_required should be true")


def test_framework_entrypoint_arms_delegation() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "/autopilot 修复路由回归并补验证",
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 1, "framework entrypoint should arm phase to 1")
    assert_true(state.get("delegation_required") is True, "framework entrypoint should require delegation")


def test_gitx_entrypoint_arms_delegation() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "/gitx 安全收口并推送当前分支",
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 1, "gitx entrypoint should arm phase to 1")
    assert_true(state.get("delegation_required") is True, "gitx entrypoint should require delegation")


def test_gitx_dollar_entrypoint_arms_delegation() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "$gitx 检查分支状态并安全收口",
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 1, "dollar gitx entrypoint should arm phase to 1")
    assert_true(state.get("delegation_required") is True, "dollar gitx entrypoint should require delegation")


def test_subagent_start_advances_phase_to_2() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "深度review这个仓库，找问题"},
    )
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "independent review lane",
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 2, "phase should advance to 2")
    assert_true(state.get("subagent_start_count") == 1, "subagent_start_count should be 1")


def test_subagent_stop_advances_phase_to_3() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "深度review这个仓库，找问题"},
    )
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "independent review lane",
        },
    )
    run_hook("subagentStop", {"session_id": session_id, "cwd": str(ROOT)})
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 3, "phase should advance to 3")
    assert_true(state.get("subagent_stop_count") == 1, "subagent_stop_count should be 1")


def test_post_tool_use_task_advances_phase() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    run_hook(
        "postToolUse",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "Task",
            "tool_input": {"subagent_type": "explore", "description": "independent review lane"},
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 2, "Task subagent tool should move phase to 2")


def test_post_tool_use_functions_subagent_advances_phase() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "/team 分路排查 host 集成问题"},
    )
    run_hook(
        "postToolUse",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "functions.subagent",
            "tool_input": {"subagent_type": "explore", "description": "independent review lane"},
        },
    )
    state = load_state_for(session_id)
    assert_true(
        state.get("phase") == 2,
        "functions.subagent evidence should move phase to 2",
    )


def test_post_tool_use_unrelated_does_not_advance() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    run_hook(
        "postToolUse",
        {"session_id": session_id, "cwd": str(ROOT), "tool_name": "Read"},
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 1, "unrelated tool should not change phase")


def test_post_tool_use_task_without_typed_subagent_does_not_advance() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    run_hook(
        "postToolUse",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "Task",
            "tool_input": {"description": "plain task without typed subagent"},
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 1, "untyped Task should not satisfy gate evidence")


def test_post_tool_use_task_with_camelcase_agent_type_advances() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    run_hook(
        "postToolUse",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "tool_name": "Task",
            "tool_input": {"agentType": "explore", "description": "camelCase typed lane"},
        },
    )
    state = load_state_for(session_id)
    assert_true(state.get("phase") == 2, "camelCase agentType should satisfy typed subagent evidence")


def test_unarmed_post_tool_use_does_not_create_state_file() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "postToolUse",
        {"session_id": session_id, "cwd": str(ROOT), "tool_name": "Task", "tool_input": {"agentType": "explore"}},
    )
    assert_true(not state_file(session_id).exists(), "unarmed postToolUse should not create state file")


def test_stop_blocks_when_armed_and_no_subagent() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "深度review这个仓库，找问题"},
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" in parsed_stdout(result), "stop should include followup_message")


def test_framework_entrypoint_stop_blocks_without_subagent() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "/team 做一次跨模块排查"},
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(
        "followup_message" in parsed_stdout(result),
        "framework entrypoint should trigger followup until subagent evidence appears",
    )


def test_autopilot_goal_mode_blocks_without_goal_evidence() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "/autopilot 修复回归"},
    )
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "independent lane",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    message = parsed_stdout(result).get("followup_message", "")
    assert_true("Autopilot goal mode requires completion evidence" in message, "autopilot should enforce goal evidence")


def test_autopilot_goal_mode_passes_with_goal_evidence() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "/autopilot Goal: 修复回归 Done when: tests pass Validation commands: pytest",
        },
    )
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "independent lane",
        },
    )
    run_hook(
        "afterAgentResponse",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "response": "checkpoint 1 progress: patch applied, next step run tests; verification passed",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(
        "followup_message" not in parsed_stdout(result),
        "autopilot should pass when goal contract/progress/verification are present",
    )


def test_gitx_entrypoint_stop_blocks_without_subagent() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "/gitx 做一次安全收口"},
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(
        "followup_message" in parsed_stdout(result),
        "gitx entrypoint should trigger followup until subagent evidence appears",
    )


def test_stop_passes_when_phase_2() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "深度review这个仓库，找问题"},
    )
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "independent review lane",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" not in parsed_stdout(result), "stop should pass at phase 2")


def test_stop_passes_when_phase_3() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "深度review这个仓库，找问题"},
    )
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "independent review lane",
        },
    )
    run_hook("subagentStop", {"session_id": session_id, "cwd": str(ROOT)})
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" not in parsed_stdout(result), "stop should pass at phase 3")


def test_evidence_accepts_even_without_lane_intent_matches() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "深度review这个仓库，找问题"},
    )
    path = state_file(session_id)
    state = load_state_for(session_id)
    state["lane_intent_matches"] = False
    path.write_text(json.dumps(state, ensure_ascii=False), encoding="utf-8")
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "worker",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(
        "followup_message" not in parsed_stdout(result),
        "real subagent evidence should satisfy even when lane_intent_matches is false",
    )


def test_override_in_prompt_disables_gate() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，但不要分路，直接处理",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" not in parsed_stdout(result), "override should disable gate")


def test_reject_reason_in_agent_response() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    run_hook(
        "afterAgentResponse",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "response": "I'll handle this. reject_reason: shared_context_heavy",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" not in parsed_stdout(result), "reject_reason in response should bypass")


def test_override_in_agent_response_does_not_bypass() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    run_hook(
        "afterAgentResponse",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "response": "do not use subagent, I will handle this locally",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(
        "followup_message" in parsed_stdout(result),
        "override in agent response must NOT bypass gate",
    )


def test_reject_reason_in_prompt_also_works() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，但 reject_reason: small_task",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" not in parsed_stdout(result), "reject_reason in prompt should bypass")


def test_reject_reason_substring_does_not_bypass() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "深度review这个仓库，备注 my_shared_context_heavy_note",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" in parsed_stdout(result), "substring should not bypass")


def test_plain_review_sentence_does_not_arm() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "I will review my notes later."},
    )
    state = load_state_for(session_id)
    assert_true(
        not state or int(state.get("phase", 0)) == 0,
        "plain 'review my notes' should NOT arm the gate",
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(
        "followup_message" not in parsed_stdout(result),
        "plain prompt must not trigger followup (gate is opt-in)",
    )


def test_loop_count_3_escalates_message() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT), "loop_count": 3})
    message = parsed_stdout(result).get("followup_message", "")
    assert_true("looped multiple times" in message, "loop_count=3 should escalate message")


def test_pre_compact_surfaces_state() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    result = run_hook("preCompact", {"session_id": session_id, "cwd": str(ROOT)})
    context = parsed_stdout(result).get("additional_context", "")
    assert_true(isinstance(context, str) and context, "preCompact should include additional_context")
    assert_true("phase=1" in context or "phase" in context, "additional_context should surface phase")


def test_session_end_deletes_state_file() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    path = state_file(session_id)
    assert_true(path.exists(), "state file should exist before sessionEnd")
    run_hook("sessionEnd", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(not path.exists(), "sessionEnd should delete state file")


def test_corrupted_state_falls_back_safely() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    path = state_file(session_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("{ broken", encoding="utf-8")
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(result.returncode == 0, "hook should not crash on corrupted state")
    assert_true("followup_message" in parsed_stdout(result), "corrupted state should still enforce followup")


def test_state_survives_cwd_drift() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    subprocess.run(
        [sys.executable, str(HOOK), "--event", "beforeSubmitPrompt"],
        input=json.dumps(
            {
                "session_id": session_id,
                "cwd": str(ROOT / "scripts"),
                "prompt": "全面review这个仓库，找问题",
            }
        ),
        text=True,
        capture_output=True,
        cwd=ROOT,
        timeout=10,
    )
    result = subprocess.run(
        [sys.executable, str(HOOK), "--event", "stop"],
        input=json.dumps({"session_id": session_id, "cwd": str(ROOT)}),
        text=True,
        capture_output=True,
        cwd=Path("/private/tmp"),
        timeout=10,
    )
    assert_true(result.returncode == 0, "hook should run from alternate process cwd")
    assert_true("followup_message" in parsed_stdout(result), "gate should still enforce with cwd drift")


def test_before_submit_merges_existing_state() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    run_hook(
        "subagentStart",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "subagent_type": "explore",
            "description": "independent review lane",
        },
    )
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "再做一次深度review"},
    )
    state = load_state_for(session_id)
    assert_true(int(state.get("phase", 0)) >= 2, "beforeSubmitPrompt should merge existing state")


def test_unknown_event_returns_empty_json() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    result = run_hook("unknownEvent", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true(result.returncode == 0, "unknown events should exit 0")
    assert_true(result.stdout.strip() in ("", "{}"), "unknown event should emit {} or empty output")


def test_atomic_write_no_partial_state() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "全面review这个仓库，找问题"},
    )
    path = state_file(session_id)
    assert_true(path.exists(), "state file should be created")
    content = path.read_text(encoding="utf-8")
    assert_true(content.endswith("\n"), "state file should end with newline")
    parsed = json.loads(content)
    assert_true(parsed.get("version") == 2, "state must be valid JSON in v2 format")


def test_pr_review_english_triggers_gate() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "please review pull request 456 for regressions",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" in parsed_stdout(result), "english PR review should trigger gate")


def test_parallel_lane_only_english_triggers_delegation_gate() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    run_hook(
        "beforeSubmitPrompt",
        {
            "session_id": session_id,
            "cwd": str(ROOT),
            "prompt": "split into independent lanes and run in parallel for backend and database checks",
        },
    )
    result = run_hook("stop", {"session_id": session_id, "cwd": str(ROOT)})
    assert_true("followup_message" in parsed_stdout(result), "english lane-only prompt should trigger gate")


def test_subagent_start_without_armed_does_nothing() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    result = run_hook(
        "subagentStart",
        {"session_id": session_id, "cwd": str(ROOT), "subagent_type": "explore"},
    )
    assert_true(result.returncode == 0, "subagentStart without arm should not crash")
    path = state_file(session_id)
    if path.exists():
        state = load_state_for(session_id)
        assert_true(
            state.get("phase") in (0, 2)
            and not state.get("review_required")
            and not state.get("delegation_required"),
            "if created unarmed, state should not set required flags even when subagent evidence appears",
        )


def test_print_json_has_trailing_newline() -> None:
    cleanup_state()
    session_id = f"cursor-{uuid.uuid4()}"
    result = run_hook(
        "beforeSubmitPrompt",
        {"session_id": session_id, "cwd": str(ROOT), "prompt": "普通消息"},
    )
    assert_true(result.stdout.endswith("\n"), "stdout JSON should end with newline")


def test_concurrent_writes_preserve_gate_state() -> None:
    cleanup_state()
    session_id = f"cursor-concurrent-{uuid.uuid4()}"
    base = {"session_id": session_id, "cwd": str(ROOT)}

    armed = run_hook(
        "beforeSubmitPrompt",
        {**base, "prompt": "全面review并行分路检查 API、数据库、UI 的回归风险"},
    )
    assert_true(armed.returncode == 0, "arming prompt should succeed")

    events: list[tuple[str, dict]] = []
    for i in range(12):
        events.append(
            (
                "postToolUse",
                {
                    **base,
                    "tool_name": "Task",
                    "description": f"independent review lane {i}",
                    "tool_input": {"subagent_type": "explore", "description": f"review lane {i}"},
                },
            )
        )
    for _ in range(8):
        events.append(("afterAgentResponse", {**base, "response": "继续分析，不给 reject reason"}))
    for _ in range(8):
        events.append(("stop", {**base, "loop_count": 1}))

    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(lambda item: run_hook(item[0], item[1]), events))

    assert_true(all(r.returncode == 0 for r in results), "all concurrent hook invocations should exit 0")

    state = load_state_for(session_id)
    assert_true(bool(state), "state file should remain readable after concurrent writes")
    assert_true(state.get("version") == 2, "state version should stay at v2")
    # Concurrent writes may end with cleared state after a satisfied stop; require structural integrity.
    for key in (
        "phase",
        "review_required",
        "delegation_required",
        "subagent_start_count",
        "subagent_stop_count",
        "followup_count",
    ):
        assert_true(key in state, f"state should retain schema key: {key}")


def main() -> int:
    test_broad_review_arms_state()
    test_parallel_lanes_arms_delegation()
    test_framework_entrypoint_arms_delegation()
    test_gitx_entrypoint_arms_delegation()
    test_gitx_dollar_entrypoint_arms_delegation()
    test_subagent_start_advances_phase_to_2()
    test_subagent_stop_advances_phase_to_3()
    test_post_tool_use_task_advances_phase()
    test_post_tool_use_functions_subagent_advances_phase()
    test_post_tool_use_unrelated_does_not_advance()
    test_post_tool_use_task_without_typed_subagent_does_not_advance()
    test_post_tool_use_task_with_camelcase_agent_type_advances()
    test_unarmed_post_tool_use_does_not_create_state_file()
    test_stop_blocks_when_armed_and_no_subagent()
    test_framework_entrypoint_stop_blocks_without_subagent()
    test_autopilot_goal_mode_blocks_without_goal_evidence()
    test_autopilot_goal_mode_passes_with_goal_evidence()
    test_gitx_entrypoint_stop_blocks_without_subagent()
    test_stop_passes_when_phase_2()
    test_stop_passes_when_phase_3()
    test_evidence_accepts_even_without_lane_intent_matches()
    test_override_in_prompt_disables_gate()
    test_reject_reason_in_agent_response()
    test_override_in_agent_response_does_not_bypass()
    test_reject_reason_in_prompt_also_works()
    test_reject_reason_substring_does_not_bypass()
    test_plain_review_sentence_does_not_arm()
    test_loop_count_3_escalates_message()
    test_pre_compact_surfaces_state()
    test_session_end_deletes_state_file()
    test_corrupted_state_falls_back_safely()
    test_state_survives_cwd_drift()
    test_before_submit_merges_existing_state()
    test_unknown_event_returns_empty_json()
    test_atomic_write_no_partial_state()
    test_pr_review_english_triggers_gate()
    test_parallel_lane_only_english_triggers_delegation_gate()
    test_subagent_start_without_armed_does_nothing()
    test_print_json_has_trailing_newline()
    test_concurrent_writes_preserve_gate_state()
    cleanup_state()
    print("cursor hook tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
