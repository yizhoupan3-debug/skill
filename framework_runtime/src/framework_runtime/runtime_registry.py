from __future__ import annotations

import json
from copy import deepcopy
from functools import lru_cache
from pathlib import Path
from typing import Any

from scripts.host_integration_rs import run_host_integration_rs

RUNTIME_REGISTRY_SCHEMA_VERSION = "framework-runtime-registry-v1"
_REPO_ROOT = Path(__file__).resolve().parents[3]
_DEFAULT_REGISTRY_PATH = _REPO_ROOT / "configs" / "framework" / "RUNTIME_REGISTRY.json"
_FALLBACK_DEFAULT_HOST_PEER_SET = (
    "codex_desktop_adapter",
    "codex_cli_adapter",
    "claude_code_adapter",
    "gemini_cli_adapter",
)
_FALLBACK_SHARED_PROJECT_MCP_SERVERS = (
    "browser-mcp",
    "framework-mcp",
    "openaiDeveloperDocs",
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
_FALLBACK_FRAMEWORK_NATIVE_ALIASES = {
    "autopilot": {
        "canonical_owner": "execution-controller-coding",
        "reroute_when_ambiguous": "idea-to-plan",
        "reroute_when_root_cause_unknown": "systematic-debugging",
        "upstream_source": {
            "repo": "https://github.com/Yeachan-Heo/oh-my-claudecode",
            "tag": "v4.13.2",
            "commit": "0ac52cdaa093d6c41763e47055e995adaa4f8987",
            "official_skill_path": "skills/autopilot/SKILL.md",
        },
        "omc_lineage": {
            "source": "oh-my-claudecode",
            "inherits_core_capabilities": True,
            "implementation_mode": "match-and-exceed",
        },
        "official_workflow": {
            "phases": [
                "expansion",
                "planning",
                "execution",
                "qa",
                "validation",
                "cleanup",
            ]
        },
        "implementation_bar": [
            "root-cause-first-when-unknown",
            "verification-evidence-required",
            "resume-and-recovery-required",
            "converge-until-bounded-scope-clean",
        ],
        "local_adaptations": [
            "replace .omc state files with rust-session-supervisor plus continuity artifacts",
            "replace .omc specs and plans with artifacts/current task-local bootstrap outputs",
            "keep deepinterview handoff as the first-class clarification gate for vague requests",
        ],
        "execution_owners": [
            "plan-to-code",
            "subagent-delegation",
            "execution-audit",
        ],
        "host_entrypoints": {"codex-cli": "$autopilot", "claude-code": "/autopilot"},
        "omc_dependency": False,
    },
    "deepinterview": {
        "canonical_owner": "code-review",
        "upstream_source": {
            "repo": "https://github.com/Yeachan-Heo/oh-my-claudecode",
            "tag": "v4.13.2",
            "commit": "0ac52cdaa093d6c41763e47055e995adaa4f8987",
            "official_skill_path": "skills/deep-interview/SKILL.md",
        },
        "omc_lineage": {
            "source": "oh-my-claudecode",
            "inherits_core_capabilities": True,
            "implementation_mode": "match-and-exceed",
        },
        "official_workflow": {
            "loop_rules": [
                "one-question-at-a-time",
                "target-weakest-clarity-dimension",
                "score-ambiguity-after-each-answer",
                "handoff-to-execution-only-below-threshold",
            ]
        },
        "implementation_bar": [
            "root-cause-first-when-unknown",
            "findings-first-with-severity-order",
            "verification-evidence-required",
            "fix-verify-loop-until-bounded-scope-clean",
        ],
        "local_adaptations": [
            "reuse official deep-interview questioning model but store progress in continuity artifacts instead of .omc state",
            "use live repo evidence first for brownfield clarification before asking the user",
            "handoff into local autopilot and rust-session-supervisor instead of OMC slash pipeline",
        ],
        "review_lanes": [
            "architect-review",
            "security-audit",
            "test-engineering",
            "execution-audit",
        ],
        "host_entrypoints": {"codex-cli": "$deepinterview", "claude-code": "/deepinterview"},
        "omc_dependency": False,
    },
    "team": {
        "canonical_owner": "execution-controller-coding",
        "delegation_gate": "subagent-delegation",
        "upstream_source": {
            "repo": "https://github.com/Yeachan-Heo/oh-my-claudecode",
            "tag": "v4.13.2",
            "commit": "0ac52cdaa093d6c41763e47055e995adaa4f8987",
            "official_skill_path": "skills/team/SKILL.md",
        },
        "omc_lineage": {
            "source": "oh-my-claudecode",
            "inherits_core_capabilities": True,
            "implementation_mode": "match-and-exceed",
        },
        "official_workflow": {
            "phases": [
                "scoping",
                "delegation",
                "execution",
                "integration",
                "qa",
                "cleanup",
            ]
        },
        "implementation_bar": [
            "worker-boundaries-required",
            "verification-evidence-required",
            "resume-and-recovery-required",
            "supervisor-owned-continuity",
        ],
        "local_adaptations": [
            "replace .omc team state with rust-session-supervisor and continuity artifacts",
            "keep shared continuity supervisor-owned while workers emit lane-local outputs",
            "bind worker lifecycle to host tmux and resume capabilities instead of OMC state directories",
        ],
        "execution_owners": [
            "execution-controller-coding",
            "subagent-delegation",
            "execution-audit",
        ],
        "host_entrypoints": {"codex-cli": "$team", "claude-code": "/team"},
        "omc_dependency": False,
    },
}
_FALLBACK_OMC_RETIREMENT_CONTRACT = {
    "replaced_object": "oh-my-claudecode",
    "runtime_authority": "rust-session-supervisor",
    "steady_state_forbidden_roots": [".omc"],
    "replacement_capabilities": [
        "external_session_supervisor",
        "rate_limit_auto_resume",
        "host_resume_entrypoint",
        "host_tmux_worker_management",
    ],
    "framework_native_alias_guarantees": {
        "autopilot": {
            "inherits_omc_core_capabilities": True,
            "implementation_bar": [
                "root-cause-first-when-unknown",
                "verification-evidence-required",
                "resume-and-recovery-required",
                "converge-until-bounded-scope-clean",
            ],
        },
        "deepinterview": {
            "inherits_omc_core_capabilities": True,
            "implementation_bar": [
                "root-cause-first-when-unknown",
                "findings-first-with-severity-order",
                "verification-evidence-required",
                "fix-verify-loop-until-bounded-scope-clean",
            ],
        },
        "team": {
            "inherits_omc_core_capabilities": True,
            "implementation_bar": [
                "worker-boundaries-required",
                "verification-evidence-required",
                "resume-and-recovery-required",
                "supervisor-owned-continuity",
            ],
        },
    },
    "omc_is_runtime_dependency": False,
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


def _validate_runtime_registry_payload(payload: dict[str, Any], *, source: str) -> dict[str, Any]:
    schema_version = payload.get("schema_version")
    if schema_version != RUNTIME_REGISTRY_SCHEMA_VERSION:
        raise ValueError(f"Unsupported runtime registry schema_version: {schema_version!r} at {source}")
    return payload


def _read_runtime_registry_payload(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    payload = json.loads(path.read_text(encoding="utf-8"))
    return _validate_runtime_registry_payload(payload, source=str(path))


def _rust_runtime_registry_payload(repo_root: Path | None = None) -> dict[str, Any] | None:
    registry_path = _repo_runtime_registry_path(repo_root)
    if registry_path is None:
        return None
    payload = run_host_integration_rs(
        "export-runtime-registry",
        "--repo-root",
        str(registry_path.parents[2]),
    )
    if not isinstance(payload, dict):
        raise ValueError("Rust runtime registry export must be a JSON object.")
    return _validate_runtime_registry_payload(payload, source="rust-host-integration")


def _last_resort_fallback_host_adapter_rows() -> tuple[dict[str, Any], ...]:
    # Keep one import-based fallback only for environments that lack the bundled
    # runtime registry snapshot entirely.
    from framework_runtime.host_adapters import list_host_adapters

    return tuple(
        _clone_json_like(spec.to_dict())
        for spec in list_host_adapters(include_legacy_aliases=True)
    )


def _embedded_default_runtime_registry_payload() -> dict[str, Any]:
    payload = _read_runtime_registry_payload(_DEFAULT_REGISTRY_PATH)
    if payload is not None:
        return payload
    return {
        "schema_version": RUNTIME_REGISTRY_SCHEMA_VERSION,
        "default_host_peer_set": list(_FALLBACK_DEFAULT_HOST_PEER_SET),
        "shared_project_mcp_servers": list(_FALLBACK_SHARED_PROJECT_MCP_SERVERS),
        "workspace_bootstrap_defaults": _clone_json_like(_FALLBACK_WORKSPACE_BOOTSTRAP_DEFAULTS),
        "framework_native_aliases": _clone_json_like(_FALLBACK_FRAMEWORK_NATIVE_ALIASES),
        "omc_retirement_contract": _clone_json_like(_FALLBACK_OMC_RETIREMENT_CONTRACT),
        "plugins": [_clone_json_like(_FALLBACK_PLUGIN_RECORD)],
        "host_adapters": list(_last_resort_fallback_host_adapter_rows()),
    }


_EMBEDDED_DEFAULT_RUNTIME_REGISTRY_PAYLOAD = _embedded_default_runtime_registry_payload()


@lru_cache(maxsize=8)
def _load_runtime_registry_cached(cache_key: tuple[str, ...]) -> dict[str, Any]:
    for raw_path in cache_key:
        path = Path(raw_path)
        payload = _read_runtime_registry_payload(path)
        if payload is not None:
            return payload
    raise FileNotFoundError(
        "Could not find configs/framework/RUNTIME_REGISTRY.json in any runtime registry search path."
    )


def _repo_runtime_registry_path(repo_root: Path | None) -> Path | None:
    if repo_root is None:
        return None
    path = repo_root.resolve() / "configs" / "framework" / "RUNTIME_REGISTRY.json"
    return path if path.is_file() else None


def _load_runtime_registry_or_none(repo_root: Path | None = None) -> dict[str, Any] | None:
    repo_payload = _rust_runtime_registry_payload(repo_root)
    if repo_payload is not None:
        return deepcopy(repo_payload)
    cache_key = tuple(str(path) for path in _registry_candidates(repo_root))
    try:
        return deepcopy(_load_runtime_registry_cached(cache_key))
    except FileNotFoundError:
        return None


def _fallback_host_adapter_records(*, include_legacy_aliases: bool) -> tuple[dict[str, Any], ...]:
    rows = _EMBEDDED_DEFAULT_RUNTIME_REGISTRY_PAYLOAD.get("host_adapters")
    if not isinstance(rows, list):
        return ()
    records: list[dict[str, Any]] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        lane = str(row.get("registry_lane", "default"))
        if not include_legacy_aliases and lane != "default":
            continue
        records.append(deepcopy(row))
    return tuple(records)


def runtime_registry_path(repo_root: Path | None = None) -> Path:
    for path in _registry_candidates(repo_root):
        if path.is_file():
            return path
    return _DEFAULT_REGISTRY_PATH


def load_runtime_registry(repo_root: Path | None = None) -> dict[str, Any]:
    repo_payload = _rust_runtime_registry_payload(repo_root)
    if repo_payload is not None:
        return deepcopy(repo_payload)
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


def shared_project_mcp_servers(*, repo_root: Path | None = None) -> tuple[str, ...]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        return _FALLBACK_SHARED_PROJECT_MCP_SERVERS
    rows = payload.get("shared_project_mcp_servers")
    if not isinstance(rows, list):
        raise ValueError("Runtime registry shared_project_mcp_servers must be a list.")
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


def framework_native_aliases(*, repo_root: Path | None = None) -> dict[str, Any]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        return _clone_json_like(_FALLBACK_FRAMEWORK_NATIVE_ALIASES)
    aliases = payload.get("framework_native_aliases")
    if not isinstance(aliases, dict):
        raise ValueError("Runtime registry framework_native_aliases must be an object.")
    return deepcopy(aliases)


def omc_retirement_contract(*, repo_root: Path | None = None) -> dict[str, Any]:
    payload = _load_runtime_registry_or_none(repo_root)
    if payload is None:
        return _clone_json_like(_FALLBACK_OMC_RETIREMENT_CONTRACT)
    contract = payload.get("omc_retirement_contract")
    if not isinstance(contract, dict):
        raise ValueError("Runtime registry omc_retirement_contract must be an object.")
    return deepcopy(contract)
