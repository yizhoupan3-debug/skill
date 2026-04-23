#!/usr/bin/env python3
"""Thin Python shell for Rust-owned framework memory recall."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "framework_runtime" / "src"))

from framework_runtime.rust_router import get_cached_route_adapter

VALID_MODES = {"stable", "active", "history", "debug"}


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def render_context(
    *,
    workspace: str,
    topic: str = "",
    max_items: int = 8,
    memory_root: Path | None = None,
    repo_root: Path | None = None,
    artifact_root: Path | None = None,
    mode: str = "stable",
    task_id: str = "",
) -> dict[str, Any]:
    """Render memory context through the Rust framework runtime read model."""

    if mode not in VALID_MODES:
        raise ValueError(f"Unsupported memory recall mode: {mode}")
    root = (repo_root or _repo_root()).resolve()
    payload = get_cached_route_adapter(_repo_root()).framework_memory_recall(
        repo_root=root,
        query=topic,
        top=max_items,
        mode=mode,
        memory_root=memory_root,
        artifact_source_dir=artifact_root,
        task_id=task_id or None,
    )
    retrieval = payload.get("retrieval")
    if not isinstance(retrieval, dict):
        raise RuntimeError("Rust memory recall returned a missing retrieval payload.")
    return {
        "workspace": str(retrieval.get("workspace") or workspace),
        "topic": str(retrieval.get("topic") or topic),
        "mode": str(retrieval.get("mode") or mode),
        "memory_root": str(retrieval.get("memory_root") or ""),
        "sqlite_path": str(retrieval.get("sqlite_path") or ""),
        "items": retrieval.get("items") if isinstance(retrieval.get("items"), list) else [],
        "context": str(retrieval.get("context") or ""),
        "active_task_included": bool(retrieval.get("active_task_included", False)),
        "freshness": retrieval.get("freshness") if isinstance(retrieval.get("freshness"), dict) else {},
        "continuity_state": retrieval.get("continuity_state"),
        "active_task_id": str(retrieval.get("active_task_id") or ""),
    }


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Retrieve workspace memory context.")
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--topic", default="")
    parser.add_argument("--top", type=int, default=8, dest="max_items")
    parser.add_argument("--mode", choices=sorted(VALID_MODES), default="stable")
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    result = render_context(
        workspace=args.workspace,
        topic=args.topic,
        max_items=args.max_items,
        repo_root=_repo_root(),
        mode=args.mode,
    )
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(result["context"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
