"""Rust route-engine adapter used by the Python host runtime."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any


class RustRouteAdapter:
    """Call the repository Rust route engine for final route decisions."""

    route_decision_schema_version = "router-rs-route-decision-v1"
    route_policy_schema_version = "router-rs-route-policy-v1"
    route_snapshot_schema_version = "router-rs-route-snapshot-v1"
    route_report_schema_version = "router-rs-route-report-v1"
    runtime_control_plane_schema_version = "router-rs-runtime-control-plane-v1"
    background_control_schema_version = "router-rs-background-control-v1"
    route_authority = "rust-route-core"
    compile_authority = "rust-route-compiler"
    runtime_control_plane_authority = "rust-runtime-control-plane"
    background_control_authority = "rust-background-control"

    def __init__(self, codex_home: Path, *, timeout_seconds: float = 5.0) -> None:
        self.codex_home = codex_home
        self.timeout_seconds = timeout_seconds
        self.runtime_path = codex_home / "skills" / "SKILL_ROUTING_RUNTIME.json"
        self.manifest_path = codex_home / "skills" / "SKILL_MANIFEST.json"
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
        """Return one Rust-backed route decision JSON payload."""

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
        return payload

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
        python_route_snapshot: dict[str, Any],
        rust_route_snapshot: dict[str, Any],
        rollback_active: bool,
    ) -> dict[str, Any]:
        """Build the stable route diff report through the Rust routing core.

        The report is for compare-only diagnostic lanes, not live authority rollback.
        """

        args = [
            "--route-report-json",
            "--route-mode",
            mode,
            "--python-route-snapshot-json",
            json.dumps(python_route_snapshot, ensure_ascii=False),
            "--rust-route-snapshot-json",
            json.dumps(rust_route_snapshot, ensure_ascii=False),
        ]
        if rollback_active:
            args.append("--rollback-active")
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
        return payload

    def route_policy(
        self,
        *,
        mode: str,
        rollback_to_python: bool,
    ) -> dict[str, Any]:
        """Resolve route-mode policy through the Rust routing core.

        The returned policy keeps Python in explicit legacy or diagnostic lanes only.
        """

        args = [
            "--route-policy-json",
            "--route-mode",
            mode,
        ]
        if rollback_to_python:
            args.append("--rollback-active")
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
        return payload

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
        return route_snapshot

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
        if allow_overlay:
            args.append("--allow-overlay")
        if first_turn:
            args.append("--first-turn")
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
