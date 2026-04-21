from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))


def test_legacy_codex_omx_hook_bridge_is_a_silent_noop(tmp_path: Path) -> None:
    script = PROJECT_ROOT / "scripts" / "codex_omx_hook_bridge.py"
    payload = json.dumps({"hook_event_name": "Stop", "cwd": str(tmp_path)}).encode("utf-8")

    result = subprocess.run(
        [
            "python3",
            str(script),
            "--repo-root",
            str(PROJECT_ROOT),
            "--omx-hook-script",
            "/definitely/missing.js",
        ],
        input=payload,
        capture_output=True,
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""
