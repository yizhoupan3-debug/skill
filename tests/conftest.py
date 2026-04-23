from __future__ import annotations

from pathlib import Path
import subprocess


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ROUTER_RS_ROOT = PROJECT_ROOT / "scripts" / "router-rs"
ROUTER_RS_DEBUG_BIN = ROUTER_RS_ROOT / "target" / "debug" / "router-rs"
ROUTER_RS_RELEASE_BIN = ROUTER_RS_ROOT / "target" / "release" / "router-rs"


def _latest_router_rs_source_mtime() -> float:
    candidates = [ROUTER_RS_ROOT / "Cargo.toml", *ROUTER_RS_ROOT.joinpath("src").rglob("*.rs")]
    return max((path.stat().st_mtime for path in candidates if path.exists()), default=0.0)


def _freshest_router_rs_binary_mtime() -> float | None:
    mtimes = [
        path.stat().st_mtime
        for path in (ROUTER_RS_DEBUG_BIN, ROUTER_RS_RELEASE_BIN)
        if path.is_file()
    ]
    return max(mtimes) if mtimes else None


def _ensure_router_rs_binary_fresh() -> None:
    if not ROUTER_RS_ROOT.exists():
        return
    latest_source_mtime = _latest_router_rs_source_mtime()
    binary_mtime = _freshest_router_rs_binary_mtime()
    if binary_mtime is not None and binary_mtime >= latest_source_mtime:
        return
    subprocess.run(
        ["cargo", "build", "--manifest-path", str(ROUTER_RS_ROOT / "Cargo.toml")],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )


def pytest_sessionstart(session) -> None:
    _ensure_router_rs_binary_fresh()
