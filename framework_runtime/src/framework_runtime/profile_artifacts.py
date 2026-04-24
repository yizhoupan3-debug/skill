"""Framework-profile artifact emission utilities."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping

from framework_runtime.framework_profile import (
    FrameworkProfile,
    merge_profile_overrides,
)
from framework_runtime.host_adapters import (
    DELEGATION_CONTRACT_ARTIFACT_ID,
    EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
    SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID,
)
from framework_runtime.rust_router import RustRouteAdapter

PROJECT_ROOT = Path(__file__).resolve().parents[3]
FRAMEWORK_SURFACE_POLICY_PATH = PROJECT_ROOT / "configs" / "framework" / "FRAMEWORK_SURFACE_POLICY.json"
DEFAULT_ARTIFACT_DIRNAME = "default"
FALLBACK_ARTIFACT_DIRNAME = "fallback"
CONTINUITY_ARTIFACT_DIRNAME = "continuity"
RUST_ARTIFACT_DIRNAME = "rust"
ARTIFACT_LAYOUT_MANIFEST_FILENAME = "framework_artifact_layout_manifest.json"


def _write_json(path: Path, payload: Any) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return str(path)


def _load_repo_framework_surface_policy() -> dict[str, Any]:
    if not FRAMEWORK_SURFACE_POLICY_PATH.exists():
        return {}
    try:
        payload = json.loads(FRAMEWORK_SURFACE_POLICY_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    return payload if isinstance(payload, dict) else {}


def _profile_with_surface_policy(profile: FrameworkProfile) -> FrameworkProfile:
    if profile.framework_surface_policy:
        return profile
    repo_policy = _load_repo_framework_surface_policy()
    if not repo_policy:
        return profile
    return merge_profile_overrides(
        profile,
        {"framework_surface_policy": repo_policy},
    )


DEFAULT_RUST_CODEX_ARTIFACT_FILENAMES = {
    "cli_common_adapter": "router_rs_cli_common_adapter.json",
    "codex_common_adapter": "router_rs_codex_common_adapter.json",
    "codex_desktop_adapter": "router_rs_codex_desktop_adapter.json",
    "codex_cli_adapter": "router_rs_codex_cli_adapter.json",
    "claude_code_adapter": "router_rs_claude_code_adapter.json",
    "gemini_cli_adapter": "router_rs_gemini_cli_adapter.json",
    "cli_family_capability_discovery": "router_rs_cli_family_capability_discovery.json",
    "cli_family_parity_snapshot": "router_rs_cli_family_parity_snapshot.json",
    "codex_dual_entry_parity_snapshot": "router_rs_codex_dual_entry_parity_snapshot.json",
    "execution_controller_contract": "router_rs_execution_controller_contract.json",
    "delegation_contract": "router_rs_delegation_contract.json",
    "supervisor_state_contract": "router_rs_supervisor_state_contract.json",
    "execution_kernel_live_fallback_retirement_status": (
        "router_rs_execution_kernel_live_fallback_retirement_status.json"
    ),
    "execution_kernel_live_response_serialization_contract": (
        "router_rs_execution_kernel_live_response_serialization_contract.json"
    ),
}
def build_framework_artifact_layout_manifest(
    *,
    output_dir: Path,
    paths: Mapping[str, str],
) -> dict[str, Any]:
    grouped_keys = {
        "default": [],
        "fallback": [],
        "continuity": [],
        "rust": [],
        "root": [],
    }
    for artifact_key, artifact_path in paths.items():
        try:
            relative = Path(artifact_path).resolve().relative_to(output_dir.resolve())
        except ValueError:
            grouped_keys["root"].append(artifact_key)
            continue
        top = relative.parts[0] if relative.parts else ""
        if top == DEFAULT_ARTIFACT_DIRNAME:
            grouped_keys["default"].append(artifact_key)
        elif top == FALLBACK_ARTIFACT_DIRNAME:
            grouped_keys["fallback"].append(artifact_key)
        elif top == CONTINUITY_ARTIFACT_DIRNAME:
            grouped_keys["continuity"].append(artifact_key)
        elif top == RUST_ARTIFACT_DIRNAME:
            grouped_keys["rust"].append(artifact_key)
        else:
            grouped_keys["root"].append(artifact_key)
    return {
        "schema_version": "framework-artifact-layout-manifest-v1",
        "authority": "framework-contract-emitter",
        "output_root": str(output_dir),
        "directory_policy": {
            "default": DEFAULT_ARTIFACT_DIRNAME,
            "fallback": FALLBACK_ARTIFACT_DIRNAME,
            "continuity": CONTINUITY_ARTIFACT_DIRNAME,
            "rust": RUST_ARTIFACT_DIRNAME,
        },
        "artifacts_by_lane": {
            lane: sorted(keys) for lane, keys in grouped_keys.items() if keys
        },
        "artifacts": {
            key: str(Path(path).resolve().relative_to(output_dir.resolve()))
            for key, path in sorted(paths.items())
        },
    }


def _build_rust_default_artifacts(
    profile: FrameworkProfile,
    *,
    rust_codex_artifacts: Mapping[str, Any],
) -> dict[str, Any]:
    artifacts = dict(rust_codex_artifacts)
    artifacts.update({
        "framework_profile": profile.to_dict(),
        "framework_surface_policy": profile.framework_surface_policy,
    })
    return artifacts


def _write_default_artifacts(
    output_dir: Path,
    python_artifacts: Mapping[str, Any],
) -> dict[str, str]:
    default_dir = output_dir / DEFAULT_ARTIFACT_DIRNAME
    return {
        "framework_profile": _write_json(
            default_dir / "framework_profile.json",
            python_artifacts["framework_profile"],
        ),
        "framework_surface_policy": _write_json(
            default_dir / "framework_surface_policy.json",
            python_artifacts["framework_surface_policy"],
        ),
        "cli_common_adapter": _write_json(
            default_dir / "cli_common_adapter.json", python_artifacts["cli_common_adapter"]
        ),
        "codex_cli_adapter": _write_json(
            default_dir / "codex_cli_adapter.json",
            python_artifacts["codex_cli_adapter"],
        ),
        "claude_code_adapter": _write_json(
            default_dir / "claude_code_adapter.json",
            python_artifacts["claude_code_adapter"],
        ),
        "gemini_cli_adapter": _write_json(
            default_dir / "gemini_cli_adapter.json",
            python_artifacts["gemini_cli_adapter"],
        ),
        "cli_family_capability_discovery": _write_json(
            default_dir / "cli_family_capability_discovery.json",
            python_artifacts["cli_family_capability_discovery"],
        ),
        "codex_desktop_adapter": _write_json(
            default_dir / "codex_desktop_adapter.json",
            python_artifacts["codex_desktop_adapter"],
        ),
        "cli_family_parity_snapshot": _write_json(
            default_dir / "cli_family_parity_snapshot.json",
            python_artifacts["cli_family_parity_snapshot"],
        ),
        "codex_dual_entry_parity_snapshot": _write_json(
            default_dir / "codex_dual_entry_parity_snapshot.json",
            python_artifacts["codex_dual_entry_parity_snapshot"],
        ),
        "execution_controller_contract": _write_json(
            default_dir / f"{EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID}.json",
            python_artifacts["execution_controller_contract"],
        ),
        "delegation_contract": _write_json(
            default_dir / f"{DELEGATION_CONTRACT_ARTIFACT_ID}.json",
            python_artifacts["delegation_contract"],
        ),
        "supervisor_state_contract": _write_json(
            default_dir / f"{SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID}.json",
            python_artifacts["supervisor_state_contract"],
        ),
        "execution_kernel_live_fallback_retirement_status": _write_json(
            default_dir / "execution_kernel_live_fallback_retirement_status.json",
            python_artifacts["execution_kernel_live_fallback_retirement_status"],
        ),
        "execution_kernel_live_response_serialization_contract": _write_json(
            default_dir / "execution_kernel_live_response_serialization_contract.json",
            python_artifacts["execution_kernel_live_response_serialization_contract"],
        ),
    }


def _write_rust_artifacts(
    output_dir: Path,
    *,
    profile_path: Path,
    rust_adapter: RustRouteAdapter,
) -> tuple[dict[str, Any], dict[str, str]]:
    rust_dir = output_dir / RUST_ARTIFACT_DIRNAME
    rust_bundle = rust_adapter.compile_profile_bundle(profile_path)
    rust_codex_artifacts = rust_adapter.compile_codex_profile_artifacts(
        profile_path,
    )

    paths = {
        "rust_profile_bundle": _write_json(
            rust_dir / "router_rs_profile_bundle.json",
            rust_bundle,
        )
    }
    for artifact_key, filename in DEFAULT_RUST_CODEX_ARTIFACT_FILENAMES.items():
        if artifact_key not in rust_codex_artifacts:
            continue
        paths[f"rust_{artifact_key}"] = _write_json(
            rust_dir / filename,
            rust_codex_artifacts[artifact_key],
        )

    return rust_codex_artifacts, paths


def emit_framework_contract_artifacts(
    output_dir: Path,
    *,
    profile: FrameworkProfile,
    host_overrides: Mapping[str, Any] | None = None,
    rust_adapter: RustRouteAdapter | None = None,
    include_fallback_artifacts: bool = False,
    include_compatibility_inventory: bool = False,
) -> dict[str, str]:
    """Write concrete framework-profile and adapter artifacts for bridge consumers."""

    if host_overrides is not None:
        raise ValueError("host_overrides are not supported; router-rs owns host projection output.")
    if include_fallback_artifacts:
        raise ValueError("fallback host artifacts are retired; router-rs owns canonical outputs.")
    if include_compatibility_inventory:
        raise ValueError("compatibility inventory artifacts are retired; router-rs owns canonical outputs.")

    output_dir.mkdir(parents=True, exist_ok=True)
    profile = _profile_with_surface_policy(profile)
    rust_adapter = rust_adapter or RustRouteAdapter(PROJECT_ROOT)

    profile_path = output_dir / DEFAULT_ARTIFACT_DIRNAME / "framework_profile.json"
    paths: dict[str, str] = {}

    _write_json(profile_path, profile.to_dict())
    rust_codex_artifacts, rust_paths = _write_rust_artifacts(
        output_dir,
        profile_path=profile_path,
        rust_adapter=rust_adapter,
    )
    paths.update(rust_paths)

    effective_default_artifacts = _build_rust_default_artifacts(
        profile,
        rust_codex_artifacts=rust_codex_artifacts,
    )
    paths.update(_write_default_artifacts(output_dir, effective_default_artifacts))

    layout_manifest = build_framework_artifact_layout_manifest(
        output_dir=output_dir,
        paths=paths,
    )
    paths["artifact_layout_manifest"] = _write_json(
        output_dir / ARTIFACT_LAYOUT_MANIFEST_FILENAME,
        layout_manifest,
    )

    return paths
