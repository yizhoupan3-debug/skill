#!/usr/bin/env python3
"""Validate skill library structure and SKILL.md metadata."""

from __future__ import annotations

import argparse
import ast
import copy
import functools
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:  # pragma: no cover - optional dependency fallback
    yaml = None


LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
SLUG_RE = re.compile(r"[^a-z0-9]+")
FENCED_CODE_BLOCK_RE = re.compile(r"^```.*?^```[ \t]*$", re.MULTILINE | re.DOTALL)
AGENT_SKILLS_NAME_RE = re.compile(r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$")
TOKEN_WARN_THRESHOLD = 4000
DESCRIPTION_WARN_THRESHOLD = 450
REQUIRED_FRONTMATTER_FIELDS = (
    "name",
    "description",
    "routing_layer",
    "routing_owner",
    "routing_gate",
    "session_start",
)
RUNTIME_REQUIREMENT_FIELDS = ("python", "commands", "env", "files")
VALID_ROUTING_GATES = {"none", "source", "artifact", "evidence", "delegation"}
VALID_SESSION_START_VALUES = {"required", "preferred", "n/a"}
CONTROL_ROUTING_LAYERS = {"L-1", "L0"}
CANONICAL_TRIGGER_HINT_FIELD = "trigger_hints"
LEGACY_TRIGGER_HINT_FIELDS = ("trigger_phrases",)
PYTHON_IMPORT_PACKAGE_ALIASES = {
    "PIL": "pillow",
    "pptx": "python-pptx",
    "yaml": "pyyaml",
}
OPTIONAL_SOURCE_VALUES = {
    "system",
    "vendor",
    "user",
    "project",
    "local",
    "community",
    "community-adapted",
    "local - trainer",
}
SESSION_START_HINTS = ("每轮对话开始", "first-turn", "conversation start")
REMOTE_PREFIXES = (
    "http://",
    "https://",
    "mailto:",
    "file://",
    "app://",
    "plugin://",
    "data:",
)


def estimate_tokens(text: str) -> int:
    """Estimate token count as chars / 4 (rough GPT-style approximation)."""
    return len(text) // 4


def validate_agentskills_name(name: str) -> list[str]:
    """Validate name against the AgentSkills open standard spec.

    Rules: 1-64 chars, lowercase alphanumeric + hyphens, no leading/trailing
    hyphens, no consecutive hyphens.
    """
    errors: list[str] = []
    if not name:
        return errors  # already caught by required field check
    if len(name) > 64:
        errors.append(f"name '{name}' exceeds 64-char limit ({len(name)} chars)")
    if name.startswith("-") or name.endswith("-"):
        errors.append(f"name '{name}' must not start or end with a hyphen")
    if "--" in name:
        errors.append(f"name '{name}' must not contain consecutive hyphens")
    if not AGENT_SKILLS_NAME_RE.match(name):
        errors.append(
            f"name '{name}' does not match AgentSkills spec "
            f"(must be lowercase alphanumeric + single hyphens)"
        )
    return errors


@dataclass
class SkillReport:
    slug: str
    path: Path
    line_count: int = 0
    byte_count: int = 0
    description_chars: int = 0
    description_tokens: int = 0
    body_tokens: int = 0
    has_references: bool = False
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


@dataclass(slots=True)
class SkillDocument:
    slug: str
    skill_dir: Path
    skill_file: Path
    text: str
    metadata: dict[str, Any]
    body: str
    description_text: str
    description_first_line: str
    body_text: str
    body_has_headings: bool
    link_targets: tuple[str, ...]
    line_count: int
    byte_count: int
    has_references: bool


def validate_codex_link(skills_root: Path, codex_link: Path) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []

    if not codex_link.exists() and not codex_link.is_symlink():
        errors.append(f"codex skills path missing: {codex_link}")
        return errors, warnings

    if not codex_link.is_symlink():
        warnings.append(f"codex skills path is not a symlink: {codex_link}")

    try:
        codex_real = codex_link.resolve(strict=True)
    except FileNotFoundError:
        errors.append(f"codex skills symlink target does not exist: {codex_link}")
        return errors, warnings

    expected_real = skills_root.resolve()
    if codex_real != expected_real:
        errors.append(
            f"codex skills realpath mismatch: expected {expected_real}, got {codex_real}"
        )
    return errors, warnings


def _iter_discovered_skill_dirs(root: Path) -> list[tuple[str, Path]]:
    """Recursively collect directories that directly own a SKILL.md file."""

    skill_dirs: list[tuple[str, Path]] = []
    skill_file = root / "SKILL.md"
    if skill_file.is_file():
        skill_dirs.append((root.name, root))
        return skill_dirs

    for entry in sorted(root.iterdir(), key=lambda p: p.name):
        if not entry.is_dir():
            continue
        if entry.name == "dist" or entry.name.startswith("."):
            continue
        skill_dirs.extend(_iter_discovered_skill_dirs(entry))
    return skill_dirs


def iter_skill_dirs(skills_root: Path, include_system: bool) -> list[tuple[str, Path]]:
    skill_dirs: list[tuple[str, Path]] = []
    for entry in sorted(skills_root.iterdir(), key=lambda p: p.name):
        if not entry.is_dir():
            continue
        if entry.name == "dist":
            continue
        if entry.name == ".system":
            if not include_system:
                continue
            skill_dirs.extend(_iter_discovered_skill_dirs(entry))
            continue
        if entry.name.startswith("."):
            continue
        skill_dirs.extend(_iter_discovered_skill_dirs(entry))
    return skill_dirs


def _iter_skill_python_files(skill_dir: Path) -> list[Path]:
    python_files: list[Path] = []
    for path in sorted(skill_dir.rglob("*.py")):
        rel_parts = path.relative_to(skill_dir).parts
        if any(part.startswith(".") for part in rel_parts):
            continue
        python_files.append(path)
    return python_files


def _discover_local_python_modules(skill_dir: Path) -> set[str]:
    local_modules: set[str] = set()
    for path in _iter_skill_python_files(skill_dir):
        local_modules.add(path.stem)
    for path in skill_dir.rglob("__init__.py"):
        rel = path.parent.relative_to(skill_dir)
        if rel.parts:
            local_modules.add(rel.parts[0])
    return local_modules


def _normalize_python_dependency_name(module_name: str) -> str:
    return PYTHON_IMPORT_PACKAGE_ALIASES.get(module_name, module_name)


def normalize_string_list(value: Any) -> list[str]:
    """Normalize frontmatter fields that should behave like string lists."""

    if value is None:
        return []
    if isinstance(value, str):
        stripped = value.strip()
        return [stripped] if stripped else []
    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    return [str(value).strip()]


def collect_trigger_hints(metadata: dict[str, Any]) -> list[str]:
    """Collect canonical trigger hints with one legacy compatibility window."""

    hints: list[str] = []
    seen: set[str] = set()
    for field_name in (CANONICAL_TRIGGER_HINT_FIELD, *LEGACY_TRIGGER_HINT_FIELDS):
        for hint in normalize_string_list(metadata.get(field_name)):
            lowered = hint.lower()
            if lowered in seen:
                continue
            seen.add(lowered)
            hints.append(hint)
    return hints


def requires_artifact_outputs(metadata: dict[str, Any]) -> bool:
    """Return whether one skill must declare artifact outputs."""

    routing_layer = str(metadata.get("routing_layer", "")).strip()
    routing_owner = str(metadata.get("routing_owner", "")).strip()
    routing_layer = str(metadata.get("routing_layer", "")).strip()
    routing_gate = str(metadata.get("routing_gate", "")).strip()
    filesystem_scope = normalize_string_list(metadata.get("filesystem_scope"))
    scope_joined = " ".join(filesystem_scope).lower()
    return (
        routing_gate == "artifact"
        or routing_layer in CONTROL_ROUTING_LAYERS
        or routing_owner == "gate"
        or "artifacts" in scope_joined
        or ".supervisor" in scope_joined
    )


def discover_python_runtime_dependencies(skill_dir: Path) -> set[str]:
    dependencies: set[str] = set()
    stdlib_modules = set(getattr(sys, "stdlib_module_names", ()))
    local_modules = _discover_local_python_modules(skill_dir)

    for path in _iter_skill_python_files(skill_dir):
        try:
            tree = ast.parse(path.read_text(encoding="utf-8"))
        except SyntaxError:
            continue

        for node in ast.walk(tree):
            module_names: list[str] = []
            if isinstance(node, ast.Import):
                module_names = [alias.name.split(".", 1)[0] for alias in node.names]
            elif isinstance(node, ast.ImportFrom) and node.module:
                module_names = [node.module.split(".", 1)[0]]

            for module_name in module_names:
                if module_name == "__future__":
                    continue
                if module_name in stdlib_modules or module_name in local_modules:
                    continue
                dependencies.add(_normalize_python_dependency_name(module_name))

    return dependencies


def slugify(value: str) -> str:
    return SLUG_RE.sub("-", value.lower()).strip("-")


def _leading_spaces(line: str) -> int:
    """Count leading spaces after expanding tabs for indentation parsing."""
    expanded = line.expandtabs(2)
    return len(expanded) - len(expanded.lstrip(" "))


def _parse_scalar(value: str) -> Any:
    """Parse a minimal YAML scalar for fallback frontmatter handling."""
    value = value.strip()
    if not value:
        return ""
    if value[0] in {'"', "'"} and value[-1:] == value[0]:
        return value[1:-1]
    if value.lower() == "true":
        return True
    if value.lower() == "false":
        return False
    if re.fullmatch(r"-?\d+", value):
        return int(value)
    if re.fullmatch(r"-?\d+\.\d+", value):
        return float(value)
    if value.startswith("[") and value.endswith("]"):
        inner = value[1:-1].strip()
        if not inner:
            return []
        return [_parse_scalar(part) for part in inner.split(",")]
    return value


def _collect_child_block(lines: list[str], start: int, parent_indent: int) -> tuple[list[str], int]:
    """Collect indented child lines for the fallback parser."""
    block: list[str] = []
    index = start

    while index < len(lines):
        line = lines[index]
        if not line.strip():
            block.append(line)
            index += 1
            continue

        indent = _leading_spaces(line)
        if indent <= parent_indent:
            break
        block.append(line)
        index += 1

    return block, index


def _parse_yaml_subset(lines: list[str], base_indent: int = 0) -> dict[str, Any]:
    """Parse a minimal YAML subset when PyYAML is unavailable."""
    result: dict[str, Any] = {}
    index = 0

    while index < len(lines):
        raw_line = lines[index]
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#"):
            index += 1
            continue

        indent = _leading_spaces(raw_line)
        if indent < base_indent:
            break
        if indent > base_indent:
            index += 1
            continue

        line = raw_line.expandtabs(2).strip()
        if ":" not in line:
            index += 1
            continue

        key, raw_value = line.split(":", 1)
        key = key.strip()
        value = raw_value.strip()

        if value in {"|", ">"}:
            block, next_index = _collect_child_block(lines, index + 1, indent)
            normalized = []
            for entry in block:
                if not entry.strip():
                    normalized.append("")
                    continue
                expanded = entry.expandtabs(2)
                trim_at = min(len(expanded), indent + 2)
                normalized.append(expanded[trim_at:])
            text = "\n".join(normalized).strip()
            result[key] = (
                text
                if value == "|"
                else " ".join(part.strip() for part in text.splitlines() if part.strip())
            )
            index = next_index
            continue

        if value == "":
            block, next_index = _collect_child_block(lines, index + 1, indent)
            if block:
                non_empty = [entry for entry in block if entry.strip()]
                first = non_empty[0].expandtabs(2).lstrip() if non_empty else ""
                if first.startswith("- "):
                    items: list[Any] = []
                    for entry in non_empty:
                        item_text = entry.expandtabs(2).lstrip()
                        if item_text.startswith("- "):
                            items.append(_parse_scalar(item_text[2:].strip()))
                    result[key] = items
                else:
                    result[key] = _parse_yaml_subset(block, base_indent=indent + 2)
            else:
                result[key] = ""
            index = next_index
            continue

        result[key] = _parse_scalar(value)
        index += 1

    return result


@functools.lru_cache(maxsize=1024)
def _parse_frontmatter_cached(text: str) -> tuple[dict[str, Any], str, str | None]:
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}, text, "missing YAML frontmatter start delimiter"

    end_idx = None
    for idx in range(1, len(lines)):
        if lines[idx].strip() == "---":
            end_idx = idx
            break
    if end_idx is None:
        return {}, text, "missing YAML frontmatter end delimiter"

    frontmatter_lines = lines[1:end_idx]
    body = "\n".join(lines[end_idx + 1 :])
    frontmatter_text = "\n".join(frontmatter_lines)

    if yaml is not None:
        try:
            parsed = yaml.safe_load(frontmatter_text) or {}
        except yaml.YAMLError as exc:
            return {}, body, f"invalid YAML frontmatter: {exc}"
        if not isinstance(parsed, dict):
            return {}, body, "frontmatter must parse to a mapping"
        return parsed, body, None

    try:
        parsed = _parse_yaml_subset(frontmatter_lines)
    except Exception as exc:  # pragma: no cover - defensive fallback path
        return {}, body, f"frontmatter fallback parser failed: {exc}"
    return parsed, body, None


def parse_frontmatter(text: str) -> tuple[dict[str, Any], str, str | None]:
    """Parse YAML frontmatter with memoization for repeated full-library scans."""

    metadata, body, error = _parse_frontmatter_cached(text)
    if metadata:
        return copy.deepcopy(metadata), body, error
    return {}, body, error


def _read_skill_document(slug: str, skill_dir: Path) -> tuple[SkillDocument | None, SkillReport | None]:
    """Read one skill document once and return either a parsed doc or an error report."""

    report = SkillReport(slug=slug, path=skill_dir)
    skill_file = skill_dir / "SKILL.md"
    if not skill_file.is_file():
        report.errors.append("missing SKILL.md")
        return None, report

    text = skill_file.read_text(encoding="utf-8")
    report.line_count = len(text.splitlines())
    report.byte_count = len(text.encode("utf-8"))
    report.has_references = (skill_dir / "references").is_dir()

    metadata, body, error = _parse_frontmatter_cached(text)
    if error:
        report.errors.append(error)
        return None, report

    description = metadata.get("description", "")
    if not isinstance(description, str):
        description = ""
    description_text = description.strip()
    description_first_line = description_text.splitlines()[0].strip() if description_text else ""
    body_text = body.strip()
    text_without_code = FENCED_CODE_BLOCK_RE.sub("", text)
    link_targets = tuple(match.group(1) for match in LINK_RE.finditer(text_without_code))

    return SkillDocument(
        slug=slug,
        skill_dir=skill_dir,
        skill_file=skill_file,
        text=text,
        metadata=metadata.copy() if metadata else {},
        body=body,
        description_text=description_text,
        description_first_line=description_first_line,
        body_text=body_text,
        body_has_headings="## " in body_text,
        link_targets=link_targets,
        line_count=report.line_count,
        byte_count=report.byte_count,
        has_references=report.has_references,
    ), None


def load_skill_documents(skills_root: Path, include_system: bool) -> list[SkillDocument]:
    """Read and parse all SKILL.md documents once for multi-consumer pipelines."""

    documents: list[SkillDocument] = []
    for slug, skill_dir in iter_skill_dirs(skills_root, include_system=include_system):
        document, report = _read_skill_document(slug, skill_dir)
        if document is None:
            if report and report.errors == ["missing SKILL.md"]:
                continue
            skill_file = skill_dir / "SKILL.md"
            message = report.errors[0] if report and report.errors else "failed to read skill"
            raise ValueError(f"{skill_file}: {message}")
        documents.append(document)
    return documents


def validate_skill_document(document: SkillDocument) -> SkillReport:
    """Validate a skill using an already-loaded document."""

    slug = document.slug
    skill_dir = document.skill_dir
    metadata = document.metadata

    report = SkillReport(slug=slug, path=skill_dir)
    report.line_count = document.line_count
    report.byte_count = document.byte_count
    report.has_references = document.has_references

    for required in REQUIRED_FRONTMATTER_FIELDS:
        if not metadata.get(required, "").strip():
            report.errors.append(f"missing required frontmatter field: {required}")

    optional_source = str(metadata.get("source", "")).strip()
    if optional_source and optional_source not in OPTIONAL_SOURCE_VALUES:
        report.errors.append(
            f"invalid source '{optional_source}' (expected one of {sorted(OPTIONAL_SOURCE_VALUES)})"
        )

    source_priority = metadata.get("source_priority")
    if source_priority is not None:
        if not isinstance(source_priority, (int, float)):
            report.errors.append("source_priority must be numeric when provided")
        report.warnings.append(
            "source_priority is deprecated; source precedence is governed by skills/SKILL_SOURCE_MANIFEST.json"
        )

    routing_layer = str(metadata.get("routing_layer", "")).strip()
    routing_gate = str(metadata.get("routing_gate", "")).strip()
    if routing_gate and routing_gate not in VALID_ROUTING_GATES:
        report.errors.append(
            f"routing_gate must be one of {sorted(VALID_ROUTING_GATES)}, got '{routing_gate}'"
        )

    session_start = str(metadata.get("session_start", "")).strip()
    if session_start and session_start not in VALID_SESSION_START_VALUES:
        report.errors.append(
            f"session_start must be one of {sorted(VALID_SESSION_START_VALUES)}, got '{session_start}'"
        )

    loadouts = metadata.get("loadouts")
    if loadouts is not None and (
        not isinstance(loadouts, list) or not all(isinstance(item, str) for item in loadouts)
    ):
        report.errors.append("loadouts must be a list of strings when provided")

    trigger_hints = metadata.get(CANONICAL_TRIGGER_HINT_FIELD)
    if trigger_hints is not None and (
        not isinstance(trigger_hints, list) or not all(isinstance(item, str) for item in trigger_hints)
    ):
        report.errors.append(f"{CANONICAL_TRIGGER_HINT_FIELD} must be a list of strings when provided")

    for legacy_field in LEGACY_TRIGGER_HINT_FIELDS:
        legacy_value = metadata.get(legacy_field)
        if legacy_value is not None and (
            not isinstance(legacy_value, list) or not all(isinstance(item, str) for item in legacy_value)
        ):
            report.errors.append(f"{legacy_field} must be a list of strings when provided")
        if legacy_value is not None:
            report.warnings.append(
                f"{legacy_field} is deprecated; migrate to {CANONICAL_TRIGGER_HINT_FIELD}"
            )

    for list_field in ("allowed_tools", "approval_required_tools", "artifact_outputs"):
        value = metadata.get(list_field)
        if value is not None and (
            not isinstance(value, list) or not all(isinstance(item, str) for item in value)
        ):
            report.errors.append(f"{list_field} must be a list of strings when provided")

    runtime_requirements = metadata.get("runtime_requirements")
    declared_python_requirements: set[str] = set()
    if runtime_requirements is not None:
        if not isinstance(runtime_requirements, dict):
            report.errors.append("runtime_requirements must be a mapping when provided")
        else:
            for field_name, field_value in runtime_requirements.items():
                if field_name not in RUNTIME_REQUIREMENT_FIELDS:
                    report.warnings.append(
                        f"runtime_requirements field '{field_name}' is not recognized"
                    )
                    continue
                if field_value is not None and (
                    not isinstance(field_value, list)
                    or not all(isinstance(item, str) for item in field_value)
                ):
                    report.errors.append(
                        f"runtime_requirements.{field_name} must be a list of strings when provided"
                    )
            python_requirements = runtime_requirements.get("python", [])
            if isinstance(python_requirements, list):
                declared_python_requirements = {
                    _normalize_python_dependency_name(item) for item in python_requirements
                }

    filesystem_scope = metadata.get("filesystem_scope")
    if filesystem_scope is not None and not isinstance(filesystem_scope, (str, list)):
        report.errors.append("filesystem_scope must be a string or list when provided")

    network_access = metadata.get("network_access")
    if network_access is not None and not isinstance(network_access, (str, bool)):
        report.errors.append("network_access must be a string or boolean when provided")

    bridge_behavior = metadata.get("bridge_behavior")
    if bridge_behavior is not None and not isinstance(bridge_behavior, (str, dict)):
        report.errors.append("bridge_behavior must be a string or mapping when provided")

    allowed_tools = normalize_string_list(metadata.get("allowed_tools"))
    approval_required_tools = normalize_string_list(metadata.get("approval_required_tools"))
    artifact_outputs = normalize_string_list(metadata.get("artifact_outputs"))
    filesystem_scope_values = normalize_string_list(filesystem_scope)
    network_access_value = metadata.get("network_access")

    if session_start == "required":
        if not allowed_tools:
            report.errors.append("session_start=required skills must declare allowed_tools")
        if not filesystem_scope_values:
            report.errors.append("session_start=required skills must declare filesystem_scope")
        if network_access_value in (None, "", "unspecified"):
            report.errors.append("session_start=required skills must declare network_access")

    is_gate_or_control = (
        str(metadata.get("routing_owner", "")).strip() == "gate"
        or routing_layer in CONTROL_ROUTING_LAYERS
        or routing_gate in VALID_ROUTING_GATES - {"none"}
    )
    if is_gate_or_control and not approval_required_tools:
        report.errors.append(
            "control-layer or gate skills must declare non-empty approval_required_tools"
        )

    if requires_artifact_outputs(metadata) and not artifact_outputs:
        report.errors.append(
            "artifact-producing skills must declare non-empty artifact_outputs"
        )

    declared_name = metadata.get("name", "").strip()
    if declared_name:
        normalized = slugify(declared_name)
        if normalized != slug:
            report.errors.append(
                f"frontmatter name '{declared_name}' normalizes to '{normalized}', expected '{slug}'"
            )
        report.warnings.extend(validate_agentskills_name(declared_name))

    line_count = report.line_count
    if line_count > 500:
        report.warnings.append(f"SKILL.md has {line_count} lines (over 500-line guideline)")

    description = document.description_text
    report.description_chars = len(description)
    report.description_tokens = estimate_tokens(description)
    session_start = metadata.get("session_start", "").strip()
    if session_start in {"required", "preferred"}:
        combined_text = f"{description}\n{document.body_text}"
        if not any(hint in combined_text for hint in SESSION_START_HINTS):
            report.errors.append(
                'session_start skills MUST mention "每轮对话开始 / first-turn / conversation start"'
            )
    if description:
        first_line = document.description_first_line
        if len(first_line) > 120:
            report.warnings.append(
                f"description first line is {len(first_line)} chars (should be ≤120)"
            )
        if report.description_chars > DESCRIPTION_WARN_THRESHOLD:
            report.warnings.append(
                f"description is {report.description_chars} chars; this text is globally loaded, "
                "so move deep examples/details into the body or references/"
            )

    body_text = document.body_text
    if not body_text:
        report.errors.append("SKILL.md body is empty")
    elif not document.body_has_headings:
        report.warnings.append("SKILL.md has no section headings")

    report.body_tokens = estimate_tokens(body_text)
    if report.body_tokens > TOKEN_WARN_THRESHOLD:
        msg = (
            f"SKILL.md body is ~{report.body_tokens} tokens "
            f"(exceeds {TOKEN_WARN_THRESHOLD} recommended by AgentSkills spec)"
        )
        if not report.has_references:
            report.errors.append(f"{msg}; MUST split into references/ to stay lean")
        else:
            report.warnings.append(msg)

    report.errors.extend(validate_links(skill_dir, document.link_targets))

    discovered_python_requirements = discover_python_runtime_dependencies(skill_dir)
    if discovered_python_requirements:
        if runtime_requirements is None:
            report.errors.append(
                "runtime_requirements.python is required when skill-local Python files depend on "
                f"non-stdlib packages: {', '.join(sorted(discovered_python_requirements))}"
            )
        elif not declared_python_requirements:
            report.errors.append(
                "runtime_requirements.python must declare detected Python package dependencies: "
                f"{', '.join(sorted(discovered_python_requirements))}"
            )
        else:
            missing_requirements = discovered_python_requirements - declared_python_requirements
            if missing_requirements:
                report.errors.append(
                    "runtime_requirements.python is missing detected Python package dependencies: "
                    f"{', '.join(sorted(missing_requirements))}"
                )
    return report


def _is_system_skill_dir(skills_root: Path, skill_dir: Path) -> bool:
    """Return whether the skill lives under skills/.system/."""

    try:
        return skill_dir.resolve().relative_to(skills_root.resolve()).parts[0] == ".system"
    except (ValueError, IndexError):
        return False


def _is_allowed_system_override(skills_root: Path, paths: list[Path]) -> bool:
    """Allow one local skill to intentionally override one system skill of the same slug."""

    if len(paths) != 2:
        return False

    system_count = sum(1 for path in paths if _is_system_skill_dir(skills_root, path))
    local_count = len(paths) - system_count
    return system_count == 1 and local_count == 1


def load_validation_state(
    skills_root: Path,
    include_system: bool,
) -> tuple[list[SkillDocument], list[SkillReport], dict[str, list[Path]]]:
    """Load skills once and build validation reports for the requested view."""

    documents: list[SkillDocument] = []
    reports: list[SkillReport] = []
    slug_to_dirs: dict[str, list[Path]] = {}

    for slug, skill_dir in iter_skill_dirs(skills_root, include_system=True):
        document, error_report = _read_skill_document(slug, skill_dir)
        is_visible = include_system or not _is_system_skill_dir(skills_root, skill_dir)
        if is_visible:
            slug_to_dirs.setdefault(slug, []).append(skill_dir)
        if document is None:
            if is_visible and error_report is not None:
                reports.append(error_report)
            continue
        documents.append(document)
        if is_visible:
            reports.append(validate_skill_document(document))

    return documents, reports, slug_to_dirs


def clean_link_target(raw_target: str) -> str:
    target = raw_target.strip()
    if target.startswith("<") and target.endswith(">"):
        target = target[1:-1].strip()
    if " " in target and not target.startswith(("http://", "https://")):
        target = target.split(" ", 1)[0]
    target = target.split("#", 1)[0].split("?", 1)[0]
    return target


def validate_links(skill_dir: Path, link_targets: tuple[str, ...]) -> list[str]:
    errors: list[str] = []
    for raw_target in link_targets:
        target = clean_link_target(raw_target)
        if not target:
            continue
        if target.startswith(REMOTE_PREFIXES) or target.startswith("/"):
            continue
        candidate = (skill_dir / target).resolve()
        if not candidate.exists():
            errors.append(f"broken relative link: {raw_target}")
    return errors


def validate_skill(slug: str, skill_dir: Path) -> SkillReport:
    document, report = _read_skill_document(slug, skill_dir)
    if document is None:
        return report or SkillReport(slug=slug, path=skill_dir)
    return validate_skill_document(document)


def build_json_report(reports: list[SkillReport]) -> dict:
    """Build a JSON-serializable summary report."""
    total_lines = sum(r.line_count for r in reports)
    total_bytes = sum(r.byte_count for r in reports)
    total_description_chars = sum(r.description_chars for r in reports)
    total_description_tokens = sum(r.description_tokens for r in reports)
    total_tokens = sum(r.body_tokens for r in reports)
    error_count = sum(len(r.errors) for r in reports)
    warning_count = sum(len(r.warnings) for r in reports)
    over_token = [r for r in reports if r.body_tokens > TOKEN_WARN_THRESHOLD]
    with_refs = [r for r in reports if r.has_references]

    skills_data = []
    for r in reports:
        skills_data.append({
            "name": r.slug,
            "lines": r.line_count,
            "bytes": r.byte_count,
            "description_chars": r.description_chars,
            "description_tokens": r.description_tokens,
            "body_tokens": r.body_tokens,
            "has_references": r.has_references,
            "errors": r.errors,
            "warnings": r.warnings,
        })

    return {
        "summary": {
            "total_skills": len(reports),
            "total_lines": total_lines,
            "total_bytes": total_bytes,
            "total_description_chars": total_description_chars,
            "avg_description_chars": total_description_chars // max(len(reports), 1),
            "total_description_tokens": total_description_tokens,
            "avg_description_tokens": total_description_tokens // max(len(reports), 1),
            "total_body_tokens": total_tokens,
            "avg_body_tokens": total_tokens // max(len(reports), 1),
            "errors": error_count,
            "warnings": warning_count,
            "skills_over_token_limit": len(over_token),
            "skills_with_references": len(with_refs),
        },
        "skills": skills_data,
    }


def get_git_root() -> Path | None:
    """Try to find the git root using git rev-parse."""
    local_root = Path(__file__).resolve().parents[1]
    if (local_root / "skills").is_dir():
        return local_root

    try:
        import subprocess
        proc = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True
        )
        return Path(proc.stdout.strip())
    except (subprocess.CalledProcessError, FileNotFoundError):
        return local_root if (local_root / "skills").is_dir() else None


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate skills/ library structure.")
    
    # Try to resolve default skills-root via git
    git_root = get_git_root()
    default_root = (git_root / "skills") if git_root else (Path(__file__).resolve().parents[1] / "skills")

    parser.add_argument(
        "--skills-root",
        type=Path,
        default=default_root,
        help="Path to the skills root directory.",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Treat warnings as failures.",
    )
    parser.add_argument(
        "--include-system",
        action="store_true",
        help="Include skills/.system/* in validation.",
    )
    parser.add_argument(
        "--verify-codex-link",
        action="store_true",
        help="Verify ~/.codex/skills resolves to the same realpath as --skills-root.",
    )
    parser.add_argument(
        "--codex-link",
        type=Path,
        default=Path.home() / ".codex" / "skills",
        help="Path to the Codex global skills directory or symlink.",
    )
    parser.add_argument(
        "--verify-sync",
        action="store_true",
        help="Check if generated routing artifacts are in sync with skills/.",
    )
    args = parser.parse_args()

    skills_root = args.skills_root.resolve()
    if not skills_root.is_dir():
        print(f"skills root does not exist: {skills_root}", file=sys.stderr)
        return 2

    skill_documents, reports, slug_to_dirs = load_validation_state(
        skills_root,
        include_system=args.include_system,
    )

    for slug, paths in sorted(slug_to_dirs.items()):
        if len(paths) > 1 and not _is_allowed_system_override(skills_root, paths):
            path_list = ", ".join(str(path) for path in paths)
            for report in reports:
                if report.slug == slug:
                    report.warnings.append(f"duplicate skill directory slug '{slug}': {path_list}")

    errors = sum(len(report.errors) for report in reports)
    warnings = sum(len(report.warnings) for report in reports)

    if args.verify_codex_link:
        link_errors, link_warnings = validate_codex_link(skills_root, args.codex_link)
        errors += len(link_errors)
        warnings += len(link_warnings)
        if link_errors or link_warnings:
            print("[codex-link]")
            for error in link_errors:
                print(f"  ERROR: {error}")
            for warning in link_warnings:
                print(f"  WARN: {warning}")

    for report in reports:
        if not report.errors and not report.warnings:
            continue
        print(f"[{report.slug}] {report.path}")
        for error in report.errors:
            print(f"  ERROR: {error}")
        for warning in report.warnings:
            print(f"  WARN: {warning}")

    if args.verify_sync:
        print("[sync-check] Verifying generated routing artifacts and shared CLI entrypoints...")
        sys.path.insert(0, str(Path(__file__).parent))
        from sync_skills import write_generated_files

        write_kwargs: dict[str, Any] = {}
        if skills_root == default_root.resolve():
            write_kwargs["skill_documents"] = skill_documents
        updated_files = write_generated_files(apply=False, **write_kwargs)
        if updated_files:
            errors += 1
            print(f"  ERROR: Generated files are out of sync: {', '.join(updated_files)}")
            print("  Run 'python3 scripts/sync_skills.py --apply' to fix.")

    checked_count = len(reports)
    if errors == 0 and warnings == 0:
        print(f"OK: checked {checked_count} skills under {skills_root}")
        return 0

    print(f"Checked {checked_count} skills: {errors} error(s), {warnings} warning(s)")
    if errors or (warnings and args.strict):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
