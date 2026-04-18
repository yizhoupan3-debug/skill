#!/usr/bin/env python3
"""Shared helpers for the framework-local memory system CLI tools."""

from __future__ import annotations

import json
import hashlib
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable

DEFAULT_CODEX_ROOT = Path.home() / ".codex"
DEFAULT_MEMORY_ROOT = DEFAULT_CODEX_ROOT / "memories"
CODEX_MEMORY_SUBDIR = Path(".codex") / "memory"
MEMORY_STATE_FILENAME = "state.json"
MEMORY_ARCHIVE_DIRNAME = "archive"
BOOTSTRAP_ARTIFACT_DIR = "bootstrap"
OPS_ARTIFACT_DIR = Path("ops") / "memory_automation"
EVIDENCE_ARTIFACT_DIR = "evidence"
SCRATCH_ARTIFACT_DIR = "scratch"
CURRENT_ALLOWED_ARTIFACT_NAMES = {
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    "active_task.json",
}
TASK_ALLOWED_ARTIFACT_NAMES = {
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    ".supervisor_state.json",
}
GENERIC_QUERY_TOKENS = {
    "context",
    "current",
    "help",
    "latest",
    "memory",
    "project",
    "repo",
    "state",
    "status",
}


@dataclass(slots=True)
class RuntimeSnapshot:
    """Normalized short-term artifact payload."""

    session_summary_text: str
    next_actions: dict[str, Any]
    evidence_index: dict[str, Any]
    trace_metadata: dict[str, Any]
    supervisor_state: dict[str, Any]
    artifact_base: Path
    current_root: Path
    mirror_root: Path
    task_root: Path
    active_task_id: str
    snapshots: list[Path]
    collected_at: str


ARTIFACT_NAMES = {
    "session_summary": "SESSION_SUMMARY.md",
    "next_actions": "NEXT_ACTIONS.json",
    "evidence_index": "EVIDENCE_INDEX.json",
    "trace_metadata": "TRACE_METADATA.json",
    "supervisor_state": ".supervisor_state.json",
}
CURRENT_ARTIFACT_DIR = "current"
ACTIVE_TASK_POINTER_NAME = "active_task.json"
NEXT_ACTIONS_SCHEMA_VERSION = "next-actions-v2"
EVIDENCE_INDEX_SCHEMA_VERSION = "evidence-index-v2"
TRACE_METADATA_SCHEMA_VERSION = "trace-metadata-v2"
SUPERVISOR_STATE_SCHEMA_VERSION = "supervisor-state-v2"
TERMINAL_STORY_STATES = {"completed", "finalized", "closed", "cancelled", "abandoned", "failed"}
TERMINAL_PHASES = {"completed", "finalized", "closed", "cancelled", "abandoned", "failed", "done"}
TERMINAL_VERIFICATION_STATUSES = {
    "completed",
    "passed",
    "verified",
    "cancelled",
    "abandoned",
    "failed",
}
STALE_STORY_STATES = {"stale", "expired", "invalid"}
ACTIVE_STORY_STATES = {"active", "in_progress", "running", "resumable"}
DEFAULT_RUNTIME_PATH = Path(__file__).resolve().parents[1] / "skills" / "SKILL_ROUTING_RUNTIME.json"


def load_routing_runtime_version(runtime_path: Path = DEFAULT_RUNTIME_PATH) -> int:
    """Load the current routing runtime version from the generated route map."""

    if not runtime_path.is_file():
        return 1
    try:
        payload = json.loads(runtime_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return 1
    value = payload.get("version")
    return value if isinstance(value, int) else 1


def resolve_effective_memory_dir(
    workspace: str | None = None,
    memory_root: Path | None = None,
    repo_root: Path | None = None,
) -> Path:
    """Return the effective memory directory for the shared CLI framework.

    When `repo_root` is provided, callers should treat `./.codex/memory/` as the
    logical framework memory root even if the path is currently backed by a
    symlink to `./memory/`.
    """

    if repo_root is not None:
        return repo_root.expanduser().resolve() / CODEX_MEMORY_SUBDIR
    root = (memory_root or DEFAULT_MEMORY_ROOT).expanduser().resolve()
    if workspace:
        return root / safe_slug(workspace)
    return root


def describe_project_local_memory_layout(repo_root: Path) -> dict[str, Any]:
    """Describe the logical and physical project-local memory roots."""

    logical_root = (repo_root.expanduser().resolve() / CODEX_MEMORY_SUBDIR)
    physical_root = logical_root.resolve() if logical_root.exists() else logical_root
    return {
        "logical_root": str(logical_root),
        "physical_root": str(physical_root),
        "is_symlink": logical_root.is_symlink(),
        "mapping_note": (
            "Treat the logical .codex/memory path and the physical target as one "
            "shared framework memory root."
        ),
    }


def describe_continuity_layout(repo_root: Path) -> dict[str, Any]:
    """Describe the task-scoped continuity truth plus compatibility mirrors."""

    root = repo_root.expanduser().resolve()
    current_root = root / "artifacts" / CURRENT_ARTIFACT_DIR
    return {
        "task_scoped_current": {
            "template": str(current_root / "<task_id>"),
            "active_task_pointer": str(current_root / ACTIVE_TASK_POINTER_NAME),
        },
        "root_task_mirror": {
            "supervisor_state": str(root / ARTIFACT_NAMES["supervisor_state"]),
            "session_summary": str(root / ARTIFACT_NAMES["session_summary"]),
            "next_actions": str(root / ARTIFACT_NAMES["next_actions"]),
            "evidence_index": str(root / ARTIFACT_NAMES["evidence_index"]),
            "trace_metadata": str(root / ARTIFACT_NAMES["trace_metadata"]),
        },
        "bridge_mirror": {
            "session_summary": str(current_root / ARTIFACT_NAMES["session_summary"]),
            "next_actions": str(current_root / ARTIFACT_NAMES["next_actions"]),
            "evidence_index": str(current_root / ARTIFACT_NAMES["evidence_index"]),
            "trace_metadata": str(current_root / ARTIFACT_NAMES["trace_metadata"]),
        },
        "artifact_lanes": {
            "bootstrap": str(root / "artifacts" / BOOTSTRAP_ARTIFACT_DIR / "<task_id>"),
            "ops_memory_automation": str(root / "artifacts" / OPS_ARTIFACT_DIR / "<run_id>"),
            "evidence": str(root / "artifacts" / EVIDENCE_ARTIFACT_DIR / "<task_id>"),
            "scratch": str(root / "artifacts" / SCRATCH_ARTIFACT_DIR / "<run_id>"),
        },
        "sync_responsibility": (
            "Supervisor writes task-scoped continuity under artifacts/current/<task_id>/ "
            "and keeps root plus artifacts/current compatibility mirrors aligned to the same task. "
            "artifacts/current/ should contain only the active-task pointer, four mirror files, "
            "and task-scoped continuity directories; bootstrap, ops, evidence, and scratch belong elsewhere."
        ),
    }


def get_repo_root() -> Path:
    """Return the repository root when possible."""

    local_root = Path(__file__).resolve().parents[1]
    if (local_root / "skills").is_dir():
        return local_root

    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        )
        return Path(proc.stdout.strip()).resolve()
    except Exception:
        return local_root


def current_local_date() -> str:
    """Return the current local date in ISO format."""

    return datetime.now().astimezone().date().isoformat()


def current_local_timestamp() -> str:
    """Return the current local timestamp in ISO format."""

    return datetime.now().astimezone().isoformat(timespec="seconds")


def read_text_if_exists(path: Path) -> str:
    """Read text from a file if it exists."""

    return path.read_text(encoding="utf-8") if path.is_file() else ""


def read_json_if_exists(path: Path) -> dict[str, Any]:
    """Read JSON from a file if it exists."""

    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}


def write_text_if_changed(path: Path, content: str) -> bool:
    """Write text only when it changed."""

    existing = read_text_if_exists(path)
    if existing == content:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return True


def write_json_if_changed(path: Path, payload: dict[str, Any] | list[Any]) -> bool:
    """Write pretty JSON only when it changed."""

    content = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    return write_text_if_changed(path, content)


def bootstrap_artifact_root(source_root: Path) -> Path:
    """Return the bootstrap artifact root."""

    return source_root.expanduser().resolve() / "artifacts" / BOOTSTRAP_ARTIFACT_DIR


def ops_memory_automation_root(source_root: Path) -> Path:
    """Return the operations artifact root for memory automation runs."""

    return source_root.expanduser().resolve() / "artifacts" / OPS_ARTIFACT_DIR


def evidence_artifact_root(source_root: Path, task_id: str | None = None) -> Path:
    """Return the evidence artifact root, optionally task-scoped."""

    root = source_root.expanduser().resolve() / "artifacts" / EVIDENCE_ARTIFACT_DIR
    return root / safe_slug(task_id) if task_id else root


def scratch_artifact_root(source_root: Path, run_id: str | None = None) -> Path:
    """Return the scratch artifact root, optionally run-scoped."""

    root = source_root.expanduser().resolve() / "artifacts" / SCRATCH_ARTIFACT_DIR
    return root / safe_slug(run_id) if run_id else root


def memory_state_path(memory_root: Path) -> Path:
    """Return the canonical memory freshness state path."""

    return memory_root / MEMORY_STATE_FILENAME


def read_memory_state(memory_root: Path) -> dict[str, Any]:
    """Read the memory freshness state payload."""

    return read_json_if_exists(memory_state_path(memory_root))


def safe_slug(value: str, fallback: str = "unknown") -> str:
    """Create a filesystem-safe slug while preserving unicode letters."""

    slug = re.sub(r"[^\w.-]+", "-", value, flags=re.UNICODE)
    slug = re.sub(r"-{2,}", "-", slug).strip("._-")
    return slug or fallback


def tokenize_query(query: str) -> list[str]:
    """Tokenize a free-form query into normalized lower-case terms."""

    return [part.lower() for part in re.split(r"[\s,/|]+", query) if part.strip()]


def is_generic_query(query: str) -> bool:
    """Return whether a query is too generic to justify active-task injection."""

    tokens = tokenize_query(query)
    if not tokens:
        return True
    if len(tokens) < 2:
        return True
    return all(token in GENERIC_QUERY_TOKENS for token in tokens)


def query_matches_task(query: str, task: str) -> bool:
    """Check whether a query clearly targets one task identity."""

    query_tokens = {safe_slug(token.casefold()) for token in query.split() if safe_slug(token.casefold())}
    task_tokens = {safe_slug(token.casefold()) for token in task.split() if safe_slug(token.casefold())}
    if not query_tokens or not task_tokens:
        return False
    if query_tokens.issubset(task_tokens) or task_tokens.issubset(query_tokens):
        return True
    return bool(query_tokens & task_tokens)


def workspace_name_from_root(repo_root: Path) -> str:
    """Derive a workspace name from a repository root path."""

    return repo_root.name


def workspace_dir(workspace: str, memory_root: Path | None = None) -> Path:
    """Return the canonical memory directory for one workspace."""

    return (memory_root or DEFAULT_MEMORY_ROOT).expanduser().resolve() / safe_slug(workspace)


def workspace_sqlite_path(workspace: str, memory_root: Path | None = None) -> Path:
    """Return the canonical SQLite path for one workspace memory store."""

    return workspace_dir(workspace, memory_root) / "memory.sqlite3"


def ensure_workspace_memory_dir(workspace: str, memory_root: Path | None = None) -> Path:
    """Create and return the canonical workspace memory directory."""

    path = workspace_dir(workspace, memory_root)
    path.mkdir(parents=True, exist_ok=True)
    return path


def _first_existing(paths: Iterable[Path]) -> Path | None:
    for path in paths:
        if path.exists():
            return path
    return None


def current_artifact_root(source_root: Path, artifact_root: Path | None = None) -> Path:
    """Return the compatibility mirror root under artifacts/current."""

    artifact_base = (artifact_root or source_root / "artifacts").resolve()
    return artifact_base / CURRENT_ARTIFACT_DIR


def active_task_pointer_path(source_root: Path, artifact_root: Path | None = None) -> Path:
    """Return the path to the active-task pointer file."""

    return current_artifact_root(source_root, artifact_root) / ACTIVE_TASK_POINTER_NAME


def task_artifact_root(
    source_root: Path,
    task_id: str,
    artifact_root: Path | None = None,
) -> Path:
    """Return the task-scoped artifact directory for one task id."""

    return current_artifact_root(source_root, artifact_root) / safe_slug(task_id)


def read_active_task_pointer(source_root: Path, artifact_root: Path | None = None) -> dict[str, Any]:
    """Read the active-task pointer if it exists."""

    return read_json_if_exists(active_task_pointer_path(source_root, artifact_root))


def build_task_id(task: str, *, created_at: str | None = None) -> str:
    """Build a stable filesystem-safe task id."""

    stamp = re.sub(r"[^0-9A-Za-z]+", "", (created_at or current_local_timestamp()))
    base = safe_slug(task or "task")
    return f"{base}-{stamp[-14:]}" if stamp else base


def build_runtime_source_hash(snapshot: RuntimeSnapshot) -> str:
    """Build a deterministic hash of the current runtime truth surfaces."""

    payload = {
        "active_task_id": snapshot.active_task_id,
        "session_summary_text": snapshot.session_summary_text,
        "next_actions": snapshot.next_actions,
        "evidence_index": snapshot.evidence_index,
        "trace_metadata": snapshot.trace_metadata,
        "supervisor_state": snapshot.supervisor_state,
    }
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def build_memory_state(snapshot: RuntimeSnapshot) -> dict[str, Any]:
    """Build the persisted freshness state for stable memory consolidation."""

    continuity = classify_runtime_continuity(snapshot)
    source_updated_at = continuity.get("continuity", {}).get("last_updated_at") or snapshot.collected_at
    return {
        "schema_version": "memory-state-v1",
        "source_task_id": snapshot.active_task_id,
        "source_task": continuity.get("task"),
        "source_phase": continuity.get("phase"),
        "source_status": continuity.get("status"),
        "continuity_state": continuity.get("state"),
        "artifact_root": str(snapshot.current_root),
        "source_updated_at": source_updated_at,
        "content_hash": build_runtime_source_hash(snapshot),
        "last_consolidated_at": current_local_timestamp(),
    }


def _memory_state_refresh_reasons(
    state: dict[str, Any],
    desired: dict[str, Any],
) -> list[str]:
    """Return reasons why `memory/state.json` should be refreshed."""

    if not state:
        return ["memory/state.json is missing"]

    reasons: list[str] = []
    if safe_slug(str(state.get("source_task_id") or "")) != safe_slug(
        str(desired.get("source_task_id") or "")
    ):
        reasons.append("memory/state.json points at a different task id")

    comparisons = {
        "source_task": "memory/state.json tracks a different task summary",
        "source_phase": "memory/state.json tracks a different phase",
        "source_status": "memory/state.json tracks a different status",
        "continuity_state": "memory/state.json tracks a different continuity state",
        "artifact_root": "memory/state.json points at a different artifact root",
        "content_hash": "runtime source hash is newer than memory/state.json",
        "source_updated_at": "continuity timestamp is newer than memory/state.json",
    }
    for key, reason in comparisons.items():
        if str(state.get(key) or "") != str(desired.get(key) or ""):
            reasons.append(reason)
    return reasons


def evaluate_memory_freshness(snapshot: RuntimeSnapshot, state: dict[str, Any]) -> dict[str, Any]:
    """Decide whether active-task recall may use the current continuity snapshot."""

    continuity = classify_runtime_continuity(snapshot)
    reasons: list[str] = []
    if continuity.get("state") != "active" or not continuity.get("current_execution"):
        reasons.append("current continuity is not active")
        return {
            "state": "blocked",
            "active_task_allowed": False,
            "reasons": reasons,
            "continuity_state": continuity.get("state"),
        }
    if not isinstance(state, dict) or not state:
        reasons.append("memory/state.json is missing")
        return {
            "state": "missing",
            "active_task_allowed": False,
            "reasons": reasons,
            "continuity_state": continuity.get("state"),
        }
    if safe_slug(str(state.get("source_task_id") or "")) != safe_slug(snapshot.active_task_id or ""):
        reasons.append("memory/state.json points at a different task id")
    expected_hash = build_runtime_source_hash(snapshot)
    if str(state.get("content_hash") or "") != expected_hash:
        reasons.append("runtime source hash is newer than memory/state.json")
    state_updated_at = _parse_iso_timestamp(state.get("source_updated_at"))
    continuity_updated_at = _parse_iso_timestamp(continuity.get("continuity", {}).get("last_updated_at"))
    if continuity_updated_at and (state_updated_at is None or continuity_updated_at > state_updated_at):
        reasons.append("continuity timestamp is newer than memory/state.json")
    freshness = "fresh" if not reasons else "stale"
    return {
        "state": freshness,
        "active_task_allowed": freshness == "fresh",
        "reasons": reasons,
        "continuity_state": continuity.get("state"),
        "source_task_id": snapshot.active_task_id,
        "state_task_id": state.get("source_task_id"),
    }


def refresh_memory_state_if_needed(snapshot: RuntimeSnapshot, memory_root: Path) -> dict[str, Any]:
    """Self-heal `memory/state.json` when the runtime snapshot is authoritative enough."""

    continuity = classify_runtime_continuity(snapshot)
    state = read_memory_state(memory_root)
    if continuity.get("state") not in {"active", "completed"}:
        return {
            "state": state,
            "refreshed": False,
            "continuity_state": continuity.get("state"),
        }
    payload = build_memory_state(snapshot)
    refresh_reasons = _memory_state_refresh_reasons(state, payload)
    if not refresh_reasons:
        return {
            "state": state,
            "refreshed": False,
            "continuity_state": continuity.get("state"),
        }
    write_json_if_changed(memory_state_path(memory_root), payload)
    return {
        "state": payload,
        "refreshed": True,
        "continuity_state": continuity.get("state"),
        "reasons": refresh_reasons,
    }


def write_active_task_pointer(
    source_root: Path,
    *,
    task_id: str,
    task: str,
    artifact_root: Path | None = None,
    updated_at: str | None = None,
) -> bool:
    """Write the active-task pointer into artifacts/current."""

    payload = {
        "task_id": safe_slug(task_id),
        "task": task,
        "updated_at": updated_at or current_local_timestamp(),
    }
    return write_json_if_changed(active_task_pointer_path(source_root, artifact_root), payload)


def _json_text(payload: dict[str, Any]) -> str:
    """Serialize JSON payloads using the shared pretty-print contract."""

    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _compatibility_mirror_outputs(
    root: Path,
    mirror_root: Path,
    *,
    summary_text: str,
    next_actions_payload: dict[str, Any],
    evidence_index: dict[str, Any],
    trace_payload: dict[str, Any],
) -> dict[Path, str]:
    """Build the root and artifacts/current compatibility mirror writes."""

    next_actions_text = _json_text(next_actions_payload)
    evidence_text = _json_text(evidence_index)
    trace_text = _json_text(trace_payload)
    return {
        root / ARTIFACT_NAMES["session_summary"]: summary_text,
        root / ARTIFACT_NAMES["next_actions"]: next_actions_text,
        root / ARTIFACT_NAMES["evidence_index"]: evidence_text,
        root / ARTIFACT_NAMES["trace_metadata"]: trace_text,
        mirror_root / ARTIFACT_NAMES["session_summary"]: summary_text,
        mirror_root / ARTIFACT_NAMES["next_actions"]: next_actions_text,
        mirror_root / ARTIFACT_NAMES["evidence_index"]: evidence_text,
        mirror_root / ARTIFACT_NAMES["trace_metadata"]: trace_text,
    }


def _fallback_route_from_supervisor(supervisor_state: dict[str, Any]) -> list[str]:
    """Derive a minimal route list when trace metadata is missing."""

    controller = supervisor_state.get("controller")
    controller = controller if isinstance(controller, dict) else {}
    return stable_line_items(
        [
            _text(controller.get("gate")),
            _text(controller.get("primary_owner")),
            _text(controller.get("overlay")),
            _text(controller.get("owner_lane")),
            _text(supervisor_state.get("primary_owner")),
        ]
    )


def _synthesized_status(supervisor_state: dict[str, Any]) -> str:
    """Return the best available lifecycle status from the supervisor state."""

    verification = supervisor_state.get("verification")
    verification = verification if isinstance(verification, dict) else {}
    continuity = supervisor_state.get("continuity")
    continuity = continuity if isinstance(continuity, dict) else {}
    return (
        _text(verification.get("verification_status"))
        or _text(continuity.get("story_state"))
        or _text(supervisor_state.get("active_phase"))
        or "in_progress"
    )


def _synthesized_summary(supervisor_state: dict[str, Any]) -> str:
    """Build a minimal summary when task-scoped continuity files are missing."""

    verification = supervisor_state.get("verification")
    verification = verification if isinstance(verification, dict) else {}
    continuity = supervisor_state.get("continuity")
    continuity = continuity if isinstance(continuity, dict) else {}
    contract = supervisor_contract(supervisor_state)
    return (
        _text(verification.get("last_verification_summary"))
        or _text(continuity.get("state_reason"))
        or _text(contract.get("goal"))
        or "Recovered from the authoritative root supervisor state."
    )


def _synthesized_next_actions_payload(next_actions: list[str]) -> dict[str, Any]:
    """Build the canonical next-actions payload from supervisor-derived actions."""

    return {
        "schema_version": NEXT_ACTIONS_SCHEMA_VERSION,
        "next_actions": next_actions,
    }


def _synthesized_trace_payload(
    *,
    supervisor_state: dict[str, Any],
    task: str,
    task_root: Path,
    route: list[str],
    status: str,
) -> dict[str, Any]:
    """Build the canonical trace payload from the authoritative supervisor state."""

    controller = supervisor_state.get("controller")
    controller = controller if isinstance(controller, dict) else {}
    return {
        "schema_version": TRACE_METADATA_SCHEMA_VERSION,
        "ts": current_local_timestamp(),
        "task": task,
        "framework_version": "phase1",
        "routing_runtime_version": load_routing_runtime_version(),
        "matched_skills": route,
        "decision": {
            "owner": _text(supervisor_state.get("primary_owner"))
            or _text(controller.get("primary_owner"))
            or _text(controller.get("owner_lane")),
            "gate": _text(controller.get("gate")) or "none",
            "overlay": _text(controller.get("overlay")) or None,
        },
        "reroute_count": 0,
        "retry_count": 0,
        "artifact_paths": [
            ARTIFACT_NAMES["session_summary"],
            ARTIFACT_NAMES["next_actions"],
            ARTIFACT_NAMES["evidence_index"],
            ARTIFACT_NAMES["trace_metadata"],
            ARTIFACT_NAMES["supervisor_state"],
            str(task_root / ARTIFACT_NAMES["session_summary"]),
            str(task_root / ARTIFACT_NAMES["next_actions"]),
            str(task_root / ARTIFACT_NAMES["evidence_index"]),
            str(task_root / ARTIFACT_NAMES["trace_metadata"]),
        ],
        "verification_status": status,
    }


def repair_runtime_continuity_artifacts(
    source_root: Path,
    artifact_root: Path | None = None,
    *,
    supervisor_state: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Repair task-scoped continuity artifacts from the authoritative supervisor state.

    This keeps `artifacts/current/active_task.json`, the task-scoped directory, and the
    bridge mirror aligned to the same task identity instead of allowing mixed-task reads.
    """

    root = source_root.expanduser().resolve()
    artifact_base = (artifact_root or root / "artifacts").resolve()
    mirror_root = artifact_base / CURRENT_ARTIFACT_DIR
    supervisor = normalize_supervisor_state(
        supervisor_state or read_json_if_exists(root / ARTIFACT_NAMES["supervisor_state"])
    )
    continuity = supervisor.get("continuity")
    continuity = dict(continuity) if isinstance(continuity, dict) else {}
    verification = supervisor.get("verification")
    verification = verification if isinstance(verification, dict) else {}
    if (
        _bool_or_none(continuity.get("resume_allowed")) is True
        and (
            _is_terminal(supervisor.get("active_phase"), TERMINAL_PHASES)
            or _is_terminal(verification.get("verification_status"), TERMINAL_VERIFICATION_STATUSES)
            or _is_terminal(continuity.get("story_state"), TERMINAL_STORY_STATES)
        )
    ):
        continuity["resume_allowed"] = False
        supervisor["continuity"] = continuity
        write_json_if_changed(root / ARTIFACT_NAMES["supervisor_state"], supervisor)
    task_id = safe_slug(_text(supervisor.get("task_id")))
    task = _text(supervisor.get("task_summary")) or task_id
    if not task_id or not task:
        return {"repaired": False, "reason": "missing supervisor task identity"}

    task_root = task_artifact_root(root, task_id, artifact_root)
    pointer = read_active_task_pointer(root, artifact_root)
    pointer_task_id = safe_slug(_text(pointer.get("task_id")))
    route = _fallback_route_from_supervisor(supervisor)
    phase = _text(supervisor.get("active_phase")) or "implementation"
    status = _synthesized_status(supervisor)
    next_actions = stable_line_items(
        str(item).strip()
        for item in supervisor.get("next_actions", [])
        if str(item).strip()
    )
    evidence_index = {"schema_version": EVIDENCE_INDEX_SCHEMA_VERSION, "artifacts": []}
    source_summary_path = task_root / ARTIFACT_NAMES["session_summary"]
    source_next_actions_path = task_root / ARTIFACT_NAMES["next_actions"]
    source_evidence_path = task_root / ARTIFACT_NAMES["evidence_index"]
    source_trace_path = task_root / ARTIFACT_NAMES["trace_metadata"]
    next_actions_payload = _synthesized_next_actions_payload(next_actions)
    trace_payload = _synthesized_trace_payload(
        supervisor_state=supervisor,
        task=task,
        task_root=task_root,
        route=route,
        status=status,
    )
    existing_summary = read_text_if_exists(source_summary_path)
    existing_fields = parse_session_summary(existing_summary)
    existing_conflicts = bool(
        existing_summary.strip()
        and (
            not _looks_same_identity(existing_fields.get("task"), task)
            or (
                _text(existing_fields.get("phase"))
                and _text(existing_fields.get("phase")) != phase
            )
            or (
                _text(existing_fields.get("status"))
                and _text(existing_fields.get("status")) != status
            )
        )
    )
    changed = False

    if task_root.is_dir() and source_summary_path.is_file() and not existing_conflicts:
        summary_text = existing_summary
        existing_next_actions_payload = read_json_if_exists(source_next_actions_path)
        if existing_next_actions_payload:
            next_actions = normalize_next_actions(existing_next_actions_payload)
            next_actions_payload = existing_next_actions_payload
        else:
            changed = write_json_if_changed(source_next_actions_path, next_actions_payload) or changed
        evidence_payload = read_json_if_exists(source_evidence_path)
        if evidence_payload:
            evidence_index = evidence_payload
        else:
            changed = write_json_if_changed(source_evidence_path, evidence_index) or changed
        existing_trace_payload = read_json_if_exists(source_trace_path)
        if existing_trace_payload:
            trace_payload = existing_trace_payload
        else:
            changed = write_json_if_changed(source_trace_path, trace_payload) or changed
    else:
        task_root.mkdir(parents=True, exist_ok=True)
        summary_text = "\n".join(
            [
                "# SESSION_SUMMARY",
                "",
                f"- task: {task}",
                f"- phase: {phase}",
                f"- status: {status}",
                "",
                "## Summary",
                _synthesized_summary(supervisor),
                "",
            ]
        )
        changed = write_text_if_changed(source_summary_path, summary_text) or changed
        changed = write_json_if_changed(source_next_actions_path, next_actions_payload) or changed
        changed = write_json_if_changed(source_evidence_path, evidence_index) or changed
        changed = write_json_if_changed(source_trace_path, trace_payload) or changed
        changed = write_json_if_changed(task_root / ARTIFACT_NAMES["supervisor_state"], supervisor) or changed

    mirror_root.mkdir(parents=True, exist_ok=True)
    repaired_paths = [
        source_summary_path,
        source_next_actions_path,
        source_evidence_path,
        source_trace_path,
    ]
    mirror_outputs = _compatibility_mirror_outputs(
        root,
        mirror_root,
        summary_text=summary_text,
        next_actions_payload=next_actions_payload,
        evidence_index=evidence_index,
        trace_payload=trace_payload,
    )
    for path, content in mirror_outputs.items():
        changed = write_text_if_changed(path, content) or changed
    changed = write_active_task_pointer(root, task_id=task_id, task=task, artifact_root=artifact_root) or changed
    changed = write_json_if_changed(task_root / ARTIFACT_NAMES["supervisor_state"], supervisor) or changed

    return {
        "repaired": changed or pointer_task_id != task_id,
        "task_id": task_id,
        "task": task,
        "task_root": str(task_root),
        "mirror_root": str(mirror_root),
        "pointer_task_id": pointer_task_id or None,
        "route_fallback": route,
        "repaired_paths": [str(path) for path in repaired_paths] + [str(path) for path in mirror_outputs],
        "supervisor_state": supervisor,
    }


def resolve_active_task_id(
    source_root: Path,
    artifact_root: Path | None = None,
    *,
    supervisor_state: dict[str, Any] | None = None,
) -> str:
    """Resolve the active task id from supervisor state first, then pointer."""

    state = normalize_supervisor_state(supervisor_state or {})
    direct = safe_slug(_text(state.get("task_id")))
    if direct:
        return direct
    pointer = read_active_task_pointer(source_root, artifact_root)
    return safe_slug(_text(pointer.get("task_id")))


def load_runtime_snapshot(source_root: Path, artifact_root: Path | None = None) -> RuntimeSnapshot:
    """Load the standard runtime artifacts used for consolidation.

    The bridge-facing read path comes from `artifacts/current/*`, while
    `.supervisor_state.json` remains the authoritative root-level execution
    anchor.
    """

    artifact_base = (artifact_root or source_root / "artifacts").resolve()
    mirror_root = artifact_base / CURRENT_ARTIFACT_DIR
    snapshots = sorted((artifact_base / "contracts").glob("*")) if (artifact_base / "contracts").exists() else []
    supervisor_state = normalize_supervisor_state(
        read_json_if_exists(source_root / ARTIFACT_NAMES["supervisor_state"])
    )
    repair_result = repair_runtime_continuity_artifacts(
        source_root,
        artifact_root,
        supervisor_state=supervisor_state,
    )
    supervisor_state = normalize_supervisor_state(
        (
            repair_result.get("supervisor_state")
            if isinstance(repair_result, dict)
            else None
        )
        or read_json_if_exists(source_root / ARTIFACT_NAMES["supervisor_state"])
    )
    active_task_id = resolve_active_task_id(
        source_root,
        artifact_root,
        supervisor_state=supervisor_state,
    )
    task_root = task_artifact_root(source_root, active_task_id, artifact_root) if active_task_id else mirror_root
    pointer = read_active_task_pointer(source_root, artifact_root)
    pointer_task_id = safe_slug(_text(pointer.get("task_id")))
    mirror_matches_selected = bool(active_task_id and active_task_id == pointer_task_id)
    if task_root.exists():
        preferred_root = task_root
    elif not active_task_id:
        preferred_root = mirror_root
    elif mirror_matches_selected:
        preferred_root = mirror_root
    else:
        preferred_root = task_root

    def _read_task_or_mirror(name: str) -> Path:
        return _first_existing(
            [
                preferred_root / ARTIFACT_NAMES[name],
                mirror_root / ARTIFACT_NAMES[name],
            ]
        ) or (preferred_root / ARTIFACT_NAMES[name])

    return RuntimeSnapshot(
        session_summary_text=read_text_if_exists(_read_task_or_mirror("session_summary")),
        next_actions=read_json_if_exists(_read_task_or_mirror("next_actions")),
        evidence_index=read_json_if_exists(_read_task_or_mirror("evidence_index")),
        trace_metadata=read_json_if_exists(_read_task_or_mirror("trace_metadata")),
        supervisor_state=supervisor_state,
        artifact_base=artifact_base,
        current_root=preferred_root,
        mirror_root=mirror_root,
        task_root=task_root,
        active_task_id=active_task_id,
        snapshots=snapshots,
        collected_at=current_local_timestamp(),
    )


def _parse_bullet_kv(text: str) -> dict[str, str]:
    """Parse simple markdown bullet key/value pairs."""

    result: dict[str, str] = {}
    for line in text.splitlines():
        if not line.startswith("- "):
            continue
        body = line[2:]
        if ":" not in body:
            continue
        key, value = body.split(":", 1)
        result[key.strip()] = value.strip()
    return result


def parse_session_summary(text: str) -> dict[str, str]:
    """Parse the current session summary markdown."""

    return _parse_bullet_kv(text)


def normalize_evidence_index(payload: dict[str, Any]) -> list[dict[str, Any]]:
    """Return evidence rows regardless of schema drift."""

    if payload.get("schema_version") == EVIDENCE_INDEX_SCHEMA_VERSION:
        items = payload.get("artifacts") or []
    else:
        items = payload.get("artifacts") or payload.get("evidence") or []
    return [item for item in items if isinstance(item, dict)]


def normalize_next_actions(payload: dict[str, Any]) -> list[str]:
    """Return next actions regardless of schema drift."""

    if payload.get("schema_version") == NEXT_ACTIONS_SCHEMA_VERSION:
        actions = payload.get("next_actions") or []
    else:
        actions = payload.get("next_actions") or payload.get("actions") or []
    return [str(item).strip() for item in actions if str(item).strip()]


def normalize_trace_skills(payload: dict[str, Any]) -> list[str]:
    """Return skill slugs from trace metadata."""

    if payload.get("schema_version") == TRACE_METADATA_SCHEMA_VERSION:
        skills = payload.get("matched_skills") or []
    else:
        skills = payload.get("matched_skills") or payload.get("skills") or []
    return [str(item).strip() for item in skills if str(item).strip()]


def normalize_supervisor_state(payload: dict[str, Any]) -> dict[str, Any]:
    """Normalize the supervisor state into the canonical nested v2 contract."""

    if not isinstance(payload, dict):
        return {}

    normalized = dict(payload)
    normalized["schema_version"] = SUPERVISOR_STATE_SCHEMA_VERSION

    delegation = payload.get("delegation")
    if not isinstance(delegation, dict):
        delegation = {
            "delegation_plan_created": payload.get("delegation_plan_created"),
            "spawn_attempted": payload.get("spawn_attempted"),
            "spawn_block_reason": payload.get("spawn_block_reason"),
            "fallback_mode": payload.get("fallback_mode"),
            "delegated_sidecars": payload.get("delegated_sidecars", []),
        }
    normalized["delegation"] = delegation

    verification = payload.get("verification")
    if not isinstance(verification, dict):
        verification = {
            "verification_status": payload.get("verification_status"),
            "last_verification_summary": payload.get("last_verification_summary"),
        }
    normalized["verification"] = verification

    continuity = payload.get("continuity")
    if not isinstance(continuity, dict):
        continuity = {
            "story_state": payload.get("story_state"),
            "resume_allowed": payload.get("resume_allowed"),
            "last_updated_at": payload.get("last_updated_at"),
            "active_lease_expires_at": payload.get("active_lease_expires_at"),
            "state_reason": payload.get("state_reason"),
        }
    else:
        continuity = dict(continuity)
    continuity.setdefault("story_state", payload.get("story_state"))
    continuity.setdefault("resume_allowed", payload.get("resume_allowed"))
    continuity.setdefault("last_updated_at", payload.get("last_updated_at"))
    continuity.setdefault("active_lease_expires_at", payload.get("active_lease_expires_at"))
    continuity.setdefault("state_reason", payload.get("state_reason"))
    normalized["continuity"] = continuity

    blockers = payload.get("blockers")
    if not isinstance(blockers, dict):
        blockers = {
            "open_blockers": payload.get("open_blockers", []),
        }
    normalized["blockers"] = blockers
    return normalized


def supervisor_contract(state: dict[str, Any]) -> dict[str, Any]:
    """Return the execution contract from the supervisor state."""

    contract = state.get("execution_contract")
    return contract if isinstance(contract, dict) else {}


def _text(value: Any) -> str:
    return str(value or "").strip()


def _bool_or_none(value: Any) -> bool | None:
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        lowered = value.strip().casefold()
        if lowered in {"true", "1", "yes"}:
            return True
        if lowered in {"false", "0", "no"}:
            return False
    return None


def _normalized_token(value: Any) -> str:
    text = _text(value)
    return safe_slug(text.casefold()) if text else ""


def _looks_same_identity(left: Any, right: Any) -> bool:
    left_token = _normalized_token(left)
    right_token = _normalized_token(right)
    if not left_token or not right_token:
        return True
    return left_token == right_token or left_token in right_token or right_token in left_token


def _parse_iso_timestamp(value: Any) -> datetime | None:
    text = _text(value)
    if not text:
        return None
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        return datetime.fromisoformat(text)
    except ValueError:
        return None


def _is_terminal(value: Any, terminal_values: set[str]) -> bool:
    return _text(value).casefold() in terminal_values


def classify_runtime_continuity(snapshot: RuntimeSnapshot) -> dict[str, Any]:
    """Classify whether the current runtime snapshot is safe to inject as active continuity."""

    summary = parse_session_summary(snapshot.session_summary_text)
    supervisor = snapshot.supervisor_state if isinstance(snapshot.supervisor_state, dict) else {}
    verification = supervisor.get("verification") if isinstance(supervisor.get("verification"), dict) else {}
    continuity = supervisor.get("continuity") if isinstance(supervisor.get("continuity"), dict) else {}
    contract = supervisor_contract(supervisor)
    trace_task = _text(snapshot.trace_metadata.get("task")) if isinstance(snapshot.trace_metadata, dict) else ""
    summary_task = _text(summary.get("task"))
    supervisor_task = _text(supervisor.get("task_summary")) or _text(supervisor.get("task_id"))
    task = summary_task or trace_task or supervisor_task
    summary_phase = _text(summary.get("phase"))
    supervisor_phase = _text(supervisor.get("active_phase"))
    phase = summary_phase or supervisor_phase
    verification_status = _text(verification.get("verification_status"))
    summary_status = _text(summary.get("status"))
    story_state = _text(continuity.get("story_state"))
    status = summary_status or verification_status or story_state
    route = normalize_trace_skills(snapshot.trace_metadata)
    next_actions = normalize_next_actions(snapshot.next_actions)
    if not next_actions:
        next_actions = stable_line_items(
            str(item).strip()
            for item in supervisor.get("next_actions", [])
            if str(item).strip()
        )
    blockers = stable_line_items(
        str(item).strip()
        for item in (supervisor.get("blockers", {}) or {}).get("open_blockers", [])
        if str(item).strip()
    )
    scope = [str(item).strip() for item in contract.get("scope", []) if str(item).strip()]
    forbidden_scope = [
        str(item).strip() for item in contract.get("forbidden_scope", []) if str(item).strip()
    ]
    acceptance_criteria = [
        str(item).strip()
        for item in contract.get("acceptance_criteria", [])
        if str(item).strip()
    ]
    evidence_required = [
        str(item).strip()
        for item in contract.get("evidence_required", [])
        if str(item).strip()
    ]
    summary_terminal = _is_terminal(summary_phase, TERMINAL_PHASES) or _is_terminal(
        summary_status, TERMINAL_VERIFICATION_STATUSES
    )
    supervisor_terminal = (
        _is_terminal(supervisor_phase, TERMINAL_PHASES)
        or _is_terminal(verification_status, TERMINAL_VERIFICATION_STATUSES)
        or _is_terminal(story_state, TERMINAL_STORY_STATES)
    )
    terminal_reasons = stable_line_items(
        [
            f"summary phase is terminal: {summary_phase}" if _is_terminal(summary_phase, TERMINAL_PHASES) else "",
            f"summary status is terminal: {summary_status}"
            if _is_terminal(summary_status, TERMINAL_VERIFICATION_STATUSES)
            else "",
            f"supervisor phase is terminal: {supervisor_phase}"
            if _is_terminal(supervisor_phase, TERMINAL_PHASES)
            else "",
            f"verification status is terminal: {verification_status}"
            if _is_terminal(verification_status, TERMINAL_VERIFICATION_STATUSES)
            else "",
            f"continuity story_state is terminal: {story_state}"
            if _is_terminal(story_state, TERMINAL_STORY_STATES)
            else "",
        ]
    )
    inconsistency_reasons = stable_line_items(
        [
            (
                f"session summary task '{summary_task}' disagrees with trace task '{trace_task}'"
                if summary_task and trace_task and not _looks_same_identity(summary_task, trace_task)
                else ""
            ),
            (
                "session summary marks the task terminal while supervisor still looks active"
                if summary_terminal and not supervisor_terminal and (supervisor_phase or verification_status)
                else ""
            ),
            (
                "supervisor marks the task terminal while the session summary still looks active"
                if supervisor_terminal and not summary_terminal and (summary_phase or summary_status)
                else ""
            ),
            (
                "continuity.resume_allowed=true conflicts with terminal lifecycle metadata"
                if _bool_or_none(continuity.get("resume_allowed")) is True and terminal_reasons
                else ""
            ),
        ]
    )
    active_lease_expires_at = _parse_iso_timestamp(continuity.get("active_lease_expires_at"))
    stale_reasons = stable_line_items(
        [
            (
                f"continuity story_state is stale: {story_state}"
                if _is_terminal(story_state, STALE_STORY_STATES)
                else ""
            ),
            (
                "continuity explicitly disallows resume"
                if _bool_or_none(continuity.get("resume_allowed")) is False and not terminal_reasons
                else ""
            ),
            (
                f"active lease expired at {continuity.get('active_lease_expires_at')}"
                if active_lease_expires_at is not None
                and active_lease_expires_at < datetime.now(active_lease_expires_at.tzinfo)
                else ""
            ),
            (
                "session summary mirror is missing while supervisor still looks active"
                if not snapshot.session_summary_text.strip()
                and not terminal_reasons
                and (task or supervisor_phase or verification_status or next_actions)
                else ""
            ),
            (
                f"state reason: {_text(continuity.get('state_reason'))}"
                if _text(continuity.get("state_reason"))
                and (
                    _is_terminal(story_state, STALE_STORY_STATES)
                    or _bool_or_none(continuity.get("resume_allowed")) is False
                )
                else ""
            ),
        ]
    )
    current_execution = {
        "task": task,
        "phase": phase,
        "status": status or ("in_progress" if task or next_actions or blockers else ""),
        "route": route,
        "next_actions": next_actions,
        "blockers": blockers,
        "scope": scope,
        "forbidden_scope": forbidden_scope,
        "acceptance_criteria": acceptance_criteria,
        "evidence_required": evidence_required,
    }
    recent_completed_execution = {
        "task": task,
        "phase": phase or story_state or supervisor_phase,
        "status": status or "completed",
        "route": route,
        "follow_up_notes": next_actions,
        "terminal_reasons": terminal_reasons,
    }
    has_any_runtime_signal = any(
        [
            snapshot.session_summary_text.strip(),
            snapshot.next_actions,
            snapshot.evidence_index,
            snapshot.trace_metadata,
            supervisor,
        ]
    )
    if not has_any_runtime_signal:
        state = "missing"
    elif inconsistency_reasons:
        state = "inconsistent"
    elif terminal_reasons:
        state = "completed"
    elif stale_reasons:
        state = "stale"
    else:
        state = "active"
    recovery_hints = {
        "missing": [
            "Refresh SESSION_SUMMARY.md, NEXT_ACTIONS.json, TRACE_METADATA.json, and .supervisor_state.json before injecting continuity.",
        ],
        "active": [],
        "completed": [
            "Keep this task only as recent-completed context; do not inject it as current execution.",
            "Start a new standalone task before resuming related work.",
        ],
        "stale": [
            "Re-read the live continuity artifacts and rebuild a fresh active task before injecting execution context.",
            "Do not continue from the stale snapshot without a new supervisor-owned continuity refresh.",
        ],
        "inconsistent": [
            "Reconcile SESSION_SUMMARY.md, TRACE_METADATA.json, and .supervisor_state.json before injecting continuity.",
            "Treat the current snapshot as blocked until the supervisor rewrites a consistent continuity bundle.",
        ],
    }[state]
    return {
        "state": state,
        "can_resume": state == "active",
        "task": task,
        "phase": phase,
        "status": status,
        "route": route,
        "next_actions": next_actions,
        "blockers": blockers,
        "current_execution": current_execution if state == "active" and task else None,
        "recent_completed_execution": recent_completed_execution if state == "completed" and task else None,
        "stale_reasons": stale_reasons,
        "terminal_reasons": terminal_reasons,
        "inconsistency_reasons": inconsistency_reasons,
        "recovery_hints": recovery_hints,
        "continuity": {
            "story_state": story_state or None,
            "resume_allowed": _bool_or_none(continuity.get("resume_allowed")),
            "last_updated_at": _text(continuity.get("last_updated_at")) or None,
            "active_lease_expires_at": _text(continuity.get("active_lease_expires_at")) or None,
            "state_reason": _text(continuity.get("state_reason")) or None,
        },
        "summary_fields": summary,
        "paths": {
            "session_summary": str(snapshot.current_root / ARTIFACT_NAMES["session_summary"]),
            "next_actions": str(snapshot.current_root / ARTIFACT_NAMES["next_actions"]),
            "evidence_index": str(snapshot.current_root / ARTIFACT_NAMES["evidence_index"]),
            "trace_metadata": str(snapshot.current_root / ARTIFACT_NAMES["trace_metadata"]),
            "task_root": str(snapshot.task_root),
            "bridge_mirror_root": str(snapshot.mirror_root),
            "supervisor_state": str(snapshot.artifact_base.parent / ARTIFACT_NAMES["supervisor_state"]),
        },
    }


def stable_line_items(items: Iterable[str]) -> list[str]:
    """Deduplicate and trim a list of human-readable lines."""

    seen: set[str] = set()
    result: list[str] = []
    for item in items:
        value = str(item).strip()
        if not value or value in seen:
            continue
        seen.add(value)
        result.append(value)
    return result


def format_bullets(items: Iterable[str]) -> str:
    """Format bullet items as markdown."""

    lines = stable_line_items(items)
    return "\n".join(f"- {line}" for line in lines) if lines else "- 暂无"


def markdown_block(title: str, bullets: Iterable[str]) -> str:
    """Render a heading plus bullet list."""

    return f"## {title}\n\n{format_bullets(bullets)}\n"


def add_section(lines: list[str], title: str, bullet_lines: Iterable[str]) -> None:
    """Append a markdown section to a list of lines."""

    lines.extend([f"## {title}", "", format_bullets(bullet_lines), ""])


def memory_workspace_root(workspace: str, repo_root: Path | None = None) -> Path:
    """Return the effective memory root for the workspace."""

    return resolve_effective_memory_dir(workspace=workspace, repo_root=repo_root)


def latest_session_note_path(workspace: str, repo_root: Path | None = None) -> Path:
    """Return the canonical session note path for today."""

    base = memory_workspace_root(workspace, repo_root=repo_root) / "sessions"
    base.mkdir(parents=True, exist_ok=True)
    return base / f"{current_local_date()}.md"
