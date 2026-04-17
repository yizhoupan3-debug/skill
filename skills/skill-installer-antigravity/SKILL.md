---
name: skill-installer-antigravity
description: |
  Install Antigravity skills into the shared workspace skill library from local folders or GitHub repositories.
  Use when the task is importing an existing Antigravity skill, copying it into the shared `skills/` tree,
  avoiding duplicate live copies, and validating the shared Codex plus Antigravity setup.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "1.1.0"
  platforms: [antigravity]
  tags:
    - antigravity
    - skill
    - installer
    - sync
risk: medium
source: local
---

# skill-installer-antigravity

This skill owns **Antigravity skill intake**: import an existing skill
into the shared library without creating a second live source of truth.

## When to use

- The user wants to import an existing Antigravity skill from GitHub or a local folder
- A skill should be copied into the shared `skills/` library
- The task is installation / intake, not writing new skill content

## Do not use

- Designing a new skill from scratch → use `$skill-developer` or `$skill-developer-codex`
- Rewriting or standardizing existing local skill docs → use `$writing-skills`
- Post-task routing repair → use `$skill-routing-repair-codex`

## Core workflow

1. Inspect the source and confirm `SKILL.md` exists.
2. Decide whether the incoming skill should keep its name or become a runtime-specific variant.
3. Copy it into the shared `skills/` tree.
4. Check folder slug, `name`, and relative links.
5. Validate the library.

## Rules

- Install into the workspace `skills/` tree, not a second live directory.
- Preserve bundled `references/`, `scripts/`, and `assets/` when needed.
- If the incoming skill is clearly Codex-specific, do not overwrite an Antigravity variant.

## Validation

```bash
cd /Users/joe/Documents/skill
python3 scripts/check_skills.py --verify-codex-link
python3 scripts/check_skills.py --include-system --verify-codex-link
```

## Completion criteria

- The imported skill lives under the shared workspace `skills/`
- No duplicate live copy was created
- Validation passes
- The user knows a fresh Antigravity conversation may be needed
