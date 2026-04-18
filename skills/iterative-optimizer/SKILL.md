---
name: iterative-optimizer
description: |
  多轮优化 overlay：优化X轮 / 自迭代N轮 / review→fix→verify，适合需要明确轮次、收敛和防偷懒的任务。
  Use when a domain owner exists but the user wants bounded optimization rounds.
framework_roles:
  - planner
  - verifier
framework_phase: 2
framework_contracts:
  emits_findings: false
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: true
routing_layer: L0
routing_owner: overlay
routing_gate: none
routing_priority: P1
session_start: n/a
short_description: N-round optimization loops with built-in laziness immunity
trigger_hints:
  - 自迭代
  - 优化X轮
  - 优化10轮
  - review fix verify
  - 收敛
  - 再打磨几轮
  - 迭代优化
  - 多轮优化
  - 优化到收敛
  - iterative optimization
  - refine rounds
metadata:
  version: "3.0.0"
  platforms: [codex, antigravity]
  tags:
    - optimization
    - iteration
    - convergence
    - dimension-rotation
    - anti-laziness
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
  - TRACE_METADATA.json
bridge_behavior: mobile_complete_once
---

# iterative-optimizer

Multi-round orchestration overlay. Never replaces the domain owner — adds
round budget, dimension rotation, convergence discipline, and **native
laziness immunity** (not stacked; fused into every step).

## When to use

- 优化 X 轮 / 自迭代 N 轮 / 再打磨几轮
- A clear domain owner exists but needs structured review→fix→verify rounds
- Framework self-optimization requiring iteration rather than one pass
- Risk of diminishing-quality without explicit stop policy

## Do not use

- Framework design / skill governance → `$skill-developer-codex`
- Post-task routing repair only → `$skill-routing-repair-codex`
- One-pass execution with no round budget → domain owner alone
- Single-pass laziness correction without iteration → `$anti-laziness` standalone

## Round protocol

Each round executes **all five phases in order**. Laziness checks are fused
into phases 1, 4, and 5 — not bolted on as a separate step.

```
┌─ Phase 1: DETECT ──────────────────────────────────┐
│  Scan previous round’s output for PUA v2.2.0      │
│  patterns (Lazy Phrasing: should be/probably,     │
│  Truncation: ..., Doc Avoidance, Blame Shifting). │
│  Round 1: scan initial input for complexity      │
│  dodging signals. If any found → ESCALATE.        │
├─ Phase 2: PLAN ────────────────────────────────────┤
│  Pick target dimension (rotate per round).         │
│  Record success criteria + delta budget.           │
├─ Phase 3: EXECUTE ─────────────────────────────────┤
│  Apply deltas only. Carry forward nothing else.    │
├─ Phase 4: VERIFY ──────────────────────────────────┤
│  Evidence required: stdout, tests, rendered output │
│  or equivalent. No “probably works” claims.        │
│  Truncated code / placeholder → instant block.     │
│  MANDATORY: Run `/Users/joe/Documents/skill/scripts/verify_turn.py <log>`. │
├─ Phase 5: CHECKPOINT ──────────────────────────────┤
│  Fill the round checkpoint (below).                │
│  Laziness self-check must PASS to mark Converged.  │
│  Audit Score from `/Users/joe/Documents/skill/scripts/verify_turn.py` must be 0. │
└────────────────────────────────────────────────────┘
```

### Laziness escalation (fused)

When a laziness pattern is detected in Phase 1 or Phase 4:

| Occurrence | Action |
|-----------|--------|
| 1st | Name the pattern; change method, not parameters |
| 2nd | List 3 hypotheses; test each separately |
| 3rd | Run false-convergence challenge (see reference) |
| 4th | Full clarity checklist from `$anti-laziness` methodology |
| 5th+ | Minimal repro or structured handoff |

## Dimension rotation — 10-round default

| Round | Focus |
|-------|-------|
| 1 | Routing accuracy |
| 2 | Boundary clarity |
| 3 | Token efficiency |
| 4 | Trigger precision |
| 5 | Duplication removal |
| 6 | Layer / index consistency |
| 7 | Verification rigor |
| 8 | Overlay discipline |
| 9 | Regression / false-convergence challenge |
| 10 | Final compression + cleanup |

Above table is the default for skill/framework optimization. For non-framework
tasks, substitute domain-appropriate dimensions (e.g., readability, performance,
security). See [references/anti-fatigue-protocol.md](references/anti-fatigue-protocol.md) for D1–D10 audit questions.

## Convergence rules

1. ≥1 concrete delta per round, or explicit record of why not
2. Convergence requires **2 consecutive null-deltas across orthogonal dimensions**
3. False-convergence challenge mandatory before early stop

## Round checkpoint

```
[ROUND N]
Dimension : ...
Deltas    : ...
Evidence  : (stdout / test / render — paste or cite)
Lazy-scan : ✓ brute-retry ✓ blame ✓ tool-idle ✓ spin ✓ passive
Self-check: PASS | FAIL → (escalation action taken)
Decision  : Continue | Converged | Blocked
```
