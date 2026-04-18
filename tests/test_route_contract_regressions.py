from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.router import SkillRouter
from codex_agno_runtime.schemas import SkillMetadata
from scripts.route import run_rust_route_json


ROUTE_FIXTURE_PATH = PROJECT_ROOT / "tests" / "routing_route_fixtures.json"
MISSING_RUNTIME_PATH = PROJECT_ROOT / "tests" / "_routing_missing_runtime.json"
ROUTE_FIXTURES = json.loads(ROUTE_FIXTURE_PATH.read_text(encoding="utf-8"))
REGRESSION_CASE_NAMES = {
    "overlay-code-review-does-not-own",
    "overlay-tdd-does-not-own",
    "wording-cleanup-does-not-hit-doc-gate",
    "explicit-word-doc-still-hits-artifact-gate",
}
REGRESSION_CASES = [
    case for case in ROUTE_FIXTURES["cases"] if case["name"] in REGRESSION_CASE_NAMES
]


def _normalize_trigger_hints(value: object) -> list[str]:
    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    text = str(value).strip()
    return [text] if text else []


def _load_fixture_skills() -> list[SkillMetadata]:
    index = {str(key): idx for idx, key in enumerate(ROUTE_FIXTURES["keys"])}
    skills: list[SkillMetadata] = []
    for row in ROUTE_FIXTURES["skills"]:
        skills.append(
            SkillMetadata(
                name=str(row[index["slug"]]),
                description=str(row[index["description"]]),
                routing_layer=str(row[index["layer"]]),
                routing_owner=str(row[index["owner"]]),
                routing_gate=str(row[index["gate"]]),
                routing_priority=str(row[index["priority"]]),
                session_start=str(row[index["session_start"]]),
                trigger_hints=_normalize_trigger_hints(row[index["trigger_hints"]]),
                health=float(row[index["health"]]),
            )
        )
    return skills


def _python_route_case(case: dict[str, object]) -> tuple[str, str | None, str]:
    router = SkillRouter(_load_fixture_skills())
    result = router.route(
        str(case["query"]),
        session_id="regression-fixture-session",
        allow_overlay=bool(case.get("allow_overlay", True)),
        first_turn=bool(case.get("first_turn", True)),
    )
    return (
        result.selected_skill.name,
        result.overlay_skill.name if result.overlay_skill else None,
        result.layer,
    )


@pytest.mark.parametrize(
    "case",
    REGRESSION_CASES,
    ids=[case["name"] for case in REGRESSION_CASES],
)
def test_routing_contract_regressions(case: dict[str, object]) -> None:
    expected = case["expected"]
    python_selected, python_overlay, python_layer = _python_route_case(case)
    rust_decision = run_rust_route_json(
        str(case["query"]),
        session_id="regression-fixture-session",
        allow_overlay=bool(case.get("allow_overlay", True)),
        first_turn=bool(case.get("first_turn", True)),
        runtime_path=MISSING_RUNTIME_PATH,
        manifest_path=ROUTE_FIXTURE_PATH,
    )

    assert (python_selected, python_overlay, python_layer) == (
        expected["selected_skill"],
        expected["overlay_skill"],
        expected["layer"],
    )
    assert rust_decision["selected_skill"] == expected["selected_skill"]
    assert rust_decision["overlay_skill"] == expected["overlay_skill"]
    assert rust_decision["layer"] == expected["layer"]
    assert rust_decision["selected_skill"] == python_selected
    assert rust_decision["overlay_skill"] == python_overlay
    assert rust_decision["layer"] == python_layer
