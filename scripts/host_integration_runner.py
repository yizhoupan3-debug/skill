#!/usr/bin/env python3
"""Shared launcher for the Rust host-integration helper."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

from scripts.rust_binary_runner import ensure_rust_binary


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CRATE_ROOT = PROJECT_ROOT / "scripts" / "host-integration-rs"


def ensure_host_integration_binary() -> Path:
    """Build or reuse the Rust host-integration binary."""

    return ensure_rust_binary(
        crate_root=CRATE_ROOT,
        binary_name="host-integration-rs",
        release=True,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
    )


def run_host_integration(*args: str, cwd: Path | None = None) -> dict[str, Any]:
    """Execute one host-integration command and require an object JSON response."""

    binary_path = ensure_host_integration_binary()
    completed = subprocess.run(
        [str(binary_path), *args],
        cwd=cwd or PROJECT_ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)
    if not isinstance(payload, dict):
        raise ValueError("Rust host-integration response must be a JSON object.")
    return payload
