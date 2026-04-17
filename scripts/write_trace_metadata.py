#!/usr/bin/env python3
"""Write normalized runtime trace metadata for complex tasks."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from pathlib import Path


def write_trace_metadata(
    path: Path,
    *,
    task: str,
    matched_skills: list[str],
    owner: str,
    gate: str,
    overlay: str | None,
    reroute_count: int,
    retry_count: int,
    artifact_paths: list[str],
    verification_status: str,
    framework_version: str = "phase1",
    routing_runtime_version: int = 1,
) -> None:
    """Write the canonical runtime trace metadata JSON file.

    Parameters:
        path: Output JSON path.
        task: Task title.
        matched_skills: Ordered list of matched skills.
        owner: Primary owner skill.
        gate: Gate used for routing.
        overlay: Optional overlay skill.
        reroute_count: Total reroute count.
        retry_count: Total retry count.
        artifact_paths: Output artifact paths.
        verification_status: Final verification status.
        framework_version: Framework version label.
        routing_runtime_version: Runtime route-map version.

    Returns:
        None.
    """

    payload = {
        "version": 1,
        "ts": datetime.now(UTC).isoformat(),
        "task": task,
        "framework_version": framework_version,
        "routing_runtime_version": routing_runtime_version,
        "matched_skills": matched_skills,
        "decision": {
            "owner": owner,
            "gate": gate,
            "overlay": overlay,
        },
        "reroute_count": reroute_count,
        "retry_count": retry_count,
        "artifact_paths": artifact_paths,
        "verification_status": verification_status,
    }
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    """CLI entry point for writing trace metadata.

    Parameters:
        None.

    Returns:
        int: Process exit code.
    """

    parser = argparse.ArgumentParser(description="Write runtime trace metadata.")
    parser.add_argument("--output", type=Path, required=True, help="Output JSON path.")
    parser.add_argument("--task", required=True, help="Task title.")
    parser.add_argument("--matched-skill", action="append", default=[], help="Repeatable matched skill.")
    parser.add_argument("--owner", required=True, help="Primary owner skill.")
    parser.add_argument("--gate", default="none", help="Routing gate used.")
    parser.add_argument("--overlay", default="", help="Optional overlay skill.")
    parser.add_argument("--reroute-count", type=int, default=0, help="Total reroute count.")
    parser.add_argument("--retry-count", type=int, default=0, help="Total retry count.")
    parser.add_argument("--artifact-path", action="append", default=[], help="Repeatable artifact path.")
    parser.add_argument("--verification-status", default="in_progress", help="Verification status.")
    parser.add_argument("--framework-version", default="phase1", help="Framework version label.")
    parser.add_argument("--routing-runtime-version", type=int, default=1, help="Runtime route version.")
    args = parser.parse_args()

    write_trace_metadata(
        args.output,
        task=args.task,
        matched_skills=args.matched_skill,
        owner=args.owner,
        gate=args.gate,
        overlay=args.overlay or None,
        reroute_count=args.reroute_count,
        retry_count=args.retry_count,
        artifact_paths=args.artifact_path,
        verification_status=args.verification_status,
        framework_version=args.framework_version,
        routing_runtime_version=args.routing_runtime_version,
    )
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
