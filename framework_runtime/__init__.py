"""Neutral package bridge for the local framework runtime."""

from __future__ import annotations

from pathlib import Path

_ROOT = Path(__file__).resolve().parent
_SRC_PACKAGE = _ROOT / "src" / "framework_runtime"

if _SRC_PACKAGE.is_dir():
    src_path = str(_SRC_PACKAGE)
    if src_path not in __path__:
        __path__.append(src_path)

from .config import RuntimeSettings
from .framework_profile import FrameworkProfile
from .framework_artifact_contracts import (
    build_cli_family_capability_discovery,
    build_codex_dual_entry_parity_snapshot,
)
from .host_adapters import (
    compile_codex_cli_adapter,
    compile_codex_desktop_adapter,
)
from .profile_artifacts import emit_framework_contract_artifacts
from .runtime import CodexAgnoRuntime

FrameworkRuntime = CodexAgnoRuntime

__all__ = [
    "CodexAgnoRuntime",
    "FrameworkProfile",
    "FrameworkRuntime",
    "RuntimeSettings",
    "build_cli_family_capability_discovery",
    "build_codex_dual_entry_parity_snapshot",
    "compile_codex_cli_adapter",
    "compile_codex_desktop_adapter",
    "emit_framework_contract_artifacts",
]
