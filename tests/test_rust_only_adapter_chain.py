from __future__ import annotations

from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def test_retired_python_adapter_bridges_stay_removed() -> None:
    retired_paths = (
        PROJECT_ROOT / "scripts" / "route.py",
        PROJECT_ROOT / "scripts" / "router_rs_runner.py",
        PROJECT_ROOT / "scripts" / "codex_omx_hook_bridge.py",
        PROJECT_ROOT / "scripts" / "rust_binary_runner",
        PROJECT_ROOT / "scripts" / "rust_binary_runner.py",
    )

    assert [path for path in retired_paths if path.exists()] == []


def test_framework_runtime_stays_off_python_bridge_helpers() -> None:
    runtime_registry_source = (
        PROJECT_ROOT / "framework_runtime" / "src" / "framework_runtime" / "runtime_registry.py"
    ).read_text(encoding="utf-8")
    rust_router_source = (
        PROJECT_ROOT / "framework_runtime" / "src" / "framework_runtime" / "rust_router.py"
    ).read_text(encoding="utf-8")

    assert "scripts.host_integration_runner" not in runtime_registry_source
    assert "scripts.rust_binary_runner" not in rust_router_source
