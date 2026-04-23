from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.memory_support import (
    RuntimeSnapshot,
    classify_runtime_continuity,
    format_repo_relative_path,
    describe_continuity_layout,
    describe_project_local_memory_layout,
    load_runtime_snapshot,
    normalize_evidence_index,
    normalize_next_actions,
    normalize_supervisor_state,
    normalize_trace_skills,
    repair_runtime_continuity_artifacts,
)


def _snapshot(
    tmp_path: Path,
    *,
    session_summary: str = "",
    next_actions: dict[str, object] | None = None,
    evidence_index: dict[str, object] | None = None,
    trace_metadata: dict[str, object] | None = None,
    supervisor_state: dict[str, object] | None = None,
) -> RuntimeSnapshot:
    artifact_base = tmp_path / "artifacts"
    current_root = artifact_base / "current"
    current_root.mkdir(parents=True, exist_ok=True)
    return RuntimeSnapshot(
        session_summary_text=session_summary,
        next_actions=next_actions or {},
        evidence_index=evidence_index or {},
        trace_metadata=trace_metadata or {},
        supervisor_state=normalize_supervisor_state(supervisor_state or {}),
        artifact_base=artifact_base,
        current_root=current_root,
        mirror_root=current_root,
        task_root=current_root,
        active_task_id="demo-task",
        snapshots=[],
        collected_at="2026-04-18T21:00:00+08:00",
    )


def test_normalize_next_actions_accepts_next_actions_and_actions() -> None:
    assert normalize_next_actions(
        {"schema_version": "next-actions-v2", "next_actions": ["a", "b"]}
    ) == ["a", "b"]
    assert normalize_next_actions({"actions": ["legacy"]}) == ["legacy"]
    assert normalize_next_actions(
        {
            "next_actions": [
                {
                    "title": "ship the real follow-up",
                    "status": "pending",
                }
            ]
        }
    ) == ["ship the real follow-up"]


def test_normalize_evidence_index_accepts_artifacts_and_evidence() -> None:
    assert normalize_evidence_index(
        {"schema_version": "evidence-index-v2", "artifacts": [{"kind": "report"}]}
    ) == [{"kind": "report"}]
    assert normalize_evidence_index({"evidence": [{"kind": "legacy"}]}) == [{"kind": "legacy"}]


def test_normalize_trace_skills_accepts_matched_skills_and_skills() -> None:
    assert normalize_trace_skills(
        {"schema_version": "trace-metadata-v2", "matched_skills": ["alpha", "beta"]}
    ) == ["alpha", "beta"]
    assert normalize_trace_skills({"skills": ["legacy"]}) == ["legacy"]


def test_normalize_supervisor_state_upgrades_flat_legacy_fields() -> None:
    normalized = normalize_supervisor_state(
        {
            "task_summary": "demo",
            "delegation_plan_created": True,
            "spawn_attempted": False,
            "fallback_mode": "local-supervisor",
            "delegated_sidecars": ["reader"],
            "verification_status": "passed",
            "open_blockers": ["none"],
            "resume_allowed": False,
        }
    )

    assert normalized["schema_version"] == "supervisor-state-v2"
    assert normalized["delegation"]["delegation_plan_created"] is True
    assert normalized["verification"]["verification_status"] == "passed"
    assert normalized["blockers"]["open_blockers"] == ["none"]
    assert normalized["continuity"]["resume_allowed"] is False


def test_memory_and_continuity_layout_descriptors_are_explicit(tmp_path: Path) -> None:
    memory_dir = tmp_path / ".codex" / "memory"
    memory_dir.parent.mkdir(parents=True, exist_ok=True)
    memory_dir.symlink_to(Path("../memory"))
    (tmp_path / "memory").mkdir()

    memory_layout = describe_project_local_memory_layout(tmp_path)
    continuity = describe_continuity_layout(tmp_path)

    assert memory_layout["logical_root"].endswith("/.codex/memory")
    assert memory_layout["physical_root"].endswith("/memory")
    assert memory_layout["is_symlink"] is True
    assert "shared framework memory root" in memory_layout["mapping_note"]
    assert continuity["root_task_mirror"]["supervisor_state"].endswith(
        "/.supervisor_state.json"
    )
    assert continuity["root_task_mirror"]["session_summary"].endswith(
        "/SESSION_SUMMARY.md"
    )
    assert continuity["bridge_mirror"]["session_summary"].endswith(
        "/artifacts/current/SESSION_SUMMARY.md"
    )
    assert continuity["task_scoped_current"]["template"].endswith("/artifacts/current/<task_id>")
    assert "task-scoped continuity" in continuity["sync_responsibility"]


def test_format_repo_relative_path_handles_alias_roots_and_fallback(tmp_path: Path) -> None:
    real_root = tmp_path / "repo"
    real_root.mkdir()
    alias_root = tmp_path / "repo-alias"
    alias_root.symlink_to(real_root, target_is_directory=True)
    target = real_root / "memory" / "anchor.txt"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("anchor", encoding="utf-8")

    assert format_repo_relative_path(target, alias_root) == "memory/anchor.txt"

    unrelated_root = tmp_path / "unrelated"
    unrelated_root.mkdir()
    assert format_repo_relative_path(target, unrelated_root) == str(target)


def test_load_runtime_snapshot_without_repair_preserves_raw_supervisor_state(tmp_path: Path) -> None:
    supervisor_state_path = tmp_path / ".supervisor_state.json"
    supervisor_state_path.write_text(
        (
            '{"task_id":"demo-task-20260418220000","task_summary":"demo task",'
            '"active_phase":"completed","verification":{"verification_status":"completed"},'
            '"continuity":{"story_state":"completed","resume_allowed":true}}\n'
        ),
        encoding="utf-8",
    )
    before = supervisor_state_path.read_text(encoding="utf-8")

    snapshot = load_runtime_snapshot(tmp_path, repair=False)

    after = supervisor_state_path.read_text(encoding="utf-8")

    assert before == after
    assert snapshot.supervisor_state["continuity"]["resume_allowed"] is True


def test_classify_runtime_continuity_active_snapshot_stays_resumable(tmp_path: Path) -> None:
    snapshot = _snapshot(
        tmp_path,
        session_summary="- task: Active routing repair\n- phase: implementation\n- status: in_progress\n",
        next_actions={"next_actions": ["Patch classifier", "Run pytest"]},
        trace_metadata={"matched_skills": ["execution-controller-coding", "skill-framework-developer"]},
        supervisor_state={
            "task_summary": "Active routing repair",
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
            "execution_contract": {
                "scope": ["scripts/memory_support.py"],
                "acceptance_criteria": ["No stale injection"],
            },
        },
    )

    continuity = classify_runtime_continuity(snapshot)

    assert continuity["state"] == "active"
    assert continuity["can_resume"] is True
    assert continuity["current_execution"]["task"] == "Active routing repair"
    assert continuity["recent_completed_execution"] is None


def test_classify_runtime_continuity_completed_snapshot_is_not_current_execution(tmp_path: Path) -> None:
    snapshot = _snapshot(
        tmp_path,
        session_summary="- task: checklist-series final closeout\n- phase: finalized\n- status: completed\n",
        next_actions={"next_actions": ["Start a new standalone task next time"]},
        trace_metadata={"task": "checklist-series final closeout", "matched_skills": ["checklist-fixer"]},
        supervisor_state={
            "task_summary": "checklist-series final closeout",
            "active_phase": "finalized",
            "verification": {"verification_status": "completed"},
            "continuity": {"story_state": "completed", "resume_allowed": False},
        },
    )

    continuity = classify_runtime_continuity(snapshot)

    assert continuity["state"] == "completed"
    assert continuity["can_resume"] is False
    assert continuity["current_execution"] is None
    assert continuity["recent_completed_execution"]["task"] == "checklist-series final closeout"
    assert continuity["terminal_reasons"]


def test_classify_runtime_continuity_stale_snapshot_is_hard_blocked(tmp_path: Path) -> None:
    snapshot = _snapshot(
        tmp_path,
        next_actions={"next_actions": ["Do not trust stale continuity"]},
        supervisor_state={
            "task_summary": "stale bootstrap lane",
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {
                "story_state": "active",
                "resume_allowed": False,
                "state_reason": "superseded by a newer supervisor-owned task",
            },
        },
    )

    continuity = classify_runtime_continuity(snapshot)

    assert continuity["state"] == "stale"
    assert continuity["can_resume"] is False
    assert continuity["current_execution"] is None
    assert any("disallows resume" in reason for reason in continuity["stale_reasons"])


def test_classify_runtime_continuity_detects_inconsistent_task_identity(tmp_path: Path) -> None:
    snapshot = _snapshot(
        tmp_path,
        session_summary="- task: bootstrap repair A\n- phase: implementation\n- status: in_progress\n",
        trace_metadata={"task": "bootstrap repair B", "matched_skills": ["skill-framework-developer"]},
        supervisor_state={
            "task_summary": "bootstrap repair A",
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
        },
    )

    continuity = classify_runtime_continuity(snapshot)

    assert continuity["state"] == "inconsistent"
    assert continuity["can_resume"] is False
    assert continuity["current_execution"] is None
    assert any("disagrees with trace task" in reason for reason in continuity["inconsistency_reasons"])


def test_classify_runtime_continuity_prefers_supervisor_actions_and_route_when_sidecars_are_stale(
    tmp_path: Path,
) -> None:
    snapshot = _snapshot(
        tmp_path,
        session_summary="- task: bootstrap repair\n- phase: implementation\n- status: in_progress\n",
        next_actions={"next_actions": ["stale task action"]},
        trace_metadata={
            "task": "bootstrap repair",
            "matched_skills": ["legacy-skill"],
            "verification_status": "completed",
            "routing_runtime_version": 0,
        },
        supervisor_state={
            "task_summary": "bootstrap repair",
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
            "next_actions": ["repair current continuity"],
            "controller": {
                "primary_owner": "execution-controller-coding",
                "gate": "subagent-delegation",
            },
        },
    )

    continuity = classify_runtime_continuity(snapshot)

    assert continuity["state"] == "active"
    assert continuity["next_actions"] == ["repair current continuity"]
    assert continuity["route"] == ["subagent-delegation", "execution-controller-coding"]


def test_load_runtime_snapshot_can_skip_contract_snapshot_scan(tmp_path: Path) -> None:
    contracts_root = tmp_path / "artifacts" / "contracts"
    contracts_root.mkdir(parents=True, exist_ok=True)
    (contracts_root / "one.json").write_text("{}\n", encoding="utf-8")
    (tmp_path / ".supervisor_state.json").write_text("{}\n", encoding="utf-8")

    snapshot = load_runtime_snapshot(tmp_path, include_contract_snapshots=False)

    assert snapshot.snapshots == []



def test_load_runtime_snapshot_prefers_task_scoped_current_root(tmp_path: Path) -> None:
    task_id = "codex-first-convergence-20260418210000"
    task_root = tmp_path / "artifacts" / "current" / task_id
    mirror_root = tmp_path / "artifacts" / "current"
    (task_root / "SESSION_SUMMARY.md").parent.mkdir(parents=True, exist_ok=True)
    (task_root / "SESSION_SUMMARY.md").write_text("- task: task scoped\n", encoding="utf-8")
    (task_root / "NEXT_ACTIONS.json").write_text('{"next_actions":["task root"]}\n', encoding="utf-8")
    (task_root / "EVIDENCE_INDEX.json").write_text('{"artifacts":[]}\n', encoding="utf-8")
    (task_root / "TRACE_METADATA.json").write_text('{"matched_skills":["skill-framework-developer"]}\n', encoding="utf-8")
    (mirror_root / "SESSION_SUMMARY.md").write_text("- task: mirror only\n", encoding="utf-8")
    (tmp_path / ".supervisor_state.json").write_text(
        '{"task_id":"codex-first-convergence-20260418210000","task_summary":"task scoped"}\n',
        encoding="utf-8",
    )

    snapshot = load_runtime_snapshot(tmp_path)

    assert snapshot.active_task_id == task_id
    assert snapshot.current_root == task_root
    assert snapshot.task_root == task_root
    assert snapshot.session_summary_text == "- task: task scoped\n"


def test_load_runtime_snapshot_uses_active_task_pointer_when_supervisor_task_id_is_missing(
    tmp_path: Path,
) -> None:
    task_id = "pointer-task-20260418210000"
    task_root = tmp_path / "artifacts" / "current" / task_id
    task_root.mkdir(parents=True, exist_ok=True)
    (task_root / "SESSION_SUMMARY.md").write_text("- task: pointer task\n", encoding="utf-8")
    (task_root / "NEXT_ACTIONS.json").write_text('{"next_actions":["task root"]}\n', encoding="utf-8")
    (task_root / "EVIDENCE_INDEX.json").write_text('{"artifacts":[]}\n', encoding="utf-8")
    (task_root / "TRACE_METADATA.json").write_text('{"matched_skills":["skill-framework-developer"]}\n', encoding="utf-8")
    (tmp_path / "artifacts" / "current" / "active_task.json").write_text(
        '{"task_id":"pointer-task-20260418210000","task":"pointer task"}\n',
        encoding="utf-8",
    )
    (tmp_path / ".supervisor_state.json").write_text(
        '{"task_summary":"pointer task","active_phase":"implementation"}\n',
        encoding="utf-8",
    )

    snapshot = load_runtime_snapshot(tmp_path, repair=False, include_contract_snapshots=False)

    assert snapshot.active_task_id == task_id
    assert snapshot.current_root == task_root
    assert snapshot.session_summary_text == "- task: pointer task\n"


def test_load_runtime_snapshot_uses_repaired_supervisor_state_after_continuity_fix(
    tmp_path: Path,
) -> None:
    (tmp_path / ".supervisor_state.json").write_text(
        (
            '{"task_id":"demo-task-20260418220000","task_summary":"demo task",'
            '"active_phase":"completed","verification":{"verification_status":"completed"},'
            '"continuity":{"story_state":"completed","resume_allowed":true}}\n'
        ),
        encoding="utf-8",
    )

    snapshot = load_runtime_snapshot(tmp_path)
    continuity = classify_runtime_continuity(snapshot)

    assert snapshot.supervisor_state["continuity"]["resume_allowed"] is False
    assert continuity["state"] == "completed"
    assert continuity["inconsistency_reasons"] == []



def test_load_runtime_snapshot_repairs_mixed_supervisor_and_mirror_truth(tmp_path: Path) -> None:
    old_task_id = "old-task-20260418210000"
    old_task_root = tmp_path / "artifacts" / "current" / old_task_id
    mirror_root = tmp_path / "artifacts" / "current"
    old_task_root.mkdir(parents=True, exist_ok=True)
    (old_task_root / "SESSION_SUMMARY.md").write_text("- task: old task\n", encoding="utf-8")
    (old_task_root / "NEXT_ACTIONS.json").write_text('{"next_actions":["legacy"]}\n', encoding="utf-8")
    (old_task_root / "EVIDENCE_INDEX.json").write_text('{"artifacts":[]}\n', encoding="utf-8")
    (old_task_root / "TRACE_METADATA.json").write_text('{"task":"old task","matched_skills":["legacy-skill"]}\n', encoding="utf-8")
    (mirror_root / "SESSION_SUMMARY.md").write_text("- task: old task\n", encoding="utf-8")
    (mirror_root / "NEXT_ACTIONS.json").write_text('{"next_actions":["legacy"]}\n', encoding="utf-8")
    (mirror_root / "EVIDENCE_INDEX.json").write_text('{"artifacts":[]}\n', encoding="utf-8")
    (mirror_root / "TRACE_METADATA.json").write_text('{"task":"old task","matched_skills":["legacy-skill"]}\n', encoding="utf-8")
    (mirror_root / "active_task.json").write_text(
        '{"task_id":"old-task-20260418210000","task":"old task"}\n',
        encoding="utf-8",
    )
    (tmp_path / ".supervisor_state.json").write_text(
        (
            '{"task_id":"new-task-20260418220000","task_summary":"new task",'
            '"active_phase":"completed","verification":{"verification_status":"completed"},'
            '"continuity":{"story_state":"completed","resume_allowed":false},'
            '"controller":{"primary_owner":"execution-controller-coding","gate":"systematic-debugging"}}\n'
        ),
        encoding="utf-8",
    )

    snapshot = load_runtime_snapshot(tmp_path)

    assert snapshot.active_task_id == "new-task-20260418220000"
    assert snapshot.current_root == tmp_path / "artifacts" / "current" / "new-task-20260418220000"
    assert "new task" in snapshot.session_summary_text
    assert "old task" not in snapshot.session_summary_text
    assert (
        tmp_path / "artifacts" / "current" / "active_task.json"
    ).read_text(encoding="utf-8").find("new-task-20260418220000") != -1
    assert "new task" in (tmp_path / "SESSION_SUMMARY.md").read_text(encoding="utf-8")
    assert "new task" in (tmp_path / "TRACE_METADATA.json").read_text(encoding="utf-8")



def test_repair_runtime_continuity_artifacts_uses_current_routing_runtime_version(
    tmp_path: Path,
) -> None:
    (tmp_path / ".supervisor_state.json").write_text(
        (
            '{"task_id":"route-audit-20260419","task_summary":"route audit",'
            '"active_phase":"completed","verification":{"verification_status":"completed"},'
            '"continuity":{"story_state":"completed","resume_allowed":false},'
            '"controller":{"primary_owner":"skill-framework-developer","gate":"subagent-delegation"}}\n'
        ),
        encoding="utf-8",
    )

    repair_runtime_continuity_artifacts(tmp_path)

    payload = json.loads((tmp_path / "TRACE_METADATA.json").read_text(encoding="utf-8"))
    runtime = json.loads(
        (PROJECT_ROOT / "skills" / "SKILL_ROUTING_RUNTIME.json").read_text(encoding="utf-8")
    )
    assert payload["routing_runtime_version"] == runtime["version"]


def test_repair_runtime_continuity_artifacts_backfills_missing_scoped_json_from_supervisor(
    tmp_path: Path,
) -> None:
    task_id = "memory-repair-20260419"
    task_root = tmp_path / "artifacts" / "current" / task_id
    task_root.mkdir(parents=True, exist_ok=True)
    (task_root / "SESSION_SUMMARY.md").write_text(
        (
            "# SESSION_SUMMARY\n\n"
            "- task: memory repair\n"
            "- phase: implementation\n"
            "- status: in_progress\n\n"
            "## Summary\n"
            "Keep the existing summary while missing JSON artifacts are repaired.\n"
        ),
        encoding="utf-8",
    )
    (tmp_path / ".supervisor_state.json").write_text(
        (
            '{"task_id":"memory-repair-20260419","task_summary":"memory repair",'
            '"active_phase":"implementation","verification":{"verification_status":"in_progress"},'
            '"continuity":{"story_state":"active","resume_allowed":true},'
            '"next_actions":["restore next actions","restore trace"],'
            '"controller":{"primary_owner":"agent-memory","gate":"subagent-delegation"}}\n'
        ),
        encoding="utf-8",
    )

    repair_runtime_continuity_artifacts(tmp_path)

    task_next_actions = json.loads((task_root / "NEXT_ACTIONS.json").read_text(encoding="utf-8"))
    task_trace = json.loads((task_root / "TRACE_METADATA.json").read_text(encoding="utf-8"))
    root_next_actions = json.loads((tmp_path / "NEXT_ACTIONS.json").read_text(encoding="utf-8"))
    mirror_trace = json.loads(
        (tmp_path / "artifacts" / "current" / "TRACE_METADATA.json").read_text(encoding="utf-8")
    )

    assert task_next_actions["next_actions"] == ["restore next actions", "restore trace"]
    assert root_next_actions == task_next_actions
    assert task_trace["matched_skills"] == ["subagent-delegation", "agent-memory"]
    assert task_trace["decision"]["owner"] == "agent-memory"
    assert mirror_trace["matched_skills"] == ["subagent-delegation", "agent-memory"]
    assert "existing summary" in (tmp_path / "SESSION_SUMMARY.md").read_text(encoding="utf-8").lower()


def test_repair_runtime_continuity_artifacts_rewrites_stale_task_json_surfaces(
    tmp_path: Path,
) -> None:
    task_id = "continuity-repair-20260422"
    task_root = tmp_path / "artifacts" / "current" / task_id
    task_root.mkdir(parents=True, exist_ok=True)
    (task_root / "SESSION_SUMMARY.md").write_text(
        (
            "# SESSION_SUMMARY\n\n"
            "- task: continuity repair\n"
            "- phase: implementation\n"
            "- status: in_progress\n\n"
            "## Summary\n"
            "Keep this summary, but do not trust the stale JSON sidecars.\n"
        ),
        encoding="utf-8",
    )
    (task_root / "NEXT_ACTIONS.json").write_text(
        json.dumps({"next_actions": ["old action"]}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    (task_root / "TRACE_METADATA.json").write_text(
        json.dumps(
            {
                "task": "legacy other task",
                "matched_skills": ["legacy-skill"],
                "verification_status": "completed",
                "routing_runtime_version": 0,
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    (tmp_path / ".supervisor_state.json").write_text(
        (
            '{"task_id":"continuity-repair-20260422","task_summary":"continuity repair",'
            '"active_phase":"implementation","verification":{"verification_status":"in_progress"},'
            '"continuity":{"story_state":"active","resume_allowed":true},'
            '"next_actions":["new action","verify carry-over"],'
            '"controller":{"primary_owner":"execution-controller-coding","gate":"subagent-delegation"}}\n'
        ),
        encoding="utf-8",
    )

    repair_runtime_continuity_artifacts(tmp_path)

    task_next_actions = json.loads((task_root / "NEXT_ACTIONS.json").read_text(encoding="utf-8"))
    task_trace = json.loads((task_root / "TRACE_METADATA.json").read_text(encoding="utf-8"))
    root_trace = json.loads((tmp_path / "TRACE_METADATA.json").read_text(encoding="utf-8"))

    assert task_next_actions["next_actions"] == ["new action", "verify carry-over"]
    assert task_trace["task"] == "continuity repair"
    assert task_trace["verification_status"] == "in_progress"
    assert task_trace["matched_skills"] == ["subagent-delegation", "execution-controller-coding"]
    assert task_trace["routing_runtime_version"] > 0
    assert root_trace["task"] == "continuity repair"
    assert root_trace["matched_skills"] == ["subagent-delegation", "execution-controller-coding"]
