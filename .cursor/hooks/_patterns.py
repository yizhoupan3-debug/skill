"""Shared regex patterns and constants for the Cursor review-subagent gate.

This module is co-located with `review_subagent_gate.py` and imported via
`sys.path` insertion. Keep this file pure: no I/O, no side effects, no logging.
"""

from __future__ import annotations

import re

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
    re.compile(
        r"\b(parallel|concurrent|in parallel|split lanes|split work)\b.*\b(frontend|backend|test|testing|database|security|performance|architecture|implementation|verification)\b",
        re.I,
    ),
    re.compile(r"\b(parallel|concurrent|in parallel|split lanes?|independent lanes?)\b", re.I),
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

REVIEW_OVERRIDE_PATTERNS = [
    re.compile(r"do not use (a )?subagent", re.I),
    re.compile(r"without (a )?subagent", re.I),
    re.compile(r"handle (this|it) locally", re.I),
    re.compile(r"do it yourself", re.I),
    re.compile(r"不要.*subagent", re.I),
    re.compile(r"不用.*subagent", re.I),
    re.compile(r"不要.*子代理", re.I),
    re.compile(r"不用.*子代理", re.I),
    re.compile(r"(你|你自己).*(本地处理|直接处理|自己做)", re.I),
]

DELEGATION_OVERRIDE_PATTERNS = [
    re.compile(r"no (parallel|delegation|delegating|split)", re.I),
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

SUBAGENT_TYPES = {
    "generalpurpose",
    "explore",
    "shell",
    "browser-use",
    "browseruse",
    "cursor-guide",
    "cursorguide",
    "ci-investigator",
    "ciinvestigator",
    "best-of-n-runner",
    "bestofnrunner",
    "explorer",
}

SUBAGENT_TOOL_NAMES = {
    "task",
    "functions.task",
    "functions.subagent",
    "functions.spawn_agent",
    "subagent",
    "spawn_agent",
}

_REVIEW_KEYWORD_RE = re.compile(r"\breview\b", re.I)
_PR_KEYWORD_RE = re.compile(r"\b(pr|pull request)\b", re.I)
_DEEP_KEYWORD_RE = re.compile(
    r"(深度|全面|全仓|跨模块|多模块|多维|架构|安全|回归风险|严重程度|findings)", re.I
)
_NARROW_REVIEW_PREFIX_RE = re.compile(
    r"^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))",
    re.I,
)
_FRAMEWORK_ENTRYPOINT_RE = re.compile(r"(^|\s)([/$])(autopilot|team|gitx)\b", re.I)
_AUTOPILOT_ENTRYPOINT_RE = re.compile(r"(^|\s)([/$])autopilot\b", re.I)
_GOAL_CONTRACT_RE = re.compile(
    r"\b(goal|done when|validation commands|checkpoint plan|non-goals)\b|"
    r"(目标|完成条件|验证命令|检查点|非目标)",
    re.I,
)
_GOAL_PROGRESS_RE = re.compile(
    r"\b(checkpoint|milestone|progress|next step)\b|"
    r"(检查点|里程碑|进度|下一步)",
    re.I,
)
_GOAL_VERIFY_OR_BLOCK_RE = re.compile(
    r"\b(verified|verification|test passed|blocker)\b|"
    r"(已验证|验证通过|测试通过|阻塞)",
    re.I,
)


def is_narrow_review_prompt(text: str) -> bool:
    """A 'review path/to/file.ext' prompt is narrow and should NOT trigger the gate."""
    value = text or ""
    if not _REVIEW_KEYWORD_RE.search(value):
        return False
    if _PR_KEYWORD_RE.search(value):
        return False
    if _DEEP_KEYWORD_RE.search(value):
        return False
    return bool(_NARROW_REVIEW_PREFIX_RE.search(value))


def is_review_prompt(text: str) -> bool:
    value = text or ""
    if is_narrow_review_prompt(value):
        return False
    return any(p.search(value) for p in REVIEW_PATTERNS)


def is_parallel_delegation_prompt(text: str) -> bool:
    return any(p.search(text or "") for p in PARALLEL_DELEGATION_PATTERNS)


def is_framework_entrypoint_prompt(text: str) -> bool:
    return bool(_FRAMEWORK_ENTRYPOINT_RE.search(text or ""))


def is_autopilot_entrypoint_prompt(text: str) -> bool:
    return bool(_AUTOPILOT_ENTRYPOINT_RE.search(text or ""))


def has_goal_contract_signal(text: str) -> bool:
    return bool(_GOAL_CONTRACT_RE.search(text or ""))


def has_goal_progress_signal(text: str) -> bool:
    return bool(_GOAL_PROGRESS_RE.search(text or ""))


def has_goal_verify_or_block_signal(text: str) -> bool:
    return bool(_GOAL_VERIFY_OR_BLOCK_RE.search(text or ""))


def has_override(text: str) -> bool:
    return any(p.search(text or "") for p in OVERRIDE_PATTERNS)


def has_review_override(text: str) -> bool:
    return any(p.search(text or "") for p in REVIEW_OVERRIDE_PATTERNS)


def has_delegation_override(text: str) -> bool:
    return any(p.search(text or "") for p in DELEGATION_OVERRIDE_PATTERNS)


def saw_reject_reason(text: str) -> bool:
    return any(p.search(text or "") for p in REJECT_REASON_PATTERNS)


def normalize_subagent_type(value: object) -> str:
    if not isinstance(value, str):
        return ""
    return value.strip().lower().replace("_", "-")


def normalize_tool_name(value: object) -> str:
    if not isinstance(value, str):
        return ""
    return value.strip().lower()
