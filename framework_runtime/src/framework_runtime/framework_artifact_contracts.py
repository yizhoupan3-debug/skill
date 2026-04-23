"""Rust-backed first-class Codex artifact builders."""

from __future__ import annotations

from typing import Any, Dict, Mapping

from framework_runtime.framework_profile import FrameworkProfile
from framework_runtime.host_adapters import (
    CLI_FAMILY_PARITY_ARTIFACT_ID,
    _compile_rust_codex_artifact,
)

CLI_FAMILY_CAPABILITY_DISCOVERY_ARTIFACT_ID = "cli_family_capability_discovery"
CODEX_DUAL_ENTRY_PARITY_ARTIFACT_ID = "codex_dual_entry_parity_snapshot"


def build_cli_family_parity_snapshot(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    del host_overrides
    return _compile_rust_codex_artifact(profile, CLI_FAMILY_PARITY_ARTIFACT_ID)


def build_cli_family_capability_discovery(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    del host_overrides
    return _compile_rust_codex_artifact(profile, CLI_FAMILY_CAPABILITY_DISCOVERY_ARTIFACT_ID)


def build_codex_dual_entry_parity_snapshot(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
) -> Dict[str, Any]:
    del host_overrides
    return _compile_rust_codex_artifact(profile, CODEX_DUAL_ENTRY_PARITY_ARTIFACT_ID)
