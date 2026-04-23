#!/usr/bin/env python3
"""Thin Python bridge for the Rust host-integration helper."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.rust_binary_runner import ensure_rust_binary


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CRATE_ROOT = PROJECT_ROOT / "scripts" / "host-integration-rs"


def _ensure_binary() -> Path:
    return ensure_rust_binary(
        crate_root=CRATE_ROOT,
        binary_name="host-integration-rs",
        release=True,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
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


def export_runtime_registry(repo_root: Path | None = None) -> dict[str, Any]:
    """Return the Rust-owned runtime registry payload for one repository root."""

    payload = run_host_integration_rs(
        "export-runtime-registry",
        "--repo-root",
        str((repo_root or PROJECT_ROOT).resolve()),
    )
    if not isinstance(payload, dict):
        raise ValueError("Rust runtime registry export must be a JSON object.")
    return payload


def main() -> int:
    payload = run_host_integration_rs(*sys.argv[1:])
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
