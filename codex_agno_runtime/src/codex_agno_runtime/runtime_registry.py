from __future__ import annotations

import json
from copy import deepcopy
from functools import lru_cache
from pathlib import Path
from typing import Any

RUNTIME_REGISTRY_SCHEMA_VERSION = "framework-runtime-registry-v1"
_REPO_ROOT = Path(__file__).resolve().parents[3]
_DEFAULT_REGISTRY_PATH = _REPO_ROOT / "configs" / "framework" / "RUNTIME_REGISTRY.json"
_FALLBACK_DEFAULT_HOST_PEER_SET = (
    "codex_desktop_adapter",
    "codex_cli_adapter",
    "claude_code_adapter",
    "gemini_cli_adapter",
)
_FALLBACK_PLUGIN_RECORD = {
    "plugin_name": "skill-framework-native",
    "source_rel": "plugins/skill-framework-native",
    "home_subpath": ".codex/plugins/skill-framework-native",
    "marketplace_name": "skill-framework-native",
    "marketplace_category": "Developer Tools",
}
_FALLBACK_WORKSPACE_BOOTSTRAP_DEFAULTS = {
    "skill_bridge": {
        "source_rel": "skills",
        "project_dir": ".codex/skills",
        "user_dir": "~/.codex/skills",
        "bridge_dir": ".aionrs/skills",
    },
    "memory_bridge": {
        "bridge_dir": ".aionrs-memory-bridge",
    },
}
_FALLBACK_HOST_ADAPTER_ORDER = (
    "aionrs_companion_adapter",
    "aionui_host_adapter",
    "cli_common_adapter",
    "codex_common_adapter",
    "codex_desktop_adapter",
    "codex_desktop_host_adapter",
    "codex_cli_adapter",
    "claude_code_adapter",
    "gemini_cli_adapter",
    "generic_host_adapter",
)
_FALLBACK_COMPATIBILITY_ADAPTER_IDS = frozenset(
    {
        "aionrs_companion_adapter",
        "aionui_host_adapter",
        "codex_desktop_host_adapter",
    }
)
_FALLBACK_HOST_ADAPTER_FIELD_OVERRIDES: dict[str, dict[str, Any]] = {
    "claude_code_adapter": {
        "notes": "Headless Claude Code projection that stays host-specific only at the projection layer.",
    },
    "gemini_cli_adapter": {
        "thin_patch_surfaces": ["cli_metadata_injection"],
        "notes": "Headless Gemini CLI projection that consumes the shared framework contract.",
    },
    "generic_host_adapter": {
        "host_id": "generic-host",
        "emits_artifacts": False,
    },
}


def _clone_json_like(value: Any) -> Any:
    if isinstance(value, dict):
        return {str(key): _clone_json_like(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_clone_json_like(item) for item in value]
    return value


def _registry_candidates(repo_root: Path | None = None) -> tuple[Path, ...]:
    candidates: list[Path] = []
    if repo_root is not None:
        candidates.append(repo_root / "configs" / "framework" / "RUNTIME_REGISTRY.json")
    candidates.append(_DEFAULT_REGISTRY_PATH)
    return tuple(dict.fromkeys(candidate.resolve() for candidate in candidates))


@lru_cache(maxsize=8)
def _load_runtime_registry_cached(cache_key: tuple[str, ...]) -> dict[str, Any]:
    for raw_path in cache_key:
        path = Path(raw_path)
        if not path.is_file():
            continue
        payload = json.loads(path.read_text(encoding="utf-8"))
        schema_version = payload.get("schema_version")
        if schema_version != RUNTIME_REGISTRY_SCHEMA_VERSION:
            raise ValueError(
                f"Unsupported runtime registry schema_version: {schema_version!r} at {path}"
            )
        return payload
    raise FileNotFoundError(
        "Could not find configs/framework/RUNTIME_REGISTRY.json in any runtime registry search path."
    )


def _load_runtime_registry_or_none(repo_root: Path | None = None) -> dict[str, Any] | None:
    cache_key = tuple(str(path) for path in _registry_candidates(repo_root))
    try:
        return deepcopy(_load_runtime_registry_cached(cache_key))
    except FileNotFoundError:
        return None


def _fallback_host_adapter_records(*, include_legacy_aliases: bool) -> tuple[dict[str, Any], ...]:
    # Import lazily so runtime_registry can still be imported by host_adapters itself.
    from codex_agno_runtime.host_adapters import list_host_adapters

    records_by_id = {
        spec.adapter_id: _clone_json_like(spec.to_dict())
        for spec in list_host_adapters(include_legacy_aliases=True)
    }
    ordered_records: list[dict[str, Any]] = []
    for adapter_id in _FALLBACK_HOST_ADAPTER_ORDER:
        record = records_by_id.get(adapter_id)
        if record is None:
            continue
        is_compatibility = adapter_id in _FALLBACK_COMPATIBILITY_ADAPTER_IDS
        if is_compatibility and not include_legacy_aliases:
            continue
        record["registry_lane"] = "compatibility" if is_compatibility else "default"
        record.update(_clone_json_like(_FALLBACK_HOST_ADAPTER_FIELD_OVERRIDES.get(adapter_id, {})))
        ordered_records.append(record)
    return tuple(ordered_records)


def runtime_registry_path(repo_root: Path | None = None) -> Path:
    for path in _registry_candidates(repo_root):
        if path.is_file():
            return path
    return _DEFAULT_REGISTRY_PATH


def load_runtime_registry(repo_root: Path | None = None) -> dict[str, Any]:
    cache_key = tuple(str(path) for path in _registry_candidates(repo_root))
    return deepcopy(_load_runtime_registry_cached(cache_key))


def host_adapter_records(
    *,
    include_legacy_aliases: bool = False,
    repo_root: Path | None = None,
) -> tuple[dict[str, Any], ...]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        return _fallback_host_adapter_records(include_legacy_aliases=include_legacy_aliases)
    rows = payload.get("host_adapters")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry host_adapters must be a list.")
    records: list[dict[str, Any]] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        lane = str(row.get("registry_lane", "default"))
        if not include_legacy_aliases and lane != "default":
            continue
        records.append(deepcopy(row))
    return tuple(records)


def host_adapter_record(
    adapter_id: str,
    *,
    include_legacy_aliases: bool = False,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    for record in host_adapter_records(
        include_legacy_aliases=include_legacy_aliases,
        repo_root=repo_root,
    ):
        if record.get("adapter_id") == adapter_id:
            return deepcopy(record)
    raise KeyError(f"Unknown runtime-registry host adapter: {adapter_id}")


def default_host_peer_set(*, repo_root: Path | None = None) -> tuple[str, ...]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        return _FALLBACK_DEFAULT_HOST_PEER_SET
    rows = payload.get("default_host_peer_set")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry default_host_peer_set must be a list.")
    return tuple(str(row) for row in rows)


def plugin_records(*, repo_root: Path | None = None) -> tuple[dict[str, Any], ...]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        return (_clone_json_like(_FALLBACK_PLUGIN_RECORD),)
    rows = payload.get("plugins")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry plugins must be a list.")
    return tuple(deepcopy(row) for row in rows if isinstance(row, dict))


def primary_plugin_record(*, repo_root: Path | None = None) -> dict[str, Any]:
    records = plugin_records(repo_root=repo_root)
    if not records:
        raise ValueError("Runtime registry must define at least one plugin record.")
    return deepcopy(records[0])


def workspace_bootstrap_defaults(*, repo_root: Path | None = None) -> dict[str, Any]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        return _clone_json_like(_FALLBACK_WORKSPACE_BOOTSTRAP_DEFAULTS)
    defaults = payload.get("workspace_bootstrap_defaults")
    if not isinstance(defaults, dict):
        raise ValueError("Runtime registry workspace_bootstrap_defaults must be an object.")
    return deepcopy(defaults)
