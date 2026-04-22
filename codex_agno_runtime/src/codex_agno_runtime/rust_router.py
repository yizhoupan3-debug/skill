"""Rust route-engine adapter used by the Python host runtime."""

from __future__ import annotations

import atexit
import json
import select
import subprocess
import threading
from pathlib import Path
from typing import Any, Mapping

from codex_agno_runtime.schemas import (
    RouteDecisionContract,
    RouteDecisionSnapshot,
    RouteDiagnosticReport,
    RouteExecutionPolicy,
)


def resolve_router_binary_candidate(*candidates: Path) -> Path | None:
    """Return the first existing router-rs binary in caller preference order."""

    for candidate in candidates:
        if candidate.is_file():
            return candidate
    return None


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
                response = json.loads(response_line)
            except json.JSONDecodeError as exc:
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


class RustRouteAdapter:
    """Call the repository Rust route engine for final route decisions."""

    route_decision_schema_version = "router-rs-route-decision-v1"
    execution_schema_version = "router-rs-execute-response-v1"
    route_policy_schema_version = "router-rs-route-policy-v1"
    route_snapshot_schema_version = "router-rs-route-snapshot-v1"
    route_report_schema_version = "router-rs-route-report-v2"
    runtime_control_plane_schema_version = "router-rs-runtime-control-plane-v1"
    background_control_schema_version = "router-rs-background-control-v1"
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
    route_authority = "rust-route-core"
    execution_authority = "rust-execution-cli"
    compile_authority = "rust-route-compiler"
    runtime_control_plane_authority = "rust-runtime-control-plane"
    background_control_authority = "rust-background-control"
    trace_descriptor_authority = "rust-runtime-trace-descriptor"
    checkpoint_resume_manifest_authority = "rust-runtime-checkpoint-manifest"
    transport_binding_write_authority = "rust-runtime-transport-binding-writer"
    checkpoint_manifest_write_authority = "rust-runtime-checkpoint-manifest-writer"
    trace_stream_io_authority = "rust-runtime-trace-io"
    framework_runtime_authority = "rust-framework-runtime-read-model"

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

    def route(
        self,
        *,
        query: str,
        session_id: str,
        allow_overlay: bool,
        first_turn: bool,
    ) -> dict[str, Any]:
        """Return one compatibility transport payload derived from the typed route contract."""

        return self.route_contract(
            query=query,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        ).to_transport_payload()

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
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="execution kernel",
            )
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
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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

    def route_report(
        self,
        *,
        mode: str,
        rust_route_snapshot: dict[str, Any] | None = None,
        route_decision_contract: RouteDecisionContract | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Build the stable Rust-owned route diagnostic report."""

        return self.route_report_contract(
            mode=mode,
            rust_route_snapshot=rust_route_snapshot,
            route_decision_contract=route_decision_contract,
        ).model_dump(mode="json")

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
        try:
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
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="route report engine",
            )
        if payload.get("report_schema_version") != self.route_report_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            payload = self._run_hot_json_command(
                "route_policy",
                {"mode": mode},
                [*self._binary_command(), *args],
                failure_label="route policy engine",
            )
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="route policy engine",
            )
        if payload.get("policy_schema_version") != self.route_policy_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
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
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="route snapshot engine",
            )
        if payload.get("snapshot_schema_version") != self.route_snapshot_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            payload = self._run_hot_json_command(
                "framework_runtime_snapshot",
                {"repo_root": str(repo_root)},
                [*self._binary_command(), *args],
                failure_label="framework runtime snapshot compiler",
            )
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="framework runtime snapshot compiler",
            )
        if payload.get("schema_version") != self.framework_runtime_snapshot_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            payload = self._run_hot_json_command(
                "framework_contract_summary",
                {"repo_root": str(repo_root)},
                [*self._binary_command(), *args],
                failure_label="framework contract summary compiler",
            )
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="framework contract summary compiler",
            )
        if payload.get("schema_version") != self.framework_contract_summary_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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

    def runtime_control_plane(self) -> dict[str, Any]:
        """Return the Rust-owned runtime control-plane authority descriptor."""

        args = ["--runtime-control-plane-json"]
        try:
            payload = self._run_hot_json_command(
                "runtime_control_plane",
                {},
                [*self._binary_command(), *args],
                failure_label="runtime control-plane compiler",
            )
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="runtime control-plane compiler",
            )
        if payload.get("schema_version") != self.runtime_control_plane_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            payload = self._run_hot_json_command(
                "runtime_observability_exporter_descriptor",
                {},
                [*self._binary_command(), *args],
                failure_label="runtime observability exporter compiler",
            )
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="runtime observability exporter compiler",
            )
        if payload.get("schema_version") != self.runtime_observability_exporter_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            payload = self._run_hot_json_command(
                "runtime_observability_metric_catalog",
                {},
                [*self._binary_command(), *args],
                failure_label="runtime observability metric catalog compiler",
            )
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="runtime observability metric catalog compiler",
            )
        if payload.get("schema_version") != self.runtime_observability_metric_catalog_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            payload = self._run_hot_json_command(
                "runtime_observability_dashboard_schema",
                {},
                [*self._binary_command(), *args],
                failure_label="runtime observability dashboard compiler",
            )
        except RuntimeError:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="runtime observability dashboard compiler",
            )
        if payload.get("schema_version") != self.runtime_observability_dashboard_schema_version:
            payload = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "runtime_metric_record",
                payload,
                [*self._binary_command(), *args],
                failure_label="runtime metric record compiler",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="runtime metric record compiler",
            )
        if resolved.get("schema_version") != self.runtime_observability_metric_record_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "background_control",
                payload,
                [*self._binary_command(), *args],
                failure_label="background control compiler",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="background control compiler",
            )
        if resolved.get("schema_version") != self.background_control_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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

    def describe_transport(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--describe-transport-json",
            "--describe-transport-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        try:
            resolved = self._run_hot_json_command(
                "describe_transport",
                payload,
                [*self._binary_command(), *args],
                failure_label="trace transport descriptor compiler",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="trace transport descriptor compiler",
            )
        if resolved.get("schema_version") != self.trace_descriptor_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "describe_handoff",
                payload,
                [*self._binary_command(), *args],
                failure_label="trace handoff descriptor compiler",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="trace handoff descriptor compiler",
            )
        if resolved.get("schema_version") != self.trace_descriptor_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "checkpoint_resume_manifest",
                payload,
                [*self._binary_command(), *args],
                failure_label="checkpoint resume manifest compiler",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="checkpoint resume manifest compiler",
            )
        if resolved.get("schema_version") != self.checkpoint_resume_manifest_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "write_transport_binding",
                payload,
                [*self._binary_command(), *args],
                failure_label="transport binding writer",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="transport binding writer",
            )
        if resolved.get("schema_version") != self.transport_binding_write_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "write_checkpoint_resume_manifest",
                payload,
                [*self._binary_command(), *args],
                failure_label="checkpoint resume manifest writer",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="checkpoint resume manifest writer",
            )
        if resolved.get("schema_version") != self.checkpoint_manifest_write_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._binary_command(), *args],
                failure_label="attached runtime event transport",
            )
        if resolved.get("attach_mode") != "process_external_artifact_replay":
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="attached runtime event transport",
            )
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
        try:
            resolved = self._run_hot_json_command(
                "subscribe_attached_runtime_events",
                payload,
                [*self._binary_command(), *args],
                failure_label="attached runtime event replay",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._binary_command(), *args],
                failure_label="attached runtime event replay",
            )
        if resolved.get("schema_version") != "runtime-event-bridge-v1":
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "cleanup_attached_runtime_event_transport",
                payload,
                [*self._binary_command(), *args],
                failure_label="attached runtime event cleanup",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="attached runtime event cleanup",
            )
        if resolved.get("cleanup_method") != "cleanup_attached_runtime_event_transport":
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "trace_stream_replay",
                payload,
                [*self._binary_command(), *args],
                failure_label="trace stream replay",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="trace stream replay",
            )
        if resolved.get("schema_version") != self.trace_stream_replay_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "trace_stream_inspect",
                payload,
                [*self._binary_command(), *args],
                failure_label="trace stream inspect",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="trace stream inspect",
            )
        if resolved.get("schema_version") != self.trace_stream_inspect_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        try:
            resolved = self._run_hot_json_command(
                "write_trace_compaction_delta",
                payload,
                [*self._binary_command(), *args],
                failure_label="trace compaction delta writer",
            )
        except RuntimeError:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
                failure_label="trace compaction delta writer",
            )
        if resolved.get("schema_version") != self.trace_compaction_delta_write_schema_version:
            resolved = self._run_json_command(
                [*self._cargo_command(), *args],
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
        latest_source_mtime = self._latest_source_mtime()
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
            "background_control_authority": self.background_control_authority,
            "route_decision_schema_version": self.route_decision_schema_version,
            "route_policy_schema_version": self.route_policy_schema_version,
            "route_snapshot_schema_version": self.route_snapshot_schema_version,
            "route_report_schema_version": self.route_report_schema_version,
            "runtime_control_plane_schema_version": self.runtime_control_plane_schema_version,
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

    def _cargo_command(self) -> list[str]:
        """Compatibility alias while older fallback call sites are retired."""

        return self._compiled_binary_command()

    def _compiled_binary_command(self) -> list[str]:
        resolved_binary = self._cached_resolved_binary()
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
        latest_source_mtime = self._latest_source_mtime()
        fresh_release = (
            self.release_bin
            if self.release_bin.is_file() and self.release_bin.stat().st_mtime >= latest_source_mtime
            else None
        )
        fresh_debug = (
            self.debug_bin
            if self.debug_bin.is_file() and self.debug_bin.stat().st_mtime >= latest_source_mtime
            else None
        )
        return resolve_router_binary_candidate(
            *(candidate for candidate in (fresh_release, fresh_debug) if candidate is not None)
        )

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
        return json.loads(proc.stdout)

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
        except RuntimeError:
            return self._run_json_command(command, failure_label=failure_label)

    def _stdio_client(self) -> _RouterStdioClient:
        command = self._stdio_command()
        key = (*command, str(self.codex_home))
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

    def _uses_default_json_runner(self) -> bool:
        return getattr(self._run_json_command, "__func__", None) is RustRouteAdapter._run_json_command
