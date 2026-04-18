#!/usr/bin/env python3
"""Run the shared CLI memory maintenance pipeline with isolated artifacts."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.audit_codex_storage import collect_storage_report
from scripts.consolidate_memory import (
    archive_legacy_memory_bundle,
    build_memory_documents,
    persist_memory_bundle,
    write_documents,
    write_memory_state,
)
from scripts.default_bootstrap import run_default_bootstrap
from scripts.memory_support import (
    DEFAULT_CODEX_ROOT,
    CURRENT_ALLOWED_ARTIFACT_NAMES,
    TASK_ALLOWED_ARTIFACT_NAMES,
    build_task_id,
    current_local_date,
    current_local_timestamp,
    evidence_artifact_root,
    get_repo_root,
    load_runtime_snapshot,
    ops_memory_automation_root,
    resolve_effective_memory_dir,
    scratch_artifact_root,
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
    archive_result: dict[str, Any],
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
        f"- legacy_rows_archived: {archive_result.get('legacy_row_count', 0)}",
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


def _move_path(source: Path, destination: Path) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        suffix = current_local_timestamp().replace(":", "").replace("+", "_")
        destination = destination.with_name(f"{destination.stem}-{suffix}{destination.suffix}")
    shutil.move(str(source), str(destination))
    return str(destination)


def migrate_current_artifact_clutter(repo_root: Path, active_task_id: str) -> list[str]:
    """Move old non-continuity files out of artifacts/current/."""

    current_root = repo_root / "artifacts" / "current"
    if not current_root.exists():
        return []
    moved: list[str] = []
    for path in sorted(current_root.iterdir()):
        if path.name in CURRENT_ALLOWED_ARTIFACT_NAMES or path.name == active_task_id:
            continue
        if path.name in {"framework_default_bootstrap.json", "hermes_default_bootstrap.json"}:
            destination = repo_root / "artifacts" / "bootstrap" / "legacy-current" / path.name
        elif path.name in {"run_summary.json", "storage_audit.json", "snapshot.json", "snapshot.md"}:
            destination = ops_memory_automation_root(repo_root) / "legacy-current" / path.name
        elif path.name.startswith("tmp-"):
            destination = repo_root / "artifacts" / "scratch" / path.name
        else:
            destination = evidence_artifact_root(repo_root, "legacy-current") / path.name
        moved.append(_move_path(path, destination))
    if active_task_id:
        task_root = current_root / active_task_id
        if task_root.is_dir():
            for path in sorted(task_root.iterdir()):
                if path.name in TASK_ALLOWED_ARTIFACT_NAMES:
                    continue
                if path.name in {"framework_default_bootstrap.json", "hermes_default_bootstrap.json"}:
                    destination = repo_root / "artifacts" / "bootstrap" / "legacy-current" / active_task_id / path.name
                elif path.name in {"run_summary.json", "storage_audit.json", "snapshot.json", "snapshot.md"}:
                    destination = ops_memory_automation_root(repo_root) / "legacy-current" / active_task_id / path.name
                elif path.name.startswith("tmp-"):
                    destination = scratch_artifact_root(repo_root, "legacy-current") / active_task_id / path.name
                else:
                    destination = evidence_artifact_root(repo_root, "legacy-current") / active_task_id / path.name
                moved.append(_move_path(path, destination))
    return moved


def migrate_legacy_artifact_roots(repo_root: Path) -> list[str]:
    """Move legacy artifact roots into the new partitioned layout."""

    artifacts_root = repo_root / "artifacts"
    moved: list[str] = []
    legacy_memory_root = artifacts_root / "memory_automation"
    if legacy_memory_root.exists():
        destination = ops_memory_automation_root(repo_root) / "legacy-root"
        moved.append(_move_path(legacy_memory_root, destination))
    for path in sorted(artifacts_root.iterdir()):
        if not path.name.startswith("tmp-"):
            continue
        moved.append(_move_path(path, scratch_artifact_root(repo_root) / path.name))
    return moved


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
    moved_current_artifacts = (
        []
        if artifact_source_dir is not None
        else migrate_current_artifact_clutter(repo_root, snapshot.active_task_id)
    )
    moved_legacy_roots = [] if artifact_source_dir is not None else migrate_legacy_artifact_roots(repo_root)
    archive_result = archive_legacy_memory_bundle(workspace, resolved_dir, memory_root=memory_root)
    documents = build_memory_documents(workspace=workspace, snapshot=snapshot)
    changed_files = write_documents(documents, resolved_dir)
    state_path = write_memory_state(snapshot, resolved_dir)
    if state_path:
        changed_files.append(state_path)
    sqlite_result = persist_memory_bundle(workspace, documents, memory_root=memory_root, resolved_dir=resolved_dir)
    report = collect_storage_report(DEFAULT_CODEX_ROOT, top=top)
    retrieval = render_context(
        workspace=workspace,
        topic=topic,
        max_items=top,
        repo_root=repo_root,
        artifact_root=artifact_source_dir,
        mode="stable",
    )
    generated_at = current_local_timestamp()
    run_id = build_task_id(f"{workspace}-memory-automation", created_at=generated_at)
    out_dir = (output_dir or (ops_memory_automation_root(repo_root) / run_id)).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    summary_md = "\n".join(
        [
            "# CLI-common memory automation pipeline",
            "",
            *_summary_lines(
                workspace=workspace,
                generated_at=generated_at,
                memory_root=resolved_dir,
                storage_root=DEFAULT_CODEX_ROOT,
                report=report,
                sqlite_result=sqlite_result,
                changed_files=changed_files,
                archive_result=archive_result,
            ),
            "",
            "## recommendations",
            "",
            *[f"- {line}" for line in _top_recommendations(report)],
        ]
    ) + "\n"
    write_json_if_changed(out_dir / "storage_audit.json", report)
    write_text_if_changed(out_dir / "snapshot.md", summary_md)
    write_json_if_changed(
        out_dir / "snapshot.json",
        {
            "workspace": workspace,
            "generated_at": generated_at,
            "archive": archive_result,
            "changed_files": changed_files,
            "moved_current_artifacts": moved_current_artifacts,
            "moved_legacy_roots": moved_legacy_roots,
            "retrieval": retrieval,
        },
    )
    run_summary = {
        "workspace": workspace,
        "generated_at": generated_at,
        "run_date": current_local_date(),
        "run_id": run_id,
        "sqlite_path": sqlite_result.get("db_path", ""),
        "memory_root": str(resolved_dir),
        "output_dir": str(out_dir),
        "changed_files": changed_files,
        "archive": archive_result,
        "moved_current_artifacts": moved_current_artifacts,
        "moved_legacy_roots": moved_legacy_roots,
        "sqlite_result": sqlite_result,
        "storage_total_mib": report.get("total_mib", 0),
        "top_storage_entries": report.get("top_entries", []),
        "retrieval": retrieval,
    }
    write_json_if_changed(out_dir / "run_summary.json", run_summary)
    bootstrap = run_default_bootstrap(
        query=topic,
        repo_root=repo_root,
        memory_root=memory_root,
        artifact_source_dir=artifact_source_dir,
        workspace=workspace,
        top=top,
    )
    return {
        "workspace": workspace,
        "memory_root": str(resolved_dir),
        "changed_files": changed_files,
        "archive": archive_result,
        "moved_current_artifacts": moved_current_artifacts,
        "moved_legacy_roots": moved_legacy_roots,
        "report": report,
        "sqlite_result": sqlite_result,
        "retrieval": retrieval,
        "bootstrap": bootstrap,
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
            f"- sqlite_counts: items={sqlite_result.get('memory_items', 0)}\n"
            f"- storage_total_mib: {results['report'].get('total_mib', 0)}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
