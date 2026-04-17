#!/usr/bin/env python3
"""Hermes-facing bridge for skill export, long-memory bootstrap, and evolution proposals."""

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

from scripts.consolidate_memory import build_memory_documents, persist_memory_bundle, write_documents
from scripts.memory_support import (
    DEFAULT_MEMORY_ROOT,
    current_local_timestamp,
    get_repo_root,
    load_runtime_snapshot,
    resolve_effective_memory_dir,
    safe_slug,
    workspace_dir,
    write_json_if_changed,
    write_text_if_changed,
)
from scripts.retrieve_memory import render_context

RUNTIME_PATH = Path(os.environ.get("HERMES_BRIDGE_RUNTIME_PATH", "skills/SKILL_ROUTING_RUNTIME.json"))
APPROVAL_PATH = Path(os.environ.get("HERMES_BRIDGE_APPROVAL_PATH", "skills/SKILL_APPROVAL_POLICY.json"))
JOURNAL_PATH = Path(os.environ.get("HERMES_BRIDGE_JOURNAL_PATH", "skills/.evolution_journal.jsonl"))
HEALTH_PATH = Path(os.environ.get("HERMES_BRIDGE_HEALTH_PATH", "skills/SKILL_HEALTH_MANIFEST.json"))


@dataclass(slots=True)
class HermesSkill:
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
class HermesProposal:
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


def export_skills_for_hermes(
    runtime_path: Path | None = None,
    approval_path: Path | None = None,
) -> dict[str, Any]:
    """Export runtime and approval data for Hermes."""

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
                        HermesSkill(
                            slug=slug,
                            layer=str(item.get("routing_layer", "")),
                            owner=str(item.get("routing_owner", "")),
                            gate=str(item.get("routing_gate", "")),
                            session_start=str(item.get("session_start", "")),
                            summary=str(item.get("description", ""))[:200],
                            triggers=list(item.get("trigger_phrases", [])) if isinstance(item.get("trigger_phrases"), list) else [],
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
                        HermesSkill(
                            slug=slug,
                            layer=str(item[1]) if len(item) > 1 else "",
                            owner="",
                            gate=str(item[2]) if len(item) > 2 else "",
                            session_start=str(item[3]) if len(item) > 3 else "",
                            summary=str(item[4]) if len(item) > 4 else "",
                            triggers=[str(item[5])] if len(item) > 5 and str(item[5]).strip() else [],
                            agent_role="",
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
            f"补强 `{skill}` 的 trigger_phrases / routing_gate / owner"
            if issue_type == "routing-miss"
            else f"为 `{skill}` 增加更窄的调度说明或修复高 struggle 路径"
        )
        reroute_rate = len(reroutes) / max(1, len(items))
        proposals.append(
            asdict(
                HermesProposal(
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
    return {"proposal_count": len(proposals[:limit]), "proposals": proposals[:limit], "valid_skills": sorted(valid_skills), "health": health}


def build_memory_bootstrap(
    *,
    workspace: str,
    query: str = "",
    source_root: Path | None = None,
    memory_root: Path | None = None,
    artifact_source_dir: Path | None = None,
    top: int = 8,
) -> dict[str, Any]:
    """Build a memory bootstrap payload for Hermes consumption."""

    repo_root = (source_root or get_repo_root()).resolve()
    memory_workspace_root = resolve_effective_memory_dir(workspace=workspace, memory_root=memory_root, repo_root=repo_root)
    memory_workspace_root.mkdir(parents=True, exist_ok=True)
    memory_exists = (memory_workspace_root / "MEMORY.md").is_file()
    sqlite_candidates = [memory_workspace_root / "memory.sqlite3", memory_workspace_root / "memory.db", memory_workspace_root / ".memory.sqlite3"]
    has_sqlite = any(path.is_file() for path in sqlite_candidates)
    has_memory_md = (memory_workspace_root / "MEMORY.md").is_file()
    changed_files: list[str] = []
    consolidation_note = ""
    if not memory_exists and artifact_source_dir is None:
        snapshot = load_runtime_snapshot(repo_root)
        documents = build_memory_documents(workspace=workspace, snapshot=snapshot)
        changed_files = write_documents(documents, memory_workspace_root)
        persist_memory_bundle(workspace, documents, resolved_dir=memory_workspace_root)
        consolidation_note = "memory_workspace was empty; bridge ran one-shot consolidation"
    retrieval = render_context(workspace=workspace, topic=query, max_items=top, repo_root=repo_root)
    return {
        "workspace": workspace,
        "using_project_local": True,
        "memory_root": str(memory_workspace_root),
        "consolidation_note": consolidation_note,
        "changed_files": changed_files,
        "sqlite": {
            "path": retrieval.get("sqlite_path", ""),
            "has_sqlite": has_sqlite,
            "has_memory_md": has_memory_md,
        },
        "retrieval": retrieval,
        "source_artifacts": {
            "session_summary": str(repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md"),
            "next_actions": str(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json"),
            "evidence_index": str(repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json"),
            "trace_metadata": str(repo_root / "artifacts" / "current" / "TRACE_METADATA.json"),
            "supervisor_state": str(repo_root / ".supervisor_state.json"),
        },
    }
