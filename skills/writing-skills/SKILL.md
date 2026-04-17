---
name: writing-skills
description: |
  Standardize and strengthen multiple `SKILL.md` files and shared skill-writing docs across a library.
  Use when the task is batch rewriting, template unification, or repository-wide wording cleanup for many skills.
  For one-skill writing guidance use `$skill-writer`; for Codex routing policy use `$skill-developer-codex`.
metadata:
  version: "2.1.0"
  platforms: [codex]
  category: meta
  tags:
    - skill-writing
    - skill-template
    - standardization
    - skill-docs
risk: low
source: local
routing_layer: L0
routing_owner: overlay
routing_gate: none
session_start: n/a
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
---
- **Dual-Dimension Audit (Pre: Batch-Logic/Template, Post: Consistency/Sync Results)** → `$execution-audit-codex` [Overlay]

# Writing Skills

This skill owns **multi-skill writing consistency**: shared structure,
reusable wording patterns, and batch documentation cleanup.

## When to use

- Many skills need the same template or structure upgrade
- A repository wants shared wording conventions across many `SKILL.md` files
- The task is batch rewrite / standardization, not one-skill boundary design

## Do not use

- Single-skill wording, token budget, or boundary design → use `$skill-writer`
- Codex routing diagnosis or framework behavior → use `$skill-developer-codex`
- Antigravity-specific trigger debugging → use `$skill-developer`
- Installing/importing skills → use `$skill-installer` / `$skill-installer-antigravity`

## Core workflow

1. Confirm this is truly multi-skill work.
2. Start from the shared template in `resources/unified-skill-template.md`.
3. Standardize in this order:
   - description
   - boundaries
   - workflow
   - output contract
   - optional detail packaging
4. Keep repeated standards in shared references instead of copying them into every skill.
5. Standardize the output for any "Polishing" (精修) task by requiring a **Revision Record (修改意见)** table that explains style changes and de-AIGC tactics.
6. Validate the library after changes.

## Hard constraints

- Do not use this skill as a substitute for one-skill routing decisions.
- Do not batch-copy vague wording.
- Do not duplicate long standards across every skill when one shared resource is enough.
- **Superior Quality Audit**: For batch documentation updates, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Reference

- `resources/unified-skill-template.md`

## Trigger examples
- "强制进行 Skill 写作深度审计 / 检查批量对齐与模板一致性。"
- "Use $execution-audit-codex to audit these skill files for consistency idealism."
