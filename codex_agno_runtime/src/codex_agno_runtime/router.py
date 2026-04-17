"""Layer-aware skill router for Codex tasks."""

from __future__ import annotations

from collections import defaultdict
from typing import Any, Mapping

from codex_agno_runtime.schemas import RoutingResult, ScoredSkill, SkillMetadata
from codex_agno_runtime.utils import normalize_text, tokenize


LAYER_ORDER = {"L-1": -1, "L0": 0, "L1": 1, "L2": 2, "L3": 3, "L4": 4}
PRIORITY_ORDER = {"P0": 0, "P1": 1, "P2": 2, "P3": 3}

ROUTING_META_HINTS = {"skill", "router", "routing", "route", "触发", "路由", "skill.md"}
COMMON_STOP_TOKENS = {"一个", "帮我", "帮我看", "我看", "先给", "给我", "给我一", "我一个", "写一", "写一个", "看这", "这张", "然后", "输出", "问题"}
# Skills that should only be used as overlays, never as the primary owner.
# Per AGENTS.md: iterative-optimizer does not count toward the overlay quota.
OVERLAY_ONLY_SKILLS = {"iterative-optimizer", "execution-audit-codex", "i18n-l10n", "humanizer"}
# Static alias hints supplement dynamic tag-based aliases
SKILL_ALIAS_HINTS = {
    "plan-writing": {"计划", "拆解", "拆任务", "里程碑", "roadmap", "outline"},
    "python-pro": {"python", "脚本", "pytest", "fastapi", "mypy", "pyright"},
    "visual-review": {"截图", "看图", "视觉", "布局", "层级", "render", "渲染", "screenshot", "ui"},
    "plan-to-code": {"实现", "落地", "开发", "按文档", "根据方案", "直接做代码"},
    "systematic-debugging": {"报错", "失败", "崩", "不工作", "异常", "bug"},
    # Language / framework skill aliases
    "rust-pro": {"rust", "cargo", "rustc", "tokio", "actix"},
    "go-pro": {"golang", "goroutine", "go mod", "go build"},
    "typescript-pro": {"typescript", "ts", "tsc", "tsconfig"},
    "react": {"react", "jsx", "tsx", "useState", "useEffect", "hook"},
    "nextjs": {"nextjs", "next.js", "app router", "server component", "page.tsx"},
    "svelte": {"svelte", "sveltekit", "$state", "$derived"},
    "vue": {"vue", "nuxt", "composable", "v-model", "defineComponent"},
    "sql-pro": {"sql", "postgresql", "mysql", "sqlite", "drizzle", "prisma"},
    "docker": {"docker", "dockerfile", "container", "compose", "k8s", "kubernetes"},
    "playwright": {"playwright", "e2e", "浏览器测试", "browser test"},
    "node-backend": {"node", "express", "hono", "elysia", "bun"},
    "tailwind-pro": {"tailwind", "tailwindcss", "tw-"},
    "git-workflow": {"git", "commit", "branch", "merge", "rebase", "pr"},
}


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
    """Select the best matching skill using Codex routing semantics."""

    def __init__(
        self,
        skills: list[SkillMetadata],
        *,
        control_plane_descriptor: Mapping[str, Any] | None = None,
    ) -> None:
        self.skills = skills
        self.control_plane_descriptor = dict(control_plane_descriptor) if isinstance(control_plane_descriptor, Mapping) else {}
        self._service_descriptor = _router_service_descriptor(control_plane_descriptor)

    def projection_descriptor(self) -> dict[str, Any]:
        """Describe the Python router as a Rust-control-plane projection."""

        projection = str(self._service_descriptor.get("projection", "")).strip()
        return {
            "authority": self._service_descriptor.get("authority"),
            "role": self._service_descriptor.get("role"),
            "projection": projection or "python-local-router",
            "delegate_kind": self._service_descriptor.get("delegate_kind"),
            "compatibility_only": projection.startswith("python-"),
        }

    def route(self, task: str, session_id: str, allow_overlay: bool = True, first_turn: bool = True) -> RoutingResult:
        scored_candidates = [self._score_skill(skill, task, first_turn) for skill in self.skills]
        viable_candidates = [candidate for candidate in scored_candidates if candidate.score > 0]
        projection = self.projection_descriptor()
        if not viable_candidates:
            fallback_skill = min(
                self.skills,
                key=lambda skill: (LAYER_ORDER.get(skill.routing_layer, 99), PRIORITY_ORDER.get(skill.routing_priority, 99), skill.name),
            )
            reasons = ["No explicit keyword hit; fell back to the highest-priority skill in layer order."]
            if projection["compatibility_only"]:
                reasons.append("Python router executed only as a thin compatibility projection under the Rust control plane.")
            return RoutingResult(
                task=task,
                session_id=session_id,
                selected_skill=fallback_skill,
                score=0,
                layer=fallback_skill.routing_layer,
                reasons=reasons,
                route_engine="python",
            )

        by_layer: dict[str, list[ScoredSkill]] = defaultdict(list)
        for candidate in viable_candidates:
            by_layer[candidate.skill.routing_layer].append(candidate)

        selected = self._pick_owner(by_layer)
        overlay = self._pick_overlay(task, allow_overlay, selected.skill)
        reasons = list(selected.reasons)
        if projection["compatibility_only"]:
            reasons.append("Python router executed only as a thin compatibility projection under the Rust control plane.")
        return RoutingResult(
            task=task,
            session_id=session_id,
            selected_skill=selected.skill,
            overlay_skill=overlay,
            score=selected.score,
            layer=selected.skill.routing_layer,
            reasons=reasons,
            route_engine="python",
        )

    def _pick_owner(self, by_layer: dict[str, list[ScoredSkill]]) -> ScoredSkill:
        # Rule 2, 3, 4, 5: Check source gates, artifact gates, evidence gates, delegation gate BEFORE owners.
        all_candidates = [candidate for layer_list in by_layer.values() for candidate in layer_list]
        gate_candidates = [c for c in all_candidates if c.skill.routing_owner == "gate" or (c.skill.routing_gate and c.skill.routing_gate != "none")]
        gate_candidates = sorted(gate_candidates, key=lambda c: (-c.score, PRIORITY_ORDER.get(c.skill.routing_priority, 99)))
        if gate_candidates and gate_candidates[0].score >= 30:
            gate_candidates[0].reasons.append("Prioritized via 6-rule gate checklist (Gate before Owner).")
            return gate_candidates[0]

        # Standard layer-precedence logic
        for layer_name in sorted(by_layer, key=lambda value: LAYER_ORDER.get(value, 99)):
            candidates = sorted(
                by_layer[layer_name],
                key=lambda candidate: (
                    -candidate.score,
                    PRIORITY_ORDER.get(candidate.skill.routing_priority, 99),
                    candidate.skill.name,
                ),
            )
            if candidates and candidates[0].score >= self._layer_threshold(layer_name):
                return candidates[0]
        return sorted(
            all_candidates,
            key=lambda candidate: (
                LAYER_ORDER.get(candidate.skill.routing_layer, 99),
                -candidate.score,
                PRIORITY_ORDER.get(candidate.skill.routing_priority, 99),
                candidate.skill.name,
            ),
        )[0]

    def _pick_overlay(self, task: str, allow_overlay: bool, selected_skill: SkillMetadata) -> SkillMetadata | None:
        if not allow_overlay:
            return None
        task_text = normalize_text(task)

        # Rule: L0/L1 tasks auto-attach anti-laziness overlay unless user already
        # requested a different explicit overlay.
        auto_anti_laziness = selected_skill.routing_layer in ("L0", "L1", "L-1")

        explicit_overlay: SkillMetadata | None = None
        anti_laziness_skill: SkillMetadata | None = None

        for skill in self.skills:
            if skill.name == selected_skill.name:
                continue
            if skill.name == "anti-laziness":
                anti_laziness_skill = skill
                continue
            is_overlay_by_role = "overlay" in [r.lower() for r in skill.framework_roles]
            is_known_overlay = skill.name in {"iterative-optimizer", "execution-audit-codex", "i18n-l10n", "humanizer"}
            is_explicitly_mentioned = skill.name in task_text or any(t.lower() in task_text for t in skill.trigger_phrases if len(t) > 3)
            if (is_overlay_by_role or is_known_overlay) and is_explicitly_mentioned:
                explicit_overlay = skill
                break

        if explicit_overlay is not None:
            return explicit_overlay
        if auto_anti_laziness and anti_laziness_skill is not None:
            return anti_laziness_skill
        return None

    def _score_skill(self, skill: SkillMetadata, task: str, first_turn: bool) -> ScoredSkill:
        """Score a skill against the current task.

        Parameters:
            skill: The candidate skill.
            task: The user task.
            first_turn: Whether the route is for the first turn.

        Returns:
            ScoredSkill: The scoring result.
        """
        reasons: list[str] = []
        normalized_task = normalize_text(task)
        task_tokens = {token for token in tokenize(task) if token not in COMMON_STOP_TOKENS}
        score = 0.0

        if skill.name.startswith("skill-") and not any(hint in normalized_task for hint in ROUTING_META_HINTS):
            return ScoredSkill(skill=skill, score=0.0, reasons=[])

        if skill.name.lower() in normalized_task:
            score += 100
            reasons.append(f"Exact skill name matched: {skill.name}.")

        gate_phrases = [g.strip() for g in skill.routing_gate.split(",") if g.strip() and g.strip().lower() != "none"]
        matched_gates = [g for g in gate_phrases if normalize_text(g) in normalized_task]
        if matched_gates:
            score += 18 + min(12, (len(matched_gates) - 1) * 6)
            reasons.append(f"Routing gate matched: {', '.join(matched_gates)}.")

        name_tokens = set(tokenize(skill.name.replace("-", " ")))
        shared_name_tokens = sorted(task_tokens & name_tokens)
        if shared_name_tokens:
            score += 14 + len(shared_name_tokens) * 4
            reasons.append(f"Name tokens matched: {', '.join(shared_name_tokens)}.")

        for phrase in skill.trigger_phrases:
            normalized_phrase = normalize_text(phrase)
            if len(normalized_phrase) < 2:
                continue
            if normalized_phrase and normalized_phrase in normalized_task:
                score += 20
                reasons.append(f"Trigger phrase matched: {phrase}.")

        # Dynamic aliases: merge static hints with skill tags
        dynamic_aliases = set(skill.tags) | SKILL_ALIAS_HINTS.get(skill.name, set())
        alias_hits = sorted((task_tokens & dynamic_aliases) - COMMON_STOP_TOKENS)
        if alias_hits:
            score += 12 + len(alias_hits) * 4
            reasons.append(f"Skill alias hints matched: {', '.join(alias_hits[:8])}.")

        keyword_pool = {token for token in set(tokenize(skill.description)) | set(tokenize(skill.when_to_use)) | set(tokenize(" ".join(skill.tags))) if token not in COMMON_STOP_TOKENS}
        shared_keywords = sorted(task_tokens & keyword_pool)
        if shared_keywords:
            score += min(24, len(shared_keywords) * 3)
            reasons.append(f"Description keywords matched: {', '.join(shared_keywords[:8])}.")

        if first_turn and score > 0:
            if skill.session_start == "required":
                score += 8
                reasons.append("Session-start required boost applied (+8).")
            elif skill.session_start == "preferred":
                score += 3
                reasons.append("Session-start preferred boost applied (+3).")

        if skill.routing_owner == "gate" and score > 0:
            score += 2

        # Overlay-only skills must not become the primary owner
        if skill.name in OVERLAY_ONLY_SKILLS and score > 0:
            score = max(0, score * 0.15)
            reasons.append(f"Owner suppression applied: {skill.name} is overlay-only.")

        # Negative signal from "Do not use" section
        if skill.do_not_use and score > 0:
            do_not_tokens = {t for t in tokenize(skill.do_not_use) if t not in COMMON_STOP_TOKENS and len(t) > 2}
            negative_hits = task_tokens & do_not_tokens
            if negative_hits:
                penalty = min(score * 0.3, len(negative_hits) * 5)
                score = max(0, score - penalty)
                reasons.append(f"Do-not-use penalty applied: {', '.join(sorted(negative_hits)[:5])}.")


        return ScoredSkill(skill=skill, score=score, reasons=reasons)

    def _layer_threshold(self, layer_name: str) -> float:
        """Return the minimum confidence threshold for a routing layer.

        Parameters:
            layer_name: The routing layer label.

        Returns:
            float: The layer threshold.
        """
        if layer_name == "L0":
            return 18
        if layer_name == "L1":
            return 16
        if layer_name == "L2":
            return 14
        if layer_name == "L3":
            return 14
        return 15
