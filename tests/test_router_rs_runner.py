from __future__ import annotations

import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = PROJECT_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import router_rs_runner


def test_parse_hot_request_for_framework_refresh(tmp_path: Path) -> None:
    binary_path = tmp_path / "router-rs"
    binary_path.write_text("", encoding="utf-8")

    request = router_rs_runner._parse_hot_request(
        [
            "--framework-refresh-json",
            "--claude-hook-max-lines",
            "6",
            "--repo-root",
            str(tmp_path),
        ],
        binary_path=binary_path,
    )

    assert request is not None
    assert request.op == "framework_refresh"
    assert request.payload["max_lines"] == 6
    assert request.payload["verbose"] is False
    assert request.payload["repo_root"] == str(tmp_path.resolve())
    assert request.socket_path.name.startswith("router-rs-hot-")


def test_parse_hot_request_for_framework_alias(tmp_path: Path) -> None:
    binary_path = tmp_path / "router-rs"
    binary_path.write_text("", encoding="utf-8")

    request = router_rs_runner._parse_hot_request(
        [
            "--framework-alias-json",
            "--framework-alias",
            "autopilot",
            "--compact-output",
            "--claude-hook-max-lines",
            "3",
            "--repo-root",
            str(tmp_path),
        ],
        binary_path=binary_path,
    )

    assert request is not None
    assert request.op == "framework_alias"
    assert request.payload["alias"] == "autopilot"
    assert request.payload["compact"] is True
    assert request.payload["max_lines"] == 3
    assert request.payload["host_id"] == "codex-cli"
    assert request.payload["repo_root"] == str(tmp_path.resolve())
    assert request.socket_path.name.startswith("router-rs-hot-")


def test_parse_hot_request_for_framework_alias_with_explicit_host_id(tmp_path: Path) -> None:
    binary_path = tmp_path / "router-rs"
    binary_path.write_text("", encoding="utf-8")

    request = router_rs_runner._parse_hot_request(
        [
            "--framework-alias-json",
            "--framework-alias",
            "team",
            "--framework-host-id",
            "claude-code",
            "--repo-root",
            str(tmp_path),
        ],
        binary_path=binary_path,
    )

    assert request is not None
    assert request.payload["alias"] == "team"
    assert request.payload["host_id"] == "claude-code"


def test_dispatch_hot_request_wraps_framework_alias_envelope(tmp_path: Path) -> None:
    request = router_rs_runner.HotRequest(
        op="framework_alias",
        payload={
            "repo_root": str(tmp_path),
            "alias": "deepinterview",
            "max_lines": 5,
            "compact": True,
            "host_id": "claude-code",
        },
        socket_path=tmp_path / "router-rs.sock",
    )

    calls: dict[str, object] = {}

    class FakeAdapter:
        framework_alias_schema_version = "router-rs-framework-alias-v1"
        framework_runtime_authority = "rust-framework-runtime-read-model"

        def framework_alias(
            self,
            *,
            repo_root: Path,
            alias: str,
            max_lines: int,
            compact: bool,
            host_id: str,
        ) -> dict[str, object]:
            calls["repo_root"] = repo_root
            calls["alias"] = alias
            calls["max_lines"] = max_lines
            calls["compact"] = compact
            calls["host_id"] = host_id
            return {"ok": True, "name": alias, "compact": compact}

    response = router_rs_runner._dispatch_hot_request(request, adapter=FakeAdapter())

    assert response["schema_version"] == "router-rs-framework-alias-v1"
    assert response["authority"] == "rust-framework-runtime-read-model"
    assert response["alias"]["name"] == "deepinterview"
    assert calls == {
        "repo_root": tmp_path,
        "alias": "deepinterview",
        "max_lines": 5,
        "compact": True,
        "host_id": "claude-code",
    }


def test_parse_hot_request_for_framework_refresh_verbose(tmp_path: Path) -> None:
    binary_path = tmp_path / "router-rs"
    binary_path.write_text("", encoding="utf-8")

    request = router_rs_runner._parse_hot_request(
        [
            "--framework-refresh-json",
            "--framework-refresh-verbose",
            "--claude-hook-max-lines",
            "6",
            "--repo-root",
            str(tmp_path),
        ],
        binary_path=binary_path,
    )

    assert request is not None
    assert request.payload["verbose"] is True


def test_dispatch_hot_request_wraps_framework_refresh_envelope(tmp_path: Path) -> None:
    request = router_rs_runner.HotRequest(
        op="framework_refresh",
        payload={
            "repo_root": str(tmp_path),
            "max_lines": 6,
            "verbose": True,
        },
        socket_path=tmp_path / "router-rs.sock",
    )

    calls: dict[str, object] = {}

    class FakeAdapter:
        framework_refresh_schema_version = "router-rs-framework-refresh-v1"
        framework_runtime_authority = "rust-framework-runtime-read-model"

        def framework_refresh(
            self,
            *,
            repo_root: Path,
            max_lines: int,
            verbose: bool,
        ) -> dict[str, object]:
            calls["repo_root"] = repo_root
            calls["max_lines"] = max_lines
            calls["verbose"] = verbose
            return {"ok": True, "confirmation": "下一轮执行 prompt 已准备好，并且已经复制到剪贴板。", "verbose": verbose}

    response = router_rs_runner._dispatch_hot_request(request, adapter=FakeAdapter())

    assert response["schema_version"] == "router-rs-framework-refresh-v1"
    assert response["authority"] == "rust-framework-runtime-read-model"
    assert response["refresh"]["ok"] is True
    assert calls == {
        "repo_root": tmp_path,
        "max_lines": 6,
        "verbose": True,
    }
