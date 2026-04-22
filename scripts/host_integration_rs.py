#!/usr/bin/env python3
"""Thin Python bridge for the Rust host-integration helper."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CRATE_ROOT = PROJECT_ROOT / "scripts" / "host-integration-rs"
MANIFEST_PATH = CRATE_ROOT / "Cargo.toml"
LOCK_PATH = CRATE_ROOT / "Cargo.lock"
RELEASE_BINARY_PATH = CRATE_ROOT / "target" / "release" / "host-integration-rs"


def _latest_source_mtime() -> float:
    candidates = [
        MANIFEST_PATH,
        LOCK_PATH,
        *CRATE_ROOT.joinpath("src").rglob("*.rs"),
    ]
    return max((path.stat().st_mtime for path in candidates if path.is_file()), default=0.0)


def _ensure_binary() -> Path:
    if RELEASE_BINARY_PATH.is_file():
        if RELEASE_BINARY_PATH.stat().st_mtime < _latest_source_mtime():
            raise RuntimeError(
                "host-integration-rs prebuilt release binary is stale; rebuild scripts/host-integration-rs before invoking the Python bridge."
            )
        return RELEASE_BINARY_PATH
    raise RuntimeError(
        "host-integration-rs requires a prebuilt release binary; build scripts/host-integration-rs before invoking the Python bridge."
    )


def run_host_integration_rs(*args: str) -> dict[str, Any]:
    """Run the Rust host-integration helper and decode its JSON payload."""

    binary_path = _ensure_binary()
    completed = subprocess.run(
        [str(binary_path), *args],
        cwd=PROJECT_ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    return json.loads(completed.stdout)


def main() -> int:
    payload = run_host_integration_rs(*sys.argv[1:])
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
