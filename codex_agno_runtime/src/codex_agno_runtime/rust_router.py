"""Rust route-engine adapter used by the Python host runtime."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

from codex_agno_runtime.schemas import (
    RouteDecisionContract,
    RouteDecisionSnapshot,
    RouteDiagnosticReport,
    RouteExecutionPolicy,
)


class RustRouteAdapter:
    """Call the repository Rust route engine for final route decisions."""

    route_decision_schema_version = "router-rs-route-decision-v1"
    route_policy_schema_version = "router-rs-route-policy-v1"
    route_snapshot_schema_version = "router-rs-route-snapshot-v1"
    route_report_schema_version = "router-rs-route-report-v2"
    runtime_control_plane_schema_version = "router-rs-runtime-control-plane-v1"
    background_control_schema_version = "router-rs-background-control-v1"
    trace_descriptor_schema_version = "router-rs-trace-descriptor-v1"
    checkpoint_resume_manifest_schema_version = "router-rs-checkpoint-resume-manifest-v1"
    transport_binding_write_schema_version = "router-rs-transport-binding-write-v1"
    checkpoint_manifest_write_schema_version = "router-rs-checkpoint-manifest-write-v1"
    trace_stream_replay_schema_version = "router-rs-trace-stream-replay-v1"
    trace_stream_inspect_schema_version = "router-rs-trace-stream-inspect-v1"
    trace_compaction_delta_write_schema_version = "router-rs-trace-compaction-delta-write-v1"
    runtime_observability_exporter_schema_version = "runtime-observability-exporter-v1"
    runtime_observability_metric_record_schema_version = "runtime-observability-metric-record-v1"
    runtime_observability_dashboard_schema_version = "runtime-observability-dashboard-v1"
    framework_runtime_snapshot_schema_version = "router-rs-framework-runtime-snapshot-v1"
    framework_contract_summary_schema_version = "router-rs-framework-contract-summary-v1"
    route_authority = "rust-route-core"
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
        command = [*self._binary_command(), *args]
        payload = self._run_json_command(command, failure_label="route engine")
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
        return self._run_json_command(command, failure_label="profile compiler")

    def route_report(
        self,
        *,
        mode: str,
        rust_route_snapshot: dict[str, Any],
    ) -> dict[str, Any]:
        """Build the stable Rust-owned route diagnostic report."""

        return self.route_report_contract(
            mode=mode,
            rust_route_snapshot=rust_route_snapshot,
        ).model_dump(mode="json")

    def route_report_contract(
        self,
        *,
        mode: str,
        rust_route_snapshot: dict[str, Any] | RouteDecisionSnapshot,
    ) -> RouteDiagnosticReport:
        """Build one typed Rust-owned route diagnostic report."""

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
        try:
            payload = self._run_json_command(
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
            payload = self._run_json_command(
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
            payload = self._run_json_command(
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
        return self._run_json_command(command, failure_label="profile artifact compiler")

    def framework_runtime_snapshot(self, *, repo_root: Path) -> dict[str, Any]:
        """Build the framework runtime snapshot read-model through router-rs."""

        args = [
            "--framework-runtime-snapshot-json",
            "--repo-root",
            str(repo_root),
        ]
        try:
            payload = self._run_json_command(
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
            payload = self._run_json_command(
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
            payload = self._run_json_command(
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
            payload = self._run_json_command(
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

    def runtime_observability_dashboard_schema(self) -> dict[str, Any]:
        """Return the Rust-owned runtime observability dashboard schema."""

        args = ["--runtime-observability-dashboard-json"]
        try:
            payload = self._run_json_command(
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
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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

    def trace_stream_replay(self, payload: dict[str, Any]) -> dict[str, Any]:
        args = [
            "--trace-stream-replay-json",
            "--trace-stream-replay-input-json",
            json.dumps(payload, ensure_ascii=False),
        ]
        try:
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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
            resolved = self._run_json_command(
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
        return {
            "runtime_path": str(self.runtime_path),
            "manifest_path": str(self.manifest_path),
            "resolved_binary": str(resolved_binary) if resolved_binary is not None else None,
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
            "runtime_observability_metric_record_schema_version": self.runtime_observability_metric_record_schema_version,
            "runtime_observability_dashboard_schema_version": self.runtime_observability_dashboard_schema_version,
            "trace_descriptor_authority": self.trace_descriptor_authority,
            "checkpoint_resume_manifest_authority": self.checkpoint_resume_manifest_authority,
            "transport_binding_write_authority": self.transport_binding_write_authority,
            "checkpoint_manifest_write_authority": self.checkpoint_manifest_write_authority,
            "trace_stream_io_authority": self.trace_stream_io_authority,
        }

    def _binary_command(self) -> list[str]:
        resolved_binary = self._resolved_binary()
        if resolved_binary is not None:
            return [str(resolved_binary)]
        return self._cargo_command()

    def _cargo_command(self) -> list[str]:
        """Return the cargo-run fallback command for a fresh Rust invocation."""

        return [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(self.router_dir / "Cargo.toml"),
            "--",
        ]

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
        for candidate in (self.debug_bin, self.release_bin):
            if candidate.is_file() and candidate.stat().st_mtime >= latest_source_mtime:
                return candidate
        return None

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
