from __future__ import annotations

import importlib.util
from pathlib import Path
from datetime import datetime, timedelta, timezone


PROJECT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = PROJECT_ROOT / "skills" / "autoresearch" / "scripts" / "research_ctl.py"


def _load_module():
    spec = importlib.util.spec_from_file_location("autoresearch_research_ctl", MODULE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("Unable to load research_ctl module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_init_workspace_creates_richer_autoresearch_scaffold(tmp_path: Path) -> None:
    module = _load_module()

    root = module.init_workspace(
        project="demo-project",
        question="Can a tighter loop improve research throughput?",
        base_dir=tmp_path,
        mode="full",
    )

    state = module.load_state(root / "research-state.yaml")

    assert state["schema_version"] == 2
    assert state["stage"] == "bootstrap"
    assert state["mode"] == "full"
    assert state["novelty_gate"]["status"] == "pending"
    assert len(state["next_actions"]) >= 2
    assert (root / "BOOTSTRAP_BRIEF.md").is_file()
    assert (root / "literature" / "NOVELTY_GATE.md").is_file()
    assert (root / "experiments" / "_templates" / "PROTOCOL_TEMPLATE.md").is_file()
    assert (root / "experiments" / "_templates" / "RUN_RECORD_TEMPLATE.md").is_file()
    assert (root / "experiments" / "_templates" / "REFLECTION_TEMPLATE.md").is_file()


def test_autoresearch_progression_moves_from_protocol_to_reflection_to_deepen() -> None:
    module = _load_module()

    state = module.default_state(
        project="demo-project",
        question="Does a structured loop improve research quality?",
        mode="quick",
    )
    state["novelty_gate"]["status"] = "passed"
    state["novelty_gate"]["claims"] = ["claim-a", "claim-b", "claim-c"]

    state = module.add_hypothesis(
        state,
        claim="Use tighter reflection prompts to reduce wasted experiments",
        prediction="The next-step recommendations become more specific",
        priority="high",
        hypothesis_id="tight-reflection",
    )
    pre_run_actions = module.recommend_next_actions(state)
    assert any("写 protocol" in action for action in pre_run_actions)

    state = module.record_run(
        state,
        hypothesis_id="tight-reflection",
        outcome="exploratory",
        summary="Recommendations became more concrete after one bounded run.",
        metric_name="specificity_score",
        metric_value="0.72",
        command="python run.py --quick",
        evidence_path="experiments/tight-reflection/run-001.md",
    )
    post_run_actions = module.recommend_next_actions(state)
    assert any("reflection" in action for action in post_run_actions)

    state = module.reflect(
        state,
        hypothesis_id="tight-reflection",
        direction="DEEPEN",
        reason="The signal is promising but still narrow.",
        next_step="Freeze the prompt format and retest on a second setting.",
        activate_hypothesis=None,
    )
    deepen_actions = module.recommend_next_actions(state)
    assert any("收紧变量" in action for action in deepen_actions)


def test_reflect_conclude_marks_workspace_as_concluded() -> None:
    module = _load_module()

    state = module.default_state(
        project="demo-project",
        question="Can this line conclude cleanly?",
        mode="quick",
    )
    state["novelty_gate"]["status"] = "passed"
    state = module.add_hypothesis(
        state,
        claim="A minimal research controller improves usability",
        prediction="Users need fewer manual state edits",
        priority="high",
        hypothesis_id="minimal-controller",
    )
    state = module.record_run(
        state,
        hypothesis_id="minimal-controller",
        outcome="confirmatory",
        summary="The workflow stayed coherent through one full loop.",
        metric_name=None,
        metric_value=None,
        command=None,
        evidence_path=None,
    )

    state = module.reflect(
        state,
        hypothesis_id="minimal-controller",
        direction="CONCLUDE",
        reason="The minimal closure target is met.",
        next_step="Freeze the handoff package.",
        activate_hypothesis=None,
    )

    assert state["status"] == "concluded"
    assert state["stage"] == "finalize"
    assert state["current_direction"] == "CONCLUDE"


def test_sync_workspace_materializes_hypothesis_run_and_reflection_files(tmp_path: Path) -> None:
    module = _load_module()
    root = module.init_workspace(
        project="materialized-project",
        question="Can workspace files stay in sync automatically?",
        base_dir=tmp_path,
        mode="quick",
    )
    state_path = root / "research-state.yaml"

    state = module.load_state(state_path)
    state["novelty_gate"]["status"] = "passed"
    state = module.add_hypothesis(
        state,
        claim="Automatic file materialization reduces research bookkeeping",
        prediction="Fewer manual file edits are needed",
        priority="high",
        hypothesis_id="auto-materialize",
    )
    state = module.record_run(
        state,
        hypothesis_id="auto-materialize",
        outcome="exploratory",
        summary="The run record was generated without manual file setup.",
        metric_name="manual_steps_saved",
        metric_value="3",
        command="python run.py --quick",
        evidence_path=None,
    )
    state = module.reflect(
        state,
        hypothesis_id="auto-materialize",
        direction="DEEPEN",
        reason="The bookkeeping savings are real but only lightly tested.",
        next_step="Retest on a second hypothesis branch.",
        activate_hypothesis=None,
    )
    module.dump_state(state_path, state)
    module.sync_workspace_files(root, state)

    hypothesis_card = root / "experiments" / "auto-materialize" / "HYPOTHESIS_CARD.md"
    protocol = root / "experiments" / "auto-materialize" / "protocol.md"
    analysis = root / "experiments" / "auto-materialize" / "analysis.md"
    run_record = root / "experiments" / "auto-materialize" / "run-001.md"
    reflection = root / "experiments" / "auto-materialize" / "run-001-reflection.md"

    assert hypothesis_card.is_file()
    assert protocol.is_file()
    assert analysis.is_file()
    assert run_record.is_file()
    assert reflection.is_file()
    assert "auto-materialize" in hypothesis_card.read_text(encoding="utf-8")
    assert "manual_steps_saved" in run_record.read_text(encoding="utf-8")
    assert "DEEPEN" in reflection.read_text(encoding="utf-8")
    novelty_gate = (root / "literature" / "NOVELTY_GATE.md").read_text(encoding="utf-8")
    findings = (root / "findings.md").read_text(encoding="utf-8")
    current_context = (root / "CURRENT_CONTEXT.md").read_text(encoding="utf-8")
    assert "<!-- autoresearch:novelty:start -->" in novelty_gate
    assert "<!-- autoresearch:findings:start -->" in findings
    assert "<!-- autoresearch:context:start -->" in current_context
    assert "source of truth" in current_context


def test_format_resume_returns_compact_handoff_summary() -> None:
    module = _load_module()
    state = module.default_state(
        project="resume-project",
        question="What should the next session know immediately?",
        mode="quick",
    )
    state["novelty_gate"]["status"] = "passed"
    state = module.add_hypothesis(
        state,
        claim="Resume output should compress the critical context",
        prediction="A later session can continue without rereading everything",
        priority="high",
        hypothesis_id="resume-compact",
    )
    state = module.record_run(
        state,
        hypothesis_id="resume-compact",
        outcome="confirmatory",
        summary="The compact summary captured the active branch and latest result.",
        metric_name=None,
        metric_value=None,
        command=None,
        evidence_path=None,
    )
    state = module.reflect(
        state,
        hypothesis_id="resume-compact",
        direction="BROADEN",
        reason="The pattern looks real enough to test on a second setting.",
        next_step="Retest on a second benchmark.",
        activate_hypothesis=None,
    )

    resume_text = module.format_resume(state)

    assert "question: What should the next session know immediately?" in resume_text
    assert "freshness: fresh" in resume_text
    assert "history_bias_risk: low" in resume_text
    assert "recommended_focus: -" in resume_text
    assert "novelty_brief_claim: -" in resume_text
    assert "active_hypothesis: resume-compact" in resume_text
    assert "latest_run: run-001 (confirmatory)" in resume_text
    assert "latest_direction: BROADEN" in resume_text
    assert "novelty_assessment: insufficient" in resume_text
    assert "next_actions:" in resume_text


def test_compare_claim_updates_state_and_novelty_summary(tmp_path: Path) -> None:
    module = _load_module()
    root = module.init_workspace(
        project="novelty-project",
        question="Is the claim matrix kept in sync?",
        base_dir=tmp_path,
        mode="quick",
    )
    state_path = root / "research-state.yaml"
    state = module.load_state(state_path)
    state = module.add_claim_comparison(
        state,
        claim="Use a dedicated research controller to reduce orchestration drift",
        axis="workflow",
        closest_prior_work="The AI Scientist",
        overlap="medium",
        difference="This version focuses on local recoverability and file-backed control.",
        confidence="high",
        verdict="defensible",
        claim_id="C1",
    )
    state["novelty_gate"]["status"] = "passed"
    state["novelty_gate"]["decision"] = "Proceed with careful positioning"
    module.dump_state(state_path, state)
    module.sync_workspace_files(root, state)

    reloaded = module.load_state(state_path)
    assert reloaded["novelty_gate"]["claim_records"][0]["claim_id"] == "C1"
    assert reloaded["novelty_gate"]["overlap_summary"] == "C1=medium"
    assert module.overall_novelty_assessment(reloaded) == "moderate"

    novelty_text = (root / "literature" / "NOVELTY_GATE.md").read_text(encoding="utf-8")
    assert "Claim Comparison Matrix" in novelty_text
    assert "The AI Scientist" in novelty_text
    assert "🟡 medium" in novelty_text


def test_plan_search_refresh_creates_query_ladder_and_search_plan_file(tmp_path: Path) -> None:
    module = _load_module()
    root = module.init_workspace(
        project="search-plan-project",
        question="Can claims become a usable search checklist automatically?",
        base_dir=tmp_path,
        mode="quick",
    )
    state_path = root / "research-state.yaml"
    state = module.load_state(state_path)
    state = module.add_claim_comparison(
        state,
        claim="Use a file-backed research controller to reduce orchestration drift",
        axis="workflow",
        closest_prior_work="Reflexion",
        overlap="medium",
        difference="This version centers resumable local artifacts instead of reflection memory only.",
        confidence="medium",
        verdict="defensible",
        claim_id="C1",
    )
    module.dump_state(state_path, state)
    module.sync_workspace_files(root, state)

    reloaded = module.load_state(state_path)
    search_plan = module.current_search_plan(reloaded)
    assert len(search_plan) == 1
    assert search_plan[0]["claim_id"] == "C1"
    assert any(query["label"] == "focused" for query in search_plan[0]["queries"])
    assert "Semantic Scholar" in search_plan[0]["sources"]
    assert module.current_brief(reloaded)["claim_id"] == "C1"

    plan_text = (root / "literature" / "NOVELTY_SEARCH_PLAN.md").read_text(encoding="utf-8")
    context_text = (root / "CURRENT_CONTEXT.md").read_text(encoding="utf-8")
    assert "<!-- autoresearch:search-plan:start -->" in plan_text
    assert "Query Ladder" in plan_text
    assert "Semantic Scholar -> arXiv -> Google Scholar" in plan_text
    assert "orchestration drift" in plan_text
    assert "Active Novelty Brief" in context_text
    assert "Decision Goal" in context_text or "decision goal" in context_text
    assert "expected baselines" in context_text.lower()


def test_plain_claims_do_not_become_active_focus_or_brief(tmp_path: Path) -> None:
    module = _load_module()
    root = module.init_workspace(
        project="plain-claims-project",
        question="Can plain stored claims mislead the next session?",
        base_dir=tmp_path,
        mode="quick",
    )
    state_path = root / "research-state.yaml"
    state = module.load_state(state_path)
    state["novelty_gate"]["status"] = "passed"
    state["novelty_gate"]["claims"] = [
        "Old unstructured claim that should not become the active focus automatically."
    ]
    module.dump_state(state_path, state)
    module.sync_workspace_files(root, state)

    reloaded = module.load_state(state_path)
    resume_text = module.format_resume(reloaded)
    context_text = (root / "CURRENT_CONTEXT.md").read_text(encoding="utf-8")

    assert module.current_recommended_focus(reloaded) is None
    assert module.current_search_plan(reloaded) == []
    assert module.current_brief(reloaded) is None
    assert "recommended_focus: -" in resume_text
    assert "novelty_brief_claim: -" in resume_text
    assert "recommended focus: -" in context_text


def test_sync_workspace_removes_legacy_novelty_brief_file(tmp_path: Path) -> None:
    module = _load_module()
    root = module.init_workspace(
        project="legacy-brief-project",
        question="Can sync delete stale novelty brief files?",
        base_dir=tmp_path,
        mode="quick",
    )
    legacy_brief = root / "literature" / "NOVELTY_BRIEF.md"
    legacy_brief.parent.mkdir(parents=True, exist_ok=True)
    legacy_brief.write_text("# Legacy brief\n", encoding="utf-8")

    state = module.load_state(root / "research-state.yaml")
    module.sync_workspace_files(root, state)

    assert not legacy_brief.exists()


def test_draft_claims_from_question_generates_claims_and_search_plan(tmp_path: Path) -> None:
    module = _load_module()
    root = module.init_workspace(
        project="draft-claims-project",
        question="Can a file-backed research controller reduce orchestration drift in deep research workflows?",
        base_dir=tmp_path,
        mode="quick",
    )
    state_path = root / "research-state.yaml"
    state = module.load_state(state_path)
    state = module.draft_claims_from_state(state, count=4)
    module.dump_state(state_path, state)
    module.sync_workspace_files(root, state)

    reloaded = module.load_state(state_path)
    draft_claims = reloaded["novelty_gate"]["draft_claims"]
    search_plan = module.current_search_plan(reloaded)
    brief = module.current_brief(reloaded)

    assert len(draft_claims) == 4
    assert draft_claims[0]["recommended_order"] == 1
    assert draft_claims[0]["priority_label"] in {"first", "next", "later"}
    assert draft_claims[0]["priority_score"] >= draft_claims[-1]["priority_score"]
    assert module.current_recommended_focus(reloaded).startswith(draft_claims[0]["claim_id"])
    assert len(search_plan) == 4
    assert search_plan[0]["recommended_order"] == 1
    assert search_plan[0]["claim_id"] == draft_claims[0]["claim_id"]
    assert any(query["label"] == "broad" for query in search_plan[0]["queries"])
    assert brief["claim_id"] == draft_claims[0]["claim_id"]
    assert len(brief["expected_baselines"]) >= 2

    claims_text = (root / "literature" / "NOVELTY_CLAIMS.md").read_text(encoding="utf-8")
    plan_text = (root / "literature" / "NOVELTY_SEARCH_PLAN.md").read_text(encoding="utf-8")
    context_text = (root / "CURRENT_CONTEXT.md").read_text(encoding="utf-8")
    assert "<!-- autoresearch:claims:start -->" in claims_text
    assert "Managed Claim Extraction" in claims_text
    assert "recommended first claim" in claims_text
    assert "why first or later" in claims_text
    assert "file-backed research controller" in claims_text
    assert "recommended first search target" in plan_text
    assert "priority:" in plan_text
    assert "deep research workflows" in plan_text
    assert "verification standard" in context_text.lower()
    assert "expected baselines" in context_text.lower()
    assert draft_claims[0]["claim"] in context_text


def test_stale_state_bias_guard_prefers_reconcile_before_old_history(tmp_path: Path) -> None:
    module = _load_module()
    root = module.init_workspace(
        project="stale-project",
        question="Can old logs bias the next action?",
        base_dir=tmp_path,
        mode="quick",
    )
    state_path = root / "research-state.yaml"
    state = module.load_state(state_path)
    stale_time = (datetime.now(timezone.utc) - timedelta(days=21)).replace(microsecond=0).isoformat()
    state["updated_at"] = stale_time
    state["run_history"] = [
        {
            "run_id": "run-001",
            "hypothesis_id": "old-branch",
            "outcome": "exploratory",
            "summary": "Old run that should not dominate the current decision.",
            "metric_name": None,
            "metric_value": None,
            "command": None,
            "evidence_path": "experiments/old-branch/run-001.md",
            "recorded_at": stale_time,
        }
    ]
    state["decisions"] = [
        {
            "hypothesis_id": "old-branch",
            "run_id": "run-001",
            "direction": "DEEPEN",
            "reason": "Old decision that should be background only.",
            "next_step": "Keep going.",
            "note_path": "experiments/old-branch/run-001-reflection.md",
            "recorded_at": stale_time,
        }
    ]
    module.dump_state(state_path, state)
    stale_state = module.load_state(state_path)
    stale_state["updated_at"] = stale_time
    module.sync_workspace_files(root, stale_state)

    actions = module.recommend_next_actions(stale_state)
    resume_text = module.format_resume(stale_state)
    context_text = (root / "CURRENT_CONTEXT.md").read_text(encoding="utf-8")

    assert "先刷新当前上下文" in actions[0]
    assert "freshness: stale" in resume_text
    assert "history_bias_risk: high" in resume_text
    assert "treat `research-log.md` and older notes as background only" in context_text
