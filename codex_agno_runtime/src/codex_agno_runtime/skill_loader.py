"""Skill discovery and loading for the Codex Agno runtime."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import yaml

from codex_agno_runtime.schemas import SkillMetadata

SECTION_RE = re.compile(r"^##\s+(?P<title>.+?)\s*$", re.MULTILINE)
EXCLUDED_PARTS = {"node_modules", "target", "dist", "__pycache__"}


def _normalize_list(value: Any) -> list[str]:
    """Normalize list-like frontmatter fields."""

    if value is None:
        return []
    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    if isinstance(value, str):
        raw = value.strip()
        if not raw:
            return []
        if "," in raw:
            return [part.strip() for part in raw.split(",") if part.strip()]
        return [raw]
    return [str(value).strip()]


def _parse_frontmatter(text: str) -> tuple[dict[str, Any], str]:
    """Parse YAML frontmatter from a skill file."""

    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}, text

    end_idx = None
    for idx in range(1, len(lines)):
        if lines[idx].strip() == "---":
            end_idx = idx
            break
    if end_idx is None:
        return {}, text

    frontmatter = yaml.safe_load("\n".join(lines[1:end_idx])) or {}
    if not isinstance(frontmatter, dict):
        frontmatter = {}
    body = "\n".join(lines[end_idx + 1 :])
    return frontmatter, body


def _extract_section(body: str, section_title: str) -> str:
    """Extract a `## Section` block from markdown body."""

    matches = list(SECTION_RE.finditer(body))
    target = section_title.casefold()
    for idx, match in enumerate(matches):
        title = match.group("title").strip().casefold()
        if title != target:
            continue
        start = match.end()
        end = matches[idx + 1].start() if idx + 1 < len(matches) else len(body)
        return body[start:end].strip()
    return ""


class SkillLoader:
    """Load and hydrate runtime skill metadata."""

    def __init__(self, skills_root: Path) -> None:
        self.skills_root = skills_root
        self._cache: list[SkillMetadata] | None = None
        self._raw_docs: dict[str, tuple[dict[str, Any], str]] = {}

    def _iter_skill_files(self) -> list[Path]:
        """Return all candidate `SKILL.md` files."""

        if not self.skills_root.is_dir():
            return []
        files: list[Path] = []
        for skill_file in self.skills_root.rglob("SKILL.md"):
            if any(part in EXCLUDED_PARTS for part in skill_file.parts):
                continue
            files.append(skill_file)
        return sorted(files)

    def load(self, refresh: bool = False, load_bodies: bool = True) -> list[SkillMetadata]:
        """Load all skill metadata from disk.

        Parameters:
            refresh: When True, rebuild the internal cache.
            load_bodies: When False, keep skill bodies lazily loaded.

        Returns:
            list[SkillMetadata]: Loaded skills.
        """

        if self._cache is not None and not refresh:
            if load_bodies:
                for skill in self._cache:
                    if not skill.body_loaded:
                        self.load_body(skill)
            return [skill.model_copy(deep=True) for skill in self._cache]

        records: list[SkillMetadata] = []
        self._raw_docs.clear()

        for skill_file in self._iter_skill_files():
            text = skill_file.read_text(encoding="utf-8")
            metadata, body = _parse_frontmatter(text)
            self._raw_docs[str(skill_file)] = (metadata, body)

            slug = str(metadata.get("name", "")).strip() or skill_file.parent.name
            description = str(metadata.get("description", "")).strip()
            skill = SkillMetadata(
                name=slug,
                description=description,
                short_description=str(metadata.get("short_description", "")).strip(),
                when_to_use=_extract_section(body, "When to use"),
                do_not_use=_extract_section(body, "Do not use"),
                routing_layer=str(metadata.get("routing_layer", "L3")).strip() or "L3",
                routing_owner=str(metadata.get("routing_owner", "owner")).strip() or "owner",
                routing_gate=str(metadata.get("routing_gate", "none")).strip() or "none",
                routing_priority=str(metadata.get("routing_priority", "P2")).strip() or "P2",
                session_start=str(metadata.get("session_start", "n/a")).strip() or "n/a",
                framework_roles=_normalize_list(metadata.get("framework_roles")),
                tags=_normalize_list(metadata.get("tags")),
                trigger_phrases=_normalize_list(metadata.get("trigger_phrases")),
                metadata=metadata,
                health=float(metadata.get("health", 100.0) or 100.0),
                body=body if load_bodies else "",
                body_loaded=load_bodies,
                source_path=str(skill_file),
            )
            records.append(skill)

        self._cache = [skill.model_copy(deep=True) for skill in records]
        return records

    def load_body(self, skill: SkillMetadata) -> None:
        """Hydrate one skill body on demand.

        Parameters:
            skill: Skill record to hydrate.

        Returns:
            None.
        """

        if skill.body_loaded or not skill.source_path:
            return

        raw = self._raw_docs.get(skill.source_path)
        if raw is None:
            text = Path(skill.source_path).read_text(encoding="utf-8")
            raw = _parse_frontmatter(text)
            self._raw_docs[skill.source_path] = raw

        _, body = raw
        skill.body = body
        if not skill.when_to_use:
            skill.when_to_use = _extract_section(body, "When to use")
        if not skill.do_not_use:
            skill.do_not_use = _extract_section(body, "Do not use")
        skill.body_loaded = True
