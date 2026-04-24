# skill-routing-repair — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Design principles

This skill is intentionally **narrower** than a generic self-improving agent.
It repairs **Codex routing behavior**, not everything the agent learns.

Borrowed lessons from public self-improvement skills:

- Keep a clear **miss taxonomy**
- Prefer **minimal repair** over broad rewrites
- Add strong **anti-loop guardrails**
- Defer major promotions or framework redesign unless the pattern truly repeats

## Finding-driven framework boundary

This skill belongs to the **outer loop** of the finding-driven framework: it repairs routing after a task, rather than participating in the inner detect → plan → execute → verify loop for the user artifact itself. Do not let inner-loop findings automatically trigger repeated routing repair in the same turn.

## Anti-loop guardrails

These rules override all other instructions in this skill:

1. **One repair pass per exposed miss**  
   Do not keep re-triggering routing repair from the edits, validations, or summaries produced by the repair itself.
2. **No chain reaction**  
   Updating one skill, index file, or registry must not automatically trigger another repair loop in the same turn unless the user explicitly asks for more.
3. **Prefer the smallest patch**  
   Fix the owner skill's description or boundaries before proposing wide library rewrites.
4. **Defer major promotion**  
   Do not escalate into a framework rewrite, new gate, or multi-skill split unless the miss is structural or recurring.
5. **At most one overlay handoff**  
   This skill may suggest `$skill-framework-developer` for bigger redesigns, but should not bounce back and forth repeatedly in one turn.

## Miss taxonomy

Classify the observed problem as one primary type:

1. **Wrong owner**
   - a broader or adjacent skill stole the task
2. **Missing owner**
   - no existing skill matched clearly enough
3. **Missing gate**
   - source/artifact/evidence/execution/delegation gate should have fired first
4. **Weak trigger surface**
   - description, trigger phrases, or top-of-file routing note is too weak
5. **Weak body guidance**
   - the correct skill triggered, but its body lacked the needed instruction
6. **Library drift**
   - index/quickref/registry/sync assumptions no longer reflect reality

Choose one primary type first. Avoid fixing five categories at once unless they are truly inseparable.

## Repair workflow

### 1. Capture the miss

Record:

- what the user asked (or what auto-trigger condition fired: routing drift, struggle, or missing gate)
- which skill should have owned or gated the task
- what actually happened
- why the miss mattered to execution quality

### 2. Pick the smallest durable fix

Fix in this order:

1. owner skill `description`
2. owner skill `## When to use`
3. owner skill `## Do not use`
4. top-of-file routing notes
5. supporting routing docs:
   - [`SKILL_ROUTING_INDEX.md`](/Users/joe/Documents/skill/skills/SKILL_ROUTING_INDEX.md)

Do not start by expanding deep body prose if the discovery surface is still weak.

### 3. Decide whether to escalate

Escalate to [`$skill-framework-developer`](/Users/joe/Documents/skill/skills/skill-framework-developer/SKILL.md) when any of these are true:

- the fix requires a new skill
- the current skill should split into owner/gate/overlay variants
- multiple skills need coordinated redesign
- the miss reveals a Codex-vs-Antigravity runtime split

### 4. Apply anti-overreach rules

Only create a **new skill** when at least one is true:

- the repair behavior is a distinct reusable workflow
- owner and overlay concerns are currently mixed
- the same routing problem recurs across more than one skill
- a minimal local patch would keep failing for structural reasons

Only create a **new gate** when routing truly depends on source, artifact, evidence, execution, or delegation status.

### 5. Validate and sync

After repair:

```bash
python3 scripts/check_skills.py --verify-codex-link
python3 scripts/check_skills.py --include-system --verify-codex-link
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json \
  --apply
```

## Output expectations

Default output should state:

- the miss type
- what was changed
- why this is the minimal durable fix
- whether deeper redesign was intentionally deferred

## Escalation heuristic

If the same miss pattern appears repeatedly, prefer this ladder:

1. patch one skill
2. patch trigger docs
3. split owner vs overlay or owner vs gate
4. only then redesign the broader framework

## Feedback loop integration

Every routing miss should be recorded in [`skills/.routing_feedback.md`](/Users/joe/Documents/skill/skills/.routing_feedback.md) for aggregate analysis.

### When to log

After completing step 1 of the repair workflow (capture the miss), append a row:

```markdown
| YYYY-MM-DD | `expected-skill` | `actual-skill` | miss reason | fix applied |
```

### When to analyze

Periodically (or when miss count grows), run the analysis script:

```bash
python3 scripts/analyze_routing.py          # text summary
python3 scripts/analyze_routing.py --json   # machine-readable
```

The script will:
- identify frequently missed skills
- detect skills missing from the trigger index
- generate description improvement suggestions automatically
