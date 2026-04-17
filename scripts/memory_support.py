#!/usr/bin/env python3
"""Shared helpers for the framework-local memory system CLI tools."""

from __future__ import annotations

import json
import re
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable

DEFAULT_CODEX_ROOT = Path.home() / ".codex"
DEFAULT_MEMORY_ROOT = DEFAULT_CODEX_ROOT / "memories"
CODEX_MEMORY_SUBDIR = Path(".codex") / "memory"


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
    snapshots: list[Path]
    collected_at: str


ARTIFACT_NAMES = {
    "session_summary": "SESSION_SUMMARY.md",
    "next_actions": "NEXT_ACTIONS.json",
    "evidence_index": "EVIDENCE_INDEX.json",
    "trace_metadata": "TRACE_METADATA.json",
    "supervisor_state": ".supervisor_state.json",
}


def resolve_effective_memory_dir(
    workspace: str | None = None,
    memory_root: Path | None = None,
    repo_root: Path | None = None,
) -> Path:
    """Return the effective memory directory for the shared CLI framework."""

    if repo_root is not None:
        return repo_root.expanduser().resolve() / CODEX_MEMORY_SUBDIR
    root = (memory_root or DEFAULT_MEMORY_ROOT).expanduser().resolve()
    if workspace:
        return root / safe_slug(workspace)
    return root


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


def safe_slug(value: str, fallback: str = "unknown") -> str:
    """Create a filesystem-safe slug while preserving unicode letters."""

    slug = re.sub(r"[^\w.-]+", "-", value, flags=re.UNICODE)
    slug = re.sub(r"-{2,}", "-", slug).strip("._-")
    return slug or fallback


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


def load_runtime_snapshot(source_root: Path, artifact_root: Path | None = None) -> RuntimeSnapshot:
    """Load the standard runtime artifacts used for consolidation."""

    artifact_base = (artifact_root or source_root / "artifacts").resolve()
    current_root = artifact_base / "current"
    snapshots = sorted((artifact_base / "contracts").glob("*")) if (artifact_base / "contracts").exists() else []
    return RuntimeSnapshot(
        session_summary_text=read_text_if_exists(current_root / ARTIFACT_NAMES["session_summary"]),
        next_actions=read_json_if_exists(current_root / ARTIFACT_NAMES["next_actions"]),
        evidence_index=read_json_if_exists(current_root / ARTIFACT_NAMES["evidence_index"]),
        trace_metadata=read_json_if_exists(current_root / ARTIFACT_NAMES["trace_metadata"]),
        supervisor_state=read_json_if_exists(source_root / ARTIFACT_NAMES["supervisor_state"]),
        artifact_base=artifact_base,
        current_root=current_root,
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

    items = payload.get("artifacts") or payload.get("evidence") or []
    return [item for item in items if isinstance(item, dict)]


def normalize_next_actions(payload: dict[str, Any]) -> list[str]:
    """Return next actions regardless of schema drift."""

    actions = payload.get("next_actions") or payload.get("actions") or []
    return [str(item).strip() for item in actions if str(item).strip()]


def normalize_trace_skills(payload: dict[str, Any]) -> list[str]:
    """Return skill slugs from trace metadata."""

    skills = payload.get("matched_skills") or payload.get("skills") or []
    return [str(item).strip() for item in skills if str(item).strip()]


def supervisor_contract(state: dict[str, Any]) -> dict[str, Any]:
    """Return the execution contract from the supervisor state."""

    contract = state.get("execution_contract")
    return contract if isinstance(contract, dict) else {}


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
