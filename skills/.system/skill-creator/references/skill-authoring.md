# skill authoring

Use these heuristics when creating or updating a Codex skill package.

## Skill anatomy

Typical shape:

```text
skill-name/
├── SKILL.md
├── agents/
│   └── openai.yaml
├── scripts/
├── references/
└── assets/
```

- `SKILL.md`: required discovery and usage instructions
- `agents/openai.yaml`: optional UI metadata
- `scripts/`: deterministic helpers or repeated execution surfaces
- `references/`: deeper material loaded only when needed
- `assets/`: files used in outputs, not context docs

## Progressive disclosure

Keep context cheap:

1. Metadata is always loaded
2. `SKILL.md` body loads when the skill triggers
3. `references/` and scripts should be loaded only when needed

Prefer a short `SKILL.md` plus focused references over one giant instruction dump.

## Naming

- Use lowercase letters, digits, and hyphens
- Keep names under 64 characters
- Prefer short action-oriented names
- Match folder name and skill name

## When to add scripts

Add `scripts/` only when:

- the same code is repeatedly rewritten
- deterministic execution matters
- the helper can be reused across multiple invocations

Do not add scripts just to mirror prose instructions.

## When to add references

Add `references/` when:

- details are useful but not always needed
- there are multiple variants/providers/frameworks
- examples or API surfaces would bloat `SKILL.md`

Avoid nested reference mazes. Keep references directly linked from `SKILL.md`.
