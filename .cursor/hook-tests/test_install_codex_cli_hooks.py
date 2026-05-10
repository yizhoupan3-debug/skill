#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
INSTALLER = ROOT / "scripts" / "install_codex_cli_hooks.sh"


def assert_true(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def run_installer(codex_home: Path) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["CODEX_HOME"] = str(codex_home)
    return subprocess.run(
        ["/usr/bin/env", "bash", str(INSTALLER)],
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
        timeout=180,
    )


def test_preserves_existing_event_hooks() -> None:
    with tempfile.TemporaryDirectory(prefix="codex-hook-install-") as tmp:
        codex_home = Path(tmp)
        hooks_path = codex_home / "hooks.json"
        hooks_path.write_text(
            json.dumps(
                {
                    "hooks": {
                        "Stop": [
                            {
                                "hooks": [
                                    {
                                        "type": "command",
                                        "command": "/usr/bin/env echo existing",
                                        "timeout": 5,
                                        "statusMessage": "existing",
                                    }
                                ]
                            }
                        ]
                    }
                },
                ensure_ascii=True,
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        result = run_installer(codex_home)
        assert_true(result.returncode == 0, f"installer failed: {result.stderr}")

        data = json.loads(hooks_path.read_text(encoding="utf-8"))
        stop_entries = data["hooks"]["Stop"]
        commands = []
        for entry in stop_entries:
            for hook in entry.get("hooks", []):
                if isinstance(hook, dict):
                    commands.append(hook.get("command"))
        assert_true("/usr/bin/env echo existing" in commands, "existing stop hook should be preserved")
        router_hooks = [
            c
            for c in commands
            if isinstance(c, str) and "codex hook --event=Stop" in c
        ]
        assert_true(len(router_hooks) == 1, "expected exactly one managed Stop command hook")
        gate_cmd = router_hooks[0]
        assert_true(
            "git rev-parse --show-toplevel" in gate_cmd,
            "hook should resolve repo root at runtime, not embed install-time path only",
        )
        assert_true(
            'ROUTER_RS_BIN=""; if [ -x "$CODEX_PROJECT_ROOT/scripts/router-rs/target/release/router-rs"'
            in gate_cmd,
            "hook should prefer in-repo router-rs (release first) before PATH",
        )
        assert_true(
            "exit 1" in gate_cmd and "fail-closed" in gate_cmd,
            "hook should fail-closed when router-rs is missing",
        )


def test_updates_features_scoped_codex_hooks_only() -> None:
    with tempfile.TemporaryDirectory(prefix="codex-hook-config-") as tmp:
        codex_home = Path(tmp)
        config_path = codex_home / "config.toml"
        config_path.write_text(
            (
                "[custom]\n"
                "codex_hooks = false\n\n"
                "[features]\n"
                "other_flag = true\n"
            ),
            encoding="utf-8",
        )
        result = run_installer(codex_home)
        assert_true(result.returncode == 0, f"installer failed: {result.stderr}")

        text = config_path.read_text(encoding="utf-8")
        assert_true("[custom]\ncodex_hooks = false" in text, "non-features codex_hooks should be untouched")
        assert_true("[features]" in text and "hooks = true" in text, "features hooks should be enabled")
        assert_true("codex_hooks = true" not in text, "deprecated features codex_hooks should not be emitted")


def main() -> int:
    test_preserves_existing_event_hooks()
    test_updates_features_scoped_codex_hooks_only()
    print("install codex cli hooks tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
