from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.check_skills import _read_skill_document, validate_skill_document


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
