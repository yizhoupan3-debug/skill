"""Continuity/fallback escape hatches kept outside the root package surface."""

from codex_agno_runtime.host_adapters import (
    build_codex_desktop_alias_retirement_status,
    build_upgrade_compatibility_matrix,
    compile_aionrs_companion_adapter,
    compile_aionui_host_adapter,
    compile_codex_common_adapter,
    compile_codex_desktop_host_adapter,
)

__all__ = [
    "build_codex_desktop_alias_retirement_status",
    "build_upgrade_compatibility_matrix",
    "compile_aionrs_companion_adapter",
    "compile_aionui_host_adapter",
    "compile_codex_common_adapter",
    "compile_codex_desktop_host_adapter",
]
