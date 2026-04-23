"""Public package exports for the local framework runtime."""

from .config import RuntimeSettings
from .framework_artifact_contracts import (
    build_cli_family_capability_discovery,
    build_codex_dual_entry_parity_snapshot,
)
from .host_adapters import (
    compile_codex_cli_adapter,
    compile_codex_desktop_adapter,
)
from .framework_profile import FrameworkProfile
from .profile_artifacts import emit_framework_contract_artifacts
from .runtime import CodexAgnoRuntime

FrameworkRuntime = CodexAgnoRuntime

__all__ = [
    "CodexAgnoRuntime",
    "FrameworkRuntime",
    "RuntimeSettings",
    "FrameworkProfile",
    "compile_codex_desktop_adapter",
    "compile_codex_cli_adapter",
    "build_cli_family_capability_discovery",
    "build_codex_dual_entry_parity_snapshot",
    "emit_framework_contract_artifacts",
]
