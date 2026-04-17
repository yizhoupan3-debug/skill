#!/usr/bin/env python3
"""Run the shared CLI memory maintenance pipeline with isolated artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.audit_codex_storage import collect_storage_report
from scripts.consolidate_memory import build_memory_documents, persist_memory_bundle, write_documents
from scripts.hermes_default_bootstrap import run_default_bootstrap
from scripts.memory_support import (
    DEFAULT_CODEX_ROOT,
    DEFAULT_MEMORY_ROOT,
    current_local_date,
    current_local_timestamp,
    get_repo_root,
    load_runtime_snapshot,
    resolve_effective_memory_dir,
    workspace_dir,
    write_json_if_changed,
    write_text_if_changed,
)
from scripts.retrieve_memory import render_context


def _summary_lines(
    *,
    workspace: str,
    generated_at: str,
    memory_root: Path,
    storage_root: Path,
    report: dict[str, Any],
    sqlite_result: dict[str, Any],
    changed_files: list[str],
) -> list[str]:
    return [
        f"- workspace: {workspace}",
        f"- generated_at: {generated_at}",
        f"- memory_root: {memory_root}",
        f"- storage_root: {storage_root}",
        f"- total_mib: {report.get('total_mib', 0)}",
        f"- memory_changed: {bool(changed_files)}",
        f"- sqlite_path: {sqlite_result.get('db_path', '')}",
        f"- sqlite_memory_items: {sqlite_result.get('memory_items', 0)}",
        f"- sqlite_session_notes: {sqlite_result.get('session_notes', 0)}",
        f"- sqlite_evidence_records: {sqlite_result.get('evidence_records', 0)}",
        "- changed_files:" if changed_files else "- changed_files: none",
        *([f"  - {path}" for path in changed_files] if changed_files else []),
    ]


def _top_recommendations(report: dict[str, Any]) -> list[str]:
    rec: list[str] = []
    for entry in report.get("top_entries", [])[:5]:
        path = str(entry.get("path", ""))
        if "__pycache__" in path:
            rec.append(f"consider pruning cache: {path}")
        elif path.endswith(".sqlite3"):
            rec.append(f"monitor sqlite growth: {path}")
    return rec


def run_pipeline(
    *,
    workspace: str,
    source_root: Path,
    memory_root: Path | None = None,
    output_dir: Path | None = None,
    artifact_source_dir: Path | None = None,
    topic: str = "",
    top: int = 8,
) -> dict[str, Any]:
    """Run the shared CLI memory maintenance pipeline."""

    repo_root = source_root.resolve()
    resolved_dir = resolve_effective_memory_dir(workspace=workspace, memory_root=memory_root, repo_root=repo_root)
    snapshot = load_runtime_snapshot(repo_root, artifact_root=artifact_source_dir)
    documents = build_memory_documents(workspace=workspace, snapshot=snapshot)
    changed_files = write_documents(documents, resolved_dir)
    sqlite_result = persist_memory_bundle(workspace, documents, memory_root=memory_root, resolved_dir=resolved_dir)
    report = collect_storage_report(DEFAULT_CODEX_ROOT, top=top)
    retrieval = render_context(workspace=workspace, topic=topic, max_items=top, repo_root=repo_root)
    generated_at = current_local_timestamp()
    out_dir = (output_dir or (repo_root / "artifacts" / "current")).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    summary_md = "\n".join(["# CLI-common memory automation pipeline", "", * _summary_lines(
        workspace=workspace,
        generated_at=generated_at,
        memory_root=resolved_dir,
        storage_root=DEFAULT_CODEX_ROOT,
        report=report,
        sqlite_result=sqlite_result,
        changed_files=changed_files,
    ), "", "## recommendations", "", *[f"- {line}" for line in _top_recommendations(report)]]) + "\n"
    evidence = {
        "version": 1,
        "artifacts": [
            {"kind": "script", "path": "scripts/consolidate_memory.py"},
            {"kind": "script", "path": "scripts/retrieve_memory.py"},
            {"kind": "script", "path": "scripts/audit_codex_storage.py"},
            {"kind": "docs", "path": str(resolved_dir / "MEMORY.md")},
            {"kind": "storage_audit", "path": str(out_dir / "storage_audit.json")},
        ],
    }
    trace = {
        "version": 1,
        "task": "CLI-common memory automation pipeline",
        "phase": "execution",
        "owner": "execution-controller-coding",
        "overlay": "skill-maintenance-codex",
    }
    write_text_if_changed(out_dir / "SESSION_SUMMARY.md", summary_md)
    write_json_if_changed(out_dir / "NEXT_ACTIONS.json", {"version": 1, "next_actions": _top_recommendations(report)})
    write_json_if_changed(out_dir / "EVIDENCE_INDEX.json", evidence)
    write_json_if_changed(out_dir / "TRACE_METADATA.json", trace)
    write_json_if_changed(out_dir / "storage_audit.json", report)
    run_summary = {
        "workspace": workspace,
        "generated_at": generated_at,
        "run_date": current_local_date(),
        "sqlite_path": sqlite_result.get("db_path", ""),
        "memory_root": str(resolved_dir),
        "output_dir": str(out_dir),
        "changed_files": changed_files,
        "sqlite_result": sqlite_result,
        "storage_total_mib": report.get("total_mib", 0),
        "top_storage_entries": report.get("top_entries", []),
        "retrieval": retrieval,
    }
    write_json_if_changed(out_dir / "run_summary.json", run_summary)
    hermes = run_default_bootstrap(
        query=topic,
        repo_root=repo_root,
        output_dir=out_dir,
        memory_root=memory_root,
        artifact_source_dir=artifact_source_dir,
        workspace=workspace,
        top=top,
    )
    return {
        "workspace": workspace,
        "memory_root": str(resolved_dir),
        "changed_files": changed_files,
        "report": report,
        "sqlite_result": sqlite_result,
        "retrieval": retrieval,
        "hermes": hermes,
        "output_dir": str(out_dir),
    }


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Run the shared CLI memory maintenance pipeline.")
    parser.add_argument("--workspace", default=Path.cwd().name, help="Workspace name. Defaults to the repo root basename.")
    parser.add_argument("--source-root", type=Path, default=None, help="Repository root containing short-term artifacts.")
    parser.add_argument("--memory-root", type=Path, default=None, help="Root directory for long-term memory.")
    parser.add_argument("--output-dir", type=Path, default=None, help="Independent artifact output directory.")
    parser.add_argument("--artifact-source-dir", type=Path, default=None, help="Optional isolated artifact directory used as the consolidation source instead of the repo root.")
    parser.add_argument("--topic", default="", help="Topic used for retrieval context filtering.")
    parser.add_argument("--top", type=int, default=8, help="Number of storage entries and retrieval snippets to keep.")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Output JSON summary.")
    args = parser.parse_args()
    results = run_pipeline(
        workspace=args.workspace,
        source_root=(args.source_root or get_repo_root()),
        memory_root=args.memory_root,
        output_dir=args.output_dir,
        artifact_source_dir=args.artifact_source_dir,
        topic=args.topic,
        top=args.top,
    )
    if args.json_output:
        print(json.dumps(results, ensure_ascii=False, indent=2))
    else:
        sqlite_result = results["sqlite_result"]
        print(
            f"CLI-common {args.workspace}\n"
            f"- sqlite_counts: items={sqlite_result.get('memory_items', 0)}, "
            f"session_notes={sqlite_result.get('session_notes', 0)}, "
            f"evidence={sqlite_result.get('evidence_records', 0)}\n"
            f"- storage_total_mib: {results['report'].get('total_mib', 0)}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
