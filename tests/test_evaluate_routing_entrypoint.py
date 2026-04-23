from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = PROJECT_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import evaluate_routing as evaluate_routing_script


def _touch(path: Path, *, mtime: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("", encoding="utf-8")
    os.utime(path, (mtime, mtime))


def test_build_routing_eval_exec_command_uses_router_rs_and_default_paths(tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    _touch(router_dir / "Cargo.toml", mtime=100.0)
    _touch(router_dir / "src" / "main.rs", mtime=100.0)
    binary_path = router_dir / "target" / "release" / "router-rs"
    _touch(binary_path, mtime=200.0)

    command = evaluate_routing_script._build_routing_eval_exec_command(
        codex_home=codex_home,
        argv=[],
    )

    assert command == [
        str(binary_path),
        "--routing-eval-json",
        "--runtime",
        str(codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"),
        "--manifest",
        str(codex_home / "skills" / "SKILL_MANIFEST.json"),
        "--cases",
        str(codex_home / "tests" / "routing_eval_cases.json"),
    ]


def test_build_routing_eval_exec_command_allows_custom_skills_root_and_cases(tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    _touch(router_dir / "Cargo.toml", mtime=100.0)
    _touch(router_dir / "src" / "main.rs", mtime=100.0)
    binary_path = router_dir / "target" / "debug" / "router-rs"
    _touch(binary_path, mtime=200.0)
    skills_root = tmp_path / "custom-skills"
    cases_path = tmp_path / "custom-cases.json"

    command = evaluate_routing_script._build_routing_eval_exec_command(
        codex_home=codex_home,
        argv=["--skills-root", str(skills_root), "--cases", str(cases_path)],
    )

    assert command == [
        str(binary_path),
        "--routing-eval-json",
        "--runtime",
        str(skills_root / "SKILL_ROUTING_RUNTIME.json"),
        "--manifest",
        str(skills_root / "SKILL_MANIFEST.json"),
        "--cases",
        str(cases_path),
    ]


def test_main_execs_router_rs_directly_for_routing_eval(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    _touch(router_dir / "Cargo.toml", mtime=100.0)
    _touch(router_dir / "src" / "main.rs", mtime=100.0)
    binary_path = router_dir / "target" / "release" / "router-rs"
    _touch(binary_path, mtime=200.0)

    calls: list[tuple[str, list[str]]] = []

    monkeypatch.setattr(evaluate_routing_script.route_cli, "_discover_codex_home", lambda _: codex_home)
    monkeypatch.setattr(
        evaluate_routing_script.os,
        "execv",
        lambda path, argv: calls.append((path, list(argv))),
    )

    result = evaluate_routing_script.main(["--cases", str(codex_home / "my-cases.json")])

    assert result is None
    assert calls == [
        (
            str(binary_path),
            [
                str(binary_path),
                "--routing-eval-json",
                "--runtime",
                str(codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"),
                "--manifest",
                str(codex_home / "skills" / "SKILL_MANIFEST.json"),
                "--cases",
                str(codex_home / "my-cases.json"),
            ],
        )
    ]
