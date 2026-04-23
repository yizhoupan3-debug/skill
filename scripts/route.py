#!/usr/bin/env python3
"""Direct Rust route CLI entrypoint with fail-closed binary freshness checks."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]


def _discover_codex_home(start_path: Path) -> Path:
    """Resolve the repository root without importing the Python runtime package."""

    if (start_path / "skills").is_dir():
        return start_path
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
            cwd=start_path,
        )
        resolved = Path(proc.stdout.strip())
        if (resolved / "skills").is_dir():
            return resolved
    except Exception:
        pass
    return start_path


def _resolve_router_binary_candidate(*candidates: Path) -> Path | None:
    """Prefer the freshest router-rs binary, keeping call order as the tiebreaker."""

    existing: list[tuple[float, int, Path]] = []
    for index, candidate in enumerate(candidates):
        if candidate.is_file():
            existing.append((candidate.stat().st_mtime, -index, candidate))
    if not existing:
        return None
    return max(existing)[2]


def _latest_router_source_mtime(router_dir: Path) -> float:
    candidates = [router_dir / "Cargo.toml"]
    source_dir = router_dir / "src"
    if source_dir.is_dir():
        candidates.extend(source_dir.rglob("*.rs"))
    return max((path.stat().st_mtime for path in candidates if path.exists()), default=0.0)


def _ensure_router_binary_current(codex_home: Path) -> Path:
    router_dir = codex_home / "scripts" / "router-rs"
    resolved_binary = _resolve_router_binary_candidate(
        router_dir / "target" / "release" / "router-rs",
        router_dir / "target" / "debug" / "router-rs",
    )
    if resolved_binary is None:
        raise RuntimeError(
            "router-rs requires a prebuilt binary; build scripts/router-rs before running the Python host runtime."
        )
    latest_source_mtime = _latest_router_source_mtime(router_dir)
    if resolved_binary.stat().st_mtime < latest_source_mtime:
        raise RuntimeError(
            "router-rs prebuilt binary is stale; rebuild scripts/router-rs before "
            "running the Python host runtime."
        )
    return resolved_binary


def _build_route_cli_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Lookup skills by query.")
    parser.add_argument("--query", type=str, required=True, help="Natural-language search query.")
    parser.add_argument("--limit", type=int, default=5, help="Max results to return.")
    parser.add_argument("--json", action="store_true", help="Output typed search contract JSON.")
    parser.add_argument("--route-json", action="store_true", help="Output final route decision in JSON format.")
    parser.add_argument("--session-id", type=str, default="route-cli", help="Session id used in route decision.")
    parser.add_argument(
        "--allow-overlay",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Allow selecting one overlay skill in route mode.",
    )
    parser.add_argument(
        "--first-turn",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Whether current task is the first turn for session-start boost.",
    )
    return parser


def _build_router_exec_command(*, codex_home: Path, argv: list[str] | None = None) -> list[str]:
    args = _build_route_cli_parser().parse_args(argv)
    if args.route_json and args.json:
        print("Error: choose either --json or --route-json.", file=sys.stderr)
        raise SystemExit(2)
    resolved_binary = _ensure_router_binary_current(codex_home)
    command = [
        str(resolved_binary),
        "--query",
        args.query,
        "--limit",
        str(args.limit),
        "--runtime",
        str(codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"),
        "--manifest",
        str(codex_home / "skills" / "SKILL_MANIFEST.json"),
    ]
    if args.json:
        command.append("--json")
    if args.route_json:
        command.extend(["--route-json", "--session-id", args.session_id])
        command.append(f"--allow-overlay={'true' if args.allow_overlay else 'false'}")
        command.append(f"--first-turn={'true' if args.first_turn else 'false'}")
    return command


def main(argv: list[str] | None = None) -> int:
    """Replace the current process with router-rs for the shared route CLI."""

    codex_home = _discover_codex_home(PROJECT_ROOT)
    command = _build_router_exec_command(codex_home=codex_home, argv=argv)
    try:
        os.execv(command[0], command)
    except OSError as exc:
        raise RuntimeError(f"router-rs route CLI exec failed: {exc}") from exc


if __name__ == "__main__":
    raise SystemExit(main())
