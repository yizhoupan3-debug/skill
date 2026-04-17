"""Skill discovery and loading for the Codex Agno runtime."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

import yaml

from codex_agno_runtime.schemas import SkillMetadata

SECTION_RE = re.compile(r"^##\s+(?P<title>.+?)\s*$", re.MULTILINE)
EXCLUDED_PARTS = {"node_modules", "target", "dist", "__pycache__"}
RUNTIME_INDEX_FILENAME = "SKILL_ROUTING_RUNTIME.json"
MANIFEST_FILENAME = "SKILL_MANIFEST.json"
TRIGGER_DELIMITER_RE = re.compile(r"\s*(?:\n+|/|\||;|；|•|·)\s*")


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


def _normalize_trigger_phrases(value: Any) -> list[str]:
    """Normalize manifest/runtime trigger strings into phrase candidates."""

    if isinstance(value, list):
        return _normalize_list(value)
    if not isinstance(value, str):
        return _normalize_list(value)
    raw = value.strip()
    if not raw:
        return []
    parts = [part.strip() for part in TRIGGER_DELIMITER_RE.split(raw) if part.strip()]
    if not parts:
        return [raw]
    phrases: list[str] = []
    seen: set[str] = set()
    for part in [raw, *parts]:
        if part in seen:
            continue
        seen.add(part)
        phrases.append(part)
    return phrases


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


def _load_keyed_index(path: Path) -> tuple[list[str], list[list[Any]]] | None:
    """Load a compact keyed skill index emitted by the repo compilers."""

    if not path.is_file():
        return None
    payload = json.loads(path.read_text(encoding="utf-8"))
    rows = payload.get("skills")
    keys = payload.get("keys")
    if not isinstance(rows, list) or not isinstance(keys, list):
        return None
    normalized_keys = [str(key) for key in keys]
    normalized_rows = [row for row in rows if isinstance(row, list)]
    return normalized_keys, normalized_rows


def _rows_to_records(keys: list[str], rows: list[list[Any]]) -> list[dict[str, Any]]:
    """Convert keyed compact rows into ordinary dictionaries."""

    index = {key: position for position, key in enumerate(keys)}
    records: list[dict[str, Any]] = []
    for row in rows:
        record: dict[str, Any] = {}
        for key, position in index.items():
            if position < len(row):
                record[key] = row[position]
        slug = str(record.get("slug", "")).strip()
        if slug:
            records.append(record)
    return records


def _safe_float(value: Any, default: float = 100.0) -> float:
    """Coerce floats from manifest/runtime indexes without failing the load."""

    try:
        return float(value)
    except (TypeError, ValueError):
        return default


class SkillLoader:
    """Load and hydrate runtime skill metadata."""

    def __init__(self, skills_root: Path) -> None:
        self.skills_root = skills_root
        self.runtime_index_path = skills_root / RUNTIME_INDEX_FILENAME
        self.manifest_path = skills_root / MANIFEST_FILENAME
        self._cache: list[SkillMetadata] | None = None
        self._raw_docs: dict[str, tuple[dict[str, Any], str]] = {}
        self._source_paths_by_slug: dict[str, str] | None = None
        self._source_paths_scanned_by_name = False

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

    def _build_source_path_index(self) -> dict[str, str]:
        """Index likely skill source paths by directory slug."""

        if self._source_paths_by_slug is None:
            self._source_paths_by_slug = {}
            for skill_file in self._iter_skill_files():
                self._source_paths_by_slug.setdefault(skill_file.parent.name, str(skill_file))
        return self._source_paths_by_slug

    def _resolve_source_path(self, slug: str) -> str | None:
        """Resolve one skill source path lazily by slug or frontmatter name."""

        if not slug:
            return None
        index = self._build_source_path_index()
        if slug in index:
            return index[slug]
        if not self._source_paths_scanned_by_name:
            self._source_paths_scanned_by_name = True
            for skill_file in self._iter_skill_files():
                text = skill_file.read_text(encoding="utf-8")
                metadata, body = _parse_frontmatter(text)
                path_str = str(skill_file)
                self._raw_docs[path_str] = (metadata, body)
                name = str(metadata.get("name", "")).strip()
                if name:
                    index.setdefault(name, path_str)
        return index.get(slug)

    def _load_from_compiled_indices(self) -> list[SkillMetadata] | None:
        """Load lean skill metadata from generated routing artifacts."""

        runtime_index = _load_keyed_index(self.runtime_index_path)
        manifest_index = _load_keyed_index(self.manifest_path)
        if runtime_index is None and manifest_index is None:
            return None

        runtime_records = (
            _rows_to_records(*runtime_index)
            if runtime_index is not None
            else []
        )
        manifest_records = (
            _rows_to_records(*manifest_index)
            if manifest_index is not None
            else []
        )
        manifest_by_slug = {
            str(record.get("slug", "")).strip(): record
            for record in manifest_records
            if str(record.get("slug", "")).strip()
        }
        ordered_slugs: list[str] = []
        seen: set[str] = set()
        for record in [*runtime_records, *manifest_records]:
            slug = str(record.get("slug", "")).strip()
            if slug and slug not in seen:
                seen.add(slug)
                ordered_slugs.append(slug)

        records: list[SkillMetadata] = []
        for slug in ordered_slugs:
            runtime_record = next((record for record in runtime_records if str(record.get("slug", "")).strip() == slug), {})
            manifest_record = manifest_by_slug.get(slug, {})
            description = str(runtime_record.get("summary", "")).strip() or str(manifest_record.get("description", "")).strip()
            short_description = str(runtime_record.get("summary", "")).strip() or str(manifest_record.get("description", "")).strip()
            trigger_source = runtime_record.get("triggers", manifest_record.get("triggers"))
            records.append(
                SkillMetadata(
                    name=slug,
                    description=description,
                    short_description=short_description,
                    when_to_use="",
                    do_not_use="",
                    routing_layer=str(runtime_record.get("layer", manifest_record.get("layer", "L3"))).strip() or "L3",
                    routing_owner=str(runtime_record.get("owner", manifest_record.get("owner", "owner"))).strip() or "owner",
                    routing_gate=str(runtime_record.get("gate", manifest_record.get("gate", "none"))).strip() or "none",
                    routing_priority=str(manifest_record.get("priority", runtime_record.get("priority", "P2"))).strip() or "P2",
                    session_start=str(runtime_record.get("session_start", manifest_record.get("session_start", "n/a"))).strip() or "n/a",
                    framework_roles=[],
                    tags=[],
                    trigger_phrases=_normalize_trigger_phrases(trigger_source),
                    metadata={
                        "compiled_index_source": "runtime" if runtime_record else "manifest",
                        "runtime_record": runtime_record,
                        "manifest_record": manifest_record,
                    },
                    health=_safe_float(runtime_record.get("health", manifest_record.get("health", 100.0))),
                    body="",
                    body_loaded=False,
                    source_path=self._resolve_source_path(slug),
                )
            )
        return records

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

        if not load_bodies:
            indexed_records = self._load_from_compiled_indices()
            if indexed_records is not None:
                self._cache = [skill.model_copy(deep=True) for skill in indexed_records]
                return indexed_records

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

        if skill.body_loaded:
            return
        if not skill.source_path:
            skill.source_path = self._resolve_source_path(skill.name)
        if not skill.source_path:
            return

        raw = self._raw_docs.get(skill.source_path)
        if raw is None:
            text = Path(skill.source_path).read_text(encoding="utf-8")
            raw = _parse_frontmatter(text)
            self._raw_docs[skill.source_path] = raw

        metadata, body = raw
        skill.body = body
        if not skill.when_to_use:
            skill.when_to_use = _extract_section(body, "When to use")
        if not skill.do_not_use:
            skill.do_not_use = _extract_section(body, "Do not use")
        if not skill.description:
            skill.description = str(metadata.get("description", "")).strip()
        if not skill.short_description:
            skill.short_description = str(metadata.get("short_description", "")).strip() or skill.description
        if not skill.trigger_phrases:
            skill.trigger_phrases = _normalize_list(metadata.get("trigger_phrases"))
        merged_metadata = dict(skill.metadata)
        merged_metadata.update(metadata)
        skill.metadata = merged_metadata
        skill.body_loaded = True
