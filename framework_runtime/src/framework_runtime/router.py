"""Layer-aware skill router for Codex tasks."""

from __future__ import annotations

from collections import defaultdict
import re
from typing import Any, Mapping

from framework_runtime.schemas import RoutingResult, ScoredSkill, SkillMetadata
from framework_runtime.utils import normalize_text, tokenize


LAYER_ORDER = {"L-1": -1, "L0": 0, "L1": 1, "L2": 2, "L3": 3, "L4": 4}
PRIORITY_ORDER = {"P0": 0, "P1": 1, "P2": 2, "P3": 3}

ROUTING_META_HINTS = {"skill", "router", "routing", "route", "触发", "路由", "skill.md"}
COMMON_STOP_TOKENS = {"一个", "帮我", "帮我看", "我看", "先给", "给我", "给我一", "我一个", "写一", "写一个", "看这", "这张", "然后", "输出", "问题", "checklist", "skill", "路由"}
# Skills that should only be used as overlays, never as the primary owner.
# Per AGENTS.md: iterative-optimizer does not count toward the overlay quota.
OVERLAY_ONLY_SKILLS = {"iterative-optimizer", "execution-audit", "i18n-l10n", "humanizer"}
OVERLAY_EXPLICIT_HINTS = {
    "anti-laziness": {"anti-laziness", "防偷懒", "别糊弄", "别装死", "严格落实", "不许偷工减料", "gsd", "get shit done", "推进到底", "别停", "直接干完"},
    "code-review": {"code-review", "code review", "pr review", "代码审查", "review"},
    "coding-standards": {"coding-standards", "编码规范", "代码风格", "持续改进", "standardize"},
    "error-handling-patterns": {"error-handling-patterns", "错误处理", "error propagation", "retry", "circuit breaker"},
    "iterative-optimizer": {"iterative-optimizer", "多轮优化", "自迭代", "优化x轮", "review→fix→verify"},
    "execution-audit": {"execution-audit", "execution-audit-codex", "强制验收", "零容忍审计", "sign-off", "高质量闭环"},
    "frontend-code-quality": {"frontend-code-quality", "前端代码质量", "early return", "roro", "frontend quality"},
    "i18n-l10n": {"i18n", "l10n", "国际化", "多语言", "localization", "internationalization", "locale", "rtl"},
    "security-audit": {"security-audit", "安全审计", "security review", "ssrf", "csrf", "鉴权"},
    "skill-routing-repair": {"skill-routing-repair", "skill-routing-repair-codex", "路由修复", "触发修复", "以后别再选错", "顺手修一下 skill"},
    "tdd-workflow": {"tdd-workflow", "tdd", "red-green-refactor", "先写测试", "测试驱动"},
    "vercel-react-best-practices": {"vercel-react-best-practices", "react 最佳实践", "next.js 最佳实践", "hydration", "server component"},
    "writing-skills": {"writing-skills", "skill 文档统一", "批量改 skill", "template unification", "standardize skill docs"},
    "humanizer": {"humanizer", "humanize", "自然化", "降 ai 味", "去 ai 感", "像人写的"},
}
WORDLIKE_TOKEN_RE = re.compile(r"^[a-z0-9.+#/_-]+$")
# Static alias hints supplement dynamic tag-based aliases
SKILL_ALIAS_HINTS = {
    "checklist-writting": {"执行清单", "先写 checklist", "写成 checklist md", "写成 checklist 文件", "放到 checklist 目录", "路线已经定了", "不用再论证", "execution-ready checklist"},
    "checklist-normalizer": {"规范化 checklist", "整理成可执行清单", "串行的写在一点", "并行的拆开", "lane 重写", "补齐验收和约束"},
    "checklist-fixer": {"逐项修复", "fix list", "按 checklist 执行", "先做 1-3", "从 p0 开始", "只做第一个"},
    "skill-routing-repair": {"这次为什么没触发", "为什么没有触发", "是不是路由问题", "路由问题吗", "以后别再选错"},
    "idea-to-plan": {"方案", "先做方案", "技术方案", "路线比较", "tradeoff", "权衡", "先调研再给计划", "先别写代码", "先探索现状再提方案", "先探索代码库再出方案", "风险评估", "decision log", "open questions", "assumptions", "critical files", "explore-plan", "outline.md", "code_list.md", "收敛"},
    "python-pro": {"python", "脚本", "pytest", "fastapi", "mypy", "pyright"},
    "visual-review": {"截图", "看图", "render", "渲染", "screenshot", "screen recording", "录屏", "图像", "图片"},
    "frontend-design": {"改ui", "ui 改版", "ui redesign", "品牌感", "视觉层级", "高级感", "dashboard", "bento ui"},
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
    "execution-controller-coding": {"高负载", "跨文件", "长运行", "系统指挥中心", ".supervisor_state.json", "checkpoint", "rollback", "gsd", "get shit done", "推进到底", "别停"},
    "skill-framework-developer": {"skill-framework-developer", "skill-developer-codex", "skill框架", "边界重叠", "owner", "gate", "overlay", "framework", "routing", "token"},
}
FRAMEWORK_ALIAS_EXPLICIT_ENTRYPOINTS = {
    "autopilot": {"/autopilot", "$autopilot"},
    "deepinterview": {"/deepinterview", "$deepinterview"},
}
PAPER_CONTEXT_SKILLS = {
    "paper-reviewer",
    "paper-reviser",
    "paper-writing",
    "paper-logic",
    "paper-visuals",
}
PAPER_CONTEXT_MARKERS = (
    "paper",
    "manuscript",
    "论文",
    "稿子",
    "稿件",
    "摘要",
    "引言",
    "审稿意见",
    "reviewer comments",
    "rebuttal",
    "appendix",
    "claim",
)
GSD_EXECUTION_MARKERS = ("gsd", "get shit done", "推进到底", "别停", "直接干完", "一路做完")
GATE_HINTS = {
    "source": {"官方", "官方文档", "文档", "docs", "readme", "api", "openai", "github", "look up", "search"},
    "artifact": {
        "pdf",
        "docx",
        "xlsx",
        "ppt",
        "pptx",
        "excel",
        "spreadsheet",
        "word 文档",
        "word 文件",
        "表格",
        "工作簿",
        "幻灯片",
    },
    "evidence": {"报错", "失败", "崩", "截图", "渲染", "日志", "traceback", "error", "bug", "why", "为什么"},
    "delegation": {"sidecar", "subagent", "delegation", "并行 sidecar", "子代理", "主线程", "local-supervisor", "跨文件", "长运行"},
}


def _normalized_owner(owner: str) -> str:
    return normalize_text(owner)


def _skill_is_overlay(skill: SkillMetadata) -> bool:
    return _normalized_owner(skill.routing_owner) == "overlay" or skill.name in OVERLAY_ONLY_SKILLS


def _can_be_primary_owner(skill: SkillMetadata) -> bool:
    return _normalized_owner(skill.routing_owner) not in {"gate", "overlay"}


def _token_matches_phrase_token(task_token: str, phrase_token: str) -> bool:
    if WORDLIKE_TOKEN_RE.fullmatch(phrase_token):
        return task_token == phrase_token
    return phrase_token in task_token


def _contains_phrase(task_tokens: list[str], phrase: str) -> bool:
    phrase_tokens = tokenize(phrase)
    if not phrase_tokens:
        return False
    if len(phrase_tokens) == 1:
        return any(_token_matches_phrase_token(task_token, phrase_tokens[0]) for task_token in task_tokens)
    token_limit = len(task_tokens) - len(phrase_tokens) + 1
    for start in range(max(0, token_limit)):
        if all(
            _token_matches_phrase_token(task_tokens[start + offset], phrase_token)
            for offset, phrase_token in enumerate(phrase_tokens)
        ):
            return True
    return False


def _framework_alias_requires_explicit_call(skill_name: str) -> bool:
    return skill_name in FRAMEWORK_ALIAS_EXPLICIT_ENTRYPOINTS


def _has_explicit_framework_alias_call(normalized_task: str, task_tokens: list[str], skill_name: str) -> bool:
    entrypoints = FRAMEWORK_ALIAS_EXPLICIT_ENTRYPOINTS.get(skill_name, set())
    return any(
        normalize_text(entrypoint) in normalized_task or entrypoint in task_tokens
        for entrypoint in entrypoints
    )


def _paper_skill_requires_context(skill_name: str) -> bool:
    return skill_name in PAPER_CONTEXT_SKILLS


def _has_paper_context(normalized_task: str, task_tokens: list[str]) -> bool:
    return any(
        marker in normalized_task or _contains_phrase(task_tokens, marker)
        for marker in PAPER_CONTEXT_MARKERS
    )


def _gate_phrases(skill: SkillMetadata) -> list[str]:
    normalized_gate = normalize_text(skill.routing_gate)
    if normalized_gate in GATE_HINTS:
        return sorted(GATE_HINTS[normalized_gate])
    return [
        part.strip()
        for part in skill.routing_gate.split(",")
        if part.strip() and normalize_text(part) != "none"
    ]


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


def _build_scored_skill(skill: SkillMetadata, *, score: float, reasons: list[str]) -> ScoredSkill:
    """Build one scored skill without depending on class identity across import lanes."""

    skill_payload = skill.model_dump(mode="python") if hasattr(skill, "model_dump") else skill
    return ScoredSkill(skill=skill_payload, score=score, reasons=reasons)


def _skill_payload(skill: SkillMetadata | None) -> dict[str, Any] | None:
    """Return one routing-result-safe skill payload across import lanes."""

    if skill is None:
        return None
    return skill.model_dump(mode="python") if hasattr(skill, "model_dump") else skill


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
        materialization = str(self._service_descriptor.get("python_projection_materialization", "")).strip()
        return {
            "authority": self._service_descriptor.get("authority"),
            "role": self._service_descriptor.get("role"),
            "projection": projection or "python-local-router",
            "delegate_kind": self._service_descriptor.get("delegate_kind"),
            "python_projection_materialization": materialization or "in-process",
            "compatibility_only": projection.startswith("python-"),
        }

    def route(self, task: str, session_id: str, allow_overlay: bool = True, first_turn: bool = True) -> RoutingResult:
        scored_candidates = [self._score_skill(skill, task, first_turn) for skill in self.skills]
        viable_candidates = [candidate for candidate in scored_candidates if candidate.score > 0]
        projection = self.projection_descriptor()
        if not viable_candidates:
            fallback_pool = [skill for skill in self.skills if _can_be_primary_owner(skill)] or self.skills
            fallback_skill = min(
                fallback_pool,
                key=lambda skill: (LAYER_ORDER.get(skill.routing_layer, 99), PRIORITY_ORDER.get(skill.routing_priority, 99), skill.name),
            )
            reasons = ["No explicit keyword hit; fell back to the highest-priority skill in layer order."]
            if projection["compatibility_only"]:
                reasons.append("Python router executed only as a thin compatibility projection under the Rust control plane.")
            return RoutingResult(
                task=task,
                session_id=session_id,
                selected_skill=_skill_payload(fallback_skill),
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
            selected_skill=_skill_payload(selected.skill),
            overlay_skill=_skill_payload(overlay),
            score=selected.score,
            layer=selected.skill.routing_layer,
            reasons=reasons,
            route_engine="python",
        )

    def _pick_owner(self, by_layer: dict[str, list[ScoredSkill]]) -> ScoredSkill:
        # Rule 2, 3, 4, 5: Check source gates, artifact gates, evidence gates, delegation gate BEFORE owners.
        all_candidates = [candidate for layer_list in by_layer.values() for candidate in layer_list]
        gate_candidates = [
            candidate
            for candidate in all_candidates
            if _normalized_owner(candidate.skill.routing_owner) == "gate"
            or normalize_text(candidate.skill.routing_gate) != "none"
        ]
        gate_candidates = sorted(gate_candidates, key=lambda c: (-c.score, PRIORITY_ORDER.get(c.skill.routing_priority, 99)))
        owner_candidates = [candidate for candidate in all_candidates if _can_be_primary_owner(candidate.skill)]
        top_owner_score = owner_candidates[0].score if owner_candidates else float("-inf")
        if gate_candidates and gate_candidates[0].score >= 30 and gate_candidates[0].score >= top_owner_score:
            gate_candidates[0].reasons.append("Prioritized via 6-rule gate checklist (Gate before Owner).")
            return gate_candidates[0]

        # Standard layer-precedence logic
        owner_by_layer: dict[str, list[ScoredSkill]] = defaultdict(list)
        for candidate in owner_candidates:
            owner_by_layer[candidate.skill.routing_layer].append(candidate)
        for layer_name in sorted(owner_by_layer, key=lambda value: LAYER_ORDER.get(value, 99)):
            candidates = sorted(
                owner_by_layer[layer_name],
                key=lambda candidate: (
                    -candidate.score,
                    PRIORITY_ORDER.get(candidate.skill.routing_priority, 99),
                    candidate.skill.name,
                ),
            )
            if candidates and candidates[0].score >= self._layer_threshold(layer_name):
                return candidates[0]
        return sorted(
            owner_candidates or all_candidates,
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
        task_tokens = tokenize(task)

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
            explicit_hints = OVERLAY_EXPLICIT_HINTS.get(skill.name, set())
            is_explicitly_mentioned = _contains_phrase(task_tokens, skill.name) or any(
                _contains_phrase(task_tokens, hint) for hint in explicit_hints
            )
            if _skill_is_overlay(skill) and is_explicitly_mentioned:
                explicit_overlay = skill
                break

        if explicit_overlay is not None:
            return explicit_overlay
        if (
            selected_skill.name == "skill-framework-developer"
            and any(
                _contains_phrase(task_tokens, marker)
                for marker in ("review", "framework-review", "routing-review", "审查", "审核")
            )
        ):
            for skill in self.skills:
                if skill.name == "code-review":
                    return skill
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
        task_token_list = tokenize(task)
        task_tokens = {token for token in task_token_list if token not in COMMON_STOP_TOKENS}
        score = 0.0

        settled_strategy_markers = {
            normalize_text("路线已经定了"),
            normalize_text("不用再论证"),
            normalize_text("执行清单"),
            normalize_text("execution-ready checklist"),
        }
        strategic_planning_markers = {
            normalize_text("先调研再给计划"),
            normalize_text("先别写代码"),
            normalize_text("先探索现状再提方案"),
            normalize_text("先探索代码库再出方案"),
            normalize_text("路线比较"),
            normalize_text("decision log"),
            normalize_text("open questions"),
            normalize_text("assumptions"),
            normalize_text("critical files"),
            normalize_text("explore-plan"),
        }

        if skill.name == "idea-to-plan" and any(marker in normalized_task for marker in settled_strategy_markers):
            return ScoredSkill(
                skill=skill.model_dump(mode="python"),
                score=0.0,
                reasons=["Suppressed: task says the strategy is already fixed and only needs execution decomposition."],
            )
        if skill.name == "checklist-writting" and any(marker in normalized_task for marker in strategic_planning_markers):
            return _build_scored_skill(
                skill,
                score=0.0,
                reasons=["Suppressed: task still needs strategic planning rather than execution checklist writing."],
            )
        if (
            skill.name == "systematic-debugging"
            and ("skill" in normalized_task or "skill.md" in normalized_task)
            and any(marker in normalized_task for marker in ("路由", "触发", "routing", "router", "route"))
        ):
            return _build_scored_skill(
                skill,
                score=0.0,
                reasons=["Suppressed: meta-routing repair request should not be treated as a generic runtime-debugging gate."],
            )
        if _framework_alias_requires_explicit_call(skill.name) and not _has_explicit_framework_alias_call(
            normalized_task,
            task_token_list,
            skill.name,
        ):
            return _build_scored_skill(
                skill,
                score=0.0,
                reasons=[
                    "Suppressed: framework alias skills only route from explicit /alias or $alias entrypoints."
                ],
            )
        if _paper_skill_requires_context(skill.name) and not _has_paper_context(
            normalized_task,
            task_token_list,
        ):
            return _build_scored_skill(
                skill,
                score=0.0,
                reasons=["Suppressed: paper skills require explicit paper or manuscript context."],
            )

        if skill.name.startswith("skill-") and not any(hint in normalized_task for hint in ROUTING_META_HINTS):
            return _build_scored_skill(skill, score=0.0, reasons=[])

        if _contains_phrase(task_token_list, skill.name):
            score += 100
            reasons.append(f"Exact skill name matched: {skill.name}.")

        matched_gates = [gate for gate in _gate_phrases(skill) if _contains_phrase(task_token_list, gate)]
        if matched_gates:
            score += 18 + min(12, (len(matched_gates) - 1) * 6)
            reasons.append(f"Routing gate matched: {', '.join(matched_gates)}.")

        name_tokens = set(tokenize(skill.name.replace("-", " ")))
        shared_name_tokens = sorted(task_tokens & name_tokens)
        if shared_name_tokens:
            score += 14 + len(shared_name_tokens) * 4
            reasons.append(f"Name tokens matched: {', '.join(shared_name_tokens)}.")

        for phrase in skill.trigger_hints:
            normalized_phrase = normalize_text(phrase)
            if len(normalized_phrase) < 2:
                continue
            if normalized_phrase in COMMON_STOP_TOKENS:
                continue
            if normalized_phrase and _contains_phrase(task_token_list, normalized_phrase):
                score += 20
                reasons.append(f"Trigger hint matched: {phrase}.")

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

        if _normalized_owner(skill.routing_owner) == "gate" and score > 0:
            score += 2

        if skill.name == "execution-controller-coding":
            controller_markers = [
                token
                for token in ("高负载", "跨文件", "长运行", ".supervisor_state.json", "主线程", "系统指挥中心")
                if token in normalized_task
            ]
            if controller_markers:
                score += 24
                reasons.append(
                    f"Execution-controller boost applied: {', '.join(controller_markers)}."
                )
            gsd_markers = [token for token in GSD_EXECUTION_MARKERS if token in normalized_task]
            if gsd_markers:
                score += 26
                reasons.append(
                    f"GSD execution boost applied: {', '.join(gsd_markers)}."
                )

        if skill.name == "subagent-delegation" and score > 0:
            explicit_delegation = any(
                token in normalized_task
                for token in ("sidecar", "subagent", "delegation", "子代理", "并行 sidecar")
            )
            controller_markers = any(
                token in normalized_task
                for token in ("高负载", "跨文件", "长运行", ".supervisor_state.json", "主线程", "系统指挥中心")
            )
            gsd_posture = any(token in normalized_task for token in GSD_EXECUTION_MARKERS)
            if controller_markers and not explicit_delegation:
                score *= 0.7
                reasons.append("Delegation-gate suppression applied: controller-orchestration signals dominate.")
            if gsd_posture and not explicit_delegation:
                score *= 0.45
                reasons.append("Delegation-gate suppression applied: gsd posture keeps the immediate blocker local.")

        if skill.name == "skill-writer" and score > 0:
            framework_policy_markers = any(
                token in normalized_task
                for token in ("owner gate overlay", "边界重叠", "routing", "framework", "skill框架", "路由策略")
            )
            if framework_policy_markers:
                score *= 0.5
                reasons.append("Single-skill-writer suppression applied: framework-policy signals dominate.")

        if skill.name == "visual-review" and score > 0:
            visual_evidence_markers = (
                "看图", "截图", "渲染", "render", "screenshot", "screen recording", "录屏", "chart", "图片", "图像",
            )
            if not any(marker in normalized_task for marker in visual_evidence_markers):
                return ScoredSkill(
                    skill=skill.model_dump(mode="python"),
                    score=0.0,
                    reasons=["Suppressed: visual-review requires visible evidence, not a generic review token."],
                )

        # Overlay-only skills must not become the primary owner
        if _skill_is_overlay(skill) and score > 0:
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

        return _build_scored_skill(skill, score=score, reasons=reasons)

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
