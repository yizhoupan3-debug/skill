#!/usr/bin/env python3
"""Materialize the default framework bootstrap bundle for shared CLI hosts."""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from framework_runtime.rust_router import get_cached_route_adapter
from scripts.framework_bridge import (
    build_evolution_proposals,
    export_framework_skills,
)

BOOTSTRAP_FILENAME = "framework_default_bootstrap.json"


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _bootstrap_artifact_root(source_root: Path) -> Path:
    return source_root / "artifacts" / "bootstrap"


def _current_local_timestamp() -> str:
    return datetime.now().astimezone().isoformat(timespec="seconds")


def _safe_slug(value: str, fallback: str = "unknown") -> str:
    import re

    slug = re.sub(r"[^\w.-]+", "-", value, flags=re.UNICODE)
    slug = re.sub(r"-{2,}", "-", slug).strip("._-")
    return slug or fallback


def _build_task_id(task: str, *, created_at: str | None = None) -> str:
    import re

    stamp = re.sub(r"[^0-9A-Za-z]+", "", (created_at or _current_local_timestamp()))
    base = _safe_slug(task or "task")
    return f"{base}-{stamp[-14:]}" if stamp else base


def _workspace_name_from_root(repo_root: Path) -> str:
    return repo_root.name


def _write_json_if_changed(path: Path, payload: dict[str, Any] | list[Any]) -> bool:
    content = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    existing = path.read_text(encoding="utf-8") if path.is_file() else ""
    if existing == content:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return True


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

    repo_root = (repo_root or _repo_root()).resolve()
    workspace = workspace or _workspace_name_from_root(repo_root)
    output_dir = (output_dir or _bootstrap_artifact_root(repo_root)).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    runtime = export_framework_skills()
    memory = get_cached_route_adapter(_repo_root()).framework_memory_recall(
        repo_root=repo_root,
        query=query,
        top=top,
        mode="active",
        memory_root=memory_root,
        artifact_source_dir=artifact_source_dir,
    )
    proposals = build_evolution_proposals()
    created_at = _current_local_timestamp()
    continuity_decision = memory.get("continuity_decision", {})
    task_id = str(
        continuity_decision.get("task_id")
        or _build_task_id(query or workspace, created_at=created_at)
    )
    primary_bootstrap_path = resolve_task_bootstrap_path(output_dir, task_id)
    mirror_bootstrap_path = resolve_bootstrap_path(output_dir)
    payload = {
        "skills-export": runtime,
        "memory-bootstrap": memory.get("prompt_payload", memory),
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
    _write_json_if_changed(primary_bootstrap_path, payload)
    _write_json_if_changed(mirror_bootstrap_path, payload)
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
