"""Rust route projection for Python host importers."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping

from framework_runtime.rust_router import RustRouteAdapter, discover_codex_home
from framework_runtime.schemas import RouteDecisionSnapshot, RoutingResult, SkillMetadata


def _router_service_descriptor(control_plane_descriptor: Mapping[str, Any] | None) -> dict[str, Any]:
    """Project the router slice from the shared runtime control plane."""

    if not isinstance(control_plane_descriptor, Mapping):
        return {}
    services = control_plane_descriptor.get("services")
    if not isinstance(services, Mapping):
        return {}
    router = services.get("router")
    if not isinstance(router, Mapping):
        return {}
    return dict(router)


class SkillRouter:
    """Route through router-rs for every decision."""

    def __init__(
        self,
        skills: list[SkillMetadata],
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
        rust_adapter: RustRouteAdapter | None = None,
    ) -> None:
        self.skills = skills
        self.control_plane_descriptor = (
            dict(control_plane_descriptor) if isinstance(control_plane_descriptor, Mapping) else {}
        )
        self._service_descriptor = _router_service_descriptor(control_plane_descriptor)
        self._rust_adapter = rust_adapter
        self._inline_adapter: RustRouteAdapter | None = None

    def projection_descriptor(self) -> dict[str, Any]:
        """Describe the Rust route projection used by Python host imports."""

        projection = str(self._service_descriptor.get("projection", "")).strip()
        materialization = str(self._service_descriptor.get("rust_projection_materialization", "")).strip()
        return {
            "authority": self._service_descriptor.get("authority"),
            "role": self._service_descriptor.get("role"),
            "projection": projection or "rust-native-projection",
            "delegate_kind": self._service_descriptor.get("delegate_kind"),
            "rust_projection_materialization": materialization or "router-rs-stdio",
            "rust_owned": True,
        }

    def route(
        self,
        task: str,
        session_id: str,
        allow_overlay: bool = True,
        first_turn: bool = True,
    ) -> RoutingResult:
        decision = self._route_contract(
            task=task,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
        )
        selected = self._resolve_skill(decision.selected_skill)
        overlay = self._resolve_skill(decision.overlay_skill) if decision.overlay_skill else None
        reasons = [str(reason) for reason in decision.reasons]
        return RoutingResult(
            task=decision.task,
            session_id=decision.session_id,
            selected_skill=selected,
            overlay_skill=overlay,
            score=float(decision.score),
            layer=decision.layer,
            reasons=reasons,
            route_snapshot=RouteDecisionSnapshot.model_validate(
                decision.route_snapshot.model_dump(mode="json")
            ),
            route_engine="rust",
        )

    def _route_contract(
        self,
        *,
        task: str,
        session_id: str,
        allow_overlay: bool,
        first_turn: bool,
    ):
        adapter = self._rust_adapter
        if adapter is not None:
            return adapter.route_contract(
                query=task,
                session_id=session_id,
                allow_overlay=allow_overlay,
                first_turn=first_turn,
            )
        return self._inline_route_adapter().route_inline_contract(
            query=task,
            session_id=session_id,
            allow_overlay=allow_overlay,
            first_turn=first_turn,
            skills=self.skills,
        )

    def _inline_route_adapter(self) -> RustRouteAdapter:
        adapter = self._inline_adapter
        if adapter is None:
            adapter = RustRouteAdapter(discover_codex_home(Path.cwd()))
            self._inline_adapter = adapter
        return adapter

    def _resolve_skill(self, skill_name: str | None) -> SkillMetadata | None:
        if not skill_name:
            return None
        for skill in self.skills:
            if skill.name == skill_name:
                return skill
        raise RuntimeError(f"Rust router selected unknown skill {skill_name!r} for the loaded skill catalog.")
