from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]


def test_repo_root_package_bridge_supports_direct_import_without_pythonpath() -> None:
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    command = [
        sys.executable,
        "-c",
        (
            "from codex_agno_runtime import RuntimeSettings; "
            "import codex_agno_runtime, codex_agno_runtime.config as config; "
            "print(RuntimeSettings.__name__); "
            "print(codex_agno_runtime.__file__); "
            "print(config.__file__)"
        ),
    ]
    proc = subprocess.run(
        command,
        cwd=PROJECT_ROOT,
        env=env,
        capture_output=True,
        text=True,
    )

    assert proc.returncode == 0, proc.stderr
    stdout_lines = proc.stdout.strip().splitlines()
    assert stdout_lines[0] == "RuntimeSettings"
    assert stdout_lines[1].startswith(str(PROJECT_ROOT / "codex_agno_runtime" / "__init__.py"))
    assert str(PROJECT_ROOT / "codex_agno_runtime" / "src" / "codex_agno_runtime") in stdout_lines[2]
