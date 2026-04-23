"""Neutral package bridge for the local framework runtime."""

from __future__ import annotations

from pathlib import Path

_ROOT = Path(__file__).resolve().parent
_SRC_PACKAGE = _ROOT.parent / "codex_agno_runtime" / "src" / "codex_agno_runtime"

if _SRC_PACKAGE.is_dir():
    src_path = str(_SRC_PACKAGE)
    if src_path not in __path__:
        __path__.append(src_path)

from codex_agno_runtime import (
    CodexAgnoRuntime,
    FrameworkProfile,
    FrameworkRuntime,
    RuntimeSettings,
    build_cli_family_capability_discovery,
    build_codex_dual_entry_parity_snapshot,
    build_delegation_contract,
    build_execution_controller_contract,
    build_execution_kernel_live_fallback_retirement_status,
    build_execution_kernel_live_response_serialization_contract,
    build_supervisor_state_contract,
    compile_codex_cli_adapter,
    compile_codex_desktop_adapter,
    emit_framework_contract_artifacts,
)

__all__ = [
    "CodexAgnoRuntime",
    "FrameworkProfile",
    "FrameworkRuntime",
    "RuntimeSettings",
    "build_cli_family_capability_discovery",
    "build_codex_dual_entry_parity_snapshot",
    "build_delegation_contract",
    "build_execution_controller_contract",
    "build_execution_kernel_live_fallback_retirement_status",
    "build_execution_kernel_live_response_serialization_contract",
    "build_supervisor_state_contract",
    "compile_codex_cli_adapter",
    "compile_codex_desktop_adapter",
    "emit_framework_contract_artifacts",
]
