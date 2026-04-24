from __future__ import annotations

import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def test_framework_runtime_python_package_stays_removed() -> None:
    assert not (PROJECT_ROOT / "framework_runtime").exists()

    proc = subprocess.run(
        [sys.executable, "-c", "import framework_runtime"],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
    )

    assert proc.returncode != 0
    assert "ModuleNotFoundError" in proc.stderr
