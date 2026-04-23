#!/usr/bin/env python3
"""Write framework-profile adapter and compatibility artifacts."""

from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from framework_runtime.rust_router import (
    discover_codex_home,
    run_framework_contract_artifacts_cli,
)

CODEX_HOME = discover_codex_home(PROJECT_ROOT)


def main() -> int:
    return run_framework_contract_artifacts_cli(codex_home=CODEX_HOME)


if __name__ == "__main__":
    raise SystemExit(main())
