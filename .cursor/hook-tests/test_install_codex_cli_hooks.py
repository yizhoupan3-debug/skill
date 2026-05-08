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
ROUTER_RS_LAUNCHER = ROOT / "scripts" / "router-rs" / "run_router_rs.sh"
ROUTER_RS_MANIFEST = ROOT / "scripts" / "router-rs" / "Cargo.toml"


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
        timeout=20,
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
        expected_command = (
            f"/usr/bin/env bash -lc '"
            f'CODEX_PROJECT_ROOT="${{CODEX_PROJECT_ROOT:-{ROOT.as_posix()}}}"; '
            f'ROUTER_RS_LAUNCHER="$CODEX_PROJECT_ROOT/scripts/router-rs/run_router_rs.sh"; '
            f'ROUTER_RS_MANIFEST="$CODEX_PROJECT_ROOT/scripts/router-rs/Cargo.toml"; '
            f'if [ ! -x "$ROUTER_RS_LAUNCHER" ]; then exit 0; fi; '
            f'"$ROUTER_RS_LAUNCHER" "$ROUTER_RS_MANIFEST" codex hook review-subagent-gate '
            f'--repo-root "$CODEX_PROJECT_ROOT"'
            f"'"
        )
        assert_true(expected_command in commands, "installer hook should be added")


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
        assert_true("[features]" in text and "codex_hooks = true" in text, "features codex_hooks should be enabled")


def main() -> int:
    test_preserves_existing_event_hooks()
    test_updates_features_scoped_codex_hooks_only()
    print("install codex cli hooks tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
