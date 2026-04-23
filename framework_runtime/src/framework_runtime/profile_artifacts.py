"""Framework-profile artifact emission utilities."""

from __future__ import annotations

import json
from pathlib import Path
from tempfile import NamedTemporaryFile
from typing import Any, Mapping

from framework_runtime.framework_profile import (
    FRAMEWORK_SHARED_CONTRACT_FIELDS,
    FrameworkProfile,
    extract_framework_workspace_bridges,
    merge_profile_overrides,
)
from framework_runtime.host_adapters import (
    DELEGATION_CONTRACT_ARTIFACT_ID,
    EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
    SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID,
)
from framework_runtime.rust_router import RustRouteAdapter
from framework_runtime.schemas import (
    FrameworkSharedContract,
    FrameworkSharedContractProjectionReport,
    FrameworkSharedContractSurface,
)

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
    "upgrade_compatibility_matrix": "router_rs_upgrade_compatibility_matrix.json",
}
def _clone_payload(payload: Any) -> Any:
    return json.loads(json.dumps(payload, ensure_ascii=False))


def _collect_diff_paths(left: Any, right: Any, prefix: str = "") -> list[str]:
    if type(left) is not type(right):
        return [prefix or "$"]
    if isinstance(left, dict):
        paths: list[str] = []
        left_keys = set(left)
        right_keys = set(right)
        for key in sorted(left_keys | right_keys):
            path = f"{prefix}.{key}" if prefix else key
            if key not in left or key not in right:
                paths.append(path)
                continue
            paths.extend(_collect_diff_paths(left[key], right[key], path))
        return paths
    if isinstance(left, list):
        if len(left) != len(right):
            return [prefix or "$"]
        paths: list[str] = []
        for index, (left_item, right_item) in enumerate(zip(left, right)):
            path = f"{prefix}[{index}]" if prefix else f"[{index}]"
            paths.extend(_collect_diff_paths(left_item, right_item, path))
        return paths
    if left != right:
        return [prefix or "$"]
    return []




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


def _extract_shared_contract_surface(
    payload: Mapping[str, Any],
    field_name: str,
) -> dict[str, Any]:
    source = payload.get(field_name, {})
    projected_surface = FrameworkSharedContractSurface().model_dump(mode="python")
    if not isinstance(source, Mapping):
        return projected_surface
    for field in FRAMEWORK_SHARED_CONTRACT_FIELDS:
        if field in source:
            projected_surface[field] = _clone_payload(source[field])
    return projected_surface


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
    emit_compatibility_inventory: bool,
) -> tuple[dict[str, Any], dict[str, str]]:
    rust_dir = output_dir / RUST_ARTIFACT_DIRNAME
    rust_bundle = rust_adapter.compile_profile_bundle(profile_path)
    rust_codex_artifacts = rust_adapter.compile_codex_profile_artifacts(
        profile_path,
        include_compatibility_inventory=emit_compatibility_inventory,
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


def _emit_shared_contract_projection_report(
    *,
    profile: FrameworkProfile,
    host_overrides: Mapping[str, Any] | None = None,
    adapter_payloads: Mapping[str, Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    report = build_framework_shared_contract_projection_report(
        profile,
        host_overrides=host_overrides,
        adapter_payloads=adapter_payloads,
    )
    if not report["all_shared_contract_projections_match"]:
        raise ValueError(
            "framework shared-contract projection drift detected: "
            f"{report['adapter_projections']}"
        )
    return report


def _write_continuity_artifacts(
    output_dir: Path,
    *,
    emit_compatibility_inventory: bool,
    rust_codex_artifacts: Mapping[str, Any],
) -> dict[str, str]:
    continuity_dir = output_dir / CONTINUITY_ARTIFACT_DIRNAME
    paths: dict[str, str] = {}
    if emit_compatibility_inventory:
        paths["codex_common_adapter"] = _write_json(
            continuity_dir / "codex_common_adapter.json",
            rust_codex_artifacts["codex_common_adapter"],
        )
        compatibility_matrix = rust_codex_artifacts.get("upgrade_compatibility_matrix")
        if compatibility_matrix is None:
            raise RuntimeError("router-rs did not emit upgrade_compatibility_matrix")
        paths["upgrade_compatibility_matrix"] = _write_json(
            continuity_dir / "upgrade_compatibility_matrix.json",
            compatibility_matrix,
        )
    return paths


def build_framework_shared_contract_projection_report(
    profile: FrameworkProfile,
    *,
    host_overrides: Mapping[str, Any] | None = None,
    adapter_payloads: Mapping[str, Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    if host_overrides is not None:
        raise ValueError("host_overrides are not supported; router-rs owns host projection output.")
    canonical_payload = FrameworkSharedContract.model_validate(
        profile.shared_contract_payload()
    ).model_dump(mode="python")
    canonical_surface = canonical_payload["shared_contract"]
    canonical_bridge_contract = extract_framework_workspace_bridges(
        canonical_surface["workspace_bootstrap"]
    )

    if adapter_payloads is not None:
        compiled_payloads = dict(adapter_payloads)
    else:
        with NamedTemporaryFile("w", encoding="utf-8", suffix=".json", delete=False) as handle:
            json.dump(profile.to_dict(), handle, ensure_ascii=False)
            handle.flush()
            profile_path = Path(handle.name)
        try:
            compiled_payloads = RustRouteAdapter(PROJECT_ROOT).compile_codex_profile_artifacts(profile_path)
        finally:
            profile_path.unlink(missing_ok=True)

    adapter_projection_map = (
        ("cli_common_adapter", "shared_contract", None, "bridge_contract"),
        ("codex_common_adapter", "shared_contract", None, "bridge_contract"),
        ("codex_desktop_adapter", "common_contract", "runtime_surface", "bridge_contract"),
        ("codex_cli_adapter", "common_contract", "runtime_surface", "bridge_contract"),
        ("claude_code_adapter", "common_contract", "runtime_surface", "bridge_contract"),
        ("gemini_cli_adapter", "common_contract", "runtime_surface", "bridge_contract"),
    )
    projections: list[dict[str, Any]] = []
    all_match = True
    all_bridge_match = True

    for adapter_id, projection_field, runtime_surface_field, bridge_contract_field in adapter_projection_map:
        payload = compiled_payloads[adapter_id]
        projected_contract = _extract_shared_contract_surface(payload, projection_field)
        shared_mismatch_fields = _collect_diff_paths(canonical_surface, projected_contract)
        shared_match = not shared_mismatch_fields
        bridge_contract = None
        bridge_contract_mismatch_fields: list[str] = []
        bridge_contract_match: bool | None = None
        if bridge_contract_field:
            source = payload.get(bridge_contract_field, {})
            bridge_contract = _clone_payload(source) if isinstance(source, Mapping) else {}
            bridge_contract_mismatch_fields = _collect_diff_paths(
                canonical_bridge_contract,
                bridge_contract,
            )
            bridge_contract_match = not bridge_contract_mismatch_fields
        runtime_surface = None
        runtime_surface_mismatch_fields: list[str] = []
        runtime_surface_match: bool | None = None
        if runtime_surface_field:
            runtime_surface = _extract_shared_contract_surface(payload, runtime_surface_field)
            runtime_surface_mismatch_fields = _collect_diff_paths(canonical_surface, runtime_surface)
            runtime_surface_match = not runtime_surface_mismatch_fields
        all_match = all_match and shared_match and (
            runtime_surface_match if runtime_surface_match is not None else True
        )
        all_bridge_match = all_bridge_match and (
            bridge_contract_match if bridge_contract_match is not None else True
        )
        projections.append(
            {
                "adapter_id": adapter_id,
                "projection_field": projection_field,
                "shared_contract_match": shared_match,
                "shared_contract_mismatch_fields": shared_mismatch_fields,
                "projected_contract": FrameworkSharedContractSurface.model_validate(
                    projected_contract
                ).model_dump(mode="python"),
                "bridge_contract_match": bridge_contract_match,
                "bridge_contract_mismatch_fields": bridge_contract_mismatch_fields,
                "bridge_contract": bridge_contract,
                "runtime_surface_match": runtime_surface_match,
                "runtime_surface_mismatch_fields": runtime_surface_mismatch_fields,
                "runtime_surface": (
                    FrameworkSharedContractSurface.model_validate(runtime_surface).model_dump(
                        mode="python"
                    )
                    if runtime_surface is not None
                    else None
                ),
            }
        )

    report = FrameworkSharedContractProjectionReport.model_validate(
        {
            "schema_version": "framework-shared-contract-projection-report-v1",
            "authority": "framework-profile-artifacts",
            "profile_id": profile.profile_id,
            "framework_profile_version": profile.framework_profile_version,
            "shared_contract_schema_version": canonical_payload["schema_version"],
            "projection_fields": list(FRAMEWORK_SHARED_CONTRACT_FIELDS),
            "canonical_shared_contract": canonical_surface,
            "canonical_bridge_contract": canonical_bridge_contract,
            "adapter_projections": projections,
            "all_shared_contract_projections_match": all_match,
            "all_bridge_contract_projections_match": all_bridge_match,
        }
    )
    return report.model_dump(mode="python")


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

    output_dir.mkdir(parents=True, exist_ok=True)
    profile = _profile_with_surface_policy(profile)
    emit_compatibility_inventory = include_compatibility_inventory
    rust_adapter = rust_adapter or RustRouteAdapter(PROJECT_ROOT)

    profile_path = output_dir / DEFAULT_ARTIFACT_DIRNAME / "framework_profile.json"
    paths: dict[str, str] = {}

    _write_json(profile_path, profile.to_dict())
    rust_codex_artifacts, rust_paths = _write_rust_artifacts(
        output_dir,
        profile_path=profile_path,
        rust_adapter=rust_adapter,
        emit_compatibility_inventory=emit_compatibility_inventory,
    )
    paths.update(rust_paths)

    effective_default_artifacts = _build_rust_default_artifacts(
        profile,
        rust_codex_artifacts=rust_codex_artifacts,
    )
    paths.update(_write_default_artifacts(output_dir, effective_default_artifacts))

    _emit_shared_contract_projection_report(
        profile=profile,
        adapter_payloads=effective_default_artifacts,
    )

    paths.update(
        _write_continuity_artifacts(
            output_dir,
            emit_compatibility_inventory=emit_compatibility_inventory,
            rust_codex_artifacts=rust_codex_artifacts,
        )
    )
    layout_manifest = build_framework_artifact_layout_manifest(
        output_dir=output_dir,
        paths=paths,
    )
    paths["artifact_layout_manifest"] = _write_json(
        output_dir / ARTIFACT_LAYOUT_MANIFEST_FILENAME,
        layout_manifest,
    )

    return paths
