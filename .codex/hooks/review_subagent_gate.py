#!/usr/bin/env python3
"""Require independent subagent review for broad/deep review prompts."""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any


REVIEW_PATTERNS = [
    re.compile(r"\b(code|security|architecture|architect)\s+review\b", re.I),
    re.compile(r"\breview\s+this\s+(pr|pull request)\b", re.I),
    re.compile(r"\breview\s+(my\s+)?(pr|pull request)\b", re.I),
    re.compile(r"\b(pr|pull request)\s+review\b", re.I),
    re.compile(r"\breview\s+(code|security|architecture)\b", re.I),
    re.compile(r"^\s*review\b.*\bagain\b", re.I),
    re.compile(r"\bfocus on finding\b.*\bproblems\b", re.I),
    re.compile(r"(深度|全面|全仓|仓库级|跨模块|多模块|多维)\s*review", re.I),
    re.compile(
        r"review.*(仓库|全仓|跨模块|多模块|严重程度|findings|severity|repo|repository|cross[- ]module|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        re.I,
    ),
    re.compile(r"(深度|全面|全仓|仓库级|跨模块|多模块|多维).*(审查|审核|审计|评审)", re.I),
    re.compile(
        r"(审查|审核|审计|评审).*(仓库|全仓|跨模块|多模块|严重程度|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        re.I,
    ),
    re.compile(r"(代码审查|安全审查|架构审查|审查这个\s*PR|审查这段代码)", re.I),
    re.compile(r"(审查|评审|审核).*(PR|pull request|合并请求)", re.I),
]

PARALLEL_DELEGATION_PATTERNS = [
    re.compile(r"(并行|同时|分头|分路|分三路|多路|多线).*(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证|模块|方向)", re.I),
    re.compile(r"(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证).*(并行|同时|分头|分路|分三路|多路|多线)", re.I),
    re.compile(r"(多个|多条|多路|多维|多方向|独立).*(假设|模块|方向|维度|lane|lanes)", re.I),
    re.compile(r"\b(parallel|concurrent|in parallel|split lanes|split work)\b.*\b(frontend|backend|test|testing|database|security|performance|architecture|implementation|verification)\b", re.I),
    re.compile(r"(并行|分路|分头|独立).*(lane|路线|路)", re.I),
]

OVERRIDE_PATTERNS = [
    re.compile(r"do not use (a )?subagent", re.I),
    re.compile(r"without (a )?subagent", re.I),
    re.compile(r"handle (this|it) locally", re.I),
    re.compile(r"do it yourself", re.I),
    re.compile(r"no (parallel|delegation|delegating|split)", re.I),
    re.compile(r"不要.*subagent", re.I),
    re.compile(r"不用.*subagent", re.I),
    re.compile(r"不要.*子代理", re.I),
    re.compile(r"不用.*子代理", re.I),
    re.compile(r"(你|你自己).*(本地处理|直接处理|自己做)", re.I),
    re.compile(r"(不要|不用).*(分工|并行|分路|分头)", re.I),
]

REJECT_REASONS = {
    "small_task",
    "shared_context_heavy",
    "write_scope_overlap",
    "next_step_blocked",
    "verification_missing",
    "token_overhead_dominates",
}
REJECT_REASON_PATTERNS = [
    re.compile(rf"(?<![a-z0-9_]){re.escape(reason)}(?![a-z0-9_])", re.I)
    for reason in REJECT_REASONS
]
MAX_GOAL_NO_PROGRESS_LOOPS = 2

SUBAGENT_TOOL_NAMES = {
    "functions.subagent",
    "functions.spawn_agent",
    "subagent",
    "spawn_agent",
}
SUBAGENT_TYPES = {
    "generalpurpose",
    "explore",
    "shell",
    "browser-use",
    "cursor-guide",
    "ci-investigator",
    "best-of-n-runner",
    "explorer",  # legacy alias
}

LANE_INTENT_PATTERNS = [
    re.compile(r"\b(review|reviewer|audit|security|architecture|regression|risk|lane|sidecar)\b", re.I),
    re.compile(r"(审查|评审|审核|审计|并行|分路|分头|多路|独立)", re.I),
]

AUTOPILOT_ENTRYPOINT_RE = re.compile(r"(^|\s)([/$])autopilot\b", re.I)
GOAL_CONTRACT_RE = re.compile(
    r"\b(goal|done when|validation commands|checkpoint plan|non-goals)\b|"
    r"(目标|完成条件|验证命令|检查点|非目标)",
    re.I,
)
GOAL_PROGRESS_RE = re.compile(
    r"\b(checkpoint|milestone|progress|next step)\b|"
    r"(检查点|里程碑|进度|下一步)",
    re.I,
)
GOAL_VERIFY_RE = re.compile(
    r"\b(verified|verification passed|test passed|tests passed|all checks passed)\b|"
    r"(已验证|验证通过|测试通过|全部检查通过)",
    re.I,
)
GOAL_BLOCKER_RE = re.compile(
    r"\b(blocker\s*:\s*[a-z0-9_.-]+|blocked\s+by\s+[a-z0-9_.-]+)\b|"
    r"(阻塞[:：]\s*[\w\u4e00-\u9fff._-]+)",
    re.I,
)


def read_event() -> dict[str, Any]:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        return {}
    return payload if isinstance(payload, dict) else {}


def event_name(event: dict[str, Any]) -> str:
    return str(event.get("hook_event_name") or event.get("event") or "").lower()


def prompt_text(event: dict[str, Any]) -> str:
    for key in ("prompt", "user_prompt", "message", "input"):
        value = event.get(key)
        if isinstance(value, str):
            return value
    return ""


def session_key(event: dict[str, Any]) -> str:
    raw = str(
        event.get("session_id")
        or event.get("conversation_id")
        or event.get("thread_id")
        or event.get("cwd")
        or "default"
    )
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:32]


def state_dir(event: dict[str, Any]) -> Path:
    return repo_root(event) / ".codex" / "hook-state"


def _candidate_paths(event: dict[str, Any]) -> list[Path]:
    candidates: list[Path] = []
    cwd = event.get("cwd")
    if isinstance(cwd, str) and cwd.strip():
        candidates.append(Path(cwd).resolve())
    candidates.append(Path(os.getcwd()).resolve())
    candidates.append(Path(__file__).resolve().parents[2])
    return candidates


def repo_root(event: dict[str, Any]) -> Path:
    for candidate in _candidate_paths(event):
        for probe in [candidate, *candidate.parents]:
            if (probe / ".git").exists() and (probe / ".codex").is_dir():
                return probe
            if (probe / ".codex" / "hooks" / "review_subagent_gate.py").exists():
                return probe
    return _candidate_paths(event)[0]


def state_path(event: dict[str, Any]) -> Path:
    return state_dir(event) / f"review-subagent-{session_key(event)}.json"


def load_state(event: dict[str, Any]) -> dict[str, Any]:
    path = state_path(event)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}
    except json.JSONDecodeError:
        return {"__state_io_error": "state_json_invalid"}
    except OSError:
        return {"__state_io_error": "state_read_failed"}
    return data if isinstance(data, dict) else {}


def save_state(event: dict[str, Any], state: dict[str, Any]) -> bool:
    directory = state_dir(event)
    target = state_path(event)
    tmp: Path | None = None
    try:
        directory.mkdir(parents=True, exist_ok=True)
        tmp = directory / f".tmp-{os.getpid()}-{target.name}"
        tmp.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        os.replace(tmp, target)
        return True
    except OSError:
        if tmp is not None:
            try:
                tmp.unlink()
            except OSError:
                pass
        return False


def is_review_prompt(text: str) -> bool:
    value = text or ""
    if is_narrow_review_prompt(value):
        return False
    return any(pattern.search(value) for pattern in REVIEW_PATTERNS)


def is_parallel_delegation_prompt(text: str) -> bool:
    return any(pattern.search(text or "") for pattern in PARALLEL_DELEGATION_PATTERNS)


def is_narrow_review_prompt(text: str) -> bool:
    value = text or ""
    if not re.search(r"\breview\b", value, re.I):
        return False
    if re.search(r"\b(pr|pull request)\b", value, re.I):
        return False
    if re.search(r"(深度|全面|全仓|跨模块|多模块|多维|架构|安全|回归风险|严重程度|findings)", value, re.I):
        return False
    return bool(re.search(r"^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))", value, re.I))


def has_override(text: str) -> bool:
    return any(pattern.search(text or "") for pattern in OVERRIDE_PATTERNS)


def saw_reject_reason(text: str) -> bool:
    return any(pattern.search(text or "") for pattern in REJECT_REASON_PATTERNS)


def normalize_subagent_type(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    return value.strip().lower()


def tool_name(event: dict[str, Any]) -> str:
    return str(event.get("tool_name") or event.get("tool") or event.get("name") or "")


def tool_input(event: dict[str, Any]) -> dict[str, Any]:
    value = event.get("tool_input") or event.get("input") or event.get("arguments") or {}
    return value if isinstance(value, dict) else {}


def lane_intent_text(event: dict[str, Any]) -> str:
    parts: list[str] = []
    for key in ("description", "message", "prompt", "task", "title", "name"):
        value = event.get(key)
        if isinstance(value, str):
            parts.append(value)
    data = tool_input(event)
    for key in ("description", "message", "prompt", "task", "title", "name"):
        value = data.get(key)
        if isinstance(value, str):
            parts.append(value)
    return " ".join(parts)


def saw_subagent(event: dict[str, Any]) -> bool:
    name = tool_name(event).lower()
    if name not in SUBAGENT_TOOL_NAMES:
        return False
    data = tool_input(event)
    typed_fields = (
        data.get("subagent_type"),
        data.get("agent_type"),
        data.get("agentType"),
    )
    normalized = [normalize_subagent_type(value) for value in typed_fields]
    return any(value in SUBAGENT_TYPES for value in normalized)


def lane_intent_matches(text: str) -> bool:
    value = text or ""
    return any(pattern.search(value) for pattern in LANE_INTENT_PATTERNS)


def is_autopilot_entrypoint_prompt(text: str) -> bool:
    return bool(AUTOPILOT_ENTRYPOINT_RE.search(text or ""))


def has_goal_contract_signal(text: str) -> bool:
    return bool(GOAL_CONTRACT_RE.search(text or ""))


def has_goal_progress_signal(text: str) -> bool:
    return bool(GOAL_PROGRESS_RE.search(text or ""))


def has_goal_verify_signal(text: str) -> bool:
    return bool(GOAL_VERIFY_RE.search(text or ""))


def has_goal_blocker_signal(text: str) -> bool:
    return bool(GOAL_BLOCKER_RE.search(text or ""))


def goal_has_advance_signal(state: dict[str, Any]) -> bool:
    return bool(state.get("goal_verify_seen") or state.get("goal_blocker_seen"))


def goal_missing_parts(state: dict[str, Any]) -> list[str]:
    missing: list[str] = []
    if not state.get("goal_contract_seen"):
        missing.append("goal_contract")
    if not state.get("goal_progress_seen"):
        missing.append("checkpoint_progress")
    if not (state.get("goal_verify_seen") or state.get("goal_blocker_seen")):
        missing.append("verification_or_blocker")
    return missing


def goal_execution_template(state: dict[str, Any]) -> str:
    missing = goal_missing_parts(state)
    template = (
        " Next action template: "
        "Goal=<one objective>; Done when=<measurable condition>; "
        "Validation commands=<exact commands>; "
        "Checkpoint progress=<what changed>; Next step=<immediate action>; "
        "Verification=<passed output> OR blocker=<typed blocker>."
    )
    if missing:
        return template + f" Fill missing now: {', '.join(missing)}."
    return template


def print_json(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, ensure_ascii=False))


def handle_prompt(event: dict[str, Any]) -> int:
    text = prompt_text(event)
    state: dict[str, Any] = {"seq": 0, "subagent_required": True}
    if is_review_prompt(text):
        state["review_required"] = True
        state["prompt"] = text[:500]
    if is_parallel_delegation_prompt(text):
        state["delegation_required"] = True
        state["prompt"] = text[:500]
    if has_override(text):
        state["review_override"] = True
        state["delegation_override"] = True
    if saw_reject_reason(text):
        state["reject_reason_seen"] = True
    if is_autopilot_entrypoint_prompt(text):
        state["goal_required"] = True
    if has_goal_contract_signal(text):
        state["goal_contract_seen"] = True
    if has_goal_progress_signal(text):
        state["goal_progress_seen"] = True
    if has_goal_verify_signal(text):
        state["goal_verify_seen"] = True
    if has_goal_blocker_signal(text):
        state["goal_blocker_seen"] = True
    state.setdefault("goal_no_progress_count", 0)
    if goal_has_advance_signal(state):
        state["goal_no_progress_count"] = 0
    save_state(event, state)
    persisted = save_state(event, state)
    if not persisted:
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Review gate state could not be persisted under .codex/hook-state. "
                    "Fail-closed to avoid silent policy bypass."
                ),
            }
        )
        return 0
    contexts: list[str] = []
    if state.get("subagent_required") and not state.get("review_override"):
        contexts.append(
            "Default subagent policy is active. Spawn one or more bounded subagent lanes NOW "
            "(before doing the main analysis). If you will not spawn, explicitly state one "
            "reject reason: small_task / shared_context_heavy / write_scope_overlap / "
            "next_step_blocked / verification_missing / token_overhead_dominates.\n\n"
            "Quick evaluation guide:\n"
            "- Use subagent by default for search, review, testing, or independent module work\n"
            "- Skip subagent only when one reject reason clearly applies\n"
        )
    if state.get("review_required") and not state.get("review_override"):
        contexts.append(
            "Broad/deep review detected. Spawn independent reviewer subagent lanes NOW "
            "(before doing the main analysis). If you will not spawn, explicitly state one "
            "reject reason: small_task / shared_context_heavy / write_scope_overlap / "
            "next_step_blocked / verification_missing / token_overhead_dominates.\n\n"
            "Suggested lanes (copy/paste and customize):\n"
            "- Security lane: threat model + secrets + auth + supply chain risks\n"
            "- Architecture lane: boundaries + data flow + invariants + sharp edges\n"
            "- Regression lane: behavior changes + tests gaps + rollout risk\n"
        )
    if state.get("delegation_required") and not state.get("delegation_override"):
        contexts.append(
            "Parallel lane request detected. Spawn bounded subagent lanes NOW (before "
            "integrating). If you will not spawn, explicitly state one reject reason.\n\n"
            "Suggested lanes (copy/paste):\n"
            "- API lane: contracts + backward compatibility + error semantics\n"
            "- DB lane: migrations + indexes + consistency + performance\n"
            "- UI lane: UX regressions + accessibility + edge cases\n"
        )
    if state.get("goal_required") and not state.get("reject_reason_seen"):
        contexts.append(
            "Autopilot goal mode requires closeout evidence. Before finalizing, provide: "
            "Goal/Done-when/Validation-commands, checkpoint progress + next step, and "
            "verification result or explicit blocker."
        )
    if contexts:
        print_json(
            {
                "hookSpecificOutput": {
                    "hookEventName": "UserPromptSubmit",
                    "additionalContext": "\n".join(contexts),
                }
            }
        )
    return 0


def handle_post_tool(event: dict[str, Any]) -> int:
    if saw_subagent(event):
        state = load_state(event)
        state["review_subagent_seen"] = True
        state["review_subagent_tool"] = tool_name(event)
        if not save_state(event, state):
            print_json(
                {
                    "decision": "block",
                    "reason": (
                        "Review gate state update failed under .codex/hook-state after subagent evidence. "
                        "Fail-closed to avoid inconsistent gating."
                    ),
                }
            )
    return 0


def handle_stop(event: dict[str, Any]) -> int:
    if event.get("stop_hook_active") or event.get("stopHookActive"):
        return 0
    state = load_state(event)
    text = prompt_text(event)
    inferred_overridden = has_override(text) or saw_reject_reason(text)
    inferred_required = bool((text or "").strip()) and not inferred_overridden
    if not state:
        reason = (
            "Review gate state is missing under .codex/hook-state. "
            "Fail-closed to avoid bypass when enforcement state is unavailable."
        )
        if not inferred_required:
            reason += " Stop payload had no review context; blocking conservatively."
        print_json(
            {
                "decision": "block",
                "reason": reason,
            }
        )
        return 0
    if state.get("__state_io_error"):
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Review gate state is unreadable or unavailable under .codex/hook-state "
                    f"({state.get('__state_io_error')}). Fail-closed to avoid silent policy bypass."
                ),
            }
        )
        return 0
    if saw_reject_reason(text):
        state["reject_reason_seen"] = True
        state["goal_blocker_seen"] = True
    if has_goal_contract_signal(text):
        state["goal_contract_seen"] = True
    if has_goal_progress_signal(text):
        state["goal_progress_seen"] = True
    if has_goal_verify_signal(text):
        state["goal_verify_seen"] = True
    if has_goal_blocker_signal(text):
        state["goal_blocker_seen"] = True
    state.setdefault("goal_no_progress_count", 0)
    if goal_has_advance_signal(state):
        state["goal_no_progress_count"] = 0
    if (
        state.get("subagent_required")
        and not state.get("review_override")
        and not state.get("reject_reason_seen")
        and not state.get("review_subagent_seen")
    ):
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Default subagent policy is active, but no subagent was observed. "
                    "Spawn a bounded subagent lane now, or explicitly record a reject reason "
                    "(small_task/shared_context_heavy/write_scope_overlap/next_step_blocked/"
                    "verification_missing/token_overhead_dominates)."
                ),
            }
        )
        return 0
    if (
        state.get("review_required")
        and not state.get("review_override")
        and not state.get("reject_reason_seen")
        and not state.get("review_subagent_seen")
    ):
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Broad/deep review was requested, but no independent subagent review was observed. "
                    "Spawn suitable reviewer sidecars now, or explicitly record why spawning is rejected."
                ),
            }
        )
        return 0
    if (
        state.get("delegation_required")
        and not state.get("delegation_override")
        and not state.get("reject_reason_seen")
        and not state.get("review_subagent_seen")
    ):
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Independent parallel lanes were requested, but no bounded subagent sidecar was observed. "
                    "Spawn suitable sidecars before finalizing, or rerun with an explicit no-subagent override."
                ),
            }
        )
        return 0
    if (
        state.get("goal_required")
        and not state.get("reject_reason_seen")
        and not state.get("goal_contract_seen")
    ):
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Autopilot goal mode requires a goal contract before closeout. Provide Goal/Done-when/Validation-commands."
                ),
            }
        )
        return 0
    if (
        state.get("goal_required")
        and not state.get("reject_reason_seen")
        and not state.get("goal_progress_seen")
    ):
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Autopilot goal mode requires checkpoint progress before closeout. Continue execution and report progress + next step."
                ),
            }
        )
        return 0
    if (
        state.get("goal_required")
        and not state.get("reject_reason_seen")
        and not (state.get("goal_verify_seen") or state.get("goal_blocker_seen"))
    ):
        state["goal_no_progress_count"] = int(state.get("goal_no_progress_count") or 0) + 1
        save_state(event, state)
        no_progress = int(state.get("goal_no_progress_count") or 0)
        escalation = (
            " No-progress threshold exceeded; execute next checkpoint immediately or provide typed blocker."
            if no_progress >= MAX_GOAL_NO_PROGRESS_LOOPS
            else ""
        )
        print_json(
            {
                "decision": "block",
                "reason": (
                    "Autopilot goal mode requires closeout evidence before finalizing. Provide "
                    "Goal/Done-when/Validation-commands, checkpoint progress + next step, and "
                    "verification result or explicit typed blocker (e.g. `blocker: missing_secret`). "
                    f"No-progress loops: {no_progress}."
                    + escalation
                    + goal_execution_template(state)
                ),
            }
        )
        return 0
    if state:
        save_state(event, {"seq": 0})
    return 0


def main() -> int:
    event = read_event()
    name = event_name(event)
    if name == "userpromptsubmit":
        return handle_prompt(event)
    if name == "posttooluse":
        return handle_post_tool(event)
    if name == "stop":
        return handle_stop(event)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
