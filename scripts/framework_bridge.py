#!/usr/bin/env python3
"""Framework-facing bridge for skill export and evolution proposals."""

from __future__ import annotations

import json
import os
import sys
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from framework_runtime.rust_router import RustRouteAdapter
from scripts.memory_support import (
    current_local_timestamp,
    get_repo_root,
    write_json_if_changed,
    write_text_if_changed,
)

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

@lru_cache(maxsize=1)
def _framework_rust_adapter() -> RustRouteAdapter:
    """Return the shared Rust adapter used by thin Python bridge paths."""

    return RustRouteAdapter(get_repo_root())

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
                            triggers=list(item.get("trigger_hints", []))
                            if isinstance(item.get("trigger_hints"), list)
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
    memory = _framework_rust_adapter().framework_memory_recall(
        repo_root=repo_root,
        query=query,
        top=top,
        mode="active",
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
