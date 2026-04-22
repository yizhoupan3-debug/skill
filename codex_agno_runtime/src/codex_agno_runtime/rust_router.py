"""Rust route-engine adapter used by the Python host runtime."""

from __future__ import annotations

import argparse
import atexit
import json
import os
import select
import subprocess
import sys
import threading
from functools import lru_cache
from pathlib import Path
from typing import Any, Mapping

from codex_agno_runtime.schemas import (
    RoutingEvalCases,
    RoutingEvalMetrics,
    RoutingEvalReport,
    RoutingEvalResult,
    RouteDecisionContract,
    RouteDecisionSnapshot,
    RouteDiagnosticReport,
    RouteExecutionPolicy,
    SearchMatchesContract,
    SearchMatchResult,
)


def resolve_router_binary_candidate(*candidates: Path) -> Path | None:
    """Prefer the freshest existing router-rs binary, keeping call order as the tiebreaker."""

    existing: list[tuple[float, int, Path]] = []
    for index, candidate in enumerate(candidates):
        if candidate.is_file():
            existing.append((candidate.stat().st_mtime, -index, candidate))
    if not existing:
        return None
    return max(existing)[2]


def _load_json_object(payload: str, *, source: str) -> dict[str, Any]:
    """Load a typed JSON object and fail with a clear transport error."""

    try:
        data = json.loads(payload)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"{source}: invalid JSON payload") from exc
    if not isinstance(data, dict):
        raise RuntimeError(f"{source}: expected JSON object, got {type(data).__name__}")
    return data


def _load_json_object_from_file(path: Path, *, source: str) -> dict[str, Any]:
    """Load and normalize one JSON file payload."""

    try:
        return _load_json_object(path.read_text(encoding="utf-8"), source=source)
    except OSError as exc:
        raise RuntimeError(f"{source}: failed reading {path}") from exc


def discover_codex_home(start_path: Path) -> Path:
    """Resolve the repository root for route CLI entrypoints."""

    if (start_path / "skills").is_dir():
        return start_path
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
            cwd=start_path,
        )
        return Path(proc.stdout.strip())
    except Exception:
        return start_path


def build_route_cli_parser() -> argparse.ArgumentParser:
    """Build the shared route CLI parser."""

    parser = argparse.ArgumentParser(description="Lookup skills by query.")
    parser.add_argument("--query", type=str, required=True, help="Natural-language search query.")
    parser.add_argument("--limit", type=int, default=5, help="Max results to return.")
    parser.add_argument("--json", action="store_true", help="Output typed search contract JSON.")
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
    return parser


def build_routing_eval_cli_parser(
    *,
    default_skills_root: Path,
    default_cases_path: Path,
) -> argparse.ArgumentParser:
    """Build the shared offline routing-eval CLI parser."""

    parser = argparse.ArgumentParser(description="Run offline routing evaluation cases.")
    parser.add_argument(
        "--skills-root",
        type=Path,
        default=default_skills_root,
        help="Skill root path.",
    )
    parser.add_argument(
        "--cases",
        type=Path,
        default=default_cases_path,
        help="Routing eval case file.",
    )
    return parser


def build_framework_contract_artifacts_cli_parser() -> argparse.ArgumentParser:
    """Build the shared framework-contract artifact CLI parser."""

    parser = argparse.ArgumentParser(description="Write framework contract artifacts.")
    parser.add_argument(
        "--framework-profile",
        type=Path,
        required=True,
        help="Input framework_profile JSON.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        required=True,
        help="Output directory for emitted artifacts.",
    )
    parser.add_argument(
        "--include-rust-bundle",
        action="store_true",
        help="Also compile the Rust-side profile bundle via router-rs.",
    )
    parser.add_argument(
        "--include-fallback-artifacts",
        action="store_true",
        help="Also write fallback/compatibility host artifacts such as aionrs_companion_adapter, aionui_host_adapter, and generic_host_adapter.",
    )
    parser.add_argument(
        "--include-compatibility-inventory",
        action="store_true",
        help="Also write the secondary compatibility inventory artifact upgrade_compatibility_matrix.",
    )
    parser.add_argument(
        "--include-legacy-alias-artifact",
        action="store_true",
        help="Force legacy codex_desktop_host_adapter artifacts to be written alongside the parity-first defaults.",
    )
    return parser


class _RouterStdioClient:
    """Keep one router-rs process alive and exchange line-delimited JSON requests."""

    def __init__(self, command: list[str], *, cwd: Path, timeout_seconds: float) -> None:
        self._command = list(command)
        self._cwd = cwd
        self._timeout_seconds = timeout_seconds
        self._lock = threading.Lock()
        self._proc: subprocess.Popen[str] | None = None
        self._next_request_id = 1

    def request(self, operation: str, payload: Mapping[str, Any] | None = None) -> dict[str, Any]:
        with self._lock:
            proc = self._ensure_process_locked()
            request_id = self._next_request_id
            self._next_request_id += 1
            message = json.dumps(
                {
                    "id": request_id,
                    "op": operation,
                    "payload": dict(payload or {}),
                },
                ensure_ascii=False,
                allow_nan=False,
            )
            try:
                assert proc.stdin is not None
                proc.stdin.write(f"{message}\n")
                proc.stdin.flush()
            except BrokenPipeError as exc:
                self._discard_process_locked()
                raise RuntimeError("router stdio pipe broke while sending request") from exc
            response_line = self._read_response_line_locked(proc)
            try:
                response = _load_json_object(response_line, source="router stdio response")
            except RuntimeError as exc:
                raise RuntimeError(f"router stdio returned invalid JSON: {response_line!r}") from exc
            if response.get("id") != request_id:
                raise RuntimeError(
                    f"router stdio returned mismatched response id: {response.get('id')!r}"
                )
            if not response.get("ok"):
                error = response.get("error")
                raise RuntimeError(str(error or "router stdio request failed"))
            resolved = response.get("payload")
            if not isinstance(resolved, dict):
                raise RuntimeError(f"router stdio returned a non-object payload: {resolved!r}")
            return resolved

    def close(self) -> None:
        with self._lock:
            self._discard_process_locked()

    def _ensure_process_locked(self) -> subprocess.Popen[str]:
        proc = self._proc
        if proc is not None and proc.poll() is None:
            return proc
        try:
            proc = subprocess.Popen(
                self._command,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                cwd=self._cwd,
                bufsize=1,
            )
        except OSError as exc:
            raise RuntimeError(f"router stdio launch failed: {exc}") from exc
        self._proc = proc
        return proc

    def _read_response_line_locked(self, proc: subprocess.Popen[str]) -> str:
        if proc.stdout is None:
            self._discard_process_locked()
            raise RuntimeError("router stdio stdout is unavailable")
        ready, _, _ = select.select([proc.stdout], [], [], self._timeout_seconds)
        if not ready:
            self._discard_process_locked()
            raise RuntimeError(f"router stdio timed out after {self._timeout_seconds}s")
        response_line = proc.stdout.readline()
        if response_line:
            return response_line
        stderr = ""
        if proc.stderr is not None:
            try:
                stderr = proc.stderr.read().strip()
            except OSError:
                stderr = ""
        returncode = proc.poll()
        self._discard_process_locked()
        if returncode is None:
            raise RuntimeError("router stdio closed the response stream unexpectedly")
        detail = stderr or f"router stdio exited with code {returncode}"
        raise RuntimeError(detail)

    def _discard_process_locked(self) -> None:
        proc = self._proc
        self._proc = None
        if proc is None:
            return
        try:
            if proc.stdin is not None:
                proc.stdin.close()
        except OSError:
            pass
        try:
            if proc.stdout is not None:
                proc.stdout.close()
        except OSError:
            pass
        try:
            if proc.stderr is not None:
                proc.stderr.close()
        except OSError:
            pass
        if proc.poll() is None:
            proc.kill()
            try:
                proc.wait(timeout=1)
            except subprocess.TimeoutExpired:
                pass


_STDIO_CLIENTS: dict[tuple[str, ...], _RouterStdioClient] = {}
_STDIO_CLIENTS_LOCK = threading.Lock()
_ROUTER_BINARY_CACHE_UNSET = object()


def _close_router_stdio_clients() -> None:
    with _STDIO_CLIENTS_LOCK:
        clients = list(_STDIO_CLIENTS.values())
        _STDIO_CLIENTS.clear()
    for client in clients:
        client.close()


atexit.register(_close_router_stdio_clients)


@lru_cache(maxsize=None)
def _cached_route_adapter_factory(
    codex_home: str,
    runtime_path: str | None,
    manifest_path: str | None,
    timeout_seconds: float,
) -> "RustRouteAdapter":
    return RustRouteAdapter(
        Path(codex_home),
        timeout_seconds=timeout_seconds,
        runtime_path=Path(runtime_path) if runtime_path is not None else None,
        manifest_path=Path(manifest_path) if manifest_path is not None else None,
    )


def get_cached_route_adapter(
    codex_home: Path,
    *,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
    timeout_seconds: float = 30.0,
) -> "RustRouteAdapter":
    """Return a cached Rust route adapter for one repo/runtime/manifest tuple."""

    return _cached_route_adapter_factory(
        str(codex_home.resolve()),
        str(runtime_path.resolve()) if runtime_path is not None else None,
        str(manifest_path.resolve()) if manifest_path is not None else None,
        timeout_seconds,
    )


def route_adapter(
    *,
    codex_home: Path,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
    timeout_seconds: float = 30.0,
) -> "RustRouteAdapter":
    """Return the cached route adapter for one repo and optional routing artifacts."""

    return get_cached_route_adapter(
        codex_home,
        runtime_path=runtime_path,
        manifest_path=manifest_path,
        timeout_seconds=timeout_seconds,
    )


def search_skills(
    query: str,
    *,
    codex_home: Path,
    limit: int = 5,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
    timeout_seconds: float = 30.0,
) -> list[SearchMatchResult]:
    """Search skills for one repo through the cached Rust route adapter."""

    return route_adapter(
        codex_home=codex_home,
        runtime_path=runtime_path,
        manifest_path=manifest_path,
        timeout_seconds=timeout_seconds,
    ).search_skill_matches(query=query, limit=limit)


def route_decision_contract(
    query: str,
    *,
    codex_home: Path,
    session_id: str = "route-cli",
    allow_overlay: bool = True,
    first_turn: bool = True,
    runtime_path: Path | None = None,
    manifest_path: Path | None = None,
    timeout_seconds: float = 30.0,
) -> RouteDecisionContract:
    """Return the typed Rust route contract for one repo."""

    return route_adapter(
        codex_home=codex_home,
        runtime_path=runtime_path,
        manifest_path=manifest_path,
        timeout_seconds=timeout_seconds,
    ).route_contract(
        query=query,
        session_id=session_id,
        allow_overlay=allow_overlay,
        first_turn=first_turn,
    )


def run_route_cli(
    *,
    codex_home: Path,
    argv: list[str] | None = None,
) -> int:
    """Run the shared Rust-owned route CLI flow for one repo."""

    args = build_route_cli_parser().parse_args(argv)
    if args.route_json and args.json:
        print("Error: choose either --json or --route-json.", file=sys.stderr)
        return 2

    adapter = route_adapter(codex_home=codex_home)
    adapter.exec_query_cli(
        query=args.query,
        limit=args.limit,
        json_output=args.json,
        route_json=args.route_json,
        session_id=args.session_id,
        allow_overlay=args.allow_overlay,
        first_turn=args.first_turn,
    )
    return 0


def load_routing_eval_cases(path: Path) -> RoutingEvalCases:
    """Load offline routing evaluation cases from JSON."""

    return RoutingEvalCases.model_validate(
        _load_json_object_from_file(path, source=f"routing eval case file: {path}")
    )


def evaluate_routing_cases(
    *,
    skills_root: Path,
    cases_payload: RoutingEvalCases | dict[str, Any],
) -> dict[str, Any]:
    """Run offline routing evaluation cases through the Rust-backed route contract."""

    runtime_path = skills_root / "SKILL_ROUTING_RUNTIME.json"
    manifest_path = skills_root / "SKILL_MANIFEST.json"
    codex_home = skills_root.parent

    if isinstance(cases_payload, dict):
        cases_payload = RoutingEvalCases.model_validate(cases_payload)
    metrics = RoutingEvalMetrics()
    results: list[RoutingEvalResult] = []

    for case in cases_payload.cases:
        task = case.task.strip()
        if not task:
            continue

        contract = route_decision_contract(
            task,
            codex_home=codex_home,
            session_id=f"routing-eval::{case.id or metrics.case_count + 1}",
            allow_overlay=True,
            first_turn=case.first_turn,
            runtime_path=runtime_path,
            manifest_path=manifest_path,
        )
        selected_owner = contract.selected_skill
        selected_overlay = contract.overlay_skill
        metrics.case_count += 1
        category = case.category.strip()
        expected_owner = case.expected_owner
        expected_overlay = case.expected_overlay
        focus_skill = case.focus_skill
        forbidden_owners = {str(item) for item in case.forbidden_owners}

        trigger_hit = False
        overtrigger = False
        owner_correct = expected_owner == selected_owner if expected_owner else False
        overlay_correct = expected_overlay == selected_overlay if expected_overlay is not None else False

        if category == "should-trigger":
            trigger_hit = focus_skill == selected_owner
            if trigger_hit:
                metrics.trigger_hit += 1
            else:
                metrics.trigger_miss += 1
        elif category == "should-not-trigger":
            overtrigger = selected_owner in forbidden_owners
            if overtrigger:
                metrics.overtrigger += 1
        elif category in {"wrong-owner-near-miss", "gate-vs-owner-conflict"}:
            trigger_hit = focus_skill == selected_owner
            if trigger_hit:
                metrics.trigger_hit += 1
            else:
                metrics.trigger_miss += 1
            if selected_owner in forbidden_owners:
                metrics.overtrigger += 1

        if owner_correct:
            metrics.owner_correct += 1
        if overlay_correct:
            metrics.overlay_correct += 1

        results.append(
            RoutingEvalResult(
                id=case.id,
                category=category,
                task=task,
                focus_skill=focus_skill,
                selected_owner=selected_owner,
                selected_overlay=selected_overlay,
                expected_owner=expected_owner,
                expected_overlay=expected_overlay,
                forbidden_owners=sorted(forbidden_owners),
                trigger_hit=trigger_hit,
                overtrigger=overtrigger,
                owner_correct=owner_correct,
                overlay_correct=overlay_correct,
            )
        )

    return RoutingEvalReport(
        schema_version="routing-eval-v1",
        metrics=metrics,
        results=results,
    ).model_dump(mode="json")


def run_routing_eval_cli(
    *,
    codex_home: Path,
    argv: list[str] | None = None,
) -> int:
    """Run the offline routing evaluation CLI for one repo."""

    args = build_routing_eval_cli_parser(
        default_skills_root=codex_home / "skills",
        default_cases_path=codex_home / "tests" / "routing_eval_cases.json",
    ).parse_args(argv)

    payload = evaluate_routing_cases(
        skills_root=args.skills_root,
        cases_payload=load_routing_eval_cases(args.cases),
    )
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def run_framework_contract_artifacts_cli(
    *,
    codex_home: Path,
    argv: list[str] | None = None,
) -> int:
    """Run the shared framework-contract artifact emission CLI for one repo."""

    from codex_agno_runtime.framework_profile import FrameworkProfile
    from codex_agno_runtime.profile_artifacts import emit_framework_contract_artifacts

    args = build_framework_contract_artifacts_cli_parser().parse_args(argv)
    profile_payload = _load_json_object_from_file(
        args.framework_profile,
        source=f"framework profile file: {args.framework_profile}",
    )
    profile = FrameworkProfile.from_dict(profile_payload)
    rust_adapter = route_adapter(codex_home=codex_home) if args.include_rust_bundle else None
    paths = emit_framework_contract_artifacts(
        args.output_dir,
        profile=profile,
        rust_adapter=rust_adapter,
        include_fallback_artifacts=args.include_fallback_artifacts,
        include_compatibility_inventory=args.include_compatibility_inventory,
        include_legacy_alias_artifact=args.include_legacy_alias_artifact,
    )
    print(json.dumps(paths, ensure_ascii=False, indent=2))
    return 0


class RustRouteAdapter:
    """Call the repository Rust route engine for final route decisions."""

    search_schema_version = "router-rs-search-results-v1"
    route_decision_schema_version = "router-rs-route-decision-v1"
    execution_schema_version = "router-rs-execute-response-v1"
    route_policy_schema_version = "router-rs-route-policy-v1"
    route_snapshot_schema_version = "router-rs-route-snapshot-v1"
    route_report_schema_version = "router-rs-route-report-v2"
    runtime_control_plane_schema_version = "router-rs-runtime-control-plane-v1"
    sandbox_control_schema_version = "router-rs-sandbox-control-v1"
    background_control_schema_version = "router-rs-background-control-v1"
    background_state_store_schema_version = "router-rs-background-state-store-v1"
    trace_descriptor_schema_version = "router-rs-trace-descriptor-v1"
    checkpoint_resume_manifest_schema_version = "router-rs-checkpoint-resume-manifest-v1"
    transport_binding_write_schema_version = "router-rs-transport-binding-write-v1"
    checkpoint_manifest_write_schema_version = "router-rs-checkpoint-manifest-write-v1"
    attached_runtime_event_transport_authority = "rust-runtime-attached-event-transport"
    trace_stream_replay_schema_version = "router-rs-trace-stream-replay-v1"
    trace_stream_inspect_schema_version = "router-rs-trace-stream-inspect-v1"
    trace_compaction_delta_write_schema_version = "router-rs-trace-compaction-delta-write-v1"
    runtime_observability_exporter_schema_version = "runtime-observability-exporter-v1"
    runtime_observability_metric_catalog_schema_version = "runtime-observability-metric-catalog-v1"
    runtime_observability_metric_record_schema_version = "runtime-observability-metric-record-v1"
    runtime_observability_dashboard_schema_version = "runtime-observability-dashboard-v1"
    framework_runtime_snapshot_schema_version = "router-rs-framework-runtime-snapshot-v1"
    framework_contract_summary_schema_version = "router-rs-framework-contract-summary-v1"
    framework_memory_recall_schema_version = "router-rs-framework-memory-recall-v1"
    framework_recap_schema_version = "router-rs-framework-recap-v1"
    framework_refresh_schema_version = "router-rs-framework-refresh-v1"
    claude_hook_schema_version = "router-rs-claude-hook-response-v1"
    route_authority = "rust-route-core"
    execution_authority = "rust-execution-cli"
    compile_authority = "rust-route-compiler"
    runtime_control_plane_authority = "rust-runtime-control-plane"
    sandbox_control_authority = "rust-sandbox-control"
    background_control_authority = "rust-background-control"
    background_state_store_authority = "rust-background-state-store"
    trace_descriptor_authority = "rust-runtime-trace-descriptor"
    checkpoint_resume_manifest_authority = "rust-runtime-checkpoint-manifest"
    transport_binding_write_authority = "rust-runtime-transport-binding-writer"
    checkpoint_manifest_write_authority = "rust-runtime-checkpoint-manifest-writer"
    trace_stream_io_authority = "rust-runtime-trace-io"
    framework_runtime_authority = "rust-framework-runtime-read-model"
    claude_hook_authority = "rust-claude-hook"

    def __init__(
        self,
        codex_home: Path,
        *,
        timeout_seconds: float = 30.0,
        runtime_path: Path | None = None,
        manifest_path: Path | None = None,
    ) -> None:
        self.codex_home = codex_home
        self.timeout_seconds = timeout_seconds
        self.runtime_path = runtime_path or (codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json")
        self.manifest_path = manifest_path or (codex_home / "skills" / "SKILL_MANIFEST.json")
        self.router_dir = codex_home / "scripts" / "router-rs"
        self.release_bin = self.router_dir / "target" / "release" / "router-rs"
        self.debug_bin = self.router_dir / "target" / "debug" / "router-rs"
        self._cached_runtime_binary: Path | None | object = _ROUTER_BINARY_CACHE_UNSET
        self._cached_latest_source_mtime: float | None = None

    def execute(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Run one Rust-owned execution request through router-rs."""

        args = [
            "--execute-json",
            "--execute-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        try:
            resolved = self._run_hot_json_command(
                "execute",
                payload,
                [*self._binary_command(), *args],
                failure_label="execution kernel",
            )
        except OSError as exc:
            raise RuntimeError(f"router-rs execute could not be launched: {exc}") from exc
        except RuntimeError as exc:
            message = str(exc)
            if message.startswith("Rust execution kernel failed:"):
                raise RuntimeError(
                    message.replace("Rust execution kernel failed:", "router-rs execute failed:", 1)
                ) from exc
            if message.startswith("Rust execution kernel timed out after"):
                raise RuntimeError("router-rs execute timed out before returning a response") from exc
            raise
        if resolved.get("execution_schema_version") != self.execution_schema_version:
            raise RuntimeError(
                "router-rs execute returned an unknown schema: "
                f"{resolved.get('execution_schema_version')!r}"
            )
        if resolved.get("authority") != self.execution_authority:
            raise RuntimeError(
                "router-rs execute returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def route_contract(
        self,
        *,
        query: str,
        session_id: str,
        allow_overlay: bool,
        first_turn: bool,
    ) -> RouteDecisionContract:
        """Return one typed Rust-backed route decision contract."""

        args = self._route_args(query, session_id, allow_overlay, first_turn)
        payload = self._run_hot_json_command(
            "route",
            {
                "query": query,
                "session_id": session_id,
                "allow_overlay": allow_overlay,
                "first_turn": first_turn,
                "runtime_path": str(self.runtime_path),
                "manifest_path": str(self.manifest_path),
            },
            [*self._binary_command(), *args],
            failure_label="route engine",
        )
        if payload.get("decision_schema_version") != self.route_decision_schema_version:
            raise RuntimeError(
                "Rust route engine returned an unknown decision schema: "
                f"{payload.get('decision_schema_version')!r}"
            )
        if payload.get("authority") != self.route_authority:
            raise RuntimeError(
                "Rust route engine returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        return RouteDecisionContract.model_validate(payload)

    def search_skill_matches_contract(
        self,
        *,
        query: str,
        limit: int,
    ) -> SearchMatchesContract:
        """Return one typed Rust-backed search contract."""

        command = self.query_cli_command(query=query, limit=limit, json_output=True)
        resolved: Any = self._run_hot_json_command(
            "search_skills",
            {
                "query": query,
                "limit": limit,
                "runtime_path": str(self.runtime_path),
                "manifest_path": str(self.manifest_path),
            },
            command,
            failure_label="search engine",
        )
        if not isinstance(resolved, Mapping):
            raise RuntimeError(
                f"Rust search engine returned an unexpected payload: {resolved!r}"
            )
        contract = SearchMatchesContract.model_validate(dict(resolved))
        if contract.search_schema_version != self.search_schema_version:
            raise RuntimeError(
                "Rust search engine returned an unknown schema: "
                f"{contract.search_schema_version!r}"
            )
        if contract.authority != self.route_authority:
            raise RuntimeError(
                "Rust search engine returned an unexpected authority marker: "
                f"{contract.authority!r}"
            )
        if contract.query != query:
            raise RuntimeError(
                "Rust search engine returned an unexpected query echo: "
                f"{contract.query!r}"
            )
        return contract

    def search_skill_matches(
        self,
        *,
        query: str,
        limit: int,
    ) -> list[SearchMatchResult]:
        """Return Rust-backed search matches as shared typed results."""

        return self.search_skill_matches_contract(query=query, limit=limit).matches

    def compiled_binary(self) -> Path | None:
        """Expose the resolved router binary for thin Python CLI shims."""

        return self._resolved_binary()

    def exec_query_cli(
        self,
        *,
        query: str,
        limit: int,
        json_output: bool = False,
        route_json: bool = False,
        session_id: str = "route-cli",
        allow_overlay: bool = True,
        first_turn: bool = True,
    ) -> None:
        """Replace the current process with router-rs and fail loudly if unavailable."""

        command = self.query_cli_command(
            query=query,
            limit=limit,
            json_output=json_output,
            route_json=route_json,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )
        try:
            os.execv(command[0], command)
        except OSError as exc:
            raise RuntimeError(f"router-rs route CLI exec failed: {exc}") from exc

    def query_cli_command(
        self,
        *,
        query: str,
        limit: int,
        json_output: bool = False,
        route_json: bool = False,
        session_id: str = "route-cli",
        allow_overlay: bool = True,
        first_turn: bool = True,
    ) -> list[str]:
        """Build one router-rs CLI command for search or route rendering."""

        return [*self._binary_command(), *self.query_cli_args(
            query=query,
            limit=limit,
            json_output=json_output,
            route_json=route_json,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )]

    def query_cli_args(
        self,
        *,
        query: str,
        limit: int,
        json_output: bool = False,
        route_json: bool = False,
        session_id: str = "route-cli",
        allow_overlay: bool = True,
        first_turn: bool = True,
    ) -> list[str]:
        """Build router-rs CLI args without the binary prefix."""

        command = [
            "--query",
            query,
            "--limit",
            str(limit),
            "--runtime",
            str(self.runtime_path),
            "--manifest",
            str(self.manifest_path),
        ]
        if json_output:
            command.append("--json")
        if route_json:
            command.extend(["--route-json", "--session-id", session_id])
            command.append(f"--allow-overlay={'true' if allow_overlay else 'false'}")
            command.append(f"--first-turn={'true' if first_turn else 'false'}")
        return command

    def compile_profile_bundle(self, profile_path: Path) -> dict[str, Any]:
        """Compile a serialized framework profile into the Rust-side companion bundle."""

        command = [
            *self._binary_command(),
            "--profile-json",
            "--framework-profile",
            str(profile_path),
        ]
        return self._run_hot_json_command(
            "compile_profile_bundle",
            {"profile_path": str(profile_path), "include_legacy_alias_artifact": False},
            command,
            failure_label="profile compiler",
        )

    def route_report_contract(
        self,
        *,
        mode: str,
        rust_route_snapshot: dict[str, Any] | RouteDecisionSnapshot | None = None,
        route_decision_contract: RouteDecisionContract | Mapping[str, Any] | None = None,
    ) -> RouteDiagnosticReport:
        """Build one typed Rust-owned route diagnostic report."""

        if rust_route_snapshot is None:
            if route_decision_contract is None:
                raise ValueError(
                    "route_report_contract requires rust_route_snapshot or route_decision_contract"
                )
            if isinstance(route_decision_contract, RouteDecisionContract):
                rust_route_snapshot = route_decision_contract.route_snapshot
            else:
                rust_route_snapshot = dict(route_decision_contract).get("route_snapshot")
        if rust_route_snapshot is None:
            raise ValueError("route_report_contract could not resolve a route snapshot from the route decision")
        args = [
            "--route-report-json",
            "--route-mode",
            mode,
            "--rust-route-snapshot-json",
            json.dumps(
                rust_route_snapshot.model_dump(mode="json")
                if isinstance(rust_route_snapshot, RouteDecisionSnapshot)
                else rust_route_snapshot,
                ensure_ascii=False,
            ),
        ]
        if route_decision_contract is not None:
            serialized_route_decision = (
                route_decision_contract.model_dump(mode="json")
                if isinstance(route_decision_contract, RouteDecisionContract)
                else dict(route_decision_contract)
            )
            args.extend(
                [
                    "--route-decision-json",
                    json.dumps(serialized_route_decision, ensure_ascii=False),
                ]
            )
        else:
            serialized_route_decision = None
        payload = self._run_hot_json_command(
            "route_report",
            {
                "mode": mode,
                "rust_route_snapshot": (
                    rust_route_snapshot.model_dump(mode="json")
                    if isinstance(rust_route_snapshot, RouteDecisionSnapshot)
                    else rust_route_snapshot
                ),
                "route_decision": serialized_route_decision,
            },
            [*self._binary_command(), *args],
            failure_label="route report engine",
        )
        if payload.get("report_schema_version") != self.route_report_schema_version:
            raise RuntimeError(
                "Rust route report engine returned an unknown schema: "
                f"{payload.get('report_schema_version')!r}"
            )
        if payload.get("authority") != self.route_authority:
            raise RuntimeError(
                "Rust route report engine returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        return RouteDiagnosticReport.model_validate(payload)

    def route_policy(
        self,
        *,
        mode: str,
    ) -> dict[str, Any]:
        """Resolve Rust-only route-mode policy through the Rust routing core."""

        return self.route_policy_contract(mode=mode).model_dump(mode="json")

    def route_policy_contract(
        self,
        *,
        mode: str,
    ) -> RouteExecutionPolicy:
        """Resolve one typed Rust-owned route-mode policy."""

        args = [
            "--route-policy-json",
            "--route-mode",
            mode,
        ]
        payload = self._run_hot_json_command(
            "route_policy",
            {"mode": mode},
            [*self._binary_command(), *args],
            failure_label="route policy engine",
        )
        if payload.get("policy_schema_version") != self.route_policy_schema_version:
            raise RuntimeError(
                "Rust route policy engine returned an unknown schema: "
                f"{payload.get('policy_schema_version')!r}"
            )
        if payload.get("authority") != self.route_authority:
            raise RuntimeError(
                "Rust route policy engine returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        return RouteExecutionPolicy.model_validate(payload)

    def route_snapshot(
        self,
        *,
        engine: str,
        selected_skill: str,
        overlay_skill: str | None,
        layer: str,
        score: float,
        reasons: list[str],
    ) -> dict[str, Any]:
        """Build a canonical route snapshot through the Rust routing core."""

        return self.route_snapshot_contract(
            engine=engine,
            selected_skill=selected_skill,
            overlay_skill=overlay_skill,
            layer=layer,
            score=score,
            reasons=reasons,
        ).model_dump(mode="json")

    def route_snapshot_contract(
        self,
        *,
        engine: str,
        selected_skill: str,
        overlay_skill: str | None,
        layer: str,
        score: float,
        reasons: list[str],
    ) -> RouteDecisionSnapshot:
        """Build one typed canonical route snapshot through the Rust routing core."""

        args = [
            "--route-snapshot-json",
            "--route-snapshot-input-json",
            json.dumps(
                {
                    "engine": engine,
                    "selected_skill": selected_skill,
                    "overlay_skill": overlay_skill,
                    "layer": layer,
                    "score": score,
                    "reasons": reasons,
                },
                ensure_ascii=False,
            ),
        ]
        payload = self._run_hot_json_command(
            "route_snapshot",
            {
                "engine": engine,
                "selected_skill": selected_skill,
                "overlay_skill": overlay_skill,
                "layer": layer,
                "score": score,
                "reasons": reasons,
            },
            [*self._binary_command(), *args],
            failure_label="route snapshot engine",
        )
        if payload.get("snapshot_schema_version") != self.route_snapshot_schema_version:
            raise RuntimeError(
                "Rust route snapshot engine returned an unknown schema: "
                f"{payload.get('snapshot_schema_version')!r}"
            )
        if payload.get("authority") != self.route_authority:
            raise RuntimeError(
                "Rust route snapshot engine returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        route_snapshot = payload.get("route_snapshot")
        if not isinstance(route_snapshot, dict):
            raise RuntimeError("Rust route snapshot engine returned a missing route_snapshot payload.")
        return RouteDecisionSnapshot.model_validate(route_snapshot)

    def compile_codex_profile_artifacts(
        self,
        profile_path: Path,
        *,
        include_legacy_alias_artifact: bool = False,
    ) -> dict[str, Any]:
        """Compile first-class Rust Codex contract/parity artifacts for one profile."""

        command = [
            *self._binary_command(),
            "--profile-artifacts-json",
            "--framework-profile",
            str(profile_path),
        ]
        if include_legacy_alias_artifact:
            command.append("--include-legacy-alias-artifact")
        return self._run_hot_json_command(
            "compile_codex_profile_artifacts",
            {
                "profile_path": str(profile_path),
                "include_legacy_alias_artifact": include_legacy_alias_artifact,
            },
            command,
            failure_label="profile artifact compiler",
        )

    def framework_runtime_snapshot(self, *, repo_root: Path) -> dict[str, Any]:
        """Build the framework runtime snapshot read-model through router-rs."""

        args = [
            "--framework-runtime-snapshot-json",
            "--repo-root",
            str(repo_root),
        ]
        payload = self._run_hot_json_command(
            "framework_runtime_snapshot",
            {"repo_root": str(repo_root)},
            [*self._binary_command(), *args],
            failure_label="framework runtime snapshot compiler",
        )
        if payload.get("schema_version") != self.framework_runtime_snapshot_schema_version:
            raise RuntimeError(
                "Rust framework runtime snapshot compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("authority") != self.framework_runtime_authority:
            raise RuntimeError(
                "Rust framework runtime snapshot compiler returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        snapshot = payload.get("runtime_snapshot")
        if not isinstance(snapshot, dict):
            raise RuntimeError(
                "Rust framework runtime snapshot compiler returned a missing runtime_snapshot payload."
            )
        return snapshot

    def framework_contract_summary(self, *, repo_root: Path) -> dict[str, Any]:
        """Build the framework contract summary read-model through router-rs."""

        args = [
            "--framework-contract-summary-json",
            "--repo-root",
            str(repo_root),
        ]
        payload = self._run_hot_json_command(
            "framework_contract_summary",
            {"repo_root": str(repo_root)},
            [*self._binary_command(), *args],
            failure_label="framework contract summary compiler",
        )
        if payload.get("schema_version") != self.framework_contract_summary_schema_version:
            raise RuntimeError(
                "Rust framework contract summary compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("authority") != self.framework_runtime_authority:
            raise RuntimeError(
                "Rust framework contract summary compiler returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        summary = payload.get("contract_summary")
        if not isinstance(summary, dict):
            raise RuntimeError(
                "Rust framework contract summary compiler returned a missing contract_summary payload."
            )
        return summary

    def framework_memory_recall(
        self,
        *,
        repo_root: Path,
        query: str = "",
        top: int = 8,
        mode: str = "stable",
    ) -> dict[str, Any]:
        """Build the Rust-owned framework memory recall payload."""

        args = [
            "--framework-memory-recall-json",
            "--repo-root",
            str(repo_root),
            "--query",
            query,
            "--limit",
            str(top),
            "--framework-memory-mode",
            mode,
        ]
        payload = self._run_hot_json_command(
            "framework_memory_recall",
            {"repo_root": str(repo_root), "query": query, "top": top, "mode": mode},
            [*self._binary_command(), *args],
            failure_label="framework memory recall compiler",
        )
        if payload.get("schema_version") != self.framework_memory_recall_schema_version:
            raise RuntimeError(
                "Rust framework memory recall compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("authority") != self.framework_runtime_authority:
            raise RuntimeError(
                "Rust framework memory recall compiler returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        recall = payload.get("memory_recall")
        if not isinstance(recall, dict):
            raise RuntimeError(
                "Rust framework memory recall compiler returned a missing memory_recall payload."
            )
        return recall

    def framework_recap(self, *, repo_root: Path, max_lines: int = 6) -> dict[str, Any]:
        """Build the Rust-owned recap surface for thin host shells."""

        args = [
            "--framework-recap-json",
            "--repo-root",
            str(repo_root),
            "--claude-hook-max-lines",
            str(max_lines),
        ]
        payload = self._run_hot_json_command(
            "framework_recap",
            {"repo_root": str(repo_root), "max_lines": max_lines},
            [*self._binary_command(), *args],
            failure_label="framework recap compiler",
        )
        if payload.get("schema_version") != self.framework_recap_schema_version:
            raise RuntimeError(
                "Rust framework recap compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("authority") != self.framework_runtime_authority:
            raise RuntimeError(
                "Rust framework recap compiler returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        recap = payload.get("recap")
        if not isinstance(recap, dict):
            raise RuntimeError(
                "Rust framework recap compiler returned a missing recap payload."
            )
        return recap

    def framework_refresh(self, *, repo_root: Path, max_lines: int = 4) -> dict[str, Any]:
        """Build and copy the compact Rust-owned refresh prompt."""

        args = [
            "--framework-refresh-json",
            "--repo-root",
            str(repo_root),
            "--claude-hook-max-lines",
            str(max_lines),
        ]
        payload = self._run_json_command(
            [*self._binary_command(), *args],
            failure_label="framework refresh compiler",
        )
        if payload.get("schema_version") != self.framework_refresh_schema_version:
            raise RuntimeError(
                "Rust framework refresh compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("authority") != self.framework_runtime_authority:
            raise RuntimeError(
                "Rust framework refresh compiler returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        refresh = payload.get("refresh")
        if not isinstance(refresh, dict):
            raise RuntimeError(
                "Rust framework refresh compiler returned a missing refresh payload."
            )
        return refresh

    def claude_lifecycle_hook(
        self,
        *,
        command: str,
        repo_root: Path,
        max_lines: int = 6,
    ) -> dict[str, Any]:
        """Run the Rust-owned Claude lifecycle hook contract."""

        args = [
            "--claude-hook-command",
            command,
            "--repo-root",
            str(repo_root),
            "--claude-hook-max-lines",
            str(max_lines),
        ]
        payload = self._run_json_command(
            [*self._binary_command(), *args],
            failure_label="Claude lifecycle hook",
        )
        if payload.get("schema_version") != self.claude_hook_schema_version:
            raise RuntimeError(
                "Rust Claude lifecycle hook returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("authority") != self.claude_hook_authority:
            raise RuntimeError(
                "Rust Claude lifecycle hook returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        return payload

    def runtime_control_plane(self) -> dict[str, Any]:
        """Return the Rust-owned runtime control-plane authority descriptor."""

        args = ["--runtime-control-plane-json"]
        payload = self._run_hot_json_command(
            "runtime_control_plane",
            {},
            [*self._binary_command(), *args],
            failure_label="runtime control-plane compiler",
        )
        if payload.get("schema_version") != self.runtime_control_plane_schema_version:
            raise RuntimeError(
                "Rust runtime control-plane compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("authority") != self.runtime_control_plane_authority:
            raise RuntimeError(
                "Rust runtime control-plane compiler returned an unexpected authority marker: "
                f"{payload.get('authority')!r}"
            )
        return payload

    def runtime_observability_exporter_descriptor(self) -> dict[str, Any]:
        """Return the Rust-owned runtime observability exporter descriptor."""

        args = ["--runtime-observability-exporter-json"]
        payload = self._run_hot_json_command(
            "runtime_observability_exporter_descriptor",
            {},
            [*self._binary_command(), *args],
            failure_label="runtime observability exporter compiler",
        )
        if payload.get("schema_version") != self.runtime_observability_exporter_schema_version:
            raise RuntimeError(
                "Rust runtime observability exporter compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        if payload.get("exporter_authority") != self.runtime_control_plane_authority:
            raise RuntimeError(
                "Rust runtime observability exporter compiler returned an unexpected authority marker: "
                f"{payload.get('exporter_authority')!r}"
            )
        return payload

    def runtime_observability_metric_catalog(self) -> dict[str, Any]:
        """Return the Rust-owned machine-readable runtime metric catalog."""

        args = ["--runtime-observability-metric-catalog-json"]
        payload = self._run_hot_json_command(
            "runtime_observability_metric_catalog",
            {},
            [*self._binary_command(), *args],
            failure_label="runtime observability metric catalog compiler",
        )
        if payload.get("schema_version") != self.runtime_observability_metric_catalog_schema_version:
            raise RuntimeError(
                "Rust runtime observability metric catalog compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        return payload

    def runtime_observability_dashboard_schema(self) -> dict[str, Any]:
        """Return the Rust-owned runtime observability dashboard schema."""

        args = ["--runtime-observability-dashboard-json"]
        payload = self._run_hot_json_command(
            "runtime_observability_dashboard_schema",
            {},
            [*self._binary_command(), *args],
            failure_label="runtime observability dashboard compiler",
        )
        if payload.get("schema_version") != self.runtime_observability_dashboard_schema_version:
            raise RuntimeError(
                "Rust runtime observability dashboard compiler returned an unknown schema: "
                f"{payload.get('schema_version')!r}"
            )
        return payload

    def runtime_metric_record(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Build one Rust-owned runtime observability metric record."""

        args = [
            "--runtime-metric-record-json",
            "--runtime-metric-record-input-json",
            json.dumps(payload, ensure_ascii=False, allow_nan=False),
        ]
        resolved = self._run_hot_json_command(
            "runtime_metric_record",
            payload,
            [*self._binary_command(), *args],
            failure_label="runtime metric record compiler",
        )
        if resolved.get("schema_version") != self.runtime_observability_metric_record_schema_version:
            raise RuntimeError(
                "Rust runtime metric record compiler returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        ownership = resolved.get("ownership")
        if not isinstance(ownership, dict) or ownership.get("exporter_authority") != self.runtime_control_plane_authority:
            raise RuntimeError(
                "Rust runtime metric record compiler returned an unexpected authority marker: "
                f"{ownership!r}"
            )
        return resolved

    def background_control(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Resolve background admission/retry policy through the Rust runtime core."""

        args = [
            "--background-control-json",
            "--background-control-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "background_control",
            payload,
            [*self._binary_command(), *args],
            failure_label="background control compiler",
        )
        if resolved.get("schema_version") != self.background_control_schema_version:
            raise RuntimeError(
                "Rust background control compiler returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.background_control_authority:
            raise RuntimeError(
                "Rust background control compiler returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def background_state(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Resolve durable background state operations through the Rust runtime core."""

        args = [
            "--background-state-json",
            "--background-state-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_json_command(
            [*self._binary_command(), *args],
            failure_label="background state compiler",
        )
        if resolved.get("schema_version") != self.background_state_store_schema_version:
            raise RuntimeError(
                "Rust background state compiler returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.background_state_store_authority:
            raise RuntimeError(
                "Rust background state compiler returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def sandbox_control(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Resolve sandbox lifecycle transition policy through the Rust runtime core."""

        args = [
            "--sandbox-control-json",
            "--sandbox-control-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "sandbox_control",
            payload,
            [*self._binary_command(), *args],
            failure_label="sandbox control compiler",
        )
        if resolved.get("schema_version") != self.sandbox_control_schema_version:
            raise RuntimeError(
                "Rust sandbox control compiler returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.sandbox_control_authority:
            raise RuntimeError(
                "Rust sandbox control compiler returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def describe_transport(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--describe-transport-json",
            "--describe-transport-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "describe_transport",
            payload,
            [*self._binary_command(), *args],
            failure_label="trace transport descriptor compiler",
        )
        if resolved.get("schema_version") != self.trace_descriptor_schema_version:
            raise RuntimeError(
                "Rust trace transport descriptor compiler returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.trace_descriptor_authority:
            raise RuntimeError(
                "Rust trace transport descriptor compiler returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        transport = resolved.get("transport")
        if not isinstance(transport, dict):
            raise RuntimeError("Rust trace transport descriptor compiler returned a missing transport payload.")
        return transport

    def describe_handoff(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--describe-handoff-json",
            "--describe-handoff-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "describe_handoff",
            payload,
            [*self._binary_command(), *args],
            failure_label="trace handoff descriptor compiler",
        )
        if resolved.get("schema_version") != self.trace_descriptor_schema_version:
            raise RuntimeError(
                "Rust trace handoff descriptor compiler returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.trace_descriptor_authority:
            raise RuntimeError(
                "Rust trace handoff descriptor compiler returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        handoff = resolved.get("handoff")
        if not isinstance(handoff, dict):
            raise RuntimeError("Rust trace handoff descriptor compiler returned a missing handoff payload.")
        return handoff

    def checkpoint_resume_manifest(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--checkpoint-resume-manifest-json",
            "--checkpoint-resume-manifest-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "checkpoint_resume_manifest",
            payload,
            [*self._binary_command(), *args],
            failure_label="checkpoint resume manifest compiler",
        )
        if resolved.get("schema_version") != self.checkpoint_resume_manifest_schema_version:
            raise RuntimeError(
                "Rust checkpoint resume manifest compiler returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.checkpoint_resume_manifest_authority:
            raise RuntimeError(
                "Rust checkpoint resume manifest compiler returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        manifest = resolved.get("resume_manifest")
        if not isinstance(manifest, dict):
            raise RuntimeError(
                "Rust checkpoint resume manifest compiler returned a missing resume_manifest payload."
            )
        return manifest

    def write_transport_binding(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--write-transport-binding-json",
            "--write-transport-binding-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "write_transport_binding",
            payload,
            [*self._binary_command(), *args],
            failure_label="transport binding writer",
        )
        if resolved.get("schema_version") != self.transport_binding_write_schema_version:
            raise RuntimeError(
                "Rust transport binding writer returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.transport_binding_write_authority:
            raise RuntimeError(
                "Rust transport binding writer returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        path = resolved.get("path")
        bytes_written = resolved.get("bytes_written")
        if not isinstance(path, str) or not path:
            raise RuntimeError("Rust transport binding writer returned a missing path.")
        if not isinstance(bytes_written, int) or bytes_written < 0:
            raise RuntimeError("Rust transport binding writer returned invalid bytes_written.")
        return resolved

    def write_checkpoint_resume_manifest(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--write-checkpoint-resume-manifest-json",
            "--write-checkpoint-resume-manifest-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "write_checkpoint_resume_manifest",
            payload,
            [*self._binary_command(), *args],
            failure_label="checkpoint resume manifest writer",
        )
        if resolved.get("schema_version") != self.checkpoint_manifest_write_schema_version:
            raise RuntimeError(
                "Rust checkpoint resume manifest writer returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.checkpoint_manifest_write_authority:
            raise RuntimeError(
                "Rust checkpoint resume manifest writer returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        path = resolved.get("path")
        bytes_written = resolved.get("bytes_written")
        if not isinstance(path, str) or not path:
            raise RuntimeError("Rust checkpoint resume manifest writer returned a missing path.")
        if not isinstance(bytes_written, int) or bytes_written < 0:
            raise RuntimeError("Rust checkpoint resume manifest writer returned invalid bytes_written.")
        return resolved

    def attach_runtime_event_transport(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--attach-runtime-event-transport-json",
            "--attach-runtime-event-transport-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        try:
            resolved = self._run_hot_json_command(
                "attach_runtime_event_transport",
                payload,
                [*self._binary_command(), *args],
                failure_label="attached runtime event transport",
            )
        except RuntimeError as exc:
            message = str(exc)
            if not message.startswith("Rust attached runtime event transport failed: "):
                raise RuntimeError(f"Rust attached runtime event transport failed: {message}") from exc
            raise
        if resolved.get("attach_mode") != "process_external_artifact_replay":
            raise RuntimeError(
                "Rust attached runtime event transport returned an unknown attach mode: "
                f"{resolved.get('attach_mode')!r}"
            )
        if resolved.get("authority") != self.attached_runtime_event_transport_authority:
            raise RuntimeError(
                "Rust attached runtime event transport returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def subscribe_attached_runtime_events(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--subscribe-attached-runtime-events-json",
            "--subscribe-attached-runtime-events-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "subscribe_attached_runtime_events",
            payload,
            [*self._binary_command(), *args],
            failure_label="attached runtime event replay",
        )
        if resolved.get("schema_version") != "runtime-event-bridge-v1":
            raise RuntimeError(
                "Rust attached runtime event replay returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        return resolved

    def cleanup_attached_runtime_event_transport(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--cleanup-attached-runtime-event-transport-json",
            "--cleanup-attached-runtime-event-transport-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "cleanup_attached_runtime_event_transport",
            payload,
            [*self._binary_command(), *args],
            failure_label="attached runtime event cleanup",
        )
        if resolved.get("cleanup_method") != "cleanup_attached_runtime_event_transport":
            raise RuntimeError(
                "Rust attached runtime event cleanup returned an unknown cleanup method: "
                f"{resolved.get('cleanup_method')!r}"
            )
        if resolved.get("authority") != self.attached_runtime_event_transport_authority:
            raise RuntimeError(
                "Rust attached runtime event cleanup returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def trace_stream_replay(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--trace-stream-replay-json",
            "--trace-stream-replay-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "trace_stream_replay",
            payload,
            [*self._binary_command(), *args],
            failure_label="trace stream replay",
        )
        if resolved.get("schema_version") != self.trace_stream_replay_schema_version:
            raise RuntimeError(
                "Rust trace stream replay returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.trace_stream_io_authority:
            raise RuntimeError(
                "Rust trace stream replay returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def trace_stream_inspect(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--trace-stream-inspect-json",
            "--trace-stream-inspect-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "trace_stream_inspect",
            payload,
            [*self._binary_command(), *args],
            failure_label="trace stream inspect",
        )
        if resolved.get("schema_version") != self.trace_stream_inspect_schema_version:
            raise RuntimeError(
                "Rust trace stream inspect returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.trace_stream_io_authority:
            raise RuntimeError(
                "Rust trace stream inspect returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        return resolved

    def write_trace_compaction_delta(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--write-trace-compaction-delta-json",
            "--write-trace-compaction-delta-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        resolved = self._run_hot_json_command(
            "write_trace_compaction_delta",
            payload,
            [*self._binary_command(), *args],
            failure_label="trace compaction delta writer",
        )
        if resolved.get("schema_version") != self.trace_compaction_delta_write_schema_version:
            raise RuntimeError(
                "Rust trace compaction delta writer returned an unknown schema: "
                f"{resolved.get('schema_version')!r}"
            )
        if resolved.get("authority") != self.trace_stream_io_authority:
            raise RuntimeError(
                "Rust trace compaction delta writer returned an unexpected authority marker: "
                f"{resolved.get('authority')!r}"
            )
        path = resolved.get("path")
        bytes_written = resolved.get("bytes_written")
        if not isinstance(path, str) or not path:
            raise RuntimeError("Rust trace compaction delta writer returned a missing path.")
        if not isinstance(bytes_written, int) or bytes_written < 0:
            raise RuntimeError("Rust trace compaction delta writer returned invalid bytes_written.")
        return resolved

    def health(self) -> dict[str, Any]:
        """Describe Rust route-adapter availability."""

        resolved_binary = self._resolved_binary()
        latest_source_mtime = self._cached_source_mtime()
        resolved_binary_mtime = resolved_binary.stat().st_mtime if resolved_binary is not None else None
        return {
            "runtime_path": str(self.runtime_path),
            "manifest_path": str(self.manifest_path),
            "resolved_binary": str(resolved_binary) if resolved_binary is not None else None,
            "resolved_binary_mtime": resolved_binary_mtime,
            "latest_source_mtime": latest_source_mtime,
            "source_newer_than_resolved_binary": (
                latest_source_mtime > resolved_binary_mtime
                if resolved_binary_mtime is not None
                else None
            ),
            "available": resolved_binary is not None or (self.router_dir / "Cargo.toml").exists(),
            "route_authority": self.route_authority,
            "compile_authority": self.compile_authority,
            "runtime_control_plane_authority": self.runtime_control_plane_authority,
            "sandbox_control_authority": self.sandbox_control_authority,
            "background_control_authority": self.background_control_authority,
            "route_decision_schema_version": self.route_decision_schema_version,
            "route_policy_schema_version": self.route_policy_schema_version,
            "route_snapshot_schema_version": self.route_snapshot_schema_version,
            "route_report_schema_version": self.route_report_schema_version,
            "runtime_control_plane_schema_version": self.runtime_control_plane_schema_version,
            "sandbox_control_schema_version": self.sandbox_control_schema_version,
            "background_control_schema_version": self.background_control_schema_version,
            "trace_descriptor_schema_version": self.trace_descriptor_schema_version,
            "checkpoint_resume_manifest_schema_version": self.checkpoint_resume_manifest_schema_version,
            "transport_binding_write_schema_version": self.transport_binding_write_schema_version,
            "checkpoint_manifest_write_schema_version": self.checkpoint_manifest_write_schema_version,
            "trace_stream_replay_schema_version": self.trace_stream_replay_schema_version,
            "trace_stream_inspect_schema_version": self.trace_stream_inspect_schema_version,
            "trace_compaction_delta_write_schema_version": self.trace_compaction_delta_write_schema_version,
            "runtime_observability_exporter_schema_version": self.runtime_observability_exporter_schema_version,
            "runtime_observability_metric_catalog_schema_version": self.runtime_observability_metric_catalog_schema_version,
            "runtime_observability_metric_record_schema_version": self.runtime_observability_metric_record_schema_version,
            "runtime_observability_dashboard_schema_version": self.runtime_observability_dashboard_schema_version,
            "trace_descriptor_authority": self.trace_descriptor_authority,
            "checkpoint_resume_manifest_authority": self.checkpoint_resume_manifest_authority,
            "transport_binding_write_authority": self.transport_binding_write_authority,
            "checkpoint_manifest_write_authority": self.checkpoint_manifest_write_authority,
            "trace_stream_io_authority": self.trace_stream_io_authority,
        }

    def _binary_command(self) -> list[str]:
        return self._compiled_binary_command()

    def _stdio_command(self) -> list[str]:
        return [*self._compiled_binary_command(), "--stdio-json"]

    def _compiled_binary_command(self) -> list[str]:
        resolved_binary = self._fresh_resolved_binary()
        if resolved_binary is None:
            raise RuntimeError(
                "router-rs requires a prebuilt binary; build scripts/router-rs before running the Python host runtime."
            )
        return [str(resolved_binary)]

    def _route_args(self, query: str, session_id: str, allow_overlay: bool, first_turn: bool) -> list[str]:
        args = [
            "--query",
            query,
            "--limit",
            "5",
            "--runtime",
            str(self.runtime_path),
            "--manifest",
            str(self.manifest_path),
            "--route-json",
            "--session-id",
            session_id,
        ]
        args.append(f"--allow-overlay={'true' if allow_overlay else 'false'}")
        args.append(f"--first-turn={'true' if first_turn else 'false'}")
        return args

    def _resolved_binary(self) -> Path | None:
        return resolve_router_binary_candidate(self.release_bin, self.debug_bin)

    def _fresh_resolved_binary(self) -> Path | None:
        resolved_binary = self._resolved_binary()
        if resolved_binary is None:
            return None
        latest_source_mtime = self._latest_source_mtime()
        if resolved_binary.stat().st_mtime < latest_source_mtime:
            raise RuntimeError(
                "router-rs prebuilt binary is stale; source files are newer than the compiled artifact. "
                "Rebuild scripts/router-rs before running the Python host runtime."
            )
        return resolved_binary

    def _cached_resolved_binary(self) -> Path | None:
        cached = self._cached_runtime_binary
        if cached is not _ROUTER_BINARY_CACHE_UNSET:
            return cached if isinstance(cached, Path) else None
        resolved_binary = self._resolved_binary()
        if resolved_binary is not None:
            self._cached_runtime_binary = resolved_binary
        return resolved_binary

    def _latest_source_mtime(self) -> float:
        candidates = [self.router_dir / "Cargo.toml"]
        source_dir = self.router_dir / "src"
        if source_dir.is_dir():
            candidates.extend(source_dir.rglob("*.rs"))
        return max((path.stat().st_mtime for path in candidates if path.exists()), default=0.0)

    def _cached_source_mtime(self) -> float:
        cached = self._cached_latest_source_mtime
        if cached is None:
            cached = self._latest_source_mtime()
            self._cached_latest_source_mtime = cached
        return cached

    def _invalidate_binary_cache(self) -> None:
        self._cached_runtime_binary = _ROUTER_BINARY_CACHE_UNSET
        self._cached_latest_source_mtime = None

    def _run_json_command(self, command: list[str], *, failure_label: str) -> dict[str, Any]:
        try:
            proc = subprocess.run(
                command,
                capture_output=True,
                text=True,
                check=True,
                timeout=self.timeout_seconds,
                cwd=self.codex_home,
            )
        except subprocess.CalledProcessError as exc:
            stderr = (exc.stderr or exc.stdout or "").strip()
            raise RuntimeError(f"Rust {failure_label} failed: {stderr}") from exc
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError(f"Rust {failure_label} timed out after {self.timeout_seconds}s.") from exc
        return _load_json_object(proc.stdout, source=f"Rust {failure_label} command output")

    def _run_hot_json_command(
        self,
        operation: str,
        payload: Mapping[str, Any],
        command: list[str],
        *,
        failure_label: str,
    ) -> dict[str, Any]:
        if not self._uses_default_json_runner():
            return self._run_json_command(command, failure_label=failure_label)
        try:
            return self._stdio_client().request(operation, payload)
        except RuntimeError as exc:
            error_text = str(exc)
            if "unsupported" in error_text and "operation" in error_text:
                self._reset_stdio_client()
                self._invalidate_binary_cache()
                return self._run_json_command(command, failure_label=failure_label)
            self._reset_stdio_client()
            return self._stdio_client().request(operation, payload)

    def _stdio_client(self) -> _RouterStdioClient:
        command = self._stdio_command()
        key = self._stdio_client_key(command)
        with _STDIO_CLIENTS_LOCK:
            client = _STDIO_CLIENTS.get(key)
            if client is None:
                client = _RouterStdioClient(
                    command,
                    cwd=self.codex_home,
                    timeout_seconds=self.timeout_seconds,
                )
                _STDIO_CLIENTS[key] = client
            return client

    def _reset_stdio_client(self) -> None:
        key = self._stdio_client_key(self._stdio_command())
        with _STDIO_CLIENTS_LOCK:
            client = _STDIO_CLIENTS.pop(key, None)
        if client is not None:
            client.close()

    def _stdio_client_key(self, command: list[str]) -> tuple[str, ...]:
        binary_mtime = ""
        if command:
            binary_path = Path(command[0])
            if binary_path.exists():
                binary_mtime = str(binary_path.stat().st_mtime_ns)
        return (*command, f"binary-mtime={binary_mtime}", str(self.codex_home))

    def _uses_default_json_runner(self) -> bool:
        return getattr(self._run_json_command, "__func__", None) is RustRouteAdapter._run_json_command
