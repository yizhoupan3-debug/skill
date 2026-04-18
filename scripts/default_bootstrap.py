#!/usr/bin/env python3
"""Materialize the default framework bootstrap bundle for shared CLI hosts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.framework_bridge import (
    build_evolution_proposals,
    build_framework_memory_bootstrap,
    export_framework_skills,
)
from scripts.memory_support import (
    bootstrap_artifact_root,
    build_task_id,
    current_local_timestamp,
    get_repo_root,
    workspace_name_from_root,
    write_json_if_changed,
)

BOOTSTRAP_FILENAME = "framework_default_bootstrap.json"


def resolve_bootstrap_path(output_dir: Path) -> Path:
    """Return the canonical bootstrap payload path."""

    return output_dir / BOOTSTRAP_FILENAME


def resolve_task_bootstrap_path(output_dir: Path, task_id: str) -> Path:
    """Return the task-scoped bootstrap payload path."""

    return output_dir / task_id / BOOTSTRAP_FILENAME


def _compact_evolution_proposals(payload: dict[str, Any]) -> dict[str, Any]:
    """Keep the bootstrap-facing proposal payload compact and prompt-safe."""

    return {
        "proposal_count": payload.get("proposal_count", 0),
        "proposals": payload.get("proposals", []),
    }


def run_default_bootstrap(
    *,
    query: str = "",
    repo_root: Path | None = None,
    output_dir: Path | None = None,
    memory_root: Path | None = None,
    artifact_source_dir: Path | None = None,
    workspace: str | None = None,
    top: int = 8,
) -> dict[str, Any]:
    """Build and write the default framework bootstrap bundle."""

    repo_root = (repo_root or get_repo_root()).resolve()
    workspace = workspace or workspace_name_from_root(repo_root)
    output_dir = (output_dir or bootstrap_artifact_root(repo_root)).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    runtime = export_framework_skills()
    memory = build_framework_memory_bootstrap(
        workspace=workspace,
        query=query,
        source_root=repo_root,
        memory_root=memory_root,
        artifact_source_dir=artifact_source_dir,
        top=top,
        mode="active",
    )
    proposals = build_evolution_proposals()
    created_at = current_local_timestamp()
    continuity_decision = memory.get("continuity_decision", {})
    task_id = str(
        continuity_decision.get("task_id")
        or build_task_id(query or workspace, created_at=created_at)
    )
    primary_bootstrap_path = resolve_task_bootstrap_path(output_dir, task_id)
    mirror_bootstrap_path = resolve_bootstrap_path(output_dir)
    payload = {
        "skills-export": runtime,
        "memory-bootstrap": memory,
        "evolution-proposals": _compact_evolution_proposals(proposals),
        "bootstrap": {
            "query": query,
            "workspace": workspace,
            "repo_root": str(repo_root),
            "task_id": task_id,
            "created_at": created_at,
            "source_task": continuity_decision.get("source_task"),
            "query_matches_active_task": bool(
                continuity_decision.get("query_matches_active_task", False)
            ),
            "ignored_root_continuity": bool(
                continuity_decision.get("ignored_root_continuity", False)
            ),
        },
    }
    write_json_if_changed(primary_bootstrap_path, payload)
    write_json_if_changed(mirror_bootstrap_path, payload)
    return {
        "bootstrap_path": str(primary_bootstrap_path),
        "paths": {
            "output_dir": str(output_dir),
            "task_output_dir": str(primary_bootstrap_path.parent),
            "repo_root": str(repo_root),
            "memory_root": memory["memory_root"],
            "mirror_bootstrap_path": str(mirror_bootstrap_path),
        },
        "memory_items": len(memory["retrieval"].get("items", [])),
        "proposal_count": proposals.get("proposal_count", 0),
        "payload": payload,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Build the default framework bootstrap bundle.")
    parser.add_argument("--query", default="")
    parser.add_argument("--output-dir", type=Path, default=None)
    parser.add_argument("--memory-root", type=Path, default=None)
    parser.add_argument("--artifact-source-dir", type=Path, default=None)
    parser.add_argument("--workspace", default=None)
    parser.add_argument("--top", type=int, default=8)
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    result = run_default_bootstrap(
        query=args.query,
        output_dir=args.output_dir,
        memory_root=args.memory_root,
        artifact_source_dir=args.artifact_source_dir,
        workspace=args.workspace,
        top=args.top,
    )
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result["bootstrap_path"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
