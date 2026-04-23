from __future__ import annotations

import json
import sys
from pathlib import Path

import framework_runtime.rust_router as rust_router_module

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.rust_router import evaluate_routing_cases, load_routing_eval_cases
from framework_runtime.schemas import RoutingEvalReport


def test_routing_eval_baseline_is_actionable() -> None:
    payload = evaluate_routing_cases(
        skills_root=PROJECT_ROOT / "skills",
        cases_payload=load_routing_eval_cases(PROJECT_ROOT / "tests" / "routing_eval_cases.json"),
    )

    assert isinstance(payload, RoutingEvalReport)
    assert payload.schema_version == "routing-eval-v1"
    assert payload.metrics.case_count >= 11
    assert payload.metrics.trigger_hit >= 9
    assert payload.metrics.owner_correct >= 9
    assert payload.metrics.overlay_correct >= 9
    assert payload.metrics.overtrigger == 0

    results_by_id = {str(row.id): row for row in payload.results}
    assert results_by_id["systematic-debugging-gate-conflict"].selected_owner == "systematic-debugging"
    assert results_by_id["skill-framework-developer-near-miss"].selected_owner == "skill-framework-developer"
    assert results_by_id["skill-framework-developer-generic-review-case"].selected_owner == "skill-framework-developer"
    assert results_by_id["skill-framework-developer-generic-review-case"].selected_overlay == "code-review"
    assert results_by_id["openai-docs-source-gate-case"].selected_owner == "openai-docs"
    assert results_by_id["idea-to-plan-strategic-plan-case"].selected_owner == "idea-to-plan"
    assert results_by_id["idea-to-plan-explore-plan-case"].selected_owner == "idea-to-plan"
    assert results_by_id["checklist-writting-post-strategy-case"].selected_owner == "checklist-writting"
    assert results_by_id["frontend-design-visual-redesign-case"].selected_owner == "frontend-design"
    assert results_by_id["frontend-design-visual-redesign-case"].selected_overlay is None
    assert results_by_id["design-agent-brand-routing-case"].selected_owner == "design-agent"
    assert results_by_id["design-agent-brand-routing-case"].selected_overlay is None
    assert results_by_id["design-agent-mixed-source-case"].selected_owner == "design-agent"
    assert results_by_id["design-agent-mixed-source-case"].selected_overlay is None
    assert results_by_id["frontend-design-screenshot-review-reroute-case"].selected_owner == "visual-review"
    assert results_by_id["frontend-design-screenshot-review-reroute-case"].selected_overlay is None
    assert results_by_id["frontend-design-motion-boundary-case"].selected_owner == "motion-design"
    assert results_by_id["frontend-design-motion-boundary-case"].selected_overlay is None
    assert results_by_id["frontend-design-css-boundary-case"].selected_owner == "css-pro"
    assert results_by_id["frontend-design-css-boundary-case"].selected_overlay is None


def test_routing_eval_helper_delegates_to_rust_owned_contract(tmp_path: Path, monkeypatch) -> None:
    captured: dict[str, object] = {}
    cases = load_routing_eval_cases(PROJECT_ROOT / "tests" / "routing_eval_cases.json")
    skills_root = tmp_path / "skills"

    class _FakeAdapter:
        def routing_eval_contract(self, *, cases_path: Path) -> RoutingEvalReport:
            captured["cases_payload"] = json.loads(cases_path.read_text(encoding="utf-8"))
            return RoutingEvalReport.model_validate(
                {
                    "schema_version": "routing-eval-v1",
                    "metrics": {
                        "case_count": 1,
                        "trigger_hit": 1,
                        "trigger_miss": 0,
                        "overtrigger": 0,
                        "owner_correct": 1,
                        "overlay_correct": 1,
                    },
                    "results": [
                        {
                            "id": "typed-first",
                            "category": "should-trigger",
                            "task": "typed-first",
                            "focus_skill": "route",
                            "selected_owner": "route",
                            "selected_overlay": None,
                            "expected_owner": "route",
                            "expected_overlay": None,
                            "forbidden_owners": [],
                            "trigger_hit": True,
                            "overtrigger": False,
                            "owner_correct": True,
                            "overlay_correct": True,
                        }
                    ],
                }
            )

    monkeypatch.setattr(rust_router_module, "route_adapter", lambda **kwargs: _FakeAdapter())

    payload = evaluate_routing_cases(skills_root=skills_root, cases_payload=cases)

    assert isinstance(payload, RoutingEvalReport)
    assert payload.metrics.case_count == 1
    assert captured["cases_payload"] == cases.model_dump(mode="json")


def test_routing_eval_case_file_is_valid_json() -> None:
    payload = json.loads((PROJECT_ROOT / "tests" / "routing_eval_cases.json").read_text(encoding="utf-8"))

    assert payload["schema_version"] == "routing-eval-cases-v1"
    assert any(case["category"] == "should-trigger" for case in payload["cases"])
    assert any(case["category"] == "should-not-trigger" for case in payload["cases"])
    assert any(case["category"] == "wrong-owner-near-miss" for case in payload["cases"])
    assert any(case["category"] == "gate-vs-owner-conflict" for case in payload["cases"])
