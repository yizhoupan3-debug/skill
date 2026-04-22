#!/usr/bin/env python3
"""Rust-first skill lookup and route transport shim."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = ROOT / "codex_agno_runtime" / "src"
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.schemas import RouteDecisionContract


def get_git_root() -> Path:
    """Return the repository root when available."""

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
        return Path(proc.stdout.strip())
    except Exception:
        return local_root

ROOT = get_git_root()
RUNTIME_PATH = ROOT / "skills" / "SKILL_ROUTING_RUNTIME.json"
MANIFEST_PATH = ROOT / "skills" / "SKILL_MANIFEST.json"
ROUTER_RS_DIR = ROOT / "scripts" / "router-rs"
ROUTER_RS_RELEASE_BIN = ROUTER_RS_DIR / "target" / "release" / "router-rs"
ROUTER_RS_DEBUG_BIN = ROUTER_RS_DIR / "target" / "debug" / "router-rs"


@dataclass
class SkillRecord:
    """Represent one searchable and routable skill row."""

    slug: str
    layer: str
    gate: str
    owner: str
    summary: str
    trigger_hints: list[str]
    health: float
    priority: str = "P2"
    session_start: str = "n/a"


@dataclass
class MatchResult:
    """Represent one ranked route match."""

    record: SkillRecord
    score: float
    matched_terms: int
    total_terms: int


def _load_manifest_route_meta(path: Path) -> dict[str, tuple[str, str]]:
    """Load `priority` and `session_start` metadata from manifest rows."""

    payload = json.loads(path.read_text(encoding="utf-8"))
    rows = payload.get("skills")
    keys = payload.get("keys")
    if not isinstance(rows, list) or not isinstance(keys, list):
        return {}

    index = {str(key): idx for idx, key in enumerate(keys)}
    idx_slug = index.get("slug")
    idx_priority = index.get("priority")
    idx_session_start = index.get("session_start")
    if idx_slug is None:
        return {}

    meta: dict[str, tuple[str, str]] = {}
    for row in rows:
        if not isinstance(row, list) or len(row) <= idx_slug:
            continue
        slug = str(row[idx_slug])
        priority = str(row[idx_priority]) if idx_priority is not None and len(row) > idx_priority else "P2"
        session_start = (
            str(row[idx_session_start]) if idx_session_start is not None and len(row) > idx_session_start else "n/a"
        )
        meta[slug] = (priority or "P2", session_start or "n/a")
    return meta


def _load_records_from_index(index_path: Path, summary_key: str) -> list[SkillRecord]:
    """Load searchable skill rows from a keyed routing index."""

    payload = json.loads(index_path.read_text(encoding="utf-8"))
    rows = payload.get("skills")
    keys = payload.get("keys")
    if not isinstance(rows, list) or not isinstance(keys, list):
        raise ValueError(f"{index_path} is missing keyed routing rows")

    index = {str(key): idx for idx, key in enumerate(keys)}
    trigger_key = "trigger_hints" if "trigger_hints" in index else "triggers"
    required = ("slug", "layer", "owner", "gate", summary_key, trigger_key, "health")
    missing = [key for key in required if key not in index]
    if missing:
        missing_str = ", ".join(missing)
        raise ValueError(f"{index_path} is missing routing keys: {missing_str}")

    idx_slug = index["slug"]
    idx_layer = index["layer"]
    idx_owner = index["owner"]
    idx_gate = index["gate"]
    idx_summary = index[summary_key]
    idx_trigger_hints = index[trigger_key]
    idx_health = index["health"]
    idx_priority = index.get("priority")
    idx_session_start = index.get("session_start")
    max_index = max(
        idx_slug,
        idx_layer,
        idx_owner,
        idx_gate,
        idx_summary,
        idx_trigger_hints,
        idx_health,
        idx_priority if idx_priority is not None else 0,
        idx_session_start if idx_session_start is not None else 0,
    )

    records: list[SkillRecord] = []
    for row in rows:
        if not isinstance(row, list) or len(row) <= max_index:
            continue
        priority = str(row[idx_priority]) if idx_priority is not None and len(row) > idx_priority else "P2"
        session_start = (
            str(row[idx_session_start]) if idx_session_start is not None and len(row) > idx_session_start else "n/a"
        )
        records.append(
            SkillRecord(
                slug=str(row[idx_slug]),
                layer=str(row[idx_layer]),
                gate=str(row[idx_gate]),
                owner=str(row[idx_owner]),
                summary=str(row[idx_summary]),
                trigger_hints=_normalize_trigger_hints(row[idx_trigger_hints]),
                health=float(row[idx_health]),
                priority=priority or "P2",
                session_start=session_start or "n/a",
            )
        )
    return records


def _normalize_trigger_hints(value: object) -> list[str]:
    """Normalize trigger hints loaded from compiled routing artifacts."""

    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    raw = str(value).strip()
    if not raw:
        return []
    if "/" in raw:
        return [part.strip() for part in raw.split("/") if part.strip()]
    return [raw]


def load_records(
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
) -> list[SkillRecord]:
    """Load searchable skill records from the runtime or manifest index."""

    runtime_target = runtime_path if runtime_path is not None else RUNTIME_PATH
    manifest_target = manifest_path if manifest_path is not None else MANIFEST_PATH

    if runtime_target.exists():
        records = _load_records_from_index(runtime_target, summary_key="summary")
        if manifest_target.exists():
            route_meta = _load_manifest_route_meta(manifest_target)
            for record in records:
                if record.slug in route_meta:
                    record.priority, record.session_start = route_meta[record.slug]
        return records
    if manifest_target.exists():
        return _load_records_from_index(manifest_target, summary_key="description")

    raise RuntimeError(f"No routing index found at {runtime_target} or {manifest_target}.")


def _hydrate_match_results(
    payload: list[dict[str, object]],
    *,
    runtime_path: Path | None,
    manifest_path: Path | None,
) -> list[MatchResult]:
    """Convert Rust JSON payloads back into Python `MatchResult` rows."""

    records = {
        record.slug: record
        for record in load_records(runtime_path=runtime_path, manifest_path=manifest_path)
    }
    hydrated: list[MatchResult] = []
    for row in payload:
        slug = str(row["slug"])
        record = records.get(slug)
        if record is None:
            continue
        hydrated.append(
            MatchResult(
                record=record,
                score=float(row["score"]),
                matched_terms=int(row["matched_terms"]),
                total_terms=int(row["total_terms"]),
            )
        )
    return hydrated


def resolve_router_binary() -> Path | None:
    """Return the compiled Rust router when available."""

    for candidate in (ROUTER_RS_DEBUG_BIN, ROUTER_RS_RELEASE_BIN):
        if candidate.is_file():
            return candidate
    return None


def build_rust_router_command(
    *,
    query: str,
    limit: int,
    runtime_path: Path | None,
    manifest_path: Path | None,
    json_output: bool = False,
    route_json: bool = False,
    session_id: str = "route-cli",
    allow_overlay: bool = True,
    first_turn: bool = True,
) -> list[str]:
    """Build command line arguments for Rust router execution."""

    command = [
        "--query",
        query,
        "--limit",
        str(limit),
    ]

    if runtime_path is not None:
        command.extend(["--runtime", str(runtime_path)])
    if manifest_path is not None:
        command.extend(["--manifest", str(manifest_path)])
    if json_output:
        command.append("--json")
    if route_json:
        command.extend(["--route-json", "--session-id", session_id])
        command.append(f"--allow-overlay={'true' if allow_overlay else 'false'}")
        command.append(f"--first-turn={'true' if first_turn else 'false'}")
    return command


def _run_rust_json_command(args: list[str], *, failure_label: str) -> dict[str, object] | list[dict[str, object]]:
    """Run the Rust router through the compiled binary when available, else cargo."""

    binary = resolve_router_binary()
    if binary is not None:
        command = [str(binary), *args]
    else:
        command = [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(ROUTER_RS_DIR / "Cargo.toml"),
            "--",
            *args,
        ]

    try:
        proc = subprocess.run(command, capture_output=True, text=True, check=True)
    except subprocess.CalledProcessError as exc:
        stderr = (exc.stderr or exc.stdout or "").strip()
        raise RuntimeError(f"{failure_label}: {stderr}") from exc

    return json.loads(proc.stdout)


def run_rust_router_json(
    query: str,
    limit: int = 5,
    *,
    runtime_path: Path | None = RUNTIME_PATH,
    manifest_path: Path | None = MANIFEST_PATH,
) -> list[dict[str, object]]:
    """Run Rust search output without execv side effects."""

    args = build_rust_router_command(
        query=query,
        limit=limit,
        runtime_path=runtime_path,
        manifest_path=manifest_path,
        json_output=True,
    )
    payload = _run_rust_json_command(args, failure_label=f"Rust router failed for {query!r}")
    return list(payload)


def run_rust_route_json(
    query: str,
    *,
    session_id: str = "route-cli",
    allow_overlay: bool = True,
    first_turn: bool = True,
    limit: int = 5,
    runtime_path: Path | None = RUNTIME_PATH,
    manifest_path: Path | None = MANIFEST_PATH,
) -> dict[str, object]:
    """Run Rust final route decision output without execv side effects."""

    args = build_rust_router_command(
        query=query,
        limit=limit,
        runtime_path=runtime_path,
        manifest_path=manifest_path,
        route_json=True,
        session_id=session_id,
        allow_overlay=allow_overlay,
        first_turn=first_turn,
    )
    payload = _run_rust_json_command(args, failure_label=f"Rust route decision failed for {query!r}")
    if "route_snapshot" not in payload:
        command = [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(ROUTER_RS_DIR / "Cargo.toml"),
            "--",
            *args,
        ]
        proc = subprocess.run(command, capture_output=True, text=True, check=True)
        payload = dict(json.loads(proc.stdout))
    return dict(payload)


def run_rust_route_contract(
    query: str,
    *,
    session_id: str = "route-cli",
    allow_overlay: bool = True,
    first_turn: bool = True,
    limit: int = 5,
    runtime_path: Path | None = RUNTIME_PATH,
    manifest_path: Path | None = MANIFEST_PATH,
) -> RouteDecisionContract:
    """Return the Rust route decision as a typed contract."""

    return RouteDecisionContract.model_validate(
        run_rust_route_json(
            query,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
            limit=limit,
            runtime_path=runtime_path,
            manifest_path=manifest_path,
        )
    )


def search_skills(
    query: str,
    limit: int = 5,
    *,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
) -> list[MatchResult]:
    """Search skills through the Rust router and hydrate Python match rows."""

    payload = run_rust_router_json(
        query,
        limit=limit,
        runtime_path=runtime_path if runtime_path is not None else RUNTIME_PATH,
        manifest_path=manifest_path if manifest_path is not None else MANIFEST_PATH,
    )
    return _hydrate_match_results(
        payload,
        runtime_path=runtime_path,
        manifest_path=manifest_path,
    )


def search_skills_json(
    query: str,
    limit: int = 5,
    *,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
) -> list[dict[str, object]]:
    """Return the Rust search JSON shape directly."""

    return run_rust_router_json(
        query,
        limit=limit,
        runtime_path=runtime_path if runtime_path is not None else RUNTIME_PATH,
        manifest_path=manifest_path if manifest_path is not None else MANIFEST_PATH,
    )


def route_decision_json(
    query: str,
    *,
    session_id: str = "route-cli",
    allow_overlay: bool = True,
    first_turn: bool = True,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
) -> dict[str, object]:
    """Return the Rust route decision in transport-shim form."""

    return run_rust_route_json(
        query,
        session_id=session_id,
        allow_overlay=allow_overlay,
        first_turn=first_turn,
        runtime_path=runtime_path if runtime_path is not None else RUNTIME_PATH,
        manifest_path=manifest_path if manifest_path is not None else MANIFEST_PATH,
    )


def route_decision_contract(
    query: str,
    *,
    session_id: str = "route-cli",
    allow_overlay: bool = True,
    first_turn: bool = True,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
) -> RouteDecisionContract:
    """Return the Rust route decision in typed shim form."""

    return run_rust_route_contract(
        query,
        session_id=session_id,
        allow_overlay=allow_overlay,
        first_turn=first_turn,
        runtime_path=runtime_path if runtime_path is not None else RUNTIME_PATH,
        manifest_path=manifest_path if manifest_path is not None else MANIFEST_PATH,
    )


def maybe_exec_rust_router(
    query: str,
    limit: int,
    json_output: bool,
    route_json: bool,
    session_id: str,
    allow_overlay: bool,
    first_turn: bool,
) -> bool:
    """Exec the Rust router in place when a compiled binary is available."""

    binary = resolve_router_binary()
    if binary is None:
        return False

    command = [
        str(binary),
        *build_rust_router_command(
            query=query,
            limit=limit,
            runtime_path=RUNTIME_PATH,
            manifest_path=MANIFEST_PATH,
            json_output=json_output,
            route_json=route_json,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        ),
    ]
    try:
        os.execv(str(binary), command)
    except OSError:
        return False
    return True


def main() -> None:
    """Run lookup flow and route-decision flow."""

    parser = argparse.ArgumentParser(description="Lookup skills by query.")
    parser.add_argument("--query", type=str, required=True, help="Natural-language search query.")
    parser.add_argument("--limit", type=int, default=5, help="Max results to return.")
    parser.add_argument("--json", action="store_true", help="Output ranked search rows in JSON format.")
    parser.add_argument("--route-json", action="store_true", help="Output final route decision in JSON format.")
    parser.add_argument("--session-id", type=str, default="route-cli", help="Session id used in route decision.")
    parser.add_argument(
        "--allow-overlay",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Allow selecting one overlay skill in route mode.",
    )
    parser.add_argument(
        "--first-turn",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Whether current task is the first turn for session-start boost.",
    )
    args = parser.parse_args()

    if args.route_json and args.json:
        print("Error: choose either --json or --route-json.", file=sys.stderr)
        sys.exit(2)

    if maybe_exec_rust_router(
        args.query,
        limit=args.limit,
        json_output=args.json,
        route_json=args.route_json,
        session_id=args.session_id,
        allow_overlay=args.allow_overlay,
        first_turn=args.first_turn,
    ):
        return

    if args.route_json:
        decision = route_decision_json(
            args.query,
            session_id=args.session_id,
            allow_overlay=args.allow_overlay,
            first_turn=args.first_turn,
        )
        print(json.dumps(decision, indent=2, ensure_ascii=False))
        return

    matches = search_skills(args.query, limit=args.limit)
    payload = search_skills_json(args.query, limit=args.limit)

    if args.json:
        print(json.dumps(payload, indent=2, ensure_ascii=False))
        return

    if not payload:
        print(f"No skills found matching: {args.query}")
        return

    print(f"Found {len(payload)} matches for '{args.query}':\n")
    print(f"{'Skill':<30} | {'Layer':<5} | {'Gate':<10} | {'Score':<6} | {'Description'}")
    print("-" * 120)
    for row, hydrated in zip(payload, matches, strict=False):
        description = str(row["description"])
        if len(description) > 60:
            description = description[:57] + "..."
        print(
            f"{hydrated.record.slug:<30} | {hydrated.record.layer:<5} | "
            f"{hydrated.record.gate:<10} | {row['score']:<6} | {description}"
        )


if __name__ == "__main__":
    main()
