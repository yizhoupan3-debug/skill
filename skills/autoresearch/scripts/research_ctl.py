#!/usr/bin/env python3
"""Minimal control-plane CLI for the autoresearch skill."""

from __future__ import annotations

import argparse
import json
import re
from copy import deepcopy
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:  # pragma: no cover - optional fallback
    yaml = None


STAGE_BOOTSTRAP = "bootstrap"
STAGE_INNER_LOOP = "inner-loop"
STAGE_OUTER_LOOP = "outer-loop"
STAGE_FINALIZE = "finalize"
VALID_DIRECTIONS = {"DEEPEN", "BROADEN", "PIVOT", "CONCLUDE"}
VALID_OUTCOMES = {"confirmatory", "exploratory", "failed", "ambiguous"}
TEMPLATES_DIR = Path(__file__).resolve().parents[1] / "templates"
NOVELTY_OVERLAPS = {"low", "medium", "high"}
NOVELTY_CONFIDENCE = {"low", "medium", "high"}
NOVELTY_VERDICTS = {"novel", "defensible", "risky", "not-novel"}
FINDINGS_BLOCK_START = "<!-- autoresearch:findings:start -->"
FINDINGS_BLOCK_END = "<!-- autoresearch:findings:end -->"
NOVELTY_BLOCK_START = "<!-- autoresearch:novelty:start -->"
NOVELTY_BLOCK_END = "<!-- autoresearch:novelty:end -->"
SEARCH_PLAN_BLOCK_START = "<!-- autoresearch:search-plan:start -->"
SEARCH_PLAN_BLOCK_END = "<!-- autoresearch:search-plan:end -->"
CLAIMS_BLOCK_START = "<!-- autoresearch:claims:start -->"
CLAIMS_BLOCK_END = "<!-- autoresearch:claims:end -->"
CONTEXT_BLOCK_START = "<!-- autoresearch:context:start -->"
CONTEXT_BLOCK_END = "<!-- autoresearch:context:end -->"
STOPWORDS = {
    "a",
    "an",
    "and",
    "are",
    "as",
    "at",
    "be",
    "by",
    "can",
    "for",
    "from",
    "in",
    "into",
    "is",
    "it",
    "of",
    "on",
    "or",
    "reduce",
    "research",
    "that",
    "the",
    "this",
    "to",
    "use",
    "using",
    "with",
}

QUESTION_PREFIX_RE = re.compile(
    r"^(can|could|does|do|did|is|are|should|would|will|how|why|what|whether)\s+",
    re.IGNORECASE,
)
AXIS_NOVELTY_WEIGHTS = {
    "method": 5,
    "workflow": 5,
    "task": 4,
    "comparison": 4,
    "setting": 3,
    "framing": 2,
    "claim": 3,
}
AXIS_COST_WEIGHTS = {
    "comparison": 1,
    "framing": 1,
    "method": 2,
    "workflow": 2,
    "task": 3,
    "setting": 4,
    "claim": 2,
}
AXIS_REVIEWER_WEIGHTS = {
    "comparison": 5,
    "task": 4,
    "framing": 4,
    "method": 3,
    "workflow": 3,
    "setting": 2,
    "claim": 3,
}
STALE_STATE_DAYS = 10
RECENT_ACTIVITY_DAYS = 14
FALLBACK_ACTIVITY_LIMIT = 3


def now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def parse_iso_timestamp(value: str | None) -> datetime | None:
    if not value:
        return None
    normalized = value.replace("Z", "+00:00")
    try:
        parsed = datetime.fromisoformat(normalized)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def days_since(value: str | None) -> int | None:
    parsed = parse_iso_timestamp(value)
    if parsed is None:
        return None
    delta = datetime.now(timezone.utc) - parsed
    return max(0, delta.days)


def slugify(text: str) -> str:
    lowered = text.strip().lower()
    cleaned = re.sub(r"[^a-z0-9]+", "-", lowered)
    collapsed = re.sub(r"-+", "-", cleaned).strip("-")
    return collapsed or "hypothesis"


def load_template(name: str) -> str:
    path = TEMPLATES_DIR / name
    if not path.exists():
        raise SystemExit(f"Missing template: {path}")
    return path.read_text(encoding="utf-8")


def hypothesis_workspace_dir(workspace: Path, hypothesis_id: str) -> Path:
    return workspace / "experiments" / hypothesis_id


def hypothesis_card_path(workspace: Path, hypothesis_id: str) -> Path:
    return hypothesis_workspace_dir(workspace, hypothesis_id) / "HYPOTHESIS_CARD.md"


def protocol_path(workspace: Path, hypothesis_id: str) -> Path:
    return hypothesis_workspace_dir(workspace, hypothesis_id) / "protocol.md"


def analysis_path(workspace: Path, hypothesis_id: str) -> Path:
    return hypothesis_workspace_dir(workspace, hypothesis_id) / "analysis.md"


def default_run_record_path(hypothesis_id: str, run_id: str) -> str:
    return f"experiments/{hypothesis_id}/{run_id}.md"


def default_reflection_path(hypothesis_id: str, run_id: str | None) -> str:
    base = run_id or "reflection"
    return f"experiments/{hypothesis_id}/{base}-reflection.md"


def format_hypothesis_card(hypothesis: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Hypothesis Card",
            "",
            "## Hypothesis ID",
            "",
            f"`{hypothesis['id']}`",
            "",
            "## Claim",
            "",
            hypothesis.get("claim", "_TBD_"),
            "",
            "## Prediction",
            "",
            hypothesis.get("prediction") or "_Add the expected observable change._",
            "",
            "## Priority",
            "",
            f"`{hypothesis.get('priority', 'medium')}`",
            "",
            "## Success Threshold",
            "",
            "_What metric or observation counts as a win?_",
            "",
            "## Stop Condition",
            "",
            "_When do we stop spending more budget on this branch?_",
            "",
        ]
    )


def format_protocol(hypothesis: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Experiment Protocol",
            "",
            "## Hypothesis",
            "",
            hypothesis.get("claim", "_Which hypothesis is being tested?_"),
            "",
            "## What Change",
            "",
            "_What changes in this run?_",
            "",
            "## Prediction",
            "",
            hypothesis.get("prediction") or "_What outcome do you expect?_",
            "",
            "## Metric",
            "",
            "_Primary metric plus sanity checks._",
            "",
            "## Success Threshold",
            "",
            "_What result counts as success?_",
            "",
            "## Command / Entry Point",
            "",
            "```bash",
            "# put the exact command here",
            "```",
            "",
            "## Seed / Environment",
            "",
            "_Record what is needed for reproducibility._",
            "",
            "## Stop Condition",
            "",
            "_When do you stop this line?_",
            "",
        ]
    )


def format_analysis_stub(hypothesis: dict[str, Any]) -> str:
    return "\n".join(
        [
            f"# Analysis — {hypothesis['id']}",
            "",
            "## Current Pattern",
            "",
            "_Summarize what repeated runs are saying._",
            "",
            "## What This Probably Means",
            "",
            "_Prefer mechanism over raw metric narration._",
            "",
            "## Open Questions",
            "",
            "_What still needs to be disambiguated?_",
            "",
        ]
    )


def format_overlap_risk(overlap: str) -> str:
    return {
        "low": "🟢 low",
        "medium": "🟡 medium",
        "high": "🔴 high",
    }.get(overlap, overlap)


def compact_words(text: str, limit: int = 6) -> list[str]:
    words = re.findall(r"[A-Za-z0-9][A-Za-z0-9_-]*", text.lower())
    filtered: list[str] = []
    for word in words:
        if len(word) <= 2 or word in STOPWORDS:
            continue
        if word not in filtered:
            filtered.append(word)
        if len(filtered) >= limit:
            break
    return filtered


def default_required_evidence(axis: str) -> list[str]:
    axis_lower = axis.lower()
    if "method" in axis_lower or "workflow" in axis_lower:
        return [
            "Direct overlap papers using the same mechanism",
            "Nearest baseline implementations or orchestration frameworks",
            "Claims about what is structurally different",
        ]
    if "setting" in axis_lower or "domain" in axis_lower or "task" in axis_lower:
        return [
            "Prior work in the same domain or task",
            "Recent competitors in the last 3 years",
            "Evidence that the constraint or setting is materially different",
        ]
    if "combination" in axis_lower:
        return [
            "Papers combining the same building blocks",
            "Closest papers combining two of the three components",
            "Evidence that the composition order or objective is different",
        ]
    return [
        "Closest prior work for the same core claim",
        "Recent competitors from the last 3 years",
        "Evidence for the exact differentiation sentence",
    ]


def build_search_queries(claim: str, axis: str) -> list[dict[str, str]]:
    keywords = compact_words(claim)
    broad_terms = " ".join(keywords[:3]) or claim
    focused_terms = " ".join(keywords[:5]) or claim
    recent_terms = focused_terms
    combination_terms = " ".join(keywords[:2] + keywords[-2:]) if len(keywords) >= 4 else focused_terms
    axis_hint = axis.strip().lower() or "claim"
    return [
        {"label": "broad", "query": broad_terms},
        {"label": "focused", "query": f"{focused_terms} {axis_hint}".strip()},
        {"label": "recent", "query": f"{recent_terms} last 3 years".strip()},
        {"label": "combination", "query": combination_terms},
    ]


def build_search_plan_entry(claim_record: dict[str, Any]) -> dict[str, Any]:
    claim = claim_record.get("claim", "")
    axis = claim_record.get("axis", "claim")
    return {
        "claim_id": claim_record.get("claim_id", "C?"),
        "claim": claim,
        "axis": axis,
        "priority_score": claim_record.get("priority_score"),
        "priority_label": claim_record.get("priority_label"),
        "priority_reason": claim_record.get("priority_reason"),
        "recommended_order": claim_record.get("recommended_order"),
        "keywords": compact_words(claim),
        "queries": build_search_queries(claim, axis),
        "sources": [
            "Semantic Scholar",
            "arXiv",
            "Google Scholar",
        ],
        "required_evidence": default_required_evidence(axis),
    }


def score_claim_priority(claim_record: dict[str, Any]) -> dict[str, Any]:
    axis = str(claim_record.get("axis", "claim")).lower()
    novelty = AXIS_NOVELTY_WEIGHTS.get(axis, 3)
    cost = AXIS_COST_WEIGHTS.get(axis, 2)
    reviewer = AXIS_REVIEWER_WEIGHTS.get(axis, 3)

    overlap = claim_record.get("overlap")
    if overlap == "low":
        novelty += 2
    elif overlap == "medium":
        novelty += 1
    elif overlap == "high":
        novelty -= 1
        reviewer += 1

    confidence = claim_record.get("confidence")
    if confidence == "high":
        cost -= 1
    elif confidence == "low":
        cost += 1

    verdict = claim_record.get("verdict")
    if verdict == "novel":
        novelty += 2
    elif verdict == "defensible":
        novelty += 1
    elif verdict == "risky":
        reviewer += 1
        cost += 1
    elif verdict == "not-novel":
        novelty -= 2
        cost += 1

    specificity = str(claim_record.get("specificity", "")).lower()
    if "testable" in specificity:
        cost -= 1
    if "paper-facing" in specificity:
        reviewer += 1

    score = novelty * 3 + reviewer * 2 - cost * 2
    if score >= 18:
        label = "first"
    elif score >= 13:
        label = "next"
    else:
        label = "later"

    if novelty >= reviewer and cost <= 2:
        reason = "high novelty upside with relatively cheap verification"
    elif reviewer >= novelty and cost <= 3:
        reason = "reviewer pressure is high, so checking this early reduces risk"
    elif cost >= 4:
        reason = "potentially useful, but verification is expensive"
    else:
        reason = "worth checking, but not the best first search target"

    return {
        **claim_record,
        "priority_score": score,
        "priority_label": label,
        "priority_reason": reason,
    }


def prioritize_claims(claims: list[dict[str, Any]]) -> list[dict[str, Any]]:
    scored = [score_claim_priority(claim) for claim in claims]
    ranked = sorted(
        scored,
        key=lambda item: (-int(item.get("priority_score", 0)), item.get("claim_id", "")),
    )
    prioritized: list[dict[str, Any]] = []
    for index, item in enumerate(ranked, start=1):
        prioritized.append(
            {
                **item,
                "recommended_order": index,
            }
        )
    return prioritized


def top_priority_claim(state: dict[str, Any]) -> dict[str, Any] | None:
    novelty_gate = state.get("novelty_gate", {})
    for key in ("claim_records", "draft_claims"):
        entries = novelty_gate.get(key, [])
        if not entries:
            continue
        ranked = sorted(
            entries,
            key=lambda item: (
                int(item.get("recommended_order", 999) or 999),
                -int(item.get("priority_score", 0) or 0),
                item.get("claim_id", ""),
            ),
        )
        return ranked[0]
    return None


def current_recommended_focus(state: dict[str, Any]) -> str | None:
    top_claim = top_priority_claim(state)
    if top_claim is None:
        return None
    return f"{top_claim.get('claim_id', 'C?')}: {top_claim.get('claim', '_No claim recorded._')}"


def current_search_plan(state: dict[str, Any]) -> list[dict[str, Any]]:
    novelty_gate = state.get("novelty_gate", {})
    claim_records = novelty_gate.get("claim_records", [])
    if claim_records:
        source_records = claim_records
    elif novelty_gate.get("draft_claims"):
        source_records = novelty_gate["draft_claims"]
    else:
        source_records = []
    plan = [build_search_plan_entry(record) for record in source_records]
    return sorted(
        plan,
        key=lambda item: (
            int(item.get("recommended_order", 999) or 999),
            -int(item.get("priority_score", 0) or 0),
            item.get("claim_id", ""),
        ),
    )


def current_brief(state: dict[str, Any]) -> dict[str, Any] | None:
    return build_novelty_brief(state)


def sort_entries_by_recency(entries: list[dict[str, Any]], *, timestamp_field: str) -> list[dict[str, Any]]:
    return sorted(
        entries,
        key=lambda item: parse_iso_timestamp(str(item.get(timestamp_field) or "")) or datetime.min.replace(tzinfo=timezone.utc),
        reverse=True,
    )


def recent_entries(
    entries: list[dict[str, Any]],
    *,
    timestamp_field: str,
    max_age_days: int = RECENT_ACTIVITY_DAYS,
    limit: int = FALLBACK_ACTIVITY_LIMIT,
    hypothesis_id: str | None = None,
) -> list[dict[str, Any]]:
    filtered: list[dict[str, Any]] = []
    for entry in sort_entries_by_recency(entries, timestamp_field=timestamp_field):
        if hypothesis_id is not None and entry.get("hypothesis_id") != hypothesis_id:
            continue
        age_days = days_since(str(entry.get(timestamp_field) or ""))
        if age_days is None or age_days > max_age_days:
            continue
        filtered.append(entry)
        if len(filtered) >= limit:
            break
    return filtered


def current_context_runs(state: dict[str, Any]) -> list[dict[str, Any]]:
    runs = state.get("run_history", [])
    active_id = state.get("active_hypothesis")
    active_recent = recent_entries(
        runs,
        timestamp_field="recorded_at",
        hypothesis_id=active_id,
    ) if active_id else []
    if active_recent:
        return active_recent
    global_recent = recent_entries(runs, timestamp_field="recorded_at")
    if global_recent:
        return global_recent
    return sort_entries_by_recency(runs, timestamp_field="recorded_at")[:FALLBACK_ACTIVITY_LIMIT]


def current_context_decisions(state: dict[str, Any]) -> list[dict[str, Any]]:
    decisions = state.get("decisions", [])
    active_id = state.get("active_hypothesis")
    active_recent = recent_entries(
        decisions,
        timestamp_field="recorded_at",
        hypothesis_id=active_id,
    ) if active_id else []
    if active_recent:
        return active_recent
    global_recent = recent_entries(decisions, timestamp_field="recorded_at")
    if global_recent:
        return global_recent
    return sort_entries_by_recency(decisions, timestamp_field="recorded_at")[:FALLBACK_ACTIVITY_LIMIT]


def state_freshness(state: dict[str, Any]) -> dict[str, Any]:
    updated_days = days_since(str(state.get("updated_at") or ""))
    recent_runs = current_context_runs(state)
    recent_decisions = current_context_decisions(state)
    stale = updated_days is not None and updated_days > STALE_STATE_DAYS
    history_bias_risk = stale or (
        bool(state.get("run_history") or state.get("decisions"))
        and not recent_runs
        and not recent_decisions
    )
    return {
        "updated_days": updated_days,
        "stale": stale,
        "history_bias_risk": history_bias_risk,
        "recent_runs": recent_runs,
        "recent_decisions": recent_decisions,
    }


def expected_baselines_for_axis(axis: str) -> list[str]:
    axis_lower = axis.lower()
    if axis_lower in {"method", "workflow"}:
        return [
            "Closest simple baseline implementation",
            "Nearest orchestration or workflow framework baseline",
            "A stripped-down version without the claimed mechanism",
        ]
    if axis_lower == "task":
        return [
            "Closest task-specific prior method",
            "Simple transfer baseline without the claimed novelty",
            "Recent strongest competitor from the last 3 years",
        ]
    if axis_lower == "setting":
        return [
            "Same method in an adjacent setting",
            "Simple baseline in the same constraint",
            "Closest unconstrained baseline to show what the setting changes",
        ]
    if axis_lower == "comparison":
        return [
            "Closest simple baseline the reviewer will ask about first",
            "A stronger but obvious comparator",
            "An ablated version removing the claimed differentiator",
        ]
    if axis_lower == "framing":
        return [
            "Closest paper making a similar framing claim",
            "A simpler framing that could explain the same result",
            "The baseline narrative a reviewer would default to",
        ]
    return [
        "Closest prior work the reviewer will expect",
        "A simple baseline explanation",
        "The strongest recent competitor in the same area",
    ]


def verification_standard_for_priority(label: str) -> str:
    if label == "first":
        return "You should be able to decide proceed vs reframe after one focused search pass."
    if label == "next":
        return "This should be checked after the first claim is clarified, not before."
    return "Useful later, but not strong enough to spend the first search budget on."


def build_novelty_brief(state: dict[str, Any]) -> dict[str, Any] | None:
    top_claim = top_priority_claim(state)
    if top_claim is None:
        return None
    search_plan = current_search_plan(state)
    matching_plan = next(
        (entry for entry in search_plan if entry.get("claim_id") == top_claim.get("claim_id")),
        None,
    )
    axis = str(top_claim.get("axis", "claim"))
    sources = matching_plan.get("sources", []) if matching_plan is not None else [
        "Semantic Scholar",
        "arXiv",
        "Google Scholar",
    ]
    queries = matching_plan.get("queries", []) if matching_plan is not None else build_search_queries(
        str(top_claim.get("claim", "")),
        axis,
    )
    required_evidence = matching_plan.get("required_evidence", []) if matching_plan is not None else default_required_evidence(axis)
    return {
        "claim_id": top_claim.get("claim_id", "C?"),
        "claim": top_claim.get("claim", "_No claim recorded._"),
        "axis": axis,
        "priority_label": top_claim.get("priority_label", "later"),
        "priority_score": top_claim.get("priority_score", 0),
        "priority_reason": top_claim.get("priority_reason", "_No reason recorded._"),
        "decision_goal": "Decide whether this claim is safe to keep, should be reframed, or should be dropped.",
        "verification_standard": verification_standard_for_priority(str(top_claim.get("priority_label", "later"))),
        "sources": sources,
        "queries": queries,
        "required_evidence": required_evidence,
        "expected_baselines": expected_baselines_for_axis(axis),
    }


def cleanup_question_text(question: str) -> str:
    cleaned = question.strip().rstrip("?.!")
    cleaned = QUESTION_PREFIX_RE.sub("", cleaned)
    return cleaned.strip() or question.strip()


def extract_question_parts(question: str) -> dict[str, str]:
    cleaned = cleanup_question_text(question)
    lowered = cleaned.lower()
    focus = cleaned
    target = "the stated task or setting"
    effect = "a meaningful measurable improvement"

    match = re.match(r"(.+?)\s+(improve|improves|reduce|reduces|increase|increases|enable|enables)\s+(.+)", lowered)
    if match:
        left, verb, right = match.groups()
        focus = left.strip()
        target = right.strip()
        effect = f"{verb} {right.strip()}"
    else:
        using_match = re.match(r"using\s+(.+?)\s+for\s+(.+)", lowered)
        if using_match:
            focus = using_match.group(1).strip()
            target = using_match.group(2).strip()
            effect = f"improve {target}"
        else:
            keywords = compact_words(cleaned, limit=8)
            if keywords:
                focus = " ".join(keywords[:4])
                if len(keywords) >= 6:
                    target = " ".join(keywords[4:8])
                    effect = f"improve {target}"
    return {
        "focus": focus,
        "target": target,
        "effect": effect,
        "cleaned_question": cleaned,
    }


def default_draft_claim_evidence(axis: str, focus: str, target: str) -> list[str]:
    axis_lower = axis.lower()
    if axis_lower == "method":
        return [
            f"Closest papers using {focus}",
            "Nearest mechanism-level baselines",
            "Evidence that the mechanism meaningfully differs",
        ]
    if axis_lower == "task":
        return [
            f"Prior work on {focus} for {target}",
            "Recent task/domain competitors from the last 3 years",
            "Evidence that the task framing is not already saturated",
        ]
    if axis_lower == "setting":
        return [
            f"Papers in the same constrained setting as {target}",
            "Evidence that the constraint changes the problem materially",
            "Comparable results in adjacent settings",
        ]
    return [
        "Papers making a similar contribution claim",
        "Reviewer-expected baseline papers",
        "Evidence for the exact differentiation sentence",
    ]


def propose_claims_from_question(question: str, count: int = 4) -> list[dict[str, Any]]:
    parts = extract_question_parts(question)
    focus = parts["focus"]
    target = parts["target"]
    effect = parts["effect"]
    templates = [
        {
            "axis": "method",
            "specificity": "testable hypothesis",
            "claim": f"Using {focus} is itself a defensible mechanism-level contribution.",
        },
        {
            "axis": "task",
            "specificity": "direction with concrete benchmark target",
            "claim": f"Applying {focus} to {target} is novel enough to justify a focused study.",
        },
        {
            "axis": "setting",
            "specificity": "testable hypothesis",
            "claim": f"The value of {focus} depends on the specific setting or constraint around {target}.",
        },
        {
            "axis": "framing",
            "specificity": "paper-facing positioning claim",
            "claim": f"The strongest paper-facing claim is that {focus} can {effect}, not that it is universally better.",
        },
        {
            "axis": "comparison",
            "specificity": "reviewer-facing claim",
            "claim": f"The core reviewer question is whether {focus} beats the closest simple baseline for {target}.",
        },
    ]
    drafts: list[dict[str, Any]] = []
    for index, template in enumerate(templates[: max(1, min(count, len(templates)))]):
        drafts.append(
            {
                "claim_id": f"C{index + 1}",
                "axis": template["axis"],
                "specificity": template["specificity"],
                "claim": template["claim"],
                "required_evidence": default_draft_claim_evidence(
                    template["axis"],
                    focus,
                    target,
                ),
            }
        )
    return drafts


def overall_novelty_assessment(state: dict[str, Any]) -> str:
    records = state.get("novelty_gate", {}).get("claim_records", [])
    if not records:
        return "insufficient"
    verdicts = [record.get("verdict") for record in records]
    if verdicts and all(verdict == "novel" for verdict in verdicts):
        return "strong"
    if any(verdict == "not-novel" for verdict in verdicts):
        not_novel_count = sum(verdict == "not-novel" for verdict in verdicts)
        return "weak" if not_novel_count >= 2 else "moderate"
    if any(verdict == "risky" for verdict in verdicts):
        return "moderate"
    return "strong" if any(verdict == "novel" for verdict in verdicts) else "moderate"


def strongest_current_claim(state: dict[str, Any]) -> str:
    records = state.get("novelty_gate", {}).get("claim_records", [])
    verdict_order = {"novel": 0, "defensible": 1, "risky": 2, "not-novel": 3}
    confidence_order = {"high": 0, "medium": 1, "low": 2}
    if records:
        ranked = sorted(
            records,
            key=lambda record: (
                verdict_order.get(record.get("verdict", "risky"), 9),
                confidence_order.get(record.get("confidence", "medium"), 1),
                record.get("claim_id", ""),
            ),
        )
        return ranked[0].get("claim", "_No strong claim recorded yet._")
    active_id = state.get("active_hypothesis")
    active = find_hypothesis(state, active_id) if active_id else None
    if active is not None:
        return active.get("claim", "_No strong claim recorded yet._")
    return "_No strong claim recorded yet._"


def summarize_rules_in(state: dict[str, Any]) -> list[str]:
    lines = [
        f"{record['run_id']}: {record.get('summary', '_No summary_')}"
        for record in current_context_runs(state)
    ]
    return lines or ["_No run-backed support recorded yet._"]


def summarize_rules_out(state: dict[str, Any]) -> list[str]:
    lines = [
        f"{record['run_id']}: {record.get('summary', '_No summary_')}"
        for record in current_context_runs(state)
        if record.get("outcome") in {"failed", "ambiguous"}
    ]
    return lines or ["_No ruled-out branch has been recorded yet._"]


def summarize_remaining_risks(state: dict[str, Any]) -> list[str]:
    blockers = state.get("blockers", [])
    if blockers:
        return [str(blocker) for blocker in blockers[:3]]
    next_actions = state.get("next_actions", [])
    return next_actions[:3] or ["_No explicit remaining risk recorded._"]


def upsert_managed_block(text: str, start_marker: str, end_marker: str, content: str) -> str:
    managed = f"{start_marker}\n{content.rstrip()}\n{end_marker}"
    if start_marker in text and end_marker in text:
        pattern = re.compile(
            rf"{re.escape(start_marker)}.*?{re.escape(end_marker)}",
            re.DOTALL,
        )
        return pattern.sub(managed, text, count=1)
    stripped = text.rstrip()
    if stripped:
        return f"{managed}\n\n{stripped}\n"
    return f"{managed}\n"


def render_findings_summary(state: dict[str, Any]) -> str:
    lines = [
        "## Managed Summary",
        "",
        f"- strongest current claim: {strongest_current_claim(state)}",
        "",
        "### What The Evidence Rules In",
    ]
    for item in summarize_rules_in(state):
        lines.append(f"- {item}")
    lines.extend(["", "### What The Evidence Rules Out"])
    for item in summarize_rules_out(state):
        lines.append(f"- {item}")
    lines.extend(["", "### Remaining Risks"])
    for item in summarize_remaining_risks(state):
        lines.append(f"- {item}")
    lines.extend(
        [
            "",
            "### Positioning Strategy",
            f"- {state.get('novelty_gate', {}).get('differentiation_strategy') or '_Not recorded yet._'}",
        ]
    )
    return "\n".join(lines)


def render_novelty_gate_summary(state: dict[str, Any]) -> str:
    novelty_gate = state.get("novelty_gate", {})
    records = novelty_gate.get("claim_records", [])
    lines = [
        "## Managed Summary",
        "",
        f"- status: {novelty_gate.get('status', 'pending')}",
        f"- overall novelty assessment: {overall_novelty_assessment(state)}",
        f"- decision: {novelty_gate.get('decision') or '_Not recorded yet._'}",
        f"- overlap summary: {novelty_gate.get('overlap_summary') or '_Not recorded yet._'}",
        "",
        "## Claim Comparison Matrix",
        "",
        "| Claim | Axis | Closest Prior Work | Overlap | Difference | Confidence | Verdict |",
        "|---|---|---|---|---|---|---|",
    ]
    if not records:
        lines.append("| _none yet_ | - | - | - | - | - | - |")
    else:
        for record in records:
            lines.append(
                "| {claim} | {axis} | {prior} | {overlap} | {difference} | {confidence} | {verdict} |".format(
                    claim=record.get("claim", "_missing_").replace("|", "/"),
                    axis=record.get("axis", "-").replace("|", "/"),
                    prior=record.get("closest_prior_work", "-").replace("|", "/"),
                    overlap=format_overlap_risk(record.get("overlap", "-")),
                    difference=record.get("difference", "-").replace("|", "/"),
                    confidence=record.get("confidence", "-"),
                    verdict=record.get("verdict", "-"),
                )
            )
    lines.extend(
        [
            "",
            "## Differentiation Strategy",
            "",
            novelty_gate.get("differentiation_strategy") or "_Not recorded yet._",
        ]
    )
    return "\n".join(lines)


def render_search_plan_summary(state: dict[str, Any]) -> str:
    plan_entries = current_search_plan(state)
    top_entry = next(
        (entry for entry in plan_entries if entry.get("recommended_order") == 1),
        None,
    )
    lines = [
        "## Managed Search Plan",
        "",
        f"- generated entries: {len(plan_entries)}",
        "- source priority: Semantic Scholar -> arXiv -> Google Scholar",
        f"- recommended first search target: {top_entry.get('claim_id')} ({top_entry.get('priority_label')})"
        if top_entry is not None
        else "- recommended first search target: _not set_",
        "",
    ]
    if not plan_entries:
        lines.append("_No search plan has been generated yet._")
        return "\n".join(lines)

    for entry in plan_entries:
        lines.extend(
            [
                f"### {entry.get('claim_id', 'C?')} — {entry.get('claim', '_missing_')}",
                "",
                f"- axis: {entry.get('axis', '-')}",
                f"- recommended order: {entry.get('recommended_order', '-')}",
                f"- priority: {entry.get('priority_label', '-')} ({entry.get('priority_score', '-')})",
                f"- why first or later: {entry.get('priority_reason', '-')}",
                f"- keywords: {', '.join(entry.get('keywords', [])) or '_none_'}",
                f"- sources: {', '.join(entry.get('sources', []))}",
                "",
                "#### Query Ladder",
            ]
        )
        for query in entry.get("queries", []):
            lines.append(f"- {query.get('label', 'query')}: `{query.get('query', '')}`")
        lines.extend(["", "#### Required Evidence"])
        for requirement in entry.get("required_evidence", []):
            lines.append(f"- {requirement}")
        lines.append("")
    return "\n".join(lines).rstrip()


def render_claims_summary(state: dict[str, Any]) -> str:
    drafts = state.get("novelty_gate", {}).get("draft_claims", [])
    top_draft = next(
        (draft for draft in drafts if draft.get("recommended_order") == 1),
        None,
    )
    lines = [
        "## Managed Claim Extraction",
        "",
        f"- generated claims: {len(drafts)}",
        f"- recommended first claim: {top_draft.get('claim_id')} ({top_draft.get('priority_label')})"
        if top_draft is not None
        else "- recommended first claim: _not set_",
        "",
    ]
    if not drafts:
        lines.append("_No draft claims have been generated yet._")
        return "\n".join(lines)
    for draft in drafts:
        lines.extend(
            [
                f"### {draft.get('claim_id', 'C?')}",
                "",
                f"- axis: {draft.get('axis', '-')}",
                f"- specificity: {draft.get('specificity', '-')}",
                f"- recommended order: {draft.get('recommended_order', '-')}",
                f"- priority: {draft.get('priority_label', '-')} ({draft.get('priority_score', '-')})",
                f"- why first or later: {draft.get('priority_reason', '-')}",
                f"- claim: {draft.get('claim', '-')}",
                "",
                "#### Required Evidence",
            ]
        )
        for item in draft.get("required_evidence", []):
            lines.append(f"- {item}")
        lines.append("")
    return "\n".join(lines).rstrip()


def render_current_context_summary(state: dict[str, Any]) -> str:
    freshness = state_freshness(state)
    recent_runs = freshness["recent_runs"]
    recent_decisions = freshness["recent_decisions"]
    brief = current_brief(state)
    lines = [
        "## Managed Current Context",
        "",
        "- source of truth: `research-state.yaml`",
        f"- state updated_at: {state.get('updated_at', '-')}",
        f"- freshness: {'stale' if freshness['stale'] else 'fresh'}",
        f"- history bias risk: {'high' if freshness['history_bias_risk'] else 'low'}",
        f"- active hypothesis: {state.get('active_hypothesis') or '-'}",
        f"- recommended focus: {current_recommended_focus(state) or '-'}",
        "- guardrail: treat `research-log.md` and older notes as background only unless they reappear in the current context window.",
        "",
        "### Recent Activity Window",
        f"- window policy: prefer the active hypothesis and the last {RECENT_ACTIVITY_DAYS} days; otherwise fall back to the latest few entries.",
        "",
        "### Recent Runs",
    ]
    if not recent_runs:
        lines.append("- _No recent runs in the current context window._")
    else:
        for record in recent_runs:
            lines.append(
                f"- {record.get('run_id', '-')}: {record.get('summary', '_No summary_')}"
            )
    lines.extend(["", "### Recent Decisions"])
    if not recent_decisions:
        lines.append("- _No recent decisions in the current context window._")
    else:
        for decision in recent_decisions:
            lines.append(
                f"- {decision.get('run_id') or 'no-run'}: {decision.get('direction', '-')} because {decision.get('reason', '_No reason_')}"
            )
    if brief is not None:
        lines.extend(
            [
                "",
                "### Active Novelty Brief",
                f"- claim: {brief.get('claim_id')} — {brief.get('claim')}",
                f"- decision goal: {brief.get('decision_goal')}",
                f"- verification standard: {brief.get('verification_standard')}",
                "- expected baselines:",
            ]
        )
        for baseline in brief.get("expected_baselines", []):
            lines.append(f"- {baseline}")
    if freshness["history_bias_risk"]:
        lines.extend(
            [
                "",
                "### Reconcile First",
                "- Confirm the active hypothesis is still the real target before trusting old notes.",
                "- Re-check live data, code, or current artifacts before extending any older conclusion.",
            ]
        )
    return "\n".join(lines)


def sync_search_plan_file(workspace: Path, state: dict[str, Any]) -> None:
    path = workspace / "literature" / "NOVELTY_SEARCH_PLAN.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    text = path.read_text(encoding="utf-8") if path.exists() else "# Novelty Search Plan\n\n"
    updated = upsert_managed_block(
        text,
        SEARCH_PLAN_BLOCK_START,
        SEARCH_PLAN_BLOCK_END,
        render_search_plan_summary(state),
    )
    path.write_text(updated, encoding="utf-8")


def sync_claims_file(workspace: Path, state: dict[str, Any]) -> None:
    path = workspace / "literature" / "NOVELTY_CLAIMS.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    text = path.read_text(encoding="utf-8") if path.exists() else "# Novelty Claims\n\n"
    updated = upsert_managed_block(
        text,
        CLAIMS_BLOCK_START,
        CLAIMS_BLOCK_END,
        render_claims_summary(state),
    )
    path.write_text(updated, encoding="utf-8")


def sync_current_context_file(workspace: Path, state: dict[str, Any]) -> None:
    path = workspace / "CURRENT_CONTEXT.md"
    text = path.read_text(encoding="utf-8") if path.exists() else "# Current Context\n\n"
    updated = upsert_managed_block(
        text,
        CONTEXT_BLOCK_START,
        CONTEXT_BLOCK_END,
        render_current_context_summary(state),
    )
    path.write_text(updated, encoding="utf-8")


def remove_legacy_context_files(workspace: Path) -> None:
    legacy_paths = [
        workspace / "literature" / "NOVELTY_BRIEF.md",
    ]
    for path in legacy_paths:
        if path.exists():
            path.unlink()


def sync_findings_file(workspace: Path, state: dict[str, Any]) -> None:
    path = workspace / "findings.md"
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    updated = upsert_managed_block(
        text,
        FINDINGS_BLOCK_START,
        FINDINGS_BLOCK_END,
        render_findings_summary(state),
    )
    path.write_text(updated, encoding="utf-8")


def sync_novelty_gate_file(workspace: Path, state: dict[str, Any]) -> None:
    path = workspace / "literature" / "NOVELTY_GATE.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    updated = upsert_managed_block(
        text,
        NOVELTY_BLOCK_START,
        NOVELTY_BLOCK_END,
        render_novelty_gate_summary(state),
    )
    path.write_text(updated, encoding="utf-8")


def format_run_record(record: dict[str, Any]) -> str:
    metric_name = record.get("metric_name") or "metric"
    metric_value = record.get("metric_value") or "value"
    command = record.get("command") or "_not recorded_"
    artifact_path = record.get("evidence_path") or "_not recorded_"
    return "\n".join(
        [
            "# Run Record",
            "",
            "## Run ID",
            "",
            f"`{record['run_id']}`",
            "",
            "## Hypothesis",
            "",
            f"`{record['hypothesis_id']}`",
            "",
            "## Outcome",
            "",
            f"`{record['outcome']}`",
            "",
            "## Summary",
            "",
            record.get("summary", "_No summary recorded._"),
            "",
            "## Metric Snapshot",
            "",
            f"- metric: {metric_name}",
            f"- value: {metric_value}",
            "- sanity: _fill in sanity checks here_",
            "",
            "## Evidence",
            "",
            f"- command: {command}",
            f"- artifact path: {artifact_path}",
            "- code version: _fill commit hash or protocol version_",
            "",
            "## Rules In / Rules Out",
            "",
            "_What changed in your belief after this run?_",
            "",
        ]
    )


def format_reflection_note(decision: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Reflection Note",
            "",
            "## Run",
            "",
            f"`{decision.get('run_id') or 'run-xxx'}`",
            "",
            "## What Happened",
            "",
            decision.get("reason", "_Summarize the observed pattern._"),
            "",
            "## Why It Probably Happened",
            "",
            decision.get("reason", "_Mechanistic explanation or best current guess._"),
            "",
            "## Rules In / Rules Out",
            "",
            "_What did this result actually eliminate or support?_",
            "",
            "## Direction",
            "",
            f"`{decision.get('direction', 'DEEPEN')}`",
            "",
            "## Next Step",
            "",
            decision.get("next_step") or "_One concrete next move only._",
            "",
        ]
    )


def write_if_missing(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        path.write_text(content, encoding="utf-8")


def append_research_log(workspace: Path, heading: str, bullets: list[str]) -> None:
    log_path = workspace / "research-log.md"
    lines = ["", f"## {datetime.now().strftime('%Y-%m-%d')} — {heading}", ""]
    for bullet in bullets:
        lines.append(f"- {bullet}")
    lines.append("")
    with log_path.open("a", encoding="utf-8") as handle:
        handle.write("\n".join(lines))


def materialize_hypothesis_workspace(workspace: Path, hypothesis: dict[str, Any]) -> None:
    hypothesis_dir = hypothesis_workspace_dir(workspace, hypothesis["id"])
    hypothesis_dir.mkdir(parents=True, exist_ok=True)
    write_if_missing(hypothesis_card_path(workspace, hypothesis["id"]), format_hypothesis_card(hypothesis))
    write_if_missing(protocol_path(workspace, hypothesis["id"]), format_protocol(hypothesis))
    write_if_missing(analysis_path(workspace, hypothesis["id"]), format_analysis_stub(hypothesis))


def materialize_run_record(workspace: Path, record: dict[str, Any]) -> Path:
    run_path = workspace / record["evidence_path"]
    write_if_missing(run_path, format_run_record(record))
    return run_path


def materialize_reflection_note(workspace: Path, decision: dict[str, Any]) -> Path:
    note_path = workspace / decision["note_path"]
    write_if_missing(note_path, format_reflection_note(decision))
    return note_path


def sync_workspace_files(workspace: Path, state: dict[str, Any]) -> None:
    remove_legacy_context_files(workspace)
    for hypothesis in state.get("hypotheses", []):
        materialize_hypothesis_workspace(workspace, hypothesis)
    for record in state.get("run_history", []):
        if record.get("evidence_path"):
            materialize_run_record(workspace, record)
    for decision in state.get("decisions", []):
        if decision.get("note_path"):
            materialize_reflection_note(workspace, decision)
    sync_novelty_gate_file(workspace, state)
    sync_claims_file(workspace, state)
    sync_search_plan_file(workspace, state)
    sync_current_context_file(workspace, state)
    sync_findings_file(workspace, state)


def default_state(project: str, question: str, mode: str) -> dict[str, Any]:
    timestamp = now_iso()
    state = {
        "schema_version": 2,
        "project": project,
        "question": question,
        "mode": mode,
        "status": "active",
        "stage": STAGE_BOOTSTRAP,
        "current_direction": None,
        "active_hypothesis": None,
        "novelty_gate": {
            "status": "pending",
            "claims": [],
            "claim_records": [],
            "draft_claims": [],
            "overlap_summary": None,
            "differentiation_strategy": None,
            "decision": None,
        },
        "hypotheses": [],
        "hypothesis_backlog": [],
        "run_history": [],
        "evidence_index": [],
        "blockers": [],
        "decisions": [],
        "next_actions": [],
        "created_at": timestamp,
        "updated_at": timestamp,
    }
    state["next_actions"] = recommend_next_actions(state)
    return state


def ensure_state_defaults(state: dict[str, Any]) -> dict[str, Any]:
    hydrated = deepcopy(state)
    hydrated.setdefault("schema_version", 2)
    hydrated.setdefault("status", "active")
    hydrated.setdefault("stage", STAGE_BOOTSTRAP)
    hydrated.setdefault("mode", "quick")
    hydrated.setdefault("current_direction", None)
    hydrated.setdefault("active_hypothesis", None)
    novelty_gate = hydrated.setdefault("novelty_gate", {})
    novelty_gate.setdefault("status", "pending")
    novelty_gate.setdefault("claims", [])
    novelty_gate.setdefault("claim_records", [])
    novelty_gate.setdefault("draft_claims", [])
    novelty_gate.setdefault("overlap_summary", None)
    novelty_gate.setdefault("differentiation_strategy", None)
    novelty_gate.setdefault("decision", None)
    novelty_gate.pop("search_plan", None)
    novelty_gate.pop("recommended_focus", None)
    novelty_gate.pop("brief", None)
    hydrated.setdefault("hypotheses", [])
    hydrated.setdefault("hypothesis_backlog", [])
    hydrated.setdefault("run_history", [])
    hydrated.setdefault("evidence_index", [])
    hydrated.setdefault("blockers", [])
    hydrated.setdefault("decisions", [])
    hydrated.setdefault("next_actions", [])
    hydrated.setdefault("created_at", now_iso())
    hydrated.setdefault("updated_at", hydrated["created_at"])
    return hydrated


def load_state(path: Path) -> dict[str, Any]:
    raw = path.read_text(encoding="utf-8")
    if yaml is not None:
        data = yaml.safe_load(raw)
    else:
        data = json.loads(raw)
    if not isinstance(data, dict):
        raise SystemExit(f"State file must be a mapping: {path}")
    return ensure_state_defaults(data)


def dump_state(path: Path, state: dict[str, Any]) -> None:
    state_to_write = deepcopy(state)
    novelty_gate = state_to_write.get("novelty_gate", {})
    if isinstance(novelty_gate, dict):
        novelty_gate.pop("search_plan", None)
        novelty_gate.pop("recommended_focus", None)
        novelty_gate.pop("brief", None)
    state_to_write["updated_at"] = now_iso()
    state_to_write["next_actions"] = recommend_next_actions(state_to_write)
    if yaml is not None:
        rendered = yaml.safe_dump(
            state_to_write,
            allow_unicode=True,
            sort_keys=False,
        )
    else:
        rendered = json.dumps(state_to_write, indent=2, ensure_ascii=False)
        rendered += "\n"
    path.write_text(rendered, encoding="utf-8")


def resolve_workspace(path_str: str) -> Path:
    candidate = Path(path_str).expanduser().resolve()
    if candidate.is_file():
        if candidate.name != "research-state.yaml":
            raise SystemExit("Workspace path must be a project directory or research-state.yaml")
        return candidate.parent
    return candidate


def ensure_workspace(path_str: str) -> tuple[Path, Path]:
    workspace = resolve_workspace(path_str)
    state_path = workspace / "research-state.yaml"
    if not state_path.exists():
        raise SystemExit(f"Missing state file: {state_path}")
    return workspace, state_path


def find_hypothesis(state: dict[str, Any], hypothesis_id: str) -> dict[str, Any] | None:
    for item in state.get("hypotheses", []):
        if item.get("id") == hypothesis_id:
            return item
    return None


def next_run_id(state: dict[str, Any]) -> str:
    return f"run-{len(state.get('run_history', [])) + 1:03d}"


def choose_backlog_hypothesis(state: dict[str, Any]) -> dict[str, Any] | None:
    hypotheses = state.get("hypotheses", [])
    if not hypotheses:
        return None
    backlog_ids = state.get("hypothesis_backlog", [])
    if backlog_ids:
        for hypothesis_id in backlog_ids:
            candidate = find_hypothesis(state, hypothesis_id)
            if candidate is None:
                continue
            if candidate.get("status") == "concluded":
                continue
            return candidate
    priority_order = {"high": 0, "medium": 1, "low": 2}
    ranked = sorted(
        hypotheses,
        key=lambda item: (
            priority_order.get(str(item.get("priority", "medium")).lower(), 1),
            item.get("id", ""),
        ),
    )
    return ranked[0] if ranked else None


def latest_run_for_hypothesis(state: dict[str, Any], hypothesis_id: str) -> dict[str, Any] | None:
    for item in reversed(state.get("run_history", [])):
        if item.get("hypothesis_id") == hypothesis_id:
            return item
    return None


def latest_decision_for_hypothesis(state: dict[str, Any], hypothesis_id: str) -> dict[str, Any] | None:
    for item in reversed(state.get("decisions", [])):
        if item.get("hypothesis_id") == hypothesis_id:
            return item
    return None


def recommend_next_actions(state: dict[str, Any]) -> list[str]:
    freshness = state_freshness(state)
    if freshness["history_bias_risk"]:
        return [
            "先刷新当前上下文：确认 active hypothesis 和当前目标，旧日志只当背景。",
            "先看 CURRENT_CONTEXT.md 和 research-state.yaml，不要直接沿用更早的 findings 或 research-log 结论。",
            "重查一遍当前代码、数据或最新实验输出，再决定要不要继续旧方向。",
        ]

    if state.get("status") == "concluded":
        return [
            "Freeze the final narrative in findings.md and to_human/.",
            "Archive the winning evidence path and keep experiment folders append-only.",
        ]

    novelty_gate = state.get("novelty_gate", {})
    gate_status = novelty_gate.get("status", "pending")
    claims = novelty_gate.get("claims", [])
    if gate_status != "passed":
        actions: list[str] = []
        if not claims:
            actions.append("提炼 3 到 5 条 novelty claims，先写进 literature/NOVELTY_GATE.md。")
        actions.append("先完成 novelty gate，再启动高成本实验。")
        if gate_status == "pending":
            actions.append("给每条 claim 标注 overlap level，并写 differentiation strategy。")
        return actions

    hypotheses = state.get("hypotheses", [])
    if not hypotheses:
        return [
            "补 3 条可比较的 hypothesis，并为每条写 prediction 和 success threshold。",
            "从最高优先级 hypothesis 开始，不要并发改同一份研究状态。",
        ]

    active_id = state.get("active_hypothesis")
    active = find_hypothesis(state, active_id) if active_id else None
    if active is None:
        candidate = choose_backlog_hypothesis(state)
        if candidate is None:
            return ["清理 hypothesis 列表，重新指定一个 active hypothesis。"]
        return [
            f"把 {candidate['id']} 设为 active hypothesis，并先写协议。",
            f"在 experiments/{candidate['id']}/ 下落 protocol 和 run record。",
        ]

    latest_run = latest_run_for_hypothesis(state, active["id"])
    if latest_run is None:
        return [
            f"先为 {active['id']} 写 protocol，再做第一轮 bounded run。",
            "跑完立刻记录 metric、sanity check 和 rules in / rules out。",
        ]

    latest_decision = latest_decision_for_hypothesis(state, active["id"])
    if latest_decision is None or latest_decision.get("run_id") != latest_run.get("run_id"):
        return [
            f"对 {latest_run['run_id']} 做 reflection，并明确选 DEEPEN/BROADEN/PIVOT/CONCLUDE。",
            "把结果写回 findings.md，而不只是留在聊天里。",
        ]

    direction = latest_decision.get("direction")
    if direction == "DEEPEN":
        return [
            f"围绕 {active['id']} 收紧变量，再做一个更小更干净的验证实验。",
            "只改一个关键因素，避免把因果解释搅混。",
        ]
    if direction == "BROADEN":
        return [
            f"把 {active['id']} 的结论扩到第二个 setting 或 baseline。",
            "保持协议不变，只扩数据面或比较面。",
        ]
    if direction == "PIVOT":
        candidate = choose_backlog_hypothesis(state)
        if candidate is not None and candidate["id"] != active["id"]:
            return [
                f"停止继续堆 {active['id']}，切到 {candidate['id']} 开新 protocol。",
                "把旧方向失败原因写清楚，避免重复试错。",
            ]
        return [
            "当前方向该 pivot，但还缺新的候选 hypothesis。",
            "先补 hypothesis backlog，再选新的 active hypothesis。",
        ]
    return [
        "进入 finalize，把 strongest claim、证据链和未解决风险收束成 handoff。",
    ]


def init_workspace(project: str, question: str, base_dir: str = ".", mode: str = "quick") -> Path:
    root = (Path(base_dir).expanduser().resolve() / project).resolve()
    dirs = [
        root,
        root / "literature",
        root / "src",
        root / "data",
        root / "experiments",
        root / "experiments" / "_templates",
        root / "to_human",
        root / "paper",
    ]
    for directory in dirs:
        directory.mkdir(parents=True, exist_ok=True)

    state_path = root / "research-state.yaml"
    if state_path.exists():
        raise SystemExit(f"Refusing to overwrite existing workspace: {root}")

    state = default_state(project=project, question=question, mode=mode)
    dump_state(state_path, state)

    (root / "research-log.md").write_text(
        load_template("research-log.md").format(
            project=project,
            question=question,
            date=datetime.now().strftime("%Y-%m-%d"),
        ),
        encoding="utf-8",
    )
    (root / "findings.md").write_text(
        load_template("findings.md").format(project=project, question=question),
        encoding="utf-8",
    )
    (root / "BOOTSTRAP_BRIEF.md").write_text(
        load_template("bootstrap-brief.md").format(project=project, question=question),
        encoding="utf-8",
    )
    (root / "literature" / "NOVELTY_GATE.md").write_text(
        load_template("novelty-gate.md").format(project=project, question=question),
        encoding="utf-8",
    )
    (root / "experiments" / "README.md").write_text(
        load_template("experiments-readme.md").format(project=project),
        encoding="utf-8",
    )
    template_map = {
        "HYPOTHESIS_CARD.md": "hypothesis-card.md",
        "PROTOCOL_TEMPLATE.md": "protocol-template.md",
        "RUN_RECORD_TEMPLATE.md": "run-record-template.md",
        "REFLECTION_TEMPLATE.md": "reflection-template.md",
    }
    for output_name, template_name in template_map.items():
        (root / "experiments" / "_templates" / output_name).write_text(
            load_template(template_name),
            encoding="utf-8",
        )
    sync_workspace_files(root, state)

    return root


def add_hypothesis(
    state: dict[str, Any],
    *,
    claim: str,
    prediction: str | None,
    priority: str,
    hypothesis_id: str | None = None,
) -> dict[str, Any]:
    next_state = ensure_state_defaults(state)
    resolved_id = hypothesis_id or slugify(claim)[:40]
    if find_hypothesis(next_state, resolved_id) is not None:
        raise SystemExit(f"Hypothesis already exists: {resolved_id}")
    entry = {
        "id": resolved_id,
        "claim": claim,
        "prediction": prediction,
        "priority": priority,
        "status": "queued",
        "created_at": now_iso(),
    }
    next_state["hypotheses"].append(entry)
    next_state["hypothesis_backlog"].append(resolved_id)
    if next_state.get("active_hypothesis") is None and next_state.get("novelty_gate", {}).get("status") == "passed":
        next_state["active_hypothesis"] = resolved_id
        entry["status"] = "active"
        next_state["hypothesis_backlog"].remove(resolved_id)
    return next_state


def record_run(
    state: dict[str, Any],
    *,
    hypothesis_id: str,
    outcome: str,
    summary: str,
    metric_name: str | None,
    metric_value: str | None,
    command: str | None,
    evidence_path: str | None,
) -> dict[str, Any]:
    next_state = ensure_state_defaults(state)
    hypothesis = find_hypothesis(next_state, hypothesis_id)
    if hypothesis is None:
        raise SystemExit(f"Unknown hypothesis: {hypothesis_id}")
    run_id = next_run_id(next_state)
    next_state["stage"] = STAGE_OUTER_LOOP
    next_state["active_hypothesis"] = hypothesis_id
    hypothesis["status"] = "active"
    if hypothesis_id in next_state["hypothesis_backlog"]:
        next_state["hypothesis_backlog"].remove(hypothesis_id)
    record = {
        "run_id": run_id,
        "hypothesis_id": hypothesis_id,
        "outcome": outcome,
        "summary": summary,
        "metric_name": metric_name,
        "metric_value": metric_value,
        "command": command,
        "evidence_path": evidence_path or default_run_record_path(hypothesis_id, run_id),
        "recorded_at": now_iso(),
    }
    next_state["run_history"].append(record)
    if evidence_path:
        next_state["evidence_index"].append(
            {
                "run_id": run_id,
                "path": evidence_path,
                "added_at": now_iso(),
            }
        )
    return next_state


def reflect(
    state: dict[str, Any],
    *,
    hypothesis_id: str,
    direction: str,
    reason: str,
    next_step: str | None,
    activate_hypothesis: str | None,
) -> dict[str, Any]:
    next_state = ensure_state_defaults(state)
    hypothesis = find_hypothesis(next_state, hypothesis_id)
    if hypothesis is None:
        raise SystemExit(f"Unknown hypothesis: {hypothesis_id}")
    latest_run = latest_run_for_hypothesis(next_state, hypothesis_id)
    decision = {
        "hypothesis_id": hypothesis_id,
        "run_id": latest_run.get("run_id") if latest_run else None,
        "direction": direction,
        "reason": reason,
        "next_step": next_step,
        "note_path": default_reflection_path(
            hypothesis_id,
            latest_run.get("run_id") if latest_run else None,
        ),
        "recorded_at": now_iso(),
    }
    next_state["decisions"].append(decision)
    next_state["current_direction"] = direction
    if direction == "CONCLUDE":
        next_state["status"] = "concluded"
        next_state["stage"] = STAGE_FINALIZE
        hypothesis["status"] = "concluded"
    else:
        next_state["stage"] = STAGE_INNER_LOOP
        hypothesis["status"] = "active"
        if direction == "PIVOT":
            hypothesis["status"] = "parked"
    if activate_hypothesis:
        target = find_hypothesis(next_state, activate_hypothesis)
        if target is None:
            raise SystemExit(f"Unknown activate_hypothesis: {activate_hypothesis}")
        next_state["active_hypothesis"] = activate_hypothesis
        target["status"] = "active"
        if activate_hypothesis in next_state["hypothesis_backlog"]:
            next_state["hypothesis_backlog"].remove(activate_hypothesis)
    return next_state


def add_claim_comparison(
    state: dict[str, Any],
    *,
    claim: str,
    axis: str,
    closest_prior_work: str,
    overlap: str,
    difference: str,
    confidence: str,
    verdict: str,
    claim_id: str | None = None,
) -> dict[str, Any]:
    next_state = ensure_state_defaults(state)
    novelty_gate = next_state["novelty_gate"]
    resolved_id = claim_id or f"C{len(novelty_gate.get('claim_records', [])) + 1}"
    record = {
        "claim_id": resolved_id,
        "claim": claim,
        "axis": axis,
        "closest_prior_work": closest_prior_work,
        "overlap": overlap,
        "difference": difference,
        "confidence": confidence,
        "verdict": verdict,
        "recorded_at": now_iso(),
    }
    claim_records = novelty_gate.setdefault("claim_records", [])
    existing_index = next(
        (idx for idx, item in enumerate(claim_records) if item.get("claim_id") == resolved_id),
        None,
    )
    if existing_index is None:
        claim_records.append(record)
    else:
        claim_records[existing_index] = record
    prioritized_records = prioritize_claims(claim_records)
    novelty_gate["claim_records"] = prioritized_records
    novelty_gate["claims"] = [item.get("claim") for item in prioritized_records]
    novelty_gate["overlap_summary"] = ", ".join(
        f"{item['claim_id']}={item['overlap']}" for item in prioritized_records
    )
    return next_state


def draft_claims_from_state(
    state: dict[str, Any],
    *,
    question_override: str | None = None,
    count: int = 4,
) -> dict[str, Any]:
    next_state = ensure_state_defaults(state)
    question = question_override or next_state.get("question", "")
    drafts = prioritize_claims(propose_claims_from_question(question, count=count))
    next_state["novelty_gate"]["draft_claims"] = drafts
    next_state["novelty_gate"]["claims"] = [draft["claim"] for draft in drafts]
    return next_state


def format_status(state: dict[str, Any]) -> str:
    hypotheses = state.get("hypotheses", [])
    runs = state.get("run_history", [])
    blockers = state.get("blockers", [])
    active = state.get("active_hypothesis") or "-"
    lines = [
        f"project: {state.get('project', '-')}",
        f"stage: {state.get('stage', '-')}",
        f"status: {state.get('status', '-')}",
        f"mode: {state.get('mode', '-')}",
        f"active_hypothesis: {active}",
        f"novelty_gate: {state.get('novelty_gate', {}).get('status', '-')}",
        f"hypotheses: {len(hypotheses)}",
        f"runs: {len(runs)}",
        f"blockers: {len(blockers)}",
        "next_actions:",
    ]
    for action in state.get("next_actions", [])[:4]:
        lines.append(f"- {action}")
    return "\n".join(lines)


def format_resume(state: dict[str, Any]) -> str:
    active_id = state.get("active_hypothesis")
    freshness = state_freshness(state)
    recent_runs = freshness["recent_runs"]
    recent_decisions = freshness["recent_decisions"]
    latest_run = recent_runs[0] if recent_runs else None
    latest_decision = recent_decisions[0] if recent_decisions else None
    brief = current_brief(state)
    lines = [
        f"question: {state.get('question', '-')}",
        f"stage: {state.get('stage', '-')}",
        f"novelty_gate: {state.get('novelty_gate', {}).get('status', '-')}",
        f"novelty_assessment: {overall_novelty_assessment(state)}",
        f"freshness: {'stale' if freshness['stale'] else 'fresh'}",
        f"history_bias_risk: {'high' if freshness['history_bias_risk'] else 'low'}",
        f"recommended_focus: {current_recommended_focus(state) or '-'}",
        f"novelty_brief_claim: {brief.get('claim_id') if brief else '-'}",
        f"active_hypothesis: {active_id or '-'}",
    ]
    if active_id:
        hypothesis = find_hypothesis(state, active_id)
        if hypothesis is not None:
            lines.append(f"active_claim: {hypothesis.get('claim', '-')}")
    if latest_run is not None:
        lines.append(f"latest_run: {latest_run['run_id']} ({latest_run['outcome']})")
        lines.append(f"latest_summary: {latest_run.get('summary', '-')}")
    if latest_decision is not None:
        lines.append(f"latest_direction: {latest_decision.get('direction', '-')}")
        lines.append(f"latest_reason: {latest_decision.get('reason', '-')}")
    draft_claims = state.get("novelty_gate", {}).get("draft_claims", [])
    lines.append(f"draft_claims: {len(draft_claims)}")
    search_plan = current_search_plan(state)
    lines.append(f"search_plan_entries: {len(search_plan)}")
    lines.append("guardrail: trust CURRENT_CONTEXT.md and research-state.yaml first; treat older logs as background.")
    lines.append("next_actions:")
    for action in state.get("next_actions", [])[:3]:
        lines.append(f"- {action}")
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Control plane for autoresearch workspaces.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    init_parser = subparsers.add_parser("init", help="Initialize a new autoresearch workspace.")
    init_parser.add_argument("--project", required=True)
    init_parser.add_argument("--question", required=True)
    init_parser.add_argument("--dir", default=".")
    init_parser.add_argument("--mode", choices=["quick", "full"], default="quick")

    status_parser = subparsers.add_parser("status", help="Show summarized research status.")
    status_parser.add_argument("--workspace", required=True)

    next_parser = subparsers.add_parser("next", help="Suggest the next high-value actions.")
    next_parser.add_argument("--workspace", required=True)

    resume_parser = subparsers.add_parser("resume", help="Print a compact handoff summary.")
    resume_parser.add_argument("--workspace", required=True)

    sync_parser = subparsers.add_parser("sync", help="Materialize workspace files from current state.")
    sync_parser.add_argument("--workspace", required=True)

    draft_parser = subparsers.add_parser("draft-claims", help="Generate 3 to 5 novelty claims from the research question.")
    draft_parser.add_argument("--workspace", required=True)
    draft_parser.add_argument("--question")
    draft_parser.add_argument("--count", type=int, default=4)

    plan_parser = subparsers.add_parser("plan-search", help="Refresh the novelty search view from structured claims.")
    plan_parser.add_argument("--workspace", required=True)

    brief_parser = subparsers.add_parser("brief-first-claim", help="Refresh the top-priority novelty brief inside CURRENT_CONTEXT.md.")
    brief_parser.add_argument("--workspace", required=True)

    compare_parser = subparsers.add_parser("compare-claim", help="Record one novelty claim comparison.")
    compare_parser.add_argument("--workspace", required=True)
    compare_parser.add_argument("--claim", required=True)
    compare_parser.add_argument("--axis", required=True)
    compare_parser.add_argument("--closest-prior-work", required=True)
    compare_parser.add_argument("--overlap", choices=sorted(NOVELTY_OVERLAPS), required=True)
    compare_parser.add_argument("--difference", required=True)
    compare_parser.add_argument("--confidence", choices=sorted(NOVELTY_CONFIDENCE), required=True)
    compare_parser.add_argument("--verdict", choices=sorted(NOVELTY_VERDICTS), required=True)
    compare_parser.add_argument("--claim-id")

    hypothesis_parser = subparsers.add_parser("add-hypothesis", help="Append a hypothesis to the queue.")
    hypothesis_parser.add_argument("--workspace", required=True)
    hypothesis_parser.add_argument("--claim", required=True)
    hypothesis_parser.add_argument("--prediction")
    hypothesis_parser.add_argument("--priority", choices=["high", "medium", "low"], default="medium")
    hypothesis_parser.add_argument("--id")

    run_parser = subparsers.add_parser("record-run", help="Record one experiment run.")
    run_parser.add_argument("--workspace", required=True)
    run_parser.add_argument("--hypothesis-id", required=True)
    run_parser.add_argument("--outcome", choices=sorted(VALID_OUTCOMES), required=True)
    run_parser.add_argument("--summary", required=True)
    run_parser.add_argument("--metric-name")
    run_parser.add_argument("--metric-value")
    run_parser.add_argument("--command", dest="entry_command")
    run_parser.add_argument("--evidence-path")

    reflect_parser = subparsers.add_parser("reflect", help="Record the next research direction.")
    reflect_parser.add_argument("--workspace", required=True)
    reflect_parser.add_argument("--hypothesis-id", required=True)
    reflect_parser.add_argument("--direction", choices=sorted(VALID_DIRECTIONS), required=True)
    reflect_parser.add_argument("--reason", required=True)
    reflect_parser.add_argument("--next-step")
    reflect_parser.add_argument("--activate-hypothesis")

    gate_parser = subparsers.add_parser("set-novelty-gate", help="Update novelty gate status.")
    gate_parser.add_argument("--workspace", required=True)
    gate_parser.add_argument("--status", choices=["pending", "passed", "pivot"], required=True)
    gate_parser.add_argument("--decision")
    gate_parser.add_argument("--overlap-summary")
    gate_parser.add_argument("--differentiation-strategy")
    gate_parser.add_argument("--claim", action="append", default=[])

    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.command == "init":
        root = init_workspace(
            project=args.project,
            question=args.question,
            base_dir=args.dir,
            mode=args.mode,
        )
        print(f"Initialized autoresearch workspace at {root}")
        return

    workspace, state_path = ensure_workspace(args.workspace)
    state = load_state(state_path)

    if args.command == "status":
        print(format_status(state))
        return

    if args.command == "next":
        for action in recommend_next_actions(state):
            print(f"- {action}")
        return

    if args.command == "resume":
        print(format_resume(state))
        return

    if args.command == "sync":
        sync_workspace_files(workspace, state)
        print(f"Synchronized workspace files for {workspace}")
        return

    if args.command == "draft-claims":
        updated = draft_claims_from_state(
            state,
            question_override=args.question,
            count=args.count,
        )
        dump_state(state_path, updated)
        sync_workspace_files(workspace, updated)
        append_research_log(
            workspace,
            "Draft claims generated",
            [
                f"claims: {len(updated['novelty_gate'].get('draft_claims', []))}",
                f"question: {args.question or updated.get('question', '-')}",
            ],
        )
        print(f"Generated draft claims for {workspace}")
        return

    if args.command == "plan-search":
        updated = ensure_state_defaults(state)
        dump_state(state_path, updated)
        sync_workspace_files(workspace, updated)
        append_research_log(
            workspace,
            "Novelty search view refreshed",
            [
                f"entries: {len(current_search_plan(updated))}",
                "source priority: Semantic Scholar -> arXiv -> Google Scholar",
            ],
        )
        print(f"Refreshed novelty search plan for {workspace}")
        return

    if args.command == "brief-first-claim":
        updated = ensure_state_defaults(state)
        dump_state(state_path, updated)
        sync_workspace_files(workspace, updated)
        brief = current_brief(updated)
        append_research_log(
            workspace,
            "Novelty brief refreshed",
            [
                f"claim: {brief.get('claim_id') if brief else '_not set_'}",
                "scope: top-priority novelty claim",
            ],
        )
        print(f"Refreshed novelty brief for {workspace}")
        return

    if args.command == "compare-claim":
        updated = add_claim_comparison(
            state,
            claim=args.claim,
            axis=args.axis,
            closest_prior_work=args.closest_prior_work,
            overlap=args.overlap,
            difference=args.difference,
            confidence=args.confidence,
            verdict=args.verdict,
            claim_id=args.claim_id,
        )
        dump_state(state_path, updated)
        sync_workspace_files(workspace, updated)
        append_research_log(
            workspace,
            f"Novelty claim compared ({args.claim_id or 'auto'})",
            [
                f"claim: {args.claim}",
                f"overlap: {args.overlap}",
                f"verdict: {args.verdict}",
            ],
        )
        print(f"Recorded novelty claim comparison for {workspace}")
        return

    if args.command == "add-hypothesis":
        updated = add_hypothesis(
            state,
            claim=args.claim,
            prediction=args.prediction,
            priority=args.priority,
            hypothesis_id=args.id,
        )
        dump_state(state_path, updated)
        sync_workspace_files(workspace, updated)
        hypothesis = find_hypothesis(updated, args.id or slugify(args.claim)[:40])
        if hypothesis is not None:
            append_research_log(
                workspace,
                f"Hypothesis added ({hypothesis['id']})",
                [
                    f"claim: {hypothesis['claim']}",
                    f"priority: {hypothesis['priority']}",
                ],
            )
        print(f"Added hypothesis in {workspace}")
        return

    if args.command == "record-run":
        updated = record_run(
            state,
            hypothesis_id=args.hypothesis_id,
            outcome=args.outcome,
            summary=args.summary,
            metric_name=args.metric_name,
            metric_value=args.metric_value,
            command=args.entry_command,
            evidence_path=args.evidence_path,
        )
        dump_state(state_path, updated)
        sync_workspace_files(workspace, updated)
        record = latest_run_for_hypothesis(updated, args.hypothesis_id)
        if record is not None:
            append_research_log(
                workspace,
                f"Run recorded ({record['run_id']})",
                [
                    f"hypothesis: {record['hypothesis_id']}",
                    f"outcome: {record['outcome']}",
                    f"summary: {record['summary']}",
                ],
            )
        print(f"Recorded run for {args.hypothesis_id}")
        return

    if args.command == "reflect":
        updated = reflect(
            state,
            hypothesis_id=args.hypothesis_id,
            direction=args.direction,
            reason=args.reason,
            next_step=args.next_step,
            activate_hypothesis=args.activate_hypothesis,
        )
        dump_state(state_path, updated)
        sync_workspace_files(workspace, updated)
        decision = latest_decision_for_hypothesis(updated, args.hypothesis_id)
        if decision is not None:
            append_research_log(
                workspace,
                f"Reflection recorded ({decision.get('run_id') or 'no-run'})",
                [
                    f"hypothesis: {decision['hypothesis_id']}",
                    f"direction: {decision['direction']}",
                    f"reason: {decision['reason']}",
                ],
            )
        print(f"Recorded reflection for {args.hypothesis_id}")
        return

    if args.command == "set-novelty-gate":
        state["novelty_gate"]["status"] = args.status
        if args.decision:
            state["novelty_gate"]["decision"] = args.decision
        if args.overlap_summary:
            state["novelty_gate"]["overlap_summary"] = args.overlap_summary
        if args.differentiation_strategy:
            state["novelty_gate"]["differentiation_strategy"] = args.differentiation_strategy
        if args.claim:
            state["novelty_gate"]["claims"] = args.claim
        dump_state(state_path, state)
        sync_workspace_files(workspace, state)
        append_research_log(
            workspace,
            "Novelty gate updated",
            [
                f"status: {state['novelty_gate']['status']}",
                f"decision: {state['novelty_gate'].get('decision') or '_not set_'}",
            ],
        )
        print(f"Updated novelty gate for {workspace}")
        return


if __name__ == "__main__":
    main()
