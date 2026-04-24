---
name: skill-developer
description: |
  Create, improve, debug, and audit Antigravity skills and `SKILL.md` files.
  Use when the task is Antigravity-specific skill authoring, trigger debugging, frontmatter cleanup,
  or skill-library restructuring for Antigravity behavior rather than Codex routing.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - antigravity
  - skill authoring
  - skill debugging
  - trigger wording
  - skill library
metadata:
  version: "2.1.0"
  platforms: [antigravity, codex]
  tags:
    - antigravity
    - skill-authoring
    - skill-debugging
    - trigger-wording
    - skill-library
risk: low
source: local
---

# skill-developer

This skill is the **Antigravity-specific owner** for skill authoring and
activation quality.

## When to use

- Writing a new Antigravity skill from scratch
- Improving an Antigravity skill's description, frontmatter, or trigger wording
- Diagnosing why an Antigravity skill does not activate
- Adapting a shared skill so Antigravity can discover it correctly
- Restructuring the Antigravity skill library

## Do not use

- Codex routing policy or Codex activation behavior → use `$skill-framework-developer`
- Multi-skill batch standardization → use `$writing-skills`
- Installing/importing Antigravity skills from elsewhere → use `$skill-installer-antigravity`
- Generic prompt engineering unrelated to skill files

## Antigravity assumptions

- Antigravity scans `SKILL.md` at conversation start
- `description` is the main activation surface
- Non-`SKILL.md` files are not auto-loaded by default
- Mid-conversation edits should be tested in a fresh session

## Core workflow

1. Confirm the task is truly Antigravity-specific.
2. Strengthen discovery surface first:
   - `name`
   - `description`
   - frontmatter
3. Tighten `When to use` / `Do not use`.
4. Keep the body concise and runtime-aware.
5. Validate after edits.

## Hard constraints

- Do not give Codex-specific advice as if it were Antigravity behavior.
- Do not bury trigger logic outside `SKILL.md`.
- Do not assume mid-conversation edits are immediately re-indexed.

## Validation

```bash
cd /Users/joe/Documents/skill
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json \
  --apply
python3 -m pytest tests/test_rust_release_entrypoints.py -q
```
