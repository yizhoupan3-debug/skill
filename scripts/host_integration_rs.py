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


def _rust_sources() -> list[Path]:
    return [
        MANIFEST_PATH,
        CRATE_ROOT / "Cargo.lock",
        *sorted((CRATE_ROOT / "src").glob("**/*.rs")),
    ]


def _binary_is_fresh(binary_path: Path) -> bool:
    if not binary_path.is_file():
        return False
    binary_mtime = binary_path.stat().st_mtime
    return all(not source.exists() or source.stat().st_mtime <= binary_mtime for source in _rust_sources())


def _ensure_binary() -> Path:
    if _binary_is_fresh(RELEASE_BINARY_PATH):
        return RELEASE_BINARY_PATH
    subprocess.run(
        ["cargo", "build", "--release", "--quiet", "--manifest-path", str(MANIFEST_PATH)],
        cwd=PROJECT_ROOT,
        check=True,
        text=True,
    )
    return RELEASE_BINARY_PATH


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
