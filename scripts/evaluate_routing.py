#!/usr/bin/env python3
"""Direct Rust routing-eval entrypoint with fail-closed binary freshness checks."""

from __future__ import annotations

import argparse
import os
from pathlib import Path

import route as route_cli

PROJECT_ROOT = Path(__file__).resolve().parents[1]


def _build_routing_eval_cli_parser(*, codex_home: Path) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run offline routing evaluation cases.")
    parser.add_argument(
        "--skills-root",
        type=Path,
        default=codex_home / "skills",
        help="Skill root path.",
    )
    parser.add_argument(
        "--cases",
        type=Path,
        default=codex_home / "tests" / "routing_eval_cases.json",
        help="Routing eval case file.",
    )
    return parser


def _build_routing_eval_exec_command(*, codex_home: Path, argv: list[str] | None = None) -> list[str]:
    args = _build_routing_eval_cli_parser(codex_home=codex_home).parse_args(argv)
    resolved_binary = route_cli._ensure_router_binary_current(codex_home)
    return [
        str(resolved_binary),
        "--routing-eval-json",
        "--runtime",
        str(args.skills_root / "SKILL_ROUTING_RUNTIME.json"),
        "--manifest",
        str(args.skills_root / "SKILL_MANIFEST.json"),
        "--cases",
        str(args.cases),
    ]


def main(argv: list[str] | None = None) -> int:
    """Replace the current process with router-rs for offline routing eval."""

    codex_home = route_cli._discover_codex_home(PROJECT_ROOT)
    command = _build_routing_eval_exec_command(codex_home=codex_home, argv=argv)
    try:
        os.execv(command[0], command)
    except OSError as exc:
        raise RuntimeError(f"router-rs routing eval CLI exec failed: {exc}") from exc


if __name__ == "__main__":
    raise SystemExit(main())
