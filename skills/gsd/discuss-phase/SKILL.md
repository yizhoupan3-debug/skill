---
name: gsd-discuss-phase
description: |
  Architecture decisions and ADR documentation with multi-round adversarial loop.
  Use when the user invokes /gsd-discuss-phase or wants to make architecture decisions.
  Provides RFV-based decision process, ADR template, and risk register.
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: true
trigger_hints:
  - /gsd-discuss-phase
  - gsd discuss
  - architecture decision
  - ADR
  - discuss design
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [gsd, architecture, ADR, decisions]
---

# gsd-discuss-phase

Make architecture decisions with adversarial loop and document as ADRs.

## Purpose

When facing architecture decisions:
- Which technology to use?
- How to structure modules?
- How to handle cross-cutting concerns?
- What are the trade-offs?

## Decision Process

### Step 1: Identify Decision Points

List all decisions that need to be made:

```
1. Database choice: PostgreSQL vs SQLite vs DynamoDB
2. API style: REST vs GraphQL vs gRPC
3. Authentication: JWT vs Session vs OAuth
4. Frontend: SPA vs SSR vs MPA
```

### Step 2: Start RFV Loop

For each significant decision:

```bash
printf '%s\n' '{"id":1,"op":"framework_rfv_loop","payload":{"operation":"start","repo_root":"<path>","goal":"Decide on <decision topic>","max_rounds":2,"allow_external_research":true,"review_scope":"architecture","verify_commands":["cat ADR-*.md"],"stop_when":["decision made","trade-offs documented"]}}' | router-rs --stdio-json
```

### Step 3: Multi-Perspective Analysis

Run parallel lanes:

**Technical Lane** (reviewer):
- Feasibility of each option
- Complexity implications
- Integration points
- Performance characteristics

**External Lane** (research):
- Community best practices
- Similar decisions in other projects
- Risks and mitigations

**Risk Lane**:
- What could go wrong?
- What's the rollback plan?
- What's the migration cost?

### Step 4: Document ADR

Create ADR file:

```markdown
# ADR-XXX: <Decision Title>

## Status
Proposed | Accepted | Deprecated | Superseded

## Context
<Problem background, constraints, decision drivers>

## Decision
<Final decision and rationale>

## Consequences
### Positive
### Negative
### Neutral

## Alternatives Considered
### Option A
### Option B

## Review Schedule
<Next review date or trigger condition>
```

### Step 5: Append Round

```bash
printf '%s\n' '{"id":2,"op":"framework_rfv_loop","payload":{"operation":"append_round","repo_root":"<path>","round":1,"review_summary":"<technical analysis>","external_research_summary":"<community research>","fix_summary":"<decision>","verify_result":"PASS","supervisor_decision":"close"}}' | router-rs --stdio-json
```

## Decision Criteria

### Must-Have Criteria
- Solves the core problem
- Meets constraints (time, budget, complexity)
- Has clear rollback plan

### Should-Have Criteria
- Follows best practices
- Has community support
- Has good tooling

### Nice-to-Have Criteria
- Innovative approach
- Performance optimization
- Future-proof design

## Risk Register

For each decision, maintain risk register:

```markdown
## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Option A fails | Medium | High | Fallback to Option B |
| Migration complexity | High | Medium | Incremental migration |
| Team learning curve | Low | Low | Training and docs |
```

## Output Artifacts

| Artifact | Location | Description |
|----------|----------|-------------|
| ADR-*.md | artifacts/current/<task_id>/ | Architecture decision records |
| RISK_REGISTER.md | artifacts/current/<task_id>/ | Risk register |
| STATE.md | artifacts/current/<task_id>/ | Updated state with decisions |

## Next Step

After decisions made, return to `/gsd-execute-phase` or `/gsd-verify-work`.

## Anti-Patterns

- Don't make decisions without analysis
- Don't skip external research
- Don't ignore risks
- Don't skip ADR documentation
- Don't skip RFV loop for significant decisions
