#!/usr/bin/env python3
"""Compatibility wrapper that delegates Codex hook shaping to router-rs."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.rust_binary_runner import ensure_rust_binary


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CRATE_ROOT = PROJECT_ROOT / "scripts" / "router-rs"


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", required=True)
    parser.add_argument(
        "--event",
        required=True,
        choices=("permission-request", "pre-tool-use", "user-prompt-submit"),
    )
    return parser.parse_args()


def _ensure_binary() -> Path:
    return ensure_rust_binary(
        crate_root=CRATE_ROOT,
        binary_name="router-rs",
        release=False,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
    )


def main() -> int:
    args = _parse_args()
    binary_path = _ensure_binary()
    completed = subprocess.run(
        [
            str(binary_path),
            "--codex-hook-command",
            args.event,
            "--repo-root",
            str(Path(args.repo_root).resolve()),
        ],
        input=sys.stdin.read(),
        text=True,
        capture_output=True,
        check=False,
        cwd=PROJECT_ROOT,
    )
    if completed.stdout:
        sys.stdout.write(completed.stdout)
    if completed.stderr:
        sys.stderr.write(completed.stderr)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
