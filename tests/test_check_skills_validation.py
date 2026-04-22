from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.check_skills import (
    _is_allowed_system_override,
    _read_skill_document,
    iter_skill_dirs,
    load_validation_state,
    validate_skill_document,
)


def _write_skill(skill_dir: Path, name: str) -> None:
    skill_dir.mkdir(parents=True, exist_ok=True)
    (skill_dir / "SKILL.md").write_text(
        f"""---
name: {name}
description: Fast skill
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - fast skill
---
## When to use
- test
""",
        encoding="utf-8",
    )


def test_validate_skill_document_uses_precomputed_link_targets(tmp_path: Path) -> None:
    skill_dir = tmp_path / "sample-skill"
    refs_dir = skill_dir / "references"
    refs_dir.mkdir(parents=True)
    (refs_dir / "ok.md").write_text("ok", encoding="utf-8")
    (skill_dir / "SKILL.md").write_text(
        """---
name: sample-skill
description: Fast skill
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - sample trigger
---
See [ok](references/ok.md).

```md
[broken](missing.md)
```
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("sample-skill", skill_dir)

    assert document is not None
    assert error_report is None
    assert document.description_text == "Fast skill"
    assert document.description_first_line == "Fast skill"
    assert document.body_text == "See [ok](references/ok.md).\n\n```md\n[broken](missing.md)\n```"
    assert document.body_has_headings is False
    assert document.link_targets == ("references/ok.md",)

    report = validate_skill_document(document)

    assert report.errors == []


def test_iter_skill_dirs_discovers_nested_bundles_and_skips_containers(tmp_path: Path) -> None:
    skills_root = tmp_path / "skills"
    _write_skill(skills_root / "top-skill", "top-skill")
    _write_skill(
        skills_root / "codex-primary-runtime" / "spreadsheets",
        "spreadsheets",
    )
    _write_skill(skills_root / ".system" / "openai-docs", "openai-docs")

    container = skills_root / "junk-container"
    (container / "nested").mkdir(parents=True)
    (container / "README.md").write_text("not a skill", encoding="utf-8")

    discovered = iter_skill_dirs(skills_root, include_system=False)
    discovered_paths = {path.relative_to(skills_root).as_posix() for _, path in discovered}

    assert discovered_paths == {
        "codex-primary-runtime/spreadsheets",
        "top-skill",
    }

    documents, reports, _ = load_validation_state(skills_root, include_system=False)

    assert sorted(report.slug for report in reports) == ["spreadsheets", "top-skill"]
    assert "openai-docs" in {document.slug for document in documents}
    assert all("missing SKILL.md" not in report.errors for report in reports)
    assert all(report.path != skills_root / "codex-primary-runtime" for report in reports)
    assert all(report.path != container for report in reports)

    discovered_with_system = iter_skill_dirs(skills_root, include_system=True)
    discovered_system_paths = {
        path.relative_to(skills_root).as_posix() for _, path in discovered_with_system
    }
    assert ".system/openai-docs" in discovered_system_paths


def test_validate_skill_document_flags_missing_runtime_prerequisites(tmp_path: Path) -> None:
    skill_dir = tmp_path / "runtime-skill"
    scripts_dir = skill_dir / "scripts"
    scripts_dir.mkdir(parents=True)
    (scripts_dir / "tool.py").write_text(
        "import pandas\n",
        encoding="utf-8",
    )
    (skill_dir / "SKILL.md").write_text(
        """---
name: runtime-skill
description: Runtime checked skill
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - runtime check
---
## When to use
- test
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("runtime-skill", skill_dir)

    assert document is not None
    assert error_report is None

    report = validate_skill_document(document)

    assert any("runtime_requirements.python is required" in error for error in report.errors)


def test_validate_skill_document_accepts_declared_runtime_prerequisites(tmp_path: Path) -> None:
    skill_dir = tmp_path / "runtime-skill"
    scripts_dir = skill_dir / "scripts"
    scripts_dir.mkdir(parents=True)
    (scripts_dir / "tool.py").write_text(
        "import pandas\n",
        encoding="utf-8",
    )
    (skill_dir / "SKILL.md").write_text(
        """---
name: runtime-skill
description: Runtime checked skill
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - runtime check
runtime_requirements:
  python:
    - pandas
    - openpyxl
---
## When to use
- test
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("runtime-skill", skill_dir)

    assert document is not None
    assert error_report is None

    report = validate_skill_document(document)

    assert report.errors == []


def test_system_skill_override_is_not_treated_as_duplicate(tmp_path: Path) -> None:
    skills_root = tmp_path / "skills"
    _write_skill(skills_root / ".system" / "imagegen", "imagegen")
    _write_skill(skills_root / "imagegen", "imagegen")

    paths = [
        skills_root / ".system" / "imagegen",
        skills_root / "imagegen",
    ]

    assert _is_allowed_system_override(skills_root, paths) is True


def test_validate_skill_document_rejects_invalid_routing_gate_value(tmp_path: Path) -> None:
    skill_dir = tmp_path / "bad-gate"
    skill_dir.mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(
        """---
name: bad-gate
description: Bad gate
routing_layer: L1
routing_owner: owner
routing_gate: fuzzy, freeform
session_start: n/a
trigger_hints:
  - bad gate
---
## When to use
- test
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("bad-gate", skill_dir)
    assert document is not None
    assert error_report is None

    report = validate_skill_document(document)

    assert any("routing_gate must be one of" in error for error in report.errors)


def test_validate_skill_document_rejects_invalid_session_start_value(tmp_path: Path) -> None:
    skill_dir = tmp_path / "bad-session"
    skill_dir.mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(
        """---
name: bad-session
description: Bad session
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: always
trigger_hints:
  - bad session
---
## When to use
- test
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("bad-session", skill_dir)
    assert document is not None
    assert error_report is None

    report = validate_skill_document(document)

    assert any("session_start must be one of" in error for error in report.errors)


def test_validate_skill_document_requires_explicit_trigger_hints(tmp_path: Path) -> None:
    skill_dir = tmp_path / "missing-triggers"
    skill_dir.mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(
        """---
name: missing-triggers
description: Missing triggers
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
---
## When to use
- test
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("missing-triggers", skill_dir)
    assert document is not None
    assert error_report is None

    report = validate_skill_document(document)

    assert any(
        "trigger_hints must be declared explicitly in frontmatter" in error
        for error in report.errors
    )


def test_validate_skill_document_rejects_empty_trigger_hints(tmp_path: Path) -> None:
    skill_dir = tmp_path / "empty-triggers"
    skill_dir.mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(
        """---
name: empty-triggers
description: Empty triggers
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints: []
---
## When to use
- test
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("empty-triggers", skill_dir)
    assert document is not None
    assert error_report is None

    report = validate_skill_document(document)

    assert any(
        "trigger_hints must be a non-empty list of strings" in error
        for error in report.errors
    )


def test_validate_skill_document_rejects_legacy_trigger_phrases(tmp_path: Path) -> None:
    skill_dir = tmp_path / "legacy-trigger-phrases"
    skill_dir.mkdir(parents=True)
    (skill_dir / "SKILL.md").write_text(
        """---
name: legacy-trigger-phrases
description: Legacy trigger field
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - canonical trigger
trigger_phrases:
  - legacy trigger
---
## When to use
- test
""",
        encoding="utf-8",
    )

    document, error_report = _read_skill_document("legacy-trigger-phrases", skill_dir)
    assert document is not None
    assert error_report is None

    report = validate_skill_document(document)

    assert any(
        "trigger_phrases is no longer supported; use trigger_hints" in error
        for error in report.errors
    )
