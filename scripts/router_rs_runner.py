#!/usr/bin/env python3
"""Stable launcher for router-rs commands used by host shells and hooks."""

from __future__ import annotations

import hashlib
import json
import socket
import subprocess
import sys
import tempfile
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from framework_runtime.rust_router import RustRouteAdapter
from scripts.rust_binary_runner import ensure_rust_binary


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CRATE_ROOT = PROJECT_ROOT / "scripts" / "router-rs"
HOT_DAEMON_IDLE_SECONDS = 900.0
HOT_DAEMON_CONNECT_TIMEOUT = 2.0
HOT_DAEMON_REQUEST_TIMEOUT = 30.0


@dataclass(slots=True)
class HotRequest:
    op: str
    payload: dict[str, Any]
    socket_path: Path


def _ensure_binary() -> Path:
    return ensure_rust_binary(
        crate_root=CRATE_ROOT,
        binary_name="router-rs",
        release=False,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
    )


def _option_value(argv: list[str], flag: str) -> str | None:
    try:
        index = argv.index(flag)
    except ValueError:
        return None
    next_index = index + 1
    if next_index >= len(argv):
        return None
    return argv[next_index]


def _socket_path(binary_path: Path, repo_root: Path) -> Path:
    binary_mtime = binary_path.stat().st_mtime_ns if binary_path.exists() else 0
    digest = hashlib.sha256(
        f"{repo_root.resolve()}|{binary_path.resolve()}|{binary_mtime}".encode("utf-8")
    ).hexdigest()[:20]
    return Path(tempfile.gettempdir()) / f"router-rs-hot-{digest}.sock"


def _parse_hot_request(argv: list[str], *, binary_path: Path) -> HotRequest | None:
    repo_root = Path(_option_value(argv, "--repo-root") or PROJECT_ROOT).resolve()
    max_lines_raw = _option_value(argv, "--claude-hook-max-lines")
    try:
        max_lines = int(max_lines_raw) if max_lines_raw is not None else 4
    except ValueError:
        max_lines = 4
    if "--framework-refresh-json" in argv:
        return HotRequest(
            op="framework_refresh",
            payload={
                "repo_root": str(repo_root),
                "max_lines": max_lines,
                "verbose": "--framework-refresh-verbose" in argv,
            },
            socket_path=_socket_path(binary_path, repo_root),
        )
    if "--framework-alias-json" not in argv:
        return None
    alias_name = _option_value(argv, "--framework-alias")
    if not alias_name:
        return None
    return HotRequest(
        op="framework_alias",
        payload={
            "repo_root": str(repo_root),
            "alias": alias_name,
            "max_lines": max_lines,
            "compact": "--compact-output" in argv,
            "host_id": _option_value(argv, "--framework-host-id") or "codex-cli",
        },
        socket_path=_socket_path(binary_path, repo_root),
    )


def _daemon_command(socket_path: Path) -> list[str]:
    return [
        sys.executable,
        str(Path(__file__).resolve()),
        "--router-stdio-daemon",
        str(socket_path),
    ]


def _spawn_hot_daemon(socket_path: Path) -> None:
    if socket_path.exists():
        socket_path.unlink()
    subprocess.Popen(
        _daemon_command(socket_path),
        cwd=PROJECT_ROOT,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
        close_fds=True,
    )


def _connect_hot_daemon(socket_path: Path) -> socket.socket:
    last_error: OSError | None = None
    for _ in range(20):
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        client.settimeout(HOT_DAEMON_CONNECT_TIMEOUT)
        try:
            client.connect(str(socket_path))
            return client
        except OSError as exc:
            client.close()
            last_error = exc
            time.sleep(0.05)
    if last_error is not None:
        raise RuntimeError(f"failed connecting router-rs hot daemon: {last_error}") from last_error
    raise RuntimeError("failed connecting router-rs hot daemon")


def _request_hot_daemon(request: HotRequest) -> dict[str, Any]:
    if not request.socket_path.exists():
        _spawn_hot_daemon(request.socket_path)
    try:
        client = _connect_hot_daemon(request.socket_path)
    except RuntimeError:
        _spawn_hot_daemon(request.socket_path)
        client = _connect_hot_daemon(request.socket_path)
    with client:
        client.settimeout(HOT_DAEMON_REQUEST_TIMEOUT)
        encoded = json.dumps(
            {
                "id": str(uuid.uuid4()),
                "op": request.op,
                "payload": request.payload,
            },
            ensure_ascii=False,
            allow_nan=False,
        ).encode("utf-8")
        client.sendall(encoded + b"\n")
        buffer = bytearray()
        while True:
            chunk = client.recv(65536)
            if not chunk:
                break
            buffer.extend(chunk)
            if b"\n" in chunk:
                break
        if not buffer:
            raise RuntimeError("router-rs hot daemon returned an empty response")
        line = buffer.split(b"\n", 1)[0].decode("utf-8")
    response = json.loads(line)
    if not isinstance(response, dict):
        raise RuntimeError("router-rs hot daemon returned a non-object response")
    if not response.get("ok"):
        raise RuntimeError(str(response.get("error") or "router-rs hot daemon request failed"))
    payload = response.get("payload")
    if not isinstance(payload, dict):
        raise RuntimeError("router-rs hot daemon returned a non-object payload")
    return payload


def _dispatch_hot_request(request: HotRequest, *, adapter: RustRouteAdapter) -> dict[str, Any]:
    if request.op == "framework_refresh":
        repo_root = Path(str(request.payload["repo_root"]))
        refresh_payload = adapter.framework_refresh(
            repo_root=repo_root,
            max_lines=int(request.payload.get("max_lines", 4)),
            verbose=bool(request.payload.get("verbose", False)),
        )
        return {
            "schema_version": adapter.framework_refresh_schema_version,
            "authority": adapter.framework_runtime_authority,
            "refresh": refresh_payload,
        }
    if request.op == "framework_alias":
        repo_root = Path(str(request.payload["repo_root"]))
        alias_payload = adapter.framework_alias(
            repo_root=repo_root,
            alias=str(request.payload["alias"]),
            max_lines=int(request.payload.get("max_lines", 4)),
            compact=bool(request.payload.get("compact", False)),
            host_id=str(request.payload.get("host_id") or "codex-cli"),
        )
        return {
            "schema_version": adapter.framework_alias_schema_version,
            "authority": adapter.framework_runtime_authority,
            "alias": alias_payload,
        }
    raise RuntimeError(f"unsupported hot request op: {request.op}")


def _run_hot_request(argv: list[str], *, binary_path: Path) -> dict[str, Any] | None:
    request = _parse_hot_request(argv, binary_path=binary_path)
    if request is None:
        return None
    return _request_hot_daemon(request)


def _run_one_shot(argv: list[str], *, binary_path: Path) -> int:
    completed = subprocess.run([str(binary_path), *argv], cwd=PROJECT_ROOT)
    return completed.returncode


def _router_stdio_daemon_main(socket_path: Path) -> int:
    if socket_path.exists():
        socket_path.unlink()
    adapter = RustRouteAdapter(PROJECT_ROOT)
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        server.bind(str(socket_path))
        server.listen()
        server.settimeout(HOT_DAEMON_IDLE_SECONDS)
        while True:
            try:
                conn, _ = server.accept()
            except socket.timeout:
                return 0
            with conn:
                conn.settimeout(HOT_DAEMON_REQUEST_TIMEOUT)
                buffer = bytearray()
                while True:
                    chunk = conn.recv(65536)
                    if not chunk:
                        break
                    buffer.extend(chunk)
                    if b"\n" in chunk:
                        break
                request_id: Any = None
                try:
                    line = buffer.split(b"\n", 1)[0].decode("utf-8")
                    request = json.loads(line)
                    if not isinstance(request, dict):
                        raise RuntimeError("request must be a JSON object")
                    request_id = request.get("id")
                    payload = request.get("payload")
                    if not isinstance(payload, dict):
                        raise RuntimeError("request payload must be a JSON object")
                    hot_request = HotRequest(
                        op=str(request.get("op") or ""),
                        payload=payload,
                        socket_path=socket_path,
                    )
                    response_payload = _dispatch_hot_request(hot_request, adapter=adapter)
                    response = {
                        "id": request_id,
                        "ok": True,
                        "payload": response_payload,
                    }
                except Exception as exc:  # pragma: no cover
                    response = {
                        "id": request_id,
                        "ok": False,
                        "error": str(exc),
                    }
                conn.sendall(json.dumps(response, ensure_ascii=False).encode("utf-8") + b"\n")
    finally:
        server.close()
        if socket_path.exists():
            socket_path.unlink()


def main() -> int:
    if len(sys.argv) >= 3 and sys.argv[1] == "--router-stdio-daemon":
        return _router_stdio_daemon_main(Path(sys.argv[2]))
    binary_path = _ensure_binary()
    argv = sys.argv[1:]
    try:
        hot_payload = _run_hot_request(argv, binary_path=binary_path)
    except Exception:
        hot_payload = None
    if hot_payload is not None:
        print(json.dumps(hot_payload, ensure_ascii=False, indent=2))
        return 0
    return _run_one_shot(argv, binary_path=binary_path)


if __name__ == "__main__":
    raise SystemExit(main())
