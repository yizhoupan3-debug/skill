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
RELEASE_BINARY_PATH = CRATE_ROOT / "target" / "release" / "host-integration-rs"

def _ensure_binary() -> Path:
    if RELEASE_BINARY_PATH.is_file():
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
