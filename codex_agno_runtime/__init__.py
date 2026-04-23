"""Repo-root bridge for the local ``codex_agno_runtime`` src layout."""

from __future__ import annotations

from pathlib import Path

_ROOT = Path(__file__).resolve().parent
_SRC_PACKAGE = _ROOT / "src" / "codex_agno_runtime"

if _SRC_PACKAGE.is_dir():
    src_path = str(_SRC_PACKAGE)
    if src_path not in __path__:
        __path__.append(src_path)

from .config import RuntimeSettings
from .framework_profile import FrameworkProfile
from .codex_artifact_contracts import (
    build_cli_family_capability_discovery,
    build_codex_dual_entry_parity_snapshot,
)
from .host_adapters import (
    compile_codex_cli_adapter,
    compile_codex_desktop_adapter,
)
from .control_plane_contracts import (
    build_delegation_contract,
    build_execution_controller_contract,
    build_execution_kernel_live_fallback_retirement_status,
    build_execution_kernel_live_response_serialization_contract,
    build_supervisor_state_contract,
)
from .profile_artifacts import emit_framework_contract_artifacts
from .runtime import CodexAgnoRuntime

__all__ = [
    "CodexAgnoRuntime",
    "RuntimeSettings",
    "FrameworkProfile",
    "compile_codex_desktop_adapter",
    "compile_codex_cli_adapter",
    "build_cli_family_capability_discovery",
    "build_codex_dual_entry_parity_snapshot",
    "build_execution_controller_contract",
    "build_delegation_contract",
    "build_execution_kernel_live_fallback_retirement_status",
    "build_execution_kernel_live_response_serialization_contract",
    "build_supervisor_state_contract",
    "emit_framework_contract_artifacts",
]
