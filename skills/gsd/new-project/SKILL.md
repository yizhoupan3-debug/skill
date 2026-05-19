---
name: gsd-new-project
description: |
  Start new project with deep exploration and mandatory adversarial review.
  DOCS ONLY: produces core artifacts under artifacts/current/<task_id>/ — no product code changes.
  Use when the user invokes /gsd-new-project or wants to start a new project.
  Provides 5-layer exploration funnel; RFV revises REQUIREMENTS/ARCHITECTURE only until plan-phase.
  Code implementation starts only at /gsd-execute-phase.
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /gsd-new-project
  - gsd new project
  - start new project
  - new project exploration
metadata:
  version: "0.2.0"
  platforms: [supported]
  tags: [gsd, exploration, requirements]
---

# gsd-new-project

Start new project with deep exploration and mandatory adversarial review.

## HARD: Docs Only (No Implementation)

This command ends at **core planning documents**. It does **not** implement features or fix the codebase.

**Must read**: [../shared/phase-boundaries.md](../shared/phase-boundaries.md)

| Allowed | Forbidden |
|---------|-----------|
| Write `artifacts/current/<task_id>/` core docs + state JSON | Edit `src/`, tests, configs, lockfiles |
| Read-only repo exploration (grep, read, readonly subagents) | `cargo fix`, scaffold, dependency changes |
| RFV revises REQUIREMENTS / ARCHITECTURE / risks | RFV that patches product source |
| EVIDENCE_INDEX: exploration + doc-review notes | Running ROADMAP verification commands to heal build |

**Next command that may touch code**: `/gsd-execute-phase` only.

## Pre-Conditions

1. User has a project idea or problem statement
2. Task ID is determined (from active_task.json or user input)
3. `artifacts/current/<task_id>/` directory is available

## 5-Layer Exploration Funnel

### Layer 1: Problem Definition

Explore these questions:
- What is the core problem? What pain point does it solve?
- Who are the direct users? Who are indirect beneficiaries?
- What are the current alternatives? Why are they insufficient?
- What does success look like? How do we quantify it?

**Output**: Problem statement (1 paragraph)

### Layer 2: Constraint Scan

Explore these constraints:
- Technical constraints (language, framework, platform limits)
- Resource constraints (time, budget, manpower)
- Compliance constraints (security, privacy, licensing)
- Existing infrastructure we can reuse

**Output**: Constraint list (bulleted)

### Layer 3: Risk Assessment

Identify risks:
- Technical risks (which parts are most likely to fail?)
- Integration risks (dependency stability?)
- Scaling risks (can current design extend?)
- Operational risks (deployment, monitoring, rollback?)

**Output**: Risk register (P0/P1/P2 rated)

### Layer 4: Competitive Analysis

Analyze landscape:
- Similar implementations in repository? Can we reuse?
- What community solutions exist? Compare pros/cons?
- What is our differentiated positioning?

**Output**: Competitive analysis table

### Layer 5: Architecture Sketch

Draft high-level architecture:
- Module boundaries (what are the major modules?)
- Data flow (input → processing → output)
- Interface contracts (how do modules communicate?)
- Extension points (future change directions?)

**Output**: Architecture diagram (ASCII) + module list

## Output Artifacts (Core Docs Only)

After exploration, generate **only** these (no other repo writes):

| Artifact | Location | Description |
|----------|----------|-------------|
| REQUIREMENTS.md | artifacts/current/<task_id>/ | Complete requirements document |
| ARCHITECTURE.md | artifacts/current/<task_id>/ | Architecture sketch |
| RISK_REGISTER.md | artifacts/current/<task_id>/ | Risk register |
| GOAL_STATE.json | artifacts/current/<task_id>/ | Planning contract (`status`: `planned`, `drive_until_done`: **false**) |
| EVIDENCE_INDEX.json | artifacts/current/<task_id>/ | Exploration / doc-review evidence (not test-run logs) |

Do **not** create implementation files, stubs, or “quick fixes” in the product tree.

## REQUIRED: Mandatory Adversarial Review

**Timing**: After REQUIREMENTS.md draft is complete

**Trigger**: Immediately start adversarial review before proceeding to planning

**Process** (documentation RFV only):
1. Call `code-review-deep` in **review-only** mode on REQUIREMENTS.md + ARCHITECTURE.md (no code rewrite)
2. Start RFV loop with 2 rounds:
   - Round 1: Internal review lane (**readonly** subagent)
   - Round 2: External research lane (competitive comparison → update docs)
3. Each round: findings → **revise documents** → verify (re-read docs; checklist; no `cargo test` on repo)
4. Review passes only when P0/P1 **documentation** findings are zero or accepted with rationale in REQUIREMENTS.md

**Review Focus**:
- Requirements completeness (all user stories covered?)
- Constraint reasonableness (any missed hard constraints?)
- Risk identification (any missed major risks?)
- Architecture feasibility (technical choices reasonable?)

**Evidence**: Write all review results to EVIDENCE_INDEX.json

## Stdio Operations

```bash
# Draft goal contract only — do NOT start autopilot execution
printf '%s\n' '{"id":1,"op":"framework_autopilot_goal","payload":{"operation":"start","repo_root":"<path>","goal":"<goal from exploration>","non_goals":["<non-goals>"],"done_when":["<done-when conditions>"],"validation_commands":["<planned commands — not run in new-project>"],"drive_until_done":false,"status":"planned"}}' | router-rs --stdio-json

# RFV on documents only (fix_scope = artifacts/current/<task_id>/*.md)
printf '%s\n' '{"id":2,"op":"framework_rfv_loop","payload":{"operation":"start","repo_root":"<path>","goal":"Review REQUIREMENTS.md for completeness","max_rounds":2,"allow_external_research":true,"review_scope":"artifacts/current/<task_id>/REQUIREMENTS.md","fix_scope":"artifacts/current/<task_id>/","verify_commands":["cat artifacts/current/<task_id>/REQUIREMENTS.md"],"stop_when":["P0 doc findings zero","P1 doc findings zero"]}}' | router-rs --stdio-json
```

## Next Step

After adversarial review passes, proceed to `/gsd-plan-phase`.

## Anti-Patterns to Avoid

- Don't rush to implementation before exploration is complete
- **Don't fix build/test/lint failures in this phase** — record as risks or ROADMAP inputs for execute-phase
- Don't skip adversarial review even if "time is short"
- Don't assume existing solutions don't exist without checking
- Don't skip risk identification "we can figure it out later"
- Don't set `drive_until_done: true` or spawn implementation agents here
