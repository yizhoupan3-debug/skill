---
name: gsd-plan-phase
description: |
  Create ROADMAP.md and wave plan for project execution. DOCS ONLY — no product code changes.
  Use after /gsd-new-project completes adversarial review.
  Defines verification commands for later; does not run them to fix the repo.
routing_layer: L1
routing_owner: owner
routing_gate: evidence
routing_gate_evidence: "GOAL_STATE.json exists, adversarial review passed"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /gsd-plan-phase
  - gsd plan
  - create roadmap
  - wave plan
  - phase breakdown
metadata:
  version: "0.2.0"
  platforms: [supported]
  tags: [gsd, planning, roadmap, waves]
---

# gsd-plan-phase

Create ROADMAP.md and wave plan based on REQUIREMENTS.md.

## HARD: Planning Only (No Implementation)

Still **pre-execution**. Produce ROADMAP + WAVE_STATE only.

**Must read**: [../shared/phase-boundaries.md](../shared/phase-boundaries.md)

- Define verification commands in ROADMAP — **do not run them** to fix the repo
- If discovery finds broken build/tests, document in ROADMAP risks — do not patch code

## Pre-Conditions

1. `REQUIREMENTS.md` exists in `artifacts/current/<task_id>/`
2. `GOAL_STATE.json` exists (from gsd-new-project)
3. Adversarial review passed (no P0/P1 findings)
4. `artifacts/current/<task_id>/` directory is writable

## Planning Process

### Step 1: Read Requirements

Read REQUIREMENTS.md and extract:
- Problem statement
- Constraints
- Success metrics
- Implementation decisions

### Step 2: Identify Phases

Decompose into logical phases:

**Typical Phase Structure**:
```
Phase 1: Foundation (infrastructure, scaffolding)
Phase 2: Core Feature Development
Phase 3: Integration
Phase 4: Polish & Optimization
Phase 5: Testing & Documentation
```

### Step 3: Decompose into Waves

Group phases into waves based on:
- Dependencies (what must complete before next can start)
- Risk (high-risk items earlier for more iteration time)
- Value (early wins for stakeholder confidence)

**Wave Model**:
```
Wave 1: [Phase 1, Phase 2] - Foundation + Core
Wave 2: [Phase 3, Phase 4] - Integration + Polish
Wave 3: [Phase 5] - Testing + Documentation
```

### Step 4: Define Verification Commands (Planned, Not Executed)

For each phase, **name** commands for execute-phase / verify-work — do **not** run them during plan-phase:

| Phase | Verification Commands (execute later) |
|-------|--------------------------------------|
| Foundation | `cargo check`, `cargo test --lib` |
| Core | `cargo test`, integration tests |
| Integration | e2e / smoke tests |
| Polish | `clippy`, `fmt`, `audit` |
| Testing | coverage, docs build |

### Step 5: Write ROADMAP.md

Create comprehensive roadmap with:

```markdown
# Project Roadmap

## Phases
### Phase 1: Foundation
- Tasks: [list]
- Verification: [commands]
- Owner: [agent/type]

### Phase 2: Core Feature Development
...

## Waves
### Wave 1
- Phases: [1, 2]
- Parallel agents: [3]
- Expected duration: [estimate]
- Exit criteria: [conditions]

### Wave 2
...

## Dependencies
- [phase A] → [phase B]
- [phase C] → [phase D]

## Risks
- [risk] → [mitigation]
```

### Step 6: Update WAVE_STATE.json

```bash
printf '%s\n' '{"schema_version":"gsd-wave-state-v1","task_id":"<task_id>","current_wave":0,"waves":[],"global_status":"planned"}' > artifacts/current/<task_id>/WAVE_STATE.json
```

## RFV Integration

Start RFV loop for planning review:

```bash
printf '%s\n' '{"id":1,"op":"framework_rfv_loop","payload":{"operation":"start","repo_root":"<path>","goal":"Review ROADMAP.md for feasibility","max_rounds":1,"allow_external_research":false,"review_scope":"roadmap","verify_commands":["cat ROADMAP.md"],"stop_when":["wave decomposition complete","verification commands defined"]}}' | router-rs --stdio-json
```

## Output Artifacts

| Artifact | Location | Description |
|----------|----------|-------------|
| ROADMAP.md | artifacts/current/<task_id>/ | Complete roadmap |
| WAVE_STATE.json | artifacts/current/<task_id>/ | Wave execution state |

## Next Step

After planning, stop and wait for explicit `/gsd-execute-phase` — **first command allowed to modify product code**.

## Anti-Patterns

- Don't run `cargo test` / `cargo clippy` to "validate the plan" by fixing the repo
- Don't scaffold modules or edit source during planning
- Don't set WAVE_STATE `global_status` to `running` — use `planned` only
