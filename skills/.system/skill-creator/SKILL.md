---
name: skill-creator
description: Create or update a Codex skill package with clear routing metadata, scope, and supporting resources.
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 创建 skill
  - 更新 skill
  - skill package
  - SKILL.md
  - skill authoring
source: system
runtime_requirements:
  python:
    - pyyaml
metadata:
  short-description: Create or update a skill
---

# Skill Creator

Create or update one Codex skill package with a clean discovery surface and a lean supporting bundle.

## When to use

- The task is to create a new Codex skill package
- The task is to update one existing skill's package structure or bundled resources
- The user needs a concrete skill folder with `SKILL.md`, optional `agents/`, `scripts/`, `references/`, or `assets/`
- The main problem is package authoring, not framework policy

## Do not use

- Framework-level owner/gate/overlay design or overlap policy -> use `$skill-developer-codex`
- Tiny wording cleanup on one skill without package-structure work -> use `$skill-writer`
- Installing a skill from elsewhere into `$CODEX_HOME/skills` -> use `$skill-installer`

## Core workflow

1. Understand the skill's job, trigger surface, and concrete user tasks.
2. Choose the smallest package shape that supports the job.
3. Initialize or update the skill directory.
4. Keep `SKILL.md` lean; move detailed reference material into `references/`.
5. Add scripts only when deterministic execution or repeated reuse justifies them.
6. Validate the result and regenerate any stale UI metadata.

## Packaging rules

- `SKILL.md` is required and should stay concise.
- `name` and `description` are the key discovery surface.
- Use `agents/openai.yaml` for UI-facing metadata when needed.
- Use `references/` for details that should not live in the main skill body.
- Use `assets/` only for output resources, not documentation.
- Do not create extra documentation clutter such as `README.md`, `CHANGELOG.md`, or installation guides inside the skill unless the user explicitly asks.

## Validation

Run from repo root:

```bash
cd /Users/joe/Documents/skill
python3 scripts/check_skills.py --include-system --verify-codex-link
```

## References

- [references/skill-authoring.md](./references/skill-authoring.md) for skill anatomy, progressive disclosure, and authoring heuristics
- [references/openai_yaml.md](./references/openai_yaml.md) for `agents/openai.yaml` field semantics
- [scripts/init_skill.py](./scripts/init_skill.py) for skill initialization
- [scripts/generate_openai_yaml.py](./scripts/generate_openai_yaml.py) for UI metadata generation
