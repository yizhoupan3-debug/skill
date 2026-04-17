#!/usr/bin/env python3
"""Materialize the default Hermes bootstrap bundle for shared CLI hosts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.hermes_bridge import build_evolution_proposals, build_memory_bootstrap, export_skills_for_hermes
from scripts.memory_support import get_repo_root, workspace_name_from_root, write_json_if_changed


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
    """Build and write the Hermes default bootstrap bundle."""

    repo_root = (repo_root or get_repo_root()).resolve()
    workspace = workspace or workspace_name_from_root(repo_root)
    output_dir = (output_dir or repo_root / "artifacts" / "current").resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    runtime = export_skills_for_hermes()
    memory = build_memory_bootstrap(
        workspace=workspace,
        query=query,
        source_root=repo_root,
        memory_root=memory_root,
        artifact_source_dir=artifact_source_dir,
        top=top,
    )
    proposals = build_evolution_proposals()
    payload = {
        "skills-export": runtime,
        "memory-bootstrap": memory,
        "evolution-proposals": proposals,
        "bootstrap": {
            "query": query,
            "workspace": workspace,
            "repo_root": str(repo_root),
        },
    }
    bootstrap_path = output_dir / "hermes_default_bootstrap.json"
    write_json_if_changed(bootstrap_path, payload)
    return {
        "bootstrap_path": str(bootstrap_path),
        "paths": {
            "output_dir": str(output_dir),
            "repo_root": str(repo_root),
            "memory_root": memory["memory_root"],
        },
        "memory_items": len(memory["retrieval"].get("items", [])),
        "proposal_count": proposals.get("proposal_count", 0),
        "payload": payload,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Build the default Hermes bootstrap bundle.")
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
