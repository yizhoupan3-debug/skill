from __future__ import annotations

import asyncio
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.middleware import MiddlewareContext, SkillInjectionMiddleware
from codex_agno_runtime.prompt_builder import PromptBuilder
from codex_agno_runtime.router import SkillRouter
from codex_agno_runtime.schemas import RoutingResult, SkillMetadata
from codex_agno_runtime.skill_loader import SkillLoader


def _routing_result(*, route_engine: str = "rust") -> RoutingResult:
    return RoutingResult(
        task="直接做代码",
        session_id="session-1",
        selected_skill=SkillMetadata(
            name="plan-to-code",
            description="Implement a concrete plan or spec into integrated code",
            routing_layer="L2",
        ),
        overlay_skill=SkillMetadata(name="rust-pro", description="Rust owner overlay"),
        layer="L2",
        reasons=["Trigger phrase matched: 直接做代码."],
        route_engine=route_engine,
    )


def test_skill_loader_uses_compiled_indices_for_progressive_mode(tmp_path: Path) -> None:
    skills_root = tmp_path / "skills"
    skill_dir = skills_root / "plan-to-code"
    skill_dir.mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(
        """---
name: plan-to-code
description: Full skill document
trigger_hints:
  - 直接做代码
---
# plan-to-code

## When to use

Use when implementation should happen immediately.

## Do not use

Do not use for pure planning.

## Core workflow

Implement the spec directly.
""",
        encoding="utf-8",
    )
    (skills_root / "SKILL_ROUTING_RUNTIME.json").write_text(
        """
{
  "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "health"],
  "skills": [
    ["plan-to-code", "L2", "owner", "none", "preferred", "Lean compiled summary", ["直接做代码", "按文档开发"], 95.8]
  ]
}
""".strip(),
        encoding="utf-8",
    )
    (skills_root / "SKILL_MANIFEST.json").write_text(
        """
{
  "keys": ["slug", "layer", "owner", "gate", "priority", "description", "session_start", "trigger_hints", "health", "source", "source_position"],
  "skills": [
    ["plan-to-code", "L2", "owner", "none", "P1", "Manifest description", "preferred", ["直接做代码"], 95.8, "project", 3]
  ]
}
""".strip(),
        encoding="utf-8",
    )

    loader = SkillLoader(skills_root)
    [skill] = loader.load(refresh=True, load_bodies=False)

    assert skill.name == "plan-to-code"
    assert skill.description == "Lean compiled summary"
    assert skill.body_loaded is False
    assert skill.source_path is not None
    assert skill.source_path.endswith("plan-to-code/SKILL.md")

    loader.load_body(skill)

    assert skill.body_loaded is True
    assert "## Core workflow" in skill.body
    assert skill.when_to_use == "Use when implementation should happen immediately."
    assert skill.do_not_use == "Do not use for pure planning."


def test_prompt_builder_passthroughs_explicit_rust_owned_prompt_preview() -> None:
    class _Loader:
        calls = 0

        def load_body(self, skill: SkillMetadata) -> None:
            self.calls += 1
            raise AssertionError("prompt preview should bypass Python body loading")

    loader = _Loader()
    builder = PromptBuilder(loader=loader)

    prompt = builder.build_prompt(_routing_result(), prompt_preview="Rust-owned preview")

    assert prompt == "Rust-owned preview"
    assert loader.calls == 0


def test_prompt_builder_marks_python_as_compatibility_projection() -> None:
    skill = SkillMetadata(
        name="plan-to-code",
        description="Implement a concrete plan or spec into integrated code",
        routing_layer="L2",
        body="""
## Core workflow

Implement the task directly.
""".strip(),
        body_loaded=True,
    )
    overlay = SkillMetadata(
        name="rust-pro",
        description="Rust owner overlay",
        routing_layer="L4",
        body="",
        body_loaded=True,
    )
    routing_result = RoutingResult(
        task="直接做代码",
        session_id="session-2",
        selected_skill=skill,
        overlay_skill=overlay,
        layer="L2",
        reasons=["Trigger phrase matched: 直接做代码."],
        route_engine="rust",
    )

    prompt = PromptBuilder().build_prompt(routing_result)

    assert "Help with the user's request directly." in prompt
    assert "Primary focus: plan-to-code" in prompt
    assert "How to reply:" in prompt
    assert "Key rules:" in prompt
    assert "Lead with the answer or result." in prompt


def test_prompt_builder_uses_skill_body_without_extra_idea_to_plan_contract() -> None:
    skill = SkillMetadata(
        name="idea-to-plan",
        description="Turn ambiguous ideas into evidence-backed plans",
        routing_layer="L-1",
        body="""
## Output Contract

- outline.md
- decision_log.md
- code_list.md
""".strip(),
        body_loaded=True,
    )
    overlay = SkillMetadata(
        name="anti-laziness",
        description="Anti laziness overlay",
        routing_layer="L1",
        body="",
        body_loaded=True,
    )
    routing_result = RoutingResult(
        task="先探索代码库现状，列出 critical files，再给我方案。",
        session_id="session-plan",
        selected_skill=skill,
        overlay_skill=overlay,
        layer="L-1",
        reasons=["Trigger hint matched: 先探索代码库再出方案."],
        route_engine="rust",
    )

    prompt = PromptBuilder().build_prompt(routing_result)

    assert "outline.md" in prompt
    assert "decision_log.md" in prompt
    assert "code_list.md" in prompt
    assert "Planning contract:" not in prompt
    assert "READ-ONLY planning route" not in prompt
    assert "<proposed_plan>" not in prompt


def test_skill_injection_middleware_prefers_route_preview() -> None:
    class _PromptBuilder:
        def __init__(self) -> None:
            self.calls = 0

        def build_prompt(self, routing_result: RoutingResult, *, prompt_preview: str | None = None) -> str:
            self.calls += 1
            return "python-generated prompt"

    prompt_builder = _PromptBuilder()
    middleware = SkillInjectionMiddleware(prompt_builder)
    ctx = MiddlewareContext(
        task="直接做代码",
        session_id="session-3",
        user_id="user-3",
        routing_result=_routing_result(),
    )
    ctx.metadata["dry_run"] = True
    ctx.metadata["routing_prompt_preview"] = "Rust preview"

    updated = asyncio.run(middleware.before_agent(ctx))

    assert updated.prompt == "Rust preview"
    assert updated.metadata["python_prompt_source"] == "routing-metadata-preview"
    assert prompt_builder.calls == 0


def test_skill_router_reports_thin_projection_under_rust_control_plane() -> None:
    skills = [
        SkillMetadata(
            name="plan-to-code",
            description="Implement a concrete plan or spec into integrated code",
            routing_layer="L2",
            routing_owner="owner",
            routing_gate="none",
            routing_priority="P1",
            session_start="preferred",
            trigger_hints=["直接做代码"],
        )
    ]
    router = SkillRouter(
        skills,
        control_plane_descriptor={
            "authority": "rust-runtime-control-plane",
            "services": {
                "router": {
                    "authority": "rust-route-core",
                    "role": "route-selection",
                    "projection": "python-thin-projection",
                    "delegate_kind": "rust-route-adapter",
                    "python_projection_materialization": "compatibility-subprocess",
                }
            },
        },
    )

    result = router.route("直接做代码", session_id="session-4")

    assert result.route_engine == "python"
    assert any("thin compatibility projection" in reason for reason in result.reasons)
    assert router.projection_descriptor()["compatibility_only"] is True
    assert router.projection_descriptor()["python_projection_materialization"] == (
        "compatibility-subprocess"
    )


def test_skill_router_overlay_skill_cannot_be_selected_as_primary_owner() -> None:
    router = SkillRouter(
        [
            SkillMetadata(
                name="repo-review-owner",
                description="Repository-wide review owner for audit and code review requests.",
                routing_layer="L2",
                routing_owner="owner",
                routing_gate="none",
                routing_priority="P1",
                trigger_hints=["全面 review", "review 这个仓库", "仓库 review"],
            ),
            SkillMetadata(
                name="code-review",
                description="Structured code review overlay for audit and review requests.",
                routing_layer="L1",
                routing_owner="overlay",
                routing_gate="none",
                routing_priority="P1",
                trigger_hints=["code review", "code-review"],
            ),
        ]
    )

    result = router.route("全面 review 这个仓库，做 code review", session_id="session-overlay")

    assert result.selected_skill.name == "repo-review-owner"
    assert result.overlay_skill is not None
    assert result.overlay_skill.name == "code-review"


def test_skill_router_wording_cleanup_query_does_not_hit_artifact_gate() -> None:
    router = SkillRouter(
        [
            SkillMetadata(
                name="doc",
                description="Word document artifact owner for docx-oriented tasks.",
                routing_layer="L3",
                routing_owner="owner",
                routing_gate="artifact",
                routing_priority="P1",
                trigger_hints=["word 文档", "docx"],
            ),
            SkillMetadata(
                name="writing-skills",
                description="Repository-wide wording cleanup and SKILL.md standardization owner.",
                routing_layer="L2",
                routing_owner="owner",
                routing_gate="none",
                routing_priority="P1",
                trigger_hints=["wording cleanup", "措辞", "模板"],
            ),
        ]
    )

    result = router.route(
        "请统一多个 SKILL.md 的措辞和模板，做仓库级 wording cleanup",
        session_id="session-wording",
    )

    assert result.selected_skill.name == "writing-skills"
