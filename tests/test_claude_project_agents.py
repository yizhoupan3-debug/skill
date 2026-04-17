from __future__ import annotations

import re
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CLAUDE_AGENTS_DIR = PROJECT_ROOT / ".claude" / "agents"
FRONTMATTER_PATTERN = re.compile(r"\A---\n(.*?)\n---\n", re.DOTALL)


def test_project_claude_agents_define_required_frontmatter() -> None:
    agent_files = sorted(
        path for path in CLAUDE_AGENTS_DIR.glob("*.md") if path.name != "README.md"
    )
    assert agent_files, "Expected project Claude agents to exist."

    for agent_file in agent_files:
        text = agent_file.read_text(encoding="utf-8")
        match = FRONTMATTER_PATTERN.match(text)
        assert match, f"{agent_file.name} is missing YAML frontmatter."

        frontmatter = match.group(1)
        assert re.search(r"(?m)^name:\s*\S+", frontmatter), (
            f"{agent_file.name} is missing required Claude subagent name."
        )
        assert re.search(r"(?m)^description:\s*\S+", frontmatter), (
            f"{agent_file.name} is missing required Claude subagent description."
        )
