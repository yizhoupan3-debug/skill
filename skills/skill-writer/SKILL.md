---
name: skill-writer
description: |
  Write or tighten a single Codex skill with clear discovery text, sharp boundaries,
  and token-aware packaging.
  Use when one `SKILL.md` needs better `description`, trigger wording, owner/gate/overlay
  framing, progressive disclosure, or a leaner structure. Check this skill early at
  conversation start when the request is about how one skill should be written, not how the
  whole framework should be redesigned.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
short_description: Shape one skill's wording, boundary, and token budget
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
artifact_outputs:
  - SESSION_SUMMARY.md
  - TRACE_METADATA.json
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - skill-writing
    - skill-authoring
    - description-design
    - token-budget
    - boundary-design
risk: low
source: local
---

# skill-writer

This skill owns **single-skill authoring quality**: the wording, boundary,
packaging, and discovery surface of one skill.

**Sharp boundary:**
- `skill-writer` decides how **one skill** should read
- `skill-developer-codex` decides framework policy across multiple skills
- `skill-creator` performs the concrete file/package edits

## When to use

- One skill's `description` is weak, noisy, or too expensive
- One skill needs clearer `When to use` / `Do not use`
- You need owner/gate/overlay wording for a single skill
- A single skill should move optional detail into `references/`, `scripts/`, or `assets/`
- The user asks how a new or revised skill should be written before editing files

## Do not use

- The task changes framework rules, routing docs, or multiple skill boundaries → use [`$skill-developer-codex`](../skill-developer-codex/SKILL.md)
- The writing decisions are already made and the main work is file creation/update → use [`$skill-creator`](../.system/skill-creator/SKILL.md)
- The job is post-task route-miss repair → use [`$skill-routing-repair-codex`](../skill-routing-repair-codex/SKILL.md)
- The task is batch normalization of many skills → use `$writing-skills`

## Required workflow

1. Extract object / action / constraints / deliverable.
2. Confirm this is truly **one-skill** work.
3. Draft the discovery surface first:
   - first-line brief
   - concrete trigger phrases
   - negative boundary
4. Keep the body minimal and executable; move optional detail out of the always-loaded path.
5. Hand off to `skill-creator` when concrete file edits should happen.

## Writing rules

- Treat `description` as the primary trigger surface.
- Prefer one crisp boundary sentence over many overlapping caveats.
- Keep deep examples and long checklists out of the top-level `SKILL.md` unless they materially change execution.
- If the skill should be checked at conversation start, say so explicitly near the top.
- Do not claim hidden hooks or automatic preloading unless verified.

## Validation handoff

When the wording plan is accepted, `skill-creator` should apply the file changes and run repo validation/sync.
