from __future__ import annotations

import json
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"

if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.prompt_builder import PromptBuilder
from codex_agno_runtime.schemas import RoutingResult, SkillMetadata


FIXTURE_PATH = PROJECT_ROOT / "tests" / "prompt_style_eval_cases.json"
FIXTURES = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def _build_routing_result(case: dict[str, object]) -> RoutingResult:
    overlay_name = str(case["overlay_skill"])
    overlay = None
    if overlay_name and overlay_name != "none":
        overlay = SkillMetadata(
            name=overlay_name,
            description=str(case["overlay_description"]),
            routing_layer="L4",
            body="",
            body_loaded=True,
        )
    return RoutingResult(
        task=str(case["task"]),
        session_id=f"style-{case['id']}",
        selected_skill=SkillMetadata(
            name=str(case["selected_skill"]),
            description=str(case["selected_description"]),
            routing_layer=str(case["layer"]),
            body="""
## Core workflow

Keep the answer practical and user-facing.
""".strip(),
            body_loaded=True,
        ),
        overlay_skill=overlay,
        layer=str(case["layer"]),
        reasons=[str(reason) for reason in case["reasons"]],
        route_engine=str(case["route_engine"]),
    )


def test_prompt_style_fixture_schema_is_stable() -> None:
    assert FIXTURES["schema_version"] == "prompt-style-eval-cases-v1"
    assert FIXTURES["cases"]
    assert FIXTURES["style_contract"]["required_prompt_markers"]
    assert FIXTURES["style_contract"]["forbidden_prompt_markers"]


def test_shared_policy_freezes_plain_language_contract() -> None:
    text = (PROJECT_ROOT / "AGENT.md").read_text(encoding="utf-8")
    for marker in FIXTURES["style_contract"]["required_policy_markers"]:
        assert marker in text


def test_prompt_builder_realistic_style_cases_stay_plain() -> None:
    builder = PromptBuilder()
    required_markers = FIXTURES["style_contract"]["required_prompt_markers"]
    forbidden_markers = FIXTURES["style_contract"]["forbidden_prompt_markers"]

    for case in FIXTURES["cases"]:
        prompt = builder.build_prompt(_build_routing_result(case))
        for marker in required_markers:
            assert marker in prompt, f"missing required marker {marker!r} for {case['id']}"
        for marker in forbidden_markers:
            assert marker not in prompt, f"found forbidden marker {marker!r} for {case['id']}"
        for reason in case["reasons"]:
            assert str(reason) in prompt
