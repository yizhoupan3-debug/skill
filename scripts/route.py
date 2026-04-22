#!/usr/bin/env python3
"""Rust-first skill lookup and route transport shim."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = ROOT / "codex_agno_runtime" / "src"
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.rust_router import (
    discover_codex_home,
    run_route_cli,
)
CODEX_HOME = discover_codex_home(ROOT)


def main() -> None:
    """Run the shared route CLI for this repository."""

    raise SystemExit(run_route_cli(codex_home=CODEX_HOME))


if __name__ == "__main__":
    main()
