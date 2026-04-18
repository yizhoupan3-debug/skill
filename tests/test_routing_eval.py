from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.evaluate_routing import evaluate_cases, load_cases


def test_routing_eval_baseline_is_actionable() -> None:
    payload = evaluate_cases(
        skills_root=PROJECT_ROOT / "skills",
        cases_payload=load_cases(PROJECT_ROOT / "tests" / "routing_eval_cases.json"),
    )

    assert payload["schema_version"] == "routing-eval-v1"
    assert payload["metrics"]["case_count"] >= 9
    assert payload["metrics"]["trigger_hit"] >= 7
    assert payload["metrics"]["owner_correct"] >= 7
    assert payload["metrics"]["overlay_correct"] >= 7
    assert payload["metrics"]["overtrigger"] == 0

    results_by_id = {row["id"]: row for row in payload["results"]}
    assert results_by_id["systematic-debugging-gate-conflict"]["selected_owner"] == "systematic-debugging"
    assert results_by_id["skill-developer-codex-near-miss"]["selected_owner"] == "skill-developer-codex"
    assert results_by_id["idea-to-plan-strategic-plan-case"]["selected_owner"] == "idea-to-plan"
    assert results_by_id["idea-to-plan-explore-plan-case"]["selected_owner"] == "idea-to-plan"
    assert results_by_id["checklist-writting-post-strategy-case"]["selected_owner"] == "checklist-writting"


def test_routing_eval_case_file_is_valid_json() -> None:
    payload = json.loads((PROJECT_ROOT / "tests" / "routing_eval_cases.json").read_text(encoding="utf-8"))

    assert payload["schema_version"] == "routing-eval-cases-v1"
    assert any(case["category"] == "should-trigger" for case in payload["cases"])
    assert any(case["category"] == "should-not-trigger" for case in payload["cases"])
    assert any(case["category"] == "wrong-owner-near-miss" for case in payload["cases"])
    assert any(case["category"] == "gate-vs-owner-conflict" for case in payload["cases"])
