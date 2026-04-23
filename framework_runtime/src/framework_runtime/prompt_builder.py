"""Prompt construction for dynamic skill injection."""

from __future__ import annotations

import re
from textwrap import dedent
from typing import Optional

_COMPACT_BODY_MAX_CHARS = 1200
_COMPACT_BODY_MAX_LINES = 12

from framework_runtime.schemas import RoutingResult, SkillMetadata

# Fuzzy-match substrings for key instructional sections found in real SKILL.md files.
# Uses `in` containment so "## Hard rules for this skill" → matches "hard rule".
_KEY_SECTION_SUBSTRINGS = {
    "hard constraint",
    "hard rule",
    "primary operating",
    "core execution",
    "core workflow",
    "core principle",
    "input contract",
    "boundary logic",
    "trigger example",
    "anti-laziness",
    "laziness",
    "execution model",
    "operating model",
    "output contract",
    "mandatory step",
    "mandatory check",
    "mandatory rule",
    "non-negotiable",
    "enforcement",
    "required tool",
    "quality gate",
    "checkpoint",
    # Added: commonly used sections in real SKILL.md files
    "standard pipeline",
    "subagent policy",
    "skill synergy",
    "automation lane",
    "result validation",
    "runtime-policy",
    "main-thread compression",
    "delegation",
    "watchdog",
    "rollback",
    "approval",
    "verification",
}
_SECTION_RE = re.compile(r"^##\s+(?P<title>.+?)\s*$", re.MULTILINE)


class PromptBuilder:
    """Build dynamic system instructions from selected skills.

    Parameters:
        loader: Optional SkillLoader for lazy body hydration.

    Returns:
        None.
    """

    def __init__(self, loader: Optional["SkillLoader"] = None) -> None:  # noqa: F821
        """Initialize with an optional loader for progressive loading.

        Parameters:
            loader: If provided, used to hydrate skill bodies on demand.

        Returns:
            None.
        """
        self._loader = loader

    def build_prompt(
        self,
        routing_result: RoutingResult,
        *,
        prompt_preview: str | None = None,
    ) -> str:
        """Build the injected prompt for the selected route.

        Parameters:
            routing_result: The routing decision.
            prompt_preview: Optional explicit Rust-owned preview text.

        Returns:
            str: The injected prompt text.
        """
        if prompt_preview:
            return prompt_preview

        selected = routing_result.selected_skill
        if not selected.body_loaded and self._loader is not None:
            self._loader.load_body(selected)

        parts = [
            self._render_route_header(routing_result),
            self._render_skill(selected, heading="Owner skill", is_overlay=False),
        ]
        if routing_result.overlay_skill is not None:
            overlay = routing_result.overlay_skill
            if not overlay.body_loaded and self._loader is not None:
                self._loader.load_body(overlay)
            parts.append(self._render_skill(overlay, heading="Overlay skill", is_overlay=True))
        return "\n\n".join(parts).strip()

    def _render_route_header(self, routing_result: RoutingResult) -> str:
        """Render the control-plane header for compatibility prompts."""

        selected = routing_result.selected_skill
        overlay_name = routing_result.overlay_skill.name if routing_result.overlay_skill else "none"
        reasons = (
            list(routing_result.route_snapshot.reasons)
            if routing_result.route_snapshot is not None and routing_result.route_snapshot.reasons
            else list(routing_result.reasons)
        ) or ["Rust route decision already resolved upstream."]
        reason_block = "\n".join(f"- {reason}" for reason in reasons[:3])
        if routing_result.route_engine == "rust":
            return dedent(
                f"""
                Help with the user's request directly. The route is already chosen, so stay on it.

                Primary focus: {selected.name}
                Extra guidance: {overlay_name}

                How to reply:
                - Lead with the answer or result.
                - Use plain Chinese unless the user asks otherwise, and keep the wording natural.
                - Keep the default reply short; only use a list when the content is naturally list-shaped.
                - For closeouts, say what was done, what effect was achieved, and what needs to happen next or that the work is finished.
                - Do not default to file inventories, evidence dumps, or step-by-step process retellings unless the user asks for them.

                Task cues:
                {reason_block}
"""
            ).strip()
        return dedent(
            f"""
            Help with the user's request directly. The route below is already selected.

            Primary focus: {selected.name}
            Extra guidance: {overlay_name}

            How to reply:
            - Lead with the answer or result.
            - Use plain Chinese unless the user asks otherwise, and keep the wording natural.
            - Keep the default reply short; only use a list when the content is naturally list-shaped.
"""
        ).strip()

    def _render_skill(self, skill: SkillMetadata, heading: str, is_overlay: bool = False) -> str:
        """Render a single skill into an instruction block.

        Parameters:
            skill: The selected skill metadata.
            heading: The human-readable heading for the block.
            is_overlay: When True, skip when_to_use/do_not_use to save tokens.

        Returns:
            str: The rendered instruction block.
        """
        sections = [
            f"{heading}: {skill.name}",
            f"Purpose: {skill.description.strip()}" if skill.description else "Purpose: not provided.",
        ]
        # Overlay skills omit when_to_use/do_not_use to reduce token overhead
        if not is_overlay:
            if skill.when_to_use:
                sections.append(f"Use it when:\n{skill.when_to_use.strip()}")
            if skill.do_not_use:
                sections.append(f"Avoid it when:\n{skill.do_not_use.strip()}")

        key_content = self._extract_key_sections(skill.body)
        if key_content:
            sections.append(f"Key rules:\n{key_content}")
        elif skill.body.strip():
            sections.append(f"Key rules:\n{self._compact_body(skill.body)}")
        return "\n\n".join(sections).strip()

    @staticmethod
    def _compact_body(body: str) -> str:
        compact = body.strip()
        if not compact:
            return ""
        lines = [line.rstrip() for line in compact.splitlines() if line.strip()]
        compact = "\n".join(lines[:_COMPACT_BODY_MAX_LINES]).strip()
        if len(compact) > _COMPACT_BODY_MAX_CHARS:
            compact = compact[: _COMPACT_BODY_MAX_CHARS - 1].rstrip() + "…"
        elif len(lines) > _COMPACT_BODY_MAX_LINES or len(body.strip()) > len(compact):
            compact = compact.rstrip() + "\n…"
        return compact

    @staticmethod
    def _extract_key_sections(body: str) -> str:
        """Extract only the key instructional sections from a skill body.

        Parameters:
            body: The full markdown body of the skill.

        Returns:
            str: Concatenated key sections, or empty string if none found.
        """
        matches = list(_SECTION_RE.finditer(body))
        extracted: list[str] = []
        for idx, m in enumerate(matches):
            title = m.group("title").strip().lower()
            # Fuzzy substring match — any known substring qualifies
            if any(substr in title for substr in _KEY_SECTION_SUBSTRINGS):
                start = m.start()
                end = matches[idx + 1].start() if idx + 1 < len(matches) else len(body)
                extracted.append(body[start:end].strip())
        return "\n\n".join(extracted)
