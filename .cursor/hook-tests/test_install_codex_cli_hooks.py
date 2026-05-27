#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def assert_true(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def run_installer(codex_home: Path) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["CODEX_HOME"] = str(codex_home)
    env.setdefault("HOME", str(codex_home.parent))
    return subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "--manifest-path",
            str(ROOT / "core/router-rs/Cargo.toml"),
            "--",
            "framework",
            "maint",
            "install-codex-user-hooks",
            "--codex-home",
            str(codex_home),
        ],
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
        timeout=900,
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
        def is_managed_router_stop(command: object) -> bool:
            if not isinstance(command, str):
                return False
            if "codex hook --event=Stop" in command:
                return True
            return "codex-router-rs-hook.sh" in command and " Stop" in command

        router_hooks = [c for c in commands if is_managed_router_stop(c)]
        assert_true(len(router_hooks) == 1, "expected exactly one managed Stop command hook")
        gate_cmd = router_hooks[0]
        assert_true(
            "git rev-parse --show-toplevel" in gate_cmd
            or "CODEX_PROJECT_ROOT" in gate_cmd
            or "SKILL_FRAMEWORK_ROOT" in gate_cmd,
            "hook should resolve repo root at runtime, not embed install-time path only",
        )
        if "codex hook --event=Stop" in gate_cmd:
            assert_true(
                'ROUTER_RS_BIN=""; if [ -x "$CODEX_PROJECT_ROOT/core/router-rs/target/release/router-rs"'
                in gate_cmd,
                "legacy inline hook should prefer in-repo router-rs before PATH",
            )
            assert_true(
                "exit 1" in gate_cmd and "fail-closed" in gate_cmd,
                "legacy inline hook should fail-closed when router-rs is missing",
            )
        else:
            assert_true(
                "codex-router-rs-hook.sh" in gate_cmd,
                "steady-state installer should use codex-router-rs-hook.sh launcher",
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
