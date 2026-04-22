"""Continuity/fallback escape hatches kept outside the root package surface."""

from codex_agno_runtime.host_adapters import (
    compile_aionrs_companion_adapter,
    compile_aionui_host_adapter,
    compile_codex_desktop_host_adapter,
)

__all__ = [
    "compile_aionrs_companion_adapter",
    "compile_aionui_host_adapter",
    "compile_codex_desktop_host_adapter",
]
