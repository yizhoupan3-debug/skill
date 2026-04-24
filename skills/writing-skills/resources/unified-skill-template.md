# Unified Skill Template

Use this as the default template for skills in:
- `/Users/joe/Documents/skill/skills`

Goal:
- make routing easier
- make boundaries clearer
- make outputs more consistent
- make weak skills easier to upgrade without inventing a new structure every time

Note:
- this is the shared generic template
- for single new-skill writing guidance on description budget, token control, boundaries, and framework embedding, use `/Users/joe/Documents/skill/skills/skill-writer/SKILL.md`

---

## 1. Recommended default skeleton

```markdown
---
name: skill-name
description: |
  [Role + domain nouns + tools/files + explicit trigger scenarios].
  Use proactively when the user asks for [exact phrases], mentions [frameworks /
  artifacts], or needs [specific action / deliverable].
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - tag-one
    - tag-two
    - tag-three
risk: low
source: local
---

# Skill Name

One-sentence identity:
- what this skill owns
- why it should be chosen instead of a broader skill

## When to use this skill

- The user asks for ...
- The task involves ...
- The user mentions ...
- Best for requests like "..."

## Do not use this skill when

- The task is really about ...
- Another skill should own ...
- The user wants ... instead of ...

## Task ownership and boundaries

This skill owns:
- ...
- ...

This skill does not own:
- ...
- ...

If the task shifts to [neighbor skill territory], route to:
- `$other-skill`

## Required workflow

1. Confirm the task shape:
   - object
   - action
   - constraints
   - deliverable
2. Gather the minimum context needed.
3. Execute the core workflow in the right order.
4. Validate the result.
5. Deliver in the expected output format.

## Core workflow

### 1. Intake
- What to inspect first
- What to ask or infer
- What inputs are required

### 2. Execution
- Step A
- Step B
- Step C

### 3. Validation / recheck
- What must be re-run, re-render, or re-read
- What counts as success
- What to do if blocked

## Output defaults

Default output should contain:
- section 1
- section 2
- section 3

Recommended structure:

````markdown
## Summary
- ...

## Findings / Plan / Result
- ...

## Risks / Assumptions
- ...
````

## Hard constraints

- Do not ...
- Never ...
- Always ...
- If evidence is missing, say so explicitly.

## Trigger examples

- "..."
- "..."
- "..."

## Optional supporting assets

If needed, place reusable material in:
- `references/`
- `scripts/`
- `assets/`
- `examples/`
```

---

## 2. What every strong skill should contain

At minimum, every skill should have:

1. **Dense description**
   - exact nouns users will say
   - action verbs
   - file/tool/framework names
   - clear trigger phrasing

2. **Use / not-use split**
   - reduces false positives
   - makes neighboring skills easier to route

3. **Ownership section**
   - says what this skill owns
   - says what it does not own
   - points to adjacent skills

4. **Executable workflow**
   - not just concepts
   - ordered steps
   - verification loop

5. **Output format**
   - review skills need review output
   - fix skills need fix log
   - planning skills need plan schema

6. **Hard constraints**
   - prevents drift
   - avoids hidden behavior changes

---

## 3. How to write the description well

Use this formula:

```text
[Role] + [domain nouns] + [tools/files] + [explicit trigger scenarios]
```

Good pattern:

```text
Audit webhook handlers for signature verification, raw-body handling, replay
protection, idempotency, and provider-specific security issues. Use when the
user asks to implement or review Stripe/GitHub/Slack webhook endpoints or
callback security.
```

Weak pattern:

```text
Helps with webhooks.
```

Checklist:
- includes user vocabulary
- includes tool/framework vocabulary
- includes the actual action
- does not summarize the whole workflow
- is specific enough to beat a generic skill

---

## 4. Output schemas by skill type

### Review / audit skill

Use:
- summary
- findings grouped by severity
- evidence
- recommended next steps

### Fix / update skill

Use:
- issue id / location
- fix action
- recheck result
- remaining risk
- status

### Planning skill

Use:
- goal
- tasks
- dependencies
- verification
- done-when criteria

### Builder / implementation skill

Use:
- approach
- files touched
- validation run
- result
- follow-up risks

---

## 5. Upgrade checklist for weak skills

When upgrading an old thin skill, do this in order:

1. rewrite `description`
2. add `When to use this skill`
3. add `Do not use this skill when`
4. add `Task ownership and boundaries`
5. add `Required workflow`
6. add `Output defaults`
7. add `Hard constraints`
8. add `Trigger examples`
9. move long detail to `references/` or `scripts/`
10. run:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root /Users/joe/Documents/skill/skills \
  --source-manifest /Users/joe/Documents/skill/skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest /Users/joe/Documents/skill/skills/SKILL_HEALTH_MANIFEST.json \
  --apply
```

---

## 6. Thin-skill warning signs

Usually a skill is too thin when:
- it is mostly philosophy, not procedure
- it has no explicit “do not use”
- it has no output contract
- it names tools but not order of use
- it has no validation loop
- it cannot distinguish itself from a nearby skill

---

## 7. Repository default rule

For this repository, prefer this template unless there is a strong reason not to.

Exceptions:
- extremely small utility skill
- system-provided skill you do not want to fork heavily
- very large platform skill that genuinely needs subdocuments
