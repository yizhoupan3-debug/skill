#!/usr/bin/env python3
"""Run a minimal offline routing evaluation suite."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = ROOT / "codex_agno_runtime" / "src"
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.router import SkillRouter
from codex_agno_runtime.skill_loader import SkillLoader


def load_cases(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def evaluate_cases(
    *,
    skills_root: Path,
    cases_payload: dict[str, Any],
) -> dict[str, Any]:
    loader = SkillLoader(skills_root)
    skills = loader.load(refresh=True, load_bodies=False)
    router = SkillRouter(skills)

    metrics = {
        "case_count": 0,
        "trigger_hit": 0,
        "trigger_miss": 0,
        "overtrigger": 0,
        "owner_correct": 0,
        "overlay_correct": 0,
    }
    results: list[dict[str, Any]] = []

    for case in cases_payload.get("cases", []):
        if not isinstance(case, dict):
            continue
        task = str(case.get("task", "")).strip()
        if not task:
            continue
        metrics["case_count"] += 1
        result = router.route(
            task,
            session_id=f"routing-eval::{case.get('id', metrics['case_count'])}",
            allow_overlay=True,
            first_turn=bool(case.get("first_turn", True)),
        )
        selected_owner = result.selected_skill.name
        selected_overlay = result.overlay_skill.name if result.overlay_skill else None
        category = str(case.get("category", "")).strip()
        expected_owner = case.get("expected_owner")
        expected_overlay = case.get("expected_overlay")
        focus_skill = case.get("focus_skill")
        forbidden_owners = {str(item) for item in case.get("forbidden_owners", [])}

        trigger_hit = False
        overtrigger = False
        owner_correct = expected_owner == selected_owner if expected_owner else False
        overlay_correct = expected_overlay == selected_overlay if "expected_overlay" in case else False

        if category == "should-trigger":
            trigger_hit = focus_skill == selected_owner
            metrics["trigger_hit" if trigger_hit else "trigger_miss"] += 1
        elif category == "should-not-trigger":
            overtrigger = selected_owner in forbidden_owners
            if overtrigger:
                metrics["overtrigger"] += 1
        elif category in {"wrong-owner-near-miss", "gate-vs-owner-conflict"}:
            trigger_hit = focus_skill == selected_owner
            metrics["trigger_hit" if trigger_hit else "trigger_miss"] += 1
            if selected_owner in forbidden_owners:
                metrics["overtrigger"] += 1

        if owner_correct:
            metrics["owner_correct"] += 1
        if overlay_correct:
            metrics["overlay_correct"] += 1

        results.append(
            {
                "id": case.get("id"),
                "category": category,
                "task": task,
                "focus_skill": focus_skill,
                "selected_owner": selected_owner,
                "selected_overlay": selected_overlay,
                "expected_owner": expected_owner,
                "expected_overlay": expected_overlay,
                "forbidden_owners": sorted(forbidden_owners),
                "trigger_hit": trigger_hit,
                "overtrigger": overtrigger,
                "owner_correct": owner_correct,
                "overlay_correct": overlay_correct,
            }
        )

    return {
        "schema_version": "routing-eval-v1",
        "metrics": metrics,
        "results": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Run offline routing evaluation cases.")
    parser.add_argument(
        "--skills-root",
        type=Path,
        default=ROOT / "skills",
        help="Skill root path.",
    )
    parser.add_argument(
        "--cases",
        type=Path,
        default=ROOT / "tests" / "routing_eval_cases.json",
        help="Routing eval case file.",
    )
    args = parser.parse_args()

    payload = evaluate_cases(skills_root=args.skills_root, cases_payload=load_cases(args.cases))
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
