---
name: framework-native
description: Use the bundled framework MCP and runtime artifacts when the task is about this repository's skill framework, routing, memory, or Codex-native integration surface.
---

When the task is about `/Users/joe/Documents/skill`, prefer the bundled MCP surface before broad repo scraping.

Workflow:
1. Call `framework_runtime_snapshot` to see the active supervisor and artifact state.
2. Call `framework_skill_search` when the user asks about routing, owner boundaries, triggers, or framework owners.
3. Call `framework_memory_recall` or read `framework://memory/project` when cross-session memory matters.
4. Read `framework://memory/claude-recap` or call `framework_recap_refresh` when resuming work, needing a handoff recap, or wanting the Claude-style next-turn recap inside Codex.
5. Call `framework_bootstrap_refresh` before heavy framework planning so the bootstrap bundle stays current.

Do not use this skill for unrelated repositories or generic MCP work outside this workspace.
