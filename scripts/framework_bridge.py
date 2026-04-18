#!/usr/bin/env python3
"""Framework-facing bridge for skill export, memory bootstrap, and evolution proposals."""

from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.consolidate_memory import (
    archive_legacy_memory_bundle,
    build_memory_documents,
    persist_memory_bundle,
    write_documents,
    write_memory_state,
)
from scripts.memory_support import (
    DEFAULT_MEMORY_ROOT,
    build_task_id,
    current_local_timestamp,
    classify_runtime_continuity,
    describe_continuity_layout,
    describe_project_local_memory_layout,
    get_repo_root,
    is_generic_query,
    load_runtime_snapshot,
    query_matches_task,
    resolve_effective_memory_dir,
    workspace_dir,
    write_json_if_changed,
    write_text_if_changed,
)
from scripts.retrieve_memory import render_context

RUNTIME_PATH = Path(
    os.environ.get(
        "FRAMEWORK_BRIDGE_RUNTIME_PATH",
        "skills/SKILL_ROUTING_RUNTIME.json",
    )
)
APPROVAL_PATH = Path(
    os.environ.get(
        "FRAMEWORK_BRIDGE_APPROVAL_PATH",
        "skills/SKILL_APPROVAL_POLICY.json",
    )
)
JOURNAL_PATH = Path(
    os.environ.get(
        "FRAMEWORK_BRIDGE_JOURNAL_PATH",
        "skills/.evolution_journal.jsonl",
    )
)
HEALTH_PATH = Path(
    os.environ.get(
        "FRAMEWORK_BRIDGE_HEALTH_PATH",
        "skills/SKILL_HEALTH_MANIFEST.json",
    )
)


@dataclass(slots=True)
class FrameworkSkill:
    slug: str
    layer: str
    owner: str
    gate: str
    session_start: str
    summary: str
    triggers: list[str]
    agent_role: str = ""
    approval: dict[str, Any] | None = None


@dataclass(slots=True)
class FrameworkProposal:
    skill: str
    issue_type: str
    evidence_count: int
    reroute_rate: float
    dominant_destinations: list[str]
    recommendation: str
    exemplar_tasks: list[str]


def _read_json(path: Path) -> dict[str, Any] | list[Any]:
    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
def export_framework_skills(
    runtime_path: Path | None = None,
    approval_path: Path | None = None,
) -> dict[str, Any]:
    """Export runtime and approval data for framework consumers."""

    runtime = _read_json((runtime_path or RUNTIME_PATH).resolve())
    approvals = _read_json((approval_path or APPROVAL_PATH).resolve())
    rows: list[dict[str, Any]] = []
    runtime_rows = runtime.get("skills", []) if isinstance(runtime, dict) else []
    if isinstance(runtime_rows, list):
        for item in runtime_rows:
            if isinstance(item, dict):
                slug = str(item.get("name") or item.get("slug") or "").strip()
                if not slug:
                    continue
                rows.append(
                    asdict(
                        FrameworkSkill(
                            slug=slug,
                            layer=str(item.get("routing_layer", "")),
                            owner=str(item.get("routing_owner", "")),
                            gate=str(item.get("routing_gate", "")),
                            session_start=str(item.get("session_start", "")),
                            summary=str(item.get("description", ""))[:200],
                            triggers=list(item.get("trigger_hints", item.get("trigger_phrases", [])))
                            if isinstance(item.get("trigger_hints", item.get("trigger_phrases")), list)
                            else [],
                            agent_role=str(item.get("routing_owner", "")),
                            approval=approvals.get(slug) if isinstance(approvals, dict) else None,
                        )
                    )
                )
                continue
            if isinstance(item, list) and item:
                slug = str(item[0]).strip()
                if not slug:
                    continue
                rows.append(
                    asdict(
                        FrameworkSkill(
                            slug=slug,
                            layer=str(item[1]) if len(item) > 1 else "",
                            owner=str(item[2]) if len(item) > 2 else "",
                            gate=str(item[3]) if len(item) > 3 else "",
                            session_start=str(item[4]) if len(item) > 4 else "",
                            summary=str(item[5]) if len(item) > 5 else "",
                            triggers=[str(value) for value in item[6]]
                            if len(item) > 6 and isinstance(item[6], list)
                            else ([str(item[6])] if len(item) > 6 and str(item[6]).strip() else []),
                            agent_role=str(item[2]) if len(item) > 2 else "",
                            approval=approvals.get(slug) if isinstance(approvals, dict) else None,
                        )
                    )
                )
    return {"skills": rows, "count": len(rows), "source": str(runtime_path or RUNTIME_PATH)}


def _load_health_map(health_path: Path | None = None) -> dict[str, dict[str, Any]]:
    payload = _read_json((health_path or HEALTH_PATH).resolve())
    if not isinstance(payload, dict):
        return {}
    return {slug: info for slug, info in payload.items() if isinstance(info, dict)}


def _runtime_skill_slugs(runtime_path: Path | None = None) -> set[str]:
    runtime = _read_json((runtime_path or RUNTIME_PATH).resolve())
    if not isinstance(runtime, dict):
        return set()
    rows = runtime.get("skills", [])
    if not isinstance(rows, list):
        return set()
    slugs: set[str] = set()
    for item in rows:
        if isinstance(item, dict):
            slug = str(item.get("name") or item.get("slug") or "").strip()
            if slug:
                slugs.add(slug)
        elif isinstance(item, list) and item:
            slug = str(item[0]).strip()
            if slug:
                slugs.add(slug)
    return slugs


def build_evolution_proposals(
    journal_path: Path | None = None,
    health_path: Path | None = None,
    *,
    limit: int = 10,
    min_events: int = 1,
) -> dict[str, Any]:
    """Build lightweight evolution proposals from the routing journal."""

    health = _load_health_map(health_path)
    valid_skills = _runtime_skill_slugs()
    entries: list[dict[str, Any]] = []
    path = (journal_path or JOURNAL_PATH).resolve()
    if path.is_file():
        for line in path.read_text(encoding="utf-8").splitlines():
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    by_init: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for entry in entries:
        by_init[str(entry.get("init", ""))].append(entry)
    proposals: list[dict[str, Any]] = []
    for skill, items in by_init.items():
        if not skill or len(items) < min_events:
            continue
        reroutes = [item for item in items if item.get("reroute")]
        struggles = [item for item in items if int(item.get("struggle", 0)) >= 3]
        destinations = Counter(str(item.get("final", "")) for item in reroutes if item.get("final"))
        issue_type = "routing-miss" if reroutes else "high-struggle" if struggles else ""
        if not issue_type:
            continue
        recommendation = (
            f"补强 `{skill}` 的 trigger_hints / routing_gate / owner"
            if issue_type == "routing-miss"
            else f"为 `{skill}` 增加更窄的调度说明或修复高 struggle 路径"
        )
        reroute_rate = len(reroutes) / max(1, len(items))
        proposals.append(
            asdict(
                FrameworkProposal(
                    skill=skill,
                    issue_type=issue_type,
                    evidence_count=len(items),
                    reroute_rate=round(reroute_rate, 2),
                    dominant_destinations=[dest for dest, _ in destinations.most_common(3)],
                    recommendation=recommendation,
                    exemplar_tasks=[str(item.get("task", "")) for item in items[:3]],
                )
            )
        )
    proposals.sort(key=lambda item: (item["evidence_count"], item["reroute_rate"]), reverse=True)
    return {
        "proposal_count": len(proposals[:limit]),
        "proposals": proposals[:limit],
        "valid_skills": sorted(valid_skills),
        "health": health,
    }


def build_framework_memory_bootstrap(
    *,
    workspace: str,
    query: str = "",
    source_root: Path | None = None,
    memory_root: Path | None = None,
    artifact_source_dir: Path | None = None,
    top: int = 8,
    mode: str = "stable",
) -> dict[str, Any]:
    """Build a memory bootstrap payload for framework consumers."""

    repo_root = (source_root or get_repo_root()).resolve()
    snapshot = load_runtime_snapshot(repo_root, artifact_root=artifact_source_dir)
    continuity = classify_runtime_continuity(snapshot)
    active_task = {
        "task_id": snapshot.active_task_id or snapshot.supervisor_state.get("task_id"),
        "task": continuity.get("task"),
        "phase": continuity.get("phase"),
        "status": continuity.get("status"),
    }
    query_matches_active_task = (
        continuity["state"] == "active"
        and continuity.get("task")
        and not is_generic_query(query)
        and query_matches_task(query, str(continuity.get("task")))
    )
    effective_continuity = continuity
    ignored_active_task: dict[str, Any] | None = None
    if continuity["state"] == "active" and continuity.get("task") and not query_matches_active_task:
        ignored_active_task = active_task
        effective_continuity = {
            "state": "query-mismatch",
            "can_resume": False,
            "task": None,
            "phase": None,
            "status": "isolated_bootstrap",
            "route": [],
            "next_actions": [],
            "blockers": [],
            "current_execution": None,
            "recent_completed_execution": None,
            "stale_reasons": [],
            "terminal_reasons": [],
            "inconsistency_reasons": [],
            "recovery_hints": [
                "Query does not match the active task; ignore live continuity and start from an isolated task scope.",
            ],
            "continuity": {
                "story_state": "query-mismatch",
                "resume_allowed": False,
                "last_updated_at": current_local_timestamp(),
                "active_lease_expires_at": None,
                "state_reason": "active task ignored because bootstrap query targets a different task",
            },
            "summary_fields": {},
            "paths": continuity.get("paths", {}),
            "ignored_active_task": ignored_active_task,
        }
    memory_workspace_root = resolve_effective_memory_dir(
        workspace=workspace,
        memory_root=memory_root,
        repo_root=repo_root,
    )
    memory_workspace_root.mkdir(parents=True, exist_ok=True)
    memory_exists = (memory_workspace_root / "MEMORY.md").is_file()
    sqlite_candidates = [
        memory_workspace_root / "memory.sqlite3",
        memory_workspace_root / "memory.db",
        memory_workspace_root / ".memory.sqlite3",
    ]
    has_sqlite = any(path.is_file() for path in sqlite_candidates)
    has_memory_md = (memory_workspace_root / "MEMORY.md").is_file()
    changed_files: list[str] = []
    consolidation_note = ""
    if not memory_exists and artifact_source_dir is None:
        archive_legacy_memory_bundle(workspace, memory_workspace_root, memory_root=memory_root)
        documents = build_memory_documents(workspace=workspace, snapshot=snapshot)
        changed_files = write_documents(documents, memory_workspace_root)
        state_path = write_memory_state(snapshot, memory_workspace_root)
        if state_path:
            changed_files.append(state_path)
        persist_memory_bundle(workspace, documents, resolved_dir=memory_workspace_root)
        consolidation_note = "memory_workspace was empty; bridge ran one-shot consolidation"
    retrieval = render_context(
        workspace=workspace,
        topic=query,
        max_items=top,
        repo_root=repo_root,
        artifact_root=artifact_source_dir,
        mode=mode,
    )
    continuity_layout = describe_continuity_layout(repo_root)
    memory_layout = describe_project_local_memory_layout(repo_root)
    return {
        "workspace": workspace,
        "using_project_local": True,
        "memory_root": str(memory_workspace_root),
        "memory_layout": memory_layout,
        "consolidation_note": consolidation_note,
        "changed_files": changed_files,
        "sqlite": {
            "path": retrieval.get("sqlite_path", ""),
            "has_sqlite": has_sqlite,
            "has_memory_md": has_memory_md,
        },
        "retrieval": retrieval,
        "continuity": effective_continuity,
        "active_task": active_task,
        "continuity_decision": {
            "query": query,
            "query_matches_active_task": bool(query_matches_active_task),
            "ignored_root_continuity": bool(ignored_active_task),
            "task_id": active_task.get("task_id") or build_task_id(query or workspace),
            "source_task": active_task.get("task"),
            "mode": mode,
        },
        "source_artifacts": {
            **continuity_layout,
        },
    }


def export_supporting_files(
    *,
    workspace: str,
    output_dir: Path,
    query: str = "",
    source_root: Path | None = None,
    top: int = 8,
) -> dict[str, Any]:
    """Write bridge-facing JSON helpers for external consumers."""

    payload = export_framework_skills()
    repo_root = (source_root or get_repo_root()).resolve()
    memory = build_framework_memory_bootstrap(
        workspace=workspace,
        query=query,
        source_root=repo_root,
        top=top,
    )
    proposals = build_evolution_proposals()
    output_dir.mkdir(parents=True, exist_ok=True)
    skills_path = output_dir / "framework_skills.json"
    memory_path = output_dir / "framework_memory_bootstrap.json"
    proposals_path = output_dir / "framework_evolution_proposals.json"
    write_json_if_changed(skills_path, payload)
    write_json_if_changed(memory_path, memory)
    write_json_if_changed(proposals_path, proposals)
    summary_path = output_dir / "framework_bridge_summary.md"
    write_text_if_changed(
        summary_path,
        "\n".join(
            [
                "# Framework Bridge Summary",
                "",
                f"- workspace: {workspace}",
                f"- query: {query or '(none)'}",
                f"- generated_at: {current_local_timestamp()}",
                f"- runtime_path: {payload['source']}",
                f"- skills: {payload['count']}",
                f"- proposal_count: {proposals['proposal_count']}",
                f"- memory_root: {memory['memory_root']}",
                f"- workspace_dir: {workspace_dir(workspace, DEFAULT_MEMORY_ROOT)}",
                "",
                "These files are bridge-friendly exports, not the canonical runtime source of truth.",
            ]
        )
        + "\n",
    )
    return {
        "skills_path": str(skills_path),
        "memory_path": str(memory_path),
        "proposals_path": str(proposals_path),
        "summary_path": str(summary_path),
    }
