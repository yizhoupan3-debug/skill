from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.framework_profile import FrameworkProfile, build_framework_profile
from framework_runtime.host_adapters import (
    compile_claude_code_adapter,
    compile_cli_common_adapter,
    compile_codex_cli_adapter,
    compile_codex_common_adapter,
    compile_codex_desktop_adapter,
    compile_gemini_cli_adapter,
)
from framework_runtime.rust_router import RustRouteAdapter

FIXTURE_PATH = PROJECT_ROOT / "tests" / "framework_profile_field_boundary_fixture.json"
FIXTURE = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def _router_rs_command() -> list[str]:
    return RustRouteAdapter(PROJECT_ROOT)._binary_command()


def _fixture_profile() -> FrameworkProfile:
    return build_framework_profile(
        profile_id="field-boundary-fixture",
        display_name="Field Boundary Fixture",
        rules_bundle={"rules": [{"id": "outer-owned"}]},
        skill_bundle={"skills": ["router", "memory-bridge"]},
        session_policy={"mode": "bounded", "approval_mode": "manual"},
        artifact_contract={"layout": "stable-v1"},
        model_policy={"provider": "openai", "model": "gpt-5"},
        memory_mounts=("project",),
        mcp_servers=("local-memory",),
    )


def _extract_boundary(payload: dict[str, object]) -> dict[str, object]:
    return {
        "bridge_contract": payload.get("bridge_contract"),
        "source_contract": payload.get("source_contract"),
    }


def _python_default_boundary_fixture(profile: FrameworkProfile) -> dict[str, object]:
    return {
        "cli_common_adapter": _extract_boundary(compile_cli_common_adapter(profile).host_payload),
        "codex_common_adapter": _extract_boundary(compile_codex_common_adapter(profile).host_payload),
        "codex_desktop_adapter": _extract_boundary(compile_codex_desktop_adapter(profile).host_payload),
        "codex_cli_adapter": _extract_boundary(compile_codex_cli_adapter(profile).host_payload),
        "claude_code_adapter": _extract_boundary(compile_claude_code_adapter(profile).host_payload),
        "gemini_cli_adapter": _extract_boundary(compile_gemini_cli_adapter(profile).host_payload),
    }


def _router_rs_boundary_fixture(
    profile: FrameworkProfile,
    *,
    include_legacy_alias_artifact: bool,
) -> dict[str, object]:
    with tempfile.TemporaryDirectory() as tmpdir:
        profile_path = Path(tmpdir) / "framework_profile.json"
        profile_path.write_text(json.dumps(profile.to_dict(), ensure_ascii=False), encoding="utf-8")
        argv = [
            *_router_rs_command(),
            "--profile-json",
            "--framework-profile",
            str(profile_path),
        ]
        if include_legacy_alias_artifact:
            argv.insert(2, "--include-legacy-alias-artifact")
        proc = subprocess.run(
            argv,
            check=True,
            capture_output=True,
            text=True,
            cwd=PROJECT_ROOT,
        )

    payload = json.loads(proc.stdout)
    boundaries: dict[str, object] = {
        "default": {
            "cli_common_adapter": _extract_boundary(payload["cli_common_adapter"]),
            "codex_common_adapter": _extract_boundary(payload["codex_common_adapter"]),
            "codex_desktop_adapter": _extract_boundary(payload["codex_desktop_adapter"]),
            "codex_cli_adapter": _extract_boundary(payload["codex_cli_adapter"]),
            "claude_code_adapter": _extract_boundary(payload["claude_code_adapter"]),
            "gemini_cli_adapter": _extract_boundary(payload["gemini_cli_adapter"]),
        },
        "legacy_opt_in": {},
    }
    if include_legacy_alias_artifact:
        boundaries["legacy_opt_in"] = {
            "codex_desktop_host_adapter": _extract_boundary(
                payload["compatibility_lane"]["codex_desktop_host_adapter"]
            )
        }
    return boundaries


def test_framework_profile_field_boundary_fixture_matches_python_projection() -> None:
    assert _python_default_boundary_fixture(_fixture_profile()) == FIXTURE["default"]


def test_framework_profile_field_boundary_fixture_matches_router_rs_projection() -> None:
    assert _router_rs_boundary_fixture(
        _fixture_profile(),
        include_legacy_alias_artifact=True,
    ) == FIXTURE
