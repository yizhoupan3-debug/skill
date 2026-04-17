"""Prompt construction for dynamic skill injection."""

from __future__ import annotations

import re
from textwrap import dedent
from typing import Optional

from codex_agno_runtime.schemas import RoutingResult, SkillMetadata

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

    def build_prompt(self, routing_result: RoutingResult) -> str:
        """Build the injected prompt for the selected route.

        Parameters:
            routing_result: The routing decision.

        Returns:
            str: The injected prompt text.
        """
        # Progressive loading: hydrate body if not yet loaded
        selected = routing_result.selected_skill
        if not selected.body_loaded and self._loader is not None:
            self._loader.load_body(selected)

        parts = [
            dedent(
                f"""
                You are the Codex Singleton running on Agno.
                Use exactly one primary owner skill and at most one overlay.
                The router already selected the active skill for this task.

                Active owner skill: {selected.name}
                Routing layer: {routing_result.layer}
                Session id: {routing_result.session_id}
                """
            ).strip(),
            self._render_skill(selected, heading="Owner skill", is_overlay=False),
        ]
        if routing_result.overlay_skill is not None:
            overlay = routing_result.overlay_skill
            if not overlay.body_loaded and self._loader is not None:
                self._loader.load_body(overlay)
            parts.append(self._render_skill(overlay, heading="Overlay skill", is_overlay=True))
        return "\n\n".join(parts).strip()

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
            f"Description: {skill.description.strip()}" if skill.description else "Description: not provided.",
        ]
        # Overlay skills omit when_to_use/do_not_use to reduce token overhead
        if not is_overlay:
            if skill.when_to_use:
                sections.append(f"When to use:\n{skill.when_to_use.strip()}")
            if skill.do_not_use:
                sections.append(f"Do not use:\n{skill.do_not_use.strip()}")

        # Token budget: L0/L-1 controllers get full body; domain skills get key sections only
        if skill.routing_layer in ("L0", "L-1"):
            sections.append(f"Skill body:\n{skill.body.strip()}")
        else:
            key_content = self._extract_key_sections(skill.body)
            if key_content:
                sections.append(f"Key instructions:\n{key_content}")
            else:
                # Fallback: use full body if no key sections found
                sections.append(f"Skill body:\n{skill.body.strip()}")
        return "\n\n".join(sections).strip()

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

