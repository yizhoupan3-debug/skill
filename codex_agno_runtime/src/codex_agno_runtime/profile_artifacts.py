"""Framework-profile artifact emission utilities."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping

from codex_agno_runtime.framework_profile import FrameworkProfile
from codex_agno_runtime.host_adapters import (
    DELEGATION_CONTRACT_ARTIFACT_ID,
    EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID,
    GENERIC_HOST_ADAPTER,
    SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID,
    adapt_framework_profile,
    build_cli_family_capability_discovery,
    build_codex_desktop_alias_retirement_status,
    build_cli_family_parity_snapshot,
    build_codex_dual_entry_parity_snapshot,
    build_execution_controller_contract,
    build_delegation_contract,
    build_execution_kernel_live_fallback_retirement_status,
    build_execution_kernel_live_response_serialization_contract,
    build_supervisor_state_contract,
    build_upgrade_compatibility_matrix,
    compile_aionrs_companion_adapter,
    compile_aionui_host_adapter,
    compile_claude_code_adapter,
    compile_codex_cli_adapter,
    compile_codex_common_adapter,
    compile_codex_desktop_adapter,
    compile_codex_desktop_host_adapter,
    compile_cli_common_adapter,
    compile_gemini_cli_adapter,
    should_emit_codex_desktop_alias_artifact,
)
from codex_agno_runtime.rust_router import RustRouteAdapter

PROJECT_ROOT = Path(__file__).resolve().parents[3]
LEGACY_DESKTOP_ALIAS_ID = "codex_desktop_host_adapter"


def _write_json(path: Path, payload: Any) -> str:
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return str(path)


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
    "codex_desktop_alias_retirement_status": "router_rs_codex_desktop_alias_retirement_status.json",
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
LEGACY_RUST_CODEX_ARTIFACT_FILENAME = (
    "codex_desktop_host_adapter",
    "router_rs_codex_desktop_host_adapter.json",
)
RUST_PYTHON_PARITY_REPORT_FILENAME = "rust_python_artifact_parity_report.json"
RUST_PYTHON_PARITY_FIELDS = {
    "cli_common_adapter": "rust_cli_common_adapter",
    "codex_common_adapter": "rust_codex_common_adapter",
    "codex_desktop_adapter": "rust_codex_desktop_adapter",
    "codex_cli_adapter": "rust_codex_cli_adapter",
    "claude_code_adapter": "rust_claude_code_adapter",
    "gemini_cli_adapter": "rust_gemini_cli_adapter",
    "cli_family_capability_discovery": "rust_cli_family_capability_discovery",
    "cli_family_parity_snapshot": "rust_cli_family_parity_snapshot",
    "codex_dual_entry_parity_snapshot": "rust_codex_dual_entry_parity_snapshot",
    "codex_desktop_alias_retirement_status": "rust_codex_desktop_alias_retirement_status",
    "execution_controller_contract": "rust_execution_controller_contract",
    "delegation_contract": "rust_delegation_contract",
    "supervisor_state_contract": "rust_supervisor_state_contract",
    "execution_kernel_live_fallback_retirement_status": (
        "rust_execution_kernel_live_fallback_retirement_status"
    ),
    "execution_kernel_live_response_serialization_contract": (
        "rust_execution_kernel_live_response_serialization_contract"
    ),
}
PYTHON_OWNED_RUST_PARITY_PATHS: dict[str, tuple[str, ...]] = {}


def _clone_payload(payload: Any) -> Any:
    return json.loads(json.dumps(payload, ensure_ascii=False))


def _drop_object_paths(payload: Any, paths: tuple[str, ...]) -> Any:
    normalized = _clone_payload(payload)
    if not isinstance(normalized, dict):
        return normalized
    for path in paths:
        cursor = normalized
        parts = path.split(".")
        for part in parts[:-1]:
            if not isinstance(cursor, dict):
                cursor = None
                break
            cursor = cursor.get(part)
        if isinstance(cursor, dict):
            cursor.pop(parts[-1], None)
    return normalized


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


def build_rust_python_artifact_parity_report(
    *,
    python_artifacts: Mapping[str, Any],
    rust_artifacts: Mapping[str, Any],
) -> dict[str, Any]:
    artifacts: dict[str, Any] = {}
    raw_all_match = True
    normalized_all_match = True

    for python_key, rust_key in RUST_PYTHON_PARITY_FIELDS.items():
        python_payload = python_artifacts[python_key]
        rust_payload = rust_artifacts[rust_key]
        ignored_paths = list(PYTHON_OWNED_RUST_PARITY_PATHS.get(python_key, ()))
        raw_diff_paths = _collect_diff_paths(python_payload, rust_payload)
        normalized_diff_paths = _collect_diff_paths(
            _drop_object_paths(python_payload, tuple(ignored_paths)),
            _drop_object_paths(rust_payload, tuple(ignored_paths)),
        )
        raw_match = not raw_diff_paths
        normalized_match = not normalized_diff_paths
        raw_all_match = raw_all_match and raw_match
        normalized_all_match = normalized_all_match and normalized_match
        artifacts[python_key] = {
            "rust_artifact_key": rust_key,
            "raw_match": raw_match,
            "normalized_match": normalized_match,
            "ignored_python_owned_paths": ignored_paths,
            "raw_diff_paths": raw_diff_paths[:25],
            "normalized_diff_paths": normalized_diff_paths[:25],
        }

    return {
        "schema_version": "rust-python-artifact-parity-report-v1",
        "authority": "framework-contract-emitter",
        "compared_artifacts": list(RUST_PYTHON_PARITY_FIELDS),
        "python_owned_paths": {
            key: list(paths) for key, paths in PYTHON_OWNED_RUST_PARITY_PATHS.items()
        },
        "raw_all_artifacts_match": raw_all_match,
        "all_artifacts_match_after_normalization": normalized_all_match,
        "artifacts": artifacts,
    }


def _classify_alias_reference(path: Path) -> tuple[str, str]:
    parts = set(path.parts)
    if path.name == "host_adapters.py":
        return "compatibility_infrastructure", "compatibility_only"
    if path.name == "profile_artifacts.py":
        return "artifact_emitter", "compatibility_only"
    if path.name == "compatibility.py":
        return "compatibility_escape_hatch", "compatibility_only"
    if path.name == "write_framework_contract_artifacts.py":
        return "compatibility_emitter_cli", "compatibility_only"
    if path.name == "__init__.py":
        return "retired_root_export_surface", "compatibility_only"
    if path.name == "framework_profile.rs":
        return "rust_contract_artifact_lane", "compatibility_only"
    if "tests" in parts:
        return "compatibility_regression_tests", "compatibility_only"
    if "docs" in parts or "aionrs_fusion_docs" in parts:
        return "compatibility_contract_docs", "compatibility_only"
    return "unclassified_code", "primary_identity_risk"


def build_codex_desktop_alias_inventory(repo_root: Path | None = None) -> dict[str, Any]:
    scan_root = repo_root or PROJECT_ROOT
    search_roots = (
        scan_root / "codex_agno_runtime" / "src",
        scan_root / "scripts",
        scan_root / "tests",
        scan_root / "docs",
        scan_root / "aionrs_fusion_docs",
    )
    references: list[dict[str, Any]] = []
    category_counts: dict[str, int] = {}
    risk_counts = {"compatibility_only": 0, "primary_identity_risk": 0}

    for root in search_roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*")):
            if not path.is_file() or path.suffix in {".pyc"}:
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            for line_number, line in enumerate(text.splitlines(), start=1):
                if LEGACY_DESKTOP_ALIAS_ID not in line:
                    continue
                category, risk = _classify_alias_reference(path)
                category_counts[category] = category_counts.get(category, 0) + 1
                risk_counts[risk] += 1
                references.append(
                    {
                        "path": str(path.relative_to(scan_root)),
                        "line": line_number,
                        "category": category,
                        "risk": risk,
                        "line_text": line.strip(),
                    }
                )

    summary = {
        "inventory_complete": True,
        "legacy_alias_id": LEGACY_DESKTOP_ALIAS_ID,
        "total_occurrences": len(references),
        "category_counts": category_counts,
        "primary_identity_risk_occurrences": risk_counts["primary_identity_risk"],
        "compatibility_only_occurrences": risk_counts["compatibility_only"],
        "translation_shim_required": risk_counts["primary_identity_risk"] > 0,
    }
    return {
        "canonical_adapter_id": "codex_desktop_adapter",
        "legacy_alias_id": LEGACY_DESKTOP_ALIAS_ID,
        "scan_root": str(scan_root),
        "summary": summary,
        "references": references,
    }


def emit_framework_contract_artifacts(
    output_dir: Path,
    *,
    profile: FrameworkProfile,
    host_overrides: Mapping[str, Any] | None = None,
    rust_adapter: RustRouteAdapter | None = None,
    include_legacy_alias_artifact: bool | None = None,
) -> dict[str, str]:
    """Write concrete framework-profile and adapter artifacts for bridge consumers."""

    output_dir.mkdir(parents=True, exist_ok=True)
    alias_inventory = build_codex_desktop_alias_inventory()
    emit_legacy_alias_artifact = (
        include_legacy_alias_artifact
        if include_legacy_alias_artifact is not None
        else should_emit_codex_desktop_alias_artifact(alias_inventory["summary"])
    )

    profile_path = output_dir / "framework_profile.json"
    python_artifacts = {
        "framework_profile": profile.to_dict(),
        "cli_common_adapter": compile_cli_common_adapter(profile, host_overrides=host_overrides).host_payload,
        "codex_common_adapter": compile_codex_common_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
        "codex_cli_adapter": compile_codex_cli_adapter(profile, host_overrides=host_overrides).host_payload,
        "claude_code_adapter": compile_claude_code_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
        "gemini_cli_adapter": compile_gemini_cli_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
        "cli_family_capability_discovery": build_cli_family_capability_discovery(
            profile,
            host_overrides=host_overrides,
        ),
        "codex_desktop_adapter": compile_codex_desktop_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
        "aionrs_companion_adapter": compile_aionrs_companion_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
        "aionui_host_adapter": compile_aionui_host_adapter(
            profile,
            host_overrides=host_overrides,
        ).host_payload,
        "generic_host_adapter": adapt_framework_profile(
            profile,
            GENERIC_HOST_ADAPTER,
            host_overrides=host_overrides,
        ).host_payload,
        "upgrade_compatibility_matrix": build_upgrade_compatibility_matrix(
            profile,
            include_legacy_aliases=emit_legacy_alias_artifact,
        ),
        "cli_family_parity_snapshot": build_cli_family_parity_snapshot(
            profile,
            host_overrides=host_overrides,
        ),
        "codex_dual_entry_parity_snapshot": build_codex_dual_entry_parity_snapshot(
            profile,
            host_overrides=host_overrides,
        ),
        "codex_desktop_alias_inventory": alias_inventory,
        "codex_desktop_alias_retirement_status": build_codex_desktop_alias_retirement_status(
            alias_inventory_summary=alias_inventory["summary"]
        ),
        "execution_controller_contract": build_execution_controller_contract(),
        "delegation_contract": build_delegation_contract(),
        "supervisor_state_contract": build_supervisor_state_contract(),
        "execution_kernel_live_fallback_retirement_status": (
            build_execution_kernel_live_fallback_retirement_status()
        ),
        "execution_kernel_live_response_serialization_contract": (
            build_execution_kernel_live_response_serialization_contract()
        ),
    }
    paths = {
        "framework_profile": _write_json(profile_path, python_artifacts["framework_profile"]),
        "cli_common_adapter": _write_json(output_dir / "cli_common_adapter.json", python_artifacts["cli_common_adapter"]),
        "codex_common_adapter": _write_json(
            output_dir / "codex_common_adapter.json",
            python_artifacts["codex_common_adapter"],
        ),
        "codex_cli_adapter": _write_json(output_dir / "codex_cli_adapter.json", python_artifacts["codex_cli_adapter"]),
        "claude_code_adapter": _write_json(
            output_dir / "claude_code_adapter.json",
            python_artifacts["claude_code_adapter"],
        ),
        "gemini_cli_adapter": _write_json(
            output_dir / "gemini_cli_adapter.json",
            python_artifacts["gemini_cli_adapter"],
        ),
        "cli_family_capability_discovery": _write_json(
            output_dir / "cli_family_capability_discovery.json",
            python_artifacts["cli_family_capability_discovery"],
        ),
        "codex_desktop_adapter": _write_json(
            output_dir / "codex_desktop_adapter.json",
            python_artifacts["codex_desktop_adapter"],
        ),
        "aionrs_companion_adapter": _write_json(
            output_dir / "aionrs_companion_adapter.json",
            python_artifacts["aionrs_companion_adapter"],
        ),
        "aionui_host_adapter": _write_json(
            output_dir / "aionui_host_adapter.json",
            python_artifacts["aionui_host_adapter"],
        ),
        "generic_host_adapter": _write_json(
            output_dir / "generic_host_adapter.json",
            python_artifacts["generic_host_adapter"],
        ),
        "upgrade_compatibility_matrix": _write_json(
            output_dir / "upgrade_compatibility_matrix.json",
            python_artifacts["upgrade_compatibility_matrix"],
        ),
        "cli_family_parity_snapshot": _write_json(
            output_dir / "cli_family_parity_snapshot.json",
            python_artifacts["cli_family_parity_snapshot"],
        ),
        "codex_dual_entry_parity_snapshot": _write_json(
            output_dir / "codex_dual_entry_parity_snapshot.json",
            python_artifacts["codex_dual_entry_parity_snapshot"],
        ),
        "codex_desktop_alias_inventory": _write_json(
            output_dir / "codex_desktop_alias_inventory.json",
            python_artifacts["codex_desktop_alias_inventory"],
        ),
        "codex_desktop_alias_retirement_status": _write_json(
            output_dir / "codex_desktop_alias_retirement_status.json",
            python_artifacts["codex_desktop_alias_retirement_status"],
        ),
        "execution_controller_contract": _write_json(
            output_dir / f"{EXECUTION_CONTROLLER_CONTRACT_ARTIFACT_ID}.json",
            python_artifacts["execution_controller_contract"],
        ),
        "delegation_contract": _write_json(
            output_dir / f"{DELEGATION_CONTRACT_ARTIFACT_ID}.json",
            python_artifacts["delegation_contract"],
        ),
        "supervisor_state_contract": _write_json(
            output_dir / f"{SUPERVISOR_STATE_CONTRACT_ARTIFACT_ID}.json",
            python_artifacts["supervisor_state_contract"],
        ),
        "execution_kernel_live_fallback_retirement_status": _write_json(
            output_dir / "execution_kernel_live_fallback_retirement_status.json",
            python_artifacts["execution_kernel_live_fallback_retirement_status"],
        ),
        "execution_kernel_live_response_serialization_contract": _write_json(
            output_dir / "execution_kernel_live_response_serialization_contract.json",
            python_artifacts["execution_kernel_live_response_serialization_contract"],
        ),
    }
    if emit_legacy_alias_artifact:
        paths["codex_desktop_host_adapter"] = _write_json(
            output_dir / "codex_desktop_host_adapter.json",
            compile_codex_desktop_host_adapter(profile, host_overrides=host_overrides).host_payload,
        )

    if rust_adapter is not None:
        rust_bundle = rust_adapter.compile_profile_bundle(profile_path)
        rust_codex_artifacts = rust_adapter.compile_codex_profile_artifacts(
            profile_path,
            include_legacy_alias_artifact=emit_legacy_alias_artifact,
        )
        paths["rust_profile_bundle"] = _write_json(
            output_dir / "router_rs_profile_bundle.json",
            rust_bundle,
        )
        for artifact_key, filename in DEFAULT_RUST_CODEX_ARTIFACT_FILENAMES.items():
            if artifact_key not in rust_codex_artifacts:
                continue
            paths[f"rust_{artifact_key}"] = _write_json(
                output_dir / filename,
                rust_codex_artifacts[artifact_key],
            )
        legacy_artifact_key, legacy_filename = LEGACY_RUST_CODEX_ARTIFACT_FILENAME
        if legacy_artifact_key in rust_codex_artifacts:
            paths[f"rust_{legacy_artifact_key}"] = _write_json(
                output_dir / legacy_filename,
                rust_codex_artifacts[legacy_artifact_key],
            )
        rust_parity_report = build_rust_python_artifact_parity_report(
            python_artifacts=python_artifacts,
            rust_artifacts={f"rust_{key}": value for key, value in rust_codex_artifacts.items()},
        )
        paths["rust_python_artifact_parity_report"] = _write_json(
            output_dir / RUST_PYTHON_PARITY_REPORT_FILENAME,
            rust_parity_report,
        )

    return paths
