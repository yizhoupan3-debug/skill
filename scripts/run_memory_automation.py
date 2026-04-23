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

from framework_runtime.rust_router import get_cached_route_adapter
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
    planned_current_artifact_migrations: list[dict[str, str]],
    planned_legacy_root_migrations: list[dict[str, str]],
    apply_artifact_migrations: bool,
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
        f"- legacy_memory_items_archived: {archive_result.get('legacy_memory_item_count', 0)}",
        f"- apply_artifact_migrations: {apply_artifact_migrations}",
        f"- planned_current_artifact_migrations: {len(planned_current_artifact_migrations)}",
        f"- planned_legacy_root_migrations: {len(planned_legacy_root_migrations)}",
        "- changed_files:" if changed_files else "- changed_files: none",
        *([f"  - {path}" for path in changed_files] if changed_files else []),
    ]


def _top_recommendations(report: dict[str, Any]) -> list[str]:
    rec: list[str] = []
    for entry in report.get("top_entries", [])[:5]:
        path = str(entry.get("path", ""))
        if "__pycache__" in path:
            rec.append(f"consider pruning cache: {path}")
        elif path.endswith(("logs_1.sqlite", "logs_2.sqlite")):
            rec.append(f"rotate or compact trace database: {path}")
        elif "/sessions/" in path and path.endswith(".jsonl"):
            rec.append(f"archive or compress old session trace: {path}")
        elif "/tmp/arg0/" in path:
            rec.append(f"clean stale tmp runtime wrappers: {path}")
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


def _destination_for_current_artifact(repo_root: Path, path: Path, active_task_id: str) -> Path | None:
    current_root = repo_root / "artifacts" / "current"
    if not path.exists() or path.parent not in {current_root, current_root / active_task_id}:
        return None
    if path.name in CURRENT_ALLOWED_ARTIFACT_NAMES or path.name == active_task_id:
        return None
    if path.parent == current_root / active_task_id and path.name in TASK_ALLOWED_ARTIFACT_NAMES:
        return None
    if path.name in {"framework_default_bootstrap.json", "hermes_default_bootstrap.json"}:
        suffix = [path.name] if path.parent == current_root else [active_task_id, path.name]
        return repo_root / "artifacts" / "bootstrap" / "legacy-current" / Path(*suffix)
    if path.name in {"run_summary.json", "storage_audit.json", "snapshot.json", "snapshot.md"}:
        suffix = [path.name] if path.parent == current_root else [active_task_id, path.name]
        return ops_memory_automation_root(repo_root) / "legacy-current" / Path(*suffix)
    if path.name.startswith("tmp-"):
        if path.parent == current_root:
            return scratch_artifact_root(repo_root) / path.name
        return scratch_artifact_root(repo_root, "legacy-current") / active_task_id / path.name
    suffix = [path.name] if path.parent == current_root else [active_task_id, path.name]
    return evidence_artifact_root(repo_root, "legacy-current") / Path(*suffix)


def plan_current_artifact_clutter_migrations(repo_root: Path, active_task_id: str) -> list[dict[str, str]]:
    """Describe which current-artifact paths would be migrated without mutating them."""

    current_root = repo_root / "artifacts" / "current"
    if not current_root.exists():
        return []
    plans: list[dict[str, str]] = []
    for path in sorted(current_root.iterdir()):
        destination = _destination_for_current_artifact(repo_root, path, active_task_id)
        if destination is not None:
            plans.append({"source": str(path), "destination": str(destination)})
    if active_task_id:
        task_root = current_root / active_task_id
        if task_root.is_dir():
            for path in sorted(task_root.iterdir()):
                destination = _destination_for_current_artifact(repo_root, path, active_task_id)
                if destination is not None:
                    plans.append({"source": str(path), "destination": str(destination)})
    return plans


def plan_legacy_artifact_root_migrations(repo_root: Path) -> list[dict[str, str]]:
    """Describe which legacy artifact roots would be relocated without mutating them."""

    artifacts_root = repo_root / "artifacts"
    plans: list[dict[str, str]] = []
    legacy_memory_root = artifacts_root / "memory_automation"
    if legacy_memory_root.exists():
        plans.append(
            {
                "source": str(legacy_memory_root),
                "destination": str(ops_memory_automation_root(repo_root) / "legacy-root"),
            }
        )
    for path in sorted(artifacts_root.iterdir()):
        if path.name.startswith("tmp-"):
            plans.append(
                {
                    "source": str(path),
                    "destination": str(scratch_artifact_root(repo_root) / path.name),
                }
            )
    return plans


def migrate_current_artifact_clutter(repo_root: Path, active_task_id: str) -> list[str]:
    """Move old non-continuity files out of artifacts/current/."""

    moved: list[str] = []
    for plan in plan_current_artifact_clutter_migrations(repo_root, active_task_id):
        moved.append(_move_path(Path(plan["source"]), Path(plan["destination"])))
    return moved


def migrate_legacy_artifact_roots(repo_root: Path) -> list[str]:
    """Move legacy artifact roots into the new partitioned layout."""

    moved: list[str] = []
    for plan in plan_legacy_artifact_root_migrations(repo_root):
        moved.append(_move_path(Path(plan["source"]), Path(plan["destination"])))
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
    apply_artifact_migrations: bool = False,
) -> dict[str, Any]:
    """Run the shared CLI memory maintenance pipeline."""

    repo_root = source_root.resolve()
    resolved_dir = resolve_effective_memory_dir(workspace=workspace, memory_root=memory_root, repo_root=repo_root)
    snapshot = load_runtime_snapshot(repo_root, artifact_root=artifact_source_dir)
    planned_current_artifact_migrations = (
        []
        if artifact_source_dir is not None
        else plan_current_artifact_clutter_migrations(repo_root, snapshot.active_task_id)
    )
    planned_legacy_root_migrations = (
        []
        if artifact_source_dir is not None
        else plan_legacy_artifact_root_migrations(repo_root)
    )
    moved_current_artifacts = (
        migrate_current_artifact_clutter(repo_root, snapshot.active_task_id)
        if apply_artifact_migrations and artifact_source_dir is None
        else []
    )
    moved_legacy_roots = (
        migrate_legacy_artifact_roots(repo_root)
        if apply_artifact_migrations and artifact_source_dir is None
        else []
    )
    archive_result = archive_legacy_memory_bundle(workspace, resolved_dir, memory_root=memory_root)
    documents = build_memory_documents(workspace=workspace, snapshot=snapshot)
    changed_files = write_documents(documents, resolved_dir)
    state_path = write_memory_state(snapshot, resolved_dir)
    if state_path:
        changed_files.append(state_path)
    sqlite_result = persist_memory_bundle(workspace, documents, memory_root=memory_root, resolved_dir=resolved_dir)
    report = collect_storage_report(DEFAULT_CODEX_ROOT, top=top)
    retrieval_payload = get_cached_route_adapter(get_repo_root()).framework_memory_recall(
        repo_root=repo_root,
        query=topic,
        top=top,
        mode="stable",
        memory_root=memory_root,
        artifact_source_dir=artifact_source_dir,
    )
    retrieval = retrieval_payload.get("retrieval")
    if not isinstance(retrieval, dict):
        raise RuntimeError("Rust memory recall returned a missing retrieval payload.")
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
                planned_current_artifact_migrations=planned_current_artifact_migrations,
                planned_legacy_root_migrations=planned_legacy_root_migrations,
                apply_artifact_migrations=apply_artifact_migrations,
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
            "planned_current_artifact_migrations": planned_current_artifact_migrations,
            "planned_legacy_root_migrations": planned_legacy_root_migrations,
            "moved_current_artifacts": moved_current_artifacts,
            "moved_legacy_roots": moved_legacy_roots,
            "retrieval": retrieval,
            "apply_artifact_migrations": apply_artifact_migrations,
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
        "planned_current_artifact_migrations": planned_current_artifact_migrations,
        "planned_legacy_root_migrations": planned_legacy_root_migrations,
        "moved_current_artifacts": moved_current_artifacts,
        "moved_legacy_roots": moved_legacy_roots,
        "apply_artifact_migrations": apply_artifact_migrations,
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
        "planned_current_artifact_migrations": planned_current_artifact_migrations,
        "planned_legacy_root_migrations": planned_legacy_root_migrations,
        "moved_current_artifacts": moved_current_artifacts,
        "moved_legacy_roots": moved_legacy_roots,
        "apply_artifact_migrations": apply_artifact_migrations,
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
    parser.add_argument(
        "--apply-artifact-migrations",
        action="store_true",
        help="Actually migrate legacy artifact paths. Default is plan-only to avoid mutating live continuity during maintenance runs.",
    )
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
        apply_artifact_migrations=args.apply_artifact_migrations,
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
