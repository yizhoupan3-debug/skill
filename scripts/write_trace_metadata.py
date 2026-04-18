#!/usr/bin/env python3
"""Write normalized runtime trace metadata for complex tasks."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from pathlib import Path

TRACE_METADATA_SCHEMA_VERSION = "trace-metadata-v2"
DEFAULT_RUNTIME_PATH = Path(__file__).resolve().parents[1] / "skills" / "SKILL_ROUTING_RUNTIME.json"


def load_routing_runtime_version(runtime_path: Path = DEFAULT_RUNTIME_PATH) -> int:
    """Load the current routing runtime version from the generated runtime map."""

    if not runtime_path.is_file():
        return 1
    try:
        payload = json.loads(runtime_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return 1
    value = payload.get("version")
    return value if isinstance(value, int) else 1


def build_trace_metadata_payload(
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
    routing_runtime_version: int | None = None,
    ts: str | None = None,
) -> dict[str, object]:
    """Build one canonical trace metadata payload for all outputs."""

    return {
        "schema_version": TRACE_METADATA_SCHEMA_VERSION,
        "ts": ts or datetime.now(UTC).isoformat(),
        "task": task,
        "framework_version": framework_version,
        "routing_runtime_version": routing_runtime_version
        if routing_runtime_version is not None
        else load_routing_runtime_version(),
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
    routing_runtime_version: int | None = None,
    mirror_paths: list[Path] | None = None,
    ts: str | None = None,
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
        routing_runtime_version: Runtime route-map version. When omitted, load it
            from `skills/SKILL_ROUTING_RUNTIME.json`.
        mirror_paths: Additional outputs that must receive the identical bytes.
        ts: Optional fixed timestamp for deterministic rematerialization.

    Returns:
        None.
    """

    payload = build_trace_metadata_payload(
        task=task,
        matched_skills=matched_skills,
        owner=owner,
        gate=gate,
        overlay=overlay,
        reroute_count=reroute_count,
        retry_count=retry_count,
        artifact_paths=artifact_paths,
        verification_status=verification_status,
        framework_version=framework_version,
        routing_runtime_version=routing_runtime_version,
        ts=ts,
    )
    serialized = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    outputs = [path, *(mirror_paths or [])]
    for output in outputs:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(serialized, encoding="utf-8")


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
    parser.add_argument("--routing-runtime-version", type=int, default=None, help="Runtime route version.")
    parser.add_argument("--mirror-output", action="append", default=[], type=Path, help="Repeatable mirror output path.")
    parser.add_argument("--timestamp", default="", help="Optional fixed ISO timestamp for deterministic rematerialization.")
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
        mirror_paths=args.mirror_output,
        ts=args.timestamp or None,
    )
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
