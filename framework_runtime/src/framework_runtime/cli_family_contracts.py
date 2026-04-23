"""Compatibility facade for legacy CLI-family contract import paths."""

from framework_runtime.framework_artifact_contracts import (
    build_cli_family_capability_discovery,
    build_cli_family_parity_snapshot,
    build_codex_dual_entry_parity_snapshot,
)

__all__ = [
    "build_cli_family_capability_discovery",
    "build_cli_family_parity_snapshot",
    "build_codex_dual_entry_parity_snapshot",
]
