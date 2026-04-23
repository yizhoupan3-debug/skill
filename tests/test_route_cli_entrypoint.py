from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = PROJECT_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import route as route_script


def _touch(path: Path, *, mtime: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("", encoding="utf-8")
    os.utime(path, (mtime, mtime))


def test_build_router_exec_command_injects_runtime_manifest_and_route_flags(tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    runtime_path = codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"
    manifest_path = codex_home / "skills" / "SKILL_MANIFEST.json"
    _touch(router_dir / "Cargo.toml", mtime=100.0)
    _touch(router_dir / "src" / "main.rs", mtime=100.0)
    binary_path = router_dir / "target" / "release" / "router-rs"
    _touch(binary_path, mtime=200.0)
    runtime_path.parent.mkdir(parents=True, exist_ok=True)
    runtime_path.write_text("{}", encoding="utf-8")
    manifest_path.write_text("{}", encoding="utf-8")

    command = route_script._build_router_exec_command(
        codex_home=codex_home,
        argv=[
            "--query",
            "typed first route cli",
            "--limit",
            "7",
            "--route-json",
            "--session-id",
            "route-cli-test",
            "--no-allow-overlay",
            "--no-first-turn",
        ],
    )

    assert command == [
        str(binary_path),
        "--query",
        "typed first route cli",
        "--limit",
        "7",
        "--runtime",
        str(runtime_path),
        "--manifest",
        str(manifest_path),
        "--route-json",
        "--session-id",
        "route-cli-test",
        "--allow-overlay=false",
        "--first-turn=false",
    ]


def test_build_router_exec_command_prefers_typed_search_json_mode(tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    _touch(router_dir / "Cargo.toml", mtime=100.0)
    _touch(router_dir / "src" / "main.rs", mtime=100.0)
    binary_path = router_dir / "target" / "debug" / "router-rs"
    _touch(binary_path, mtime=200.0)

    command = route_script._build_router_exec_command(
        codex_home=codex_home,
        argv=["--query", "typed first search", "--json"],
    )

    assert command == [
        str(binary_path),
        "--query",
        "typed first search",
        "--limit",
        "5",
        "--runtime",
        str(codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"),
        "--manifest",
        str(codex_home / "skills" / "SKILL_MANIFEST.json"),
        "--json",
    ]


def test_build_router_exec_command_requires_prebuilt_router_binary(tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    _touch(router_dir / "Cargo.toml", mtime=100.0)
    _touch(router_dir / "src" / "main.rs", mtime=100.0)

    with pytest.raises(RuntimeError, match="requires a prebuilt binary"):
        route_script._build_router_exec_command(
            codex_home=codex_home,
            argv=["--query", "typed first route cli"],
        )


def test_build_router_exec_command_rejects_stale_router_binary(tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    _touch(router_dir / "Cargo.toml", mtime=200.0)
    _touch(router_dir / "src" / "main.rs", mtime=300.0)
    _touch(router_dir / "target" / "release" / "router-rs", mtime=100.0)

    with pytest.raises(RuntimeError, match="prebuilt binary is stale"):
        route_script._build_router_exec_command(
            codex_home=codex_home,
            argv=["--query", "typed first route cli"],
        )


def test_build_router_exec_command_rejects_conflicting_json_modes(capsys: pytest.CaptureFixture[str], tmp_path: Path) -> None:
    codex_home = tmp_path

    with pytest.raises(SystemExit) as exc_info:
        route_script._build_router_exec_command(
            codex_home=codex_home,
            argv=["--query", "typed first route cli", "--json", "--route-json"],
        )

    assert exc_info.value.code == 2
    assert capsys.readouterr().err.strip() == "Error: choose either --json or --route-json."


def test_main_execs_router_rs_directly_without_importing_runtime(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    codex_home = tmp_path
    router_dir = codex_home / "scripts" / "router-rs"
    _touch(router_dir / "Cargo.toml", mtime=100.0)
    _touch(router_dir / "src" / "main.rs", mtime=100.0)
    binary_path = router_dir / "target" / "release" / "router-rs"
    _touch(binary_path, mtime=200.0)

    calls: list[tuple[str, list[str]]] = []

    monkeypatch.setattr(route_script, "_discover_codex_home", lambda _: codex_home)
    monkeypatch.setattr(route_script.os, "execv", lambda path, argv: calls.append((path, list(argv))))

    result = route_script.main(["--query", "direct rust route cli", "--route-json"])

    assert result is None
    assert calls == [
        (
            str(binary_path),
            [
                str(binary_path),
                "--query",
                "direct rust route cli",
                "--limit",
                "5",
                "--runtime",
                str(codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"),
                "--manifest",
                str(codex_home / "skills" / "SKILL_MANIFEST.json"),
                "--route-json",
                "--session-id",
                "route-cli",
                "--allow-overlay=true",
                "--first-turn=true",
            ],
        )
    ]
