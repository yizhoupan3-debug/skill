---
name: brainstorm-research
description: |
  Expand early research ideas into multiple comparable directions and preserve them in a local workspace.
  Use when the user asks 帮我 brainstorm 研究点、先发散研究思路、给我多个论文方向、把脑暴记录下来, or wants structured option generation before committing to a question, method, dataset, experiment plan, or paper framing. Check this skill early at 每轮对话开始 / first-turn / conversation start for early-stage ideation.
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: preferred
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags:
    - research
    - brainstorming
    - ideation
    - local-files
    - decision-log
risk: low
source: local
---

# Brainstorm Research

## Overview

Turn thin research inputs into a broad option space **and** preserve the reasoning on disk when the task is more than a one-shot brainstorm. This skill follows a **brainstorm + file-backed planning** pattern: generate many directions → keep them comparable → record what was considered → make the space resumable.

This skill is a valid **opening-turn owner** for early-stage research ideation.

## When to use

- User has an early research idea, vague topic, paper seed, or undeveloped proposal
- User wants many distinct research directions before choosing one
- User wants the ideation process structured, logged, or localized to files
- User wants to iterate on the same research space across multiple turns

## Do not use

- The user already has a focused topic and wants a deep novelty check or systematic literature review → use `$literature-synthesis`
- The user has existing papers and wants to find research gaps from that literature → use `$literature-synthesis` Mode E (Research-Gap Memo)
- Mature corpus → use `$literature-synthesis`
- Execution plan → use `$plan-writing`
- Implementation from spec → use `$plan-to-code`
- Reviewer-style critique → use `$paper-reviewer` or `$paper-logic`
- The user wants autonomous multi-hypothesis experiment orchestration → use `$autoresearch`

## Primary operating principle

This owner should behave like an **ideation orchestrator**:

1. generate multiple real branches, not cosmetic variants
2. keep branch detail and iteration logs outside the main thread
3. reserve the main thread for branch framing, selection logic, and next-step choice
4. if runtime policy permits, sidecar bounded novelty checks or feasibility probes
5. if spawning is blocked, preserve the same branch structure in local-supervisor mode

## Main-thread compression contract

The main thread should contain only:

- topic framing
- branch set summary
- why branches differ
- shortlist / rejection logic
- next research owner or handoff

## Runtime-policy adaptation

If bounded novelty checks or feasibility probes help and runtime policy permits:

- route them through [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)
- if a feasibility probe becomes runnable prototype code, add a companion check through [`$code-acceleration`](../code-acceleration/SKILL.md) and the relevant memory-control owner before expensive runs

If runtime policy does **not** permit spawning:

- keep those probes in a local-supervisor branch queue
- keep detailed ideation notes in files rather than chat

## Cross-references

- `$autoresearch` bootstrap phase may invoke this skill for initial direction generation
- `$literature-synthesis` Mode E (Research-Gap Memo) provides evidence-grounded gaps; this skill provides divergent expansion from thin input
- `$academic-search` can be invoked during the novelty gate for quick verification searches

## Boundary: `brainstorm-research` vs `autoresearch`

- This skill generates **directions** (pre-experiment). `$autoresearch` **executes** experiments across those directions.
- Typical flow: `brainstorm-research` → pick direction → `autoresearch` runs the experiment cycle.
- Pure ideation stays single-owner here; once a branch becomes a feasibility prototype or benchmarked code path, companion checks for acceleration and memory control should activate.

## Operating Modes

### 1. Quick divergence mode
For fast one-shot brainstorms without local file needs. Output directly in chat.

### 2. File-backed exploration mode
Use when: 本地化 / 记录 / 持续迭代, multi-turn tasks, 8+ candidates, paper/proposal evolution.

Local workspace: `research/brainstorms/<YYYY-MM-DD>-<topic-slug>/`
- `brainstorm_plan.md` → [template](templates/brainstorm_plan.md)
- `direction_map.md` → [template](templates/direction_map.md)
- `iteration_log.md` → [template](templates/iteration_log.md)

## Core Principles

- Treat ambiguity as permission to expand, not a reason to stall
- Ask at most one blocking question
- Prefer breadth with structure: many directions, grouped, named, and comparable
- Generate meaningfully different options, not cosmetic variants
- Record why a direction matters and why it was rejected

## Brainstorm Workflow

1. **Identify seed** — Extract topic, gap, domain, constraints, target output
2. **Decide mode** — Quick vs file-backed
3. **Expand across axes** — Multiple research axes for divergence
4. **Package as research bets** — Short name, core idea, novelty, risk, next step, status
5. **Cluster and compare** — Group into 3-6 buckets
6. **Converge intentionally** — Shortlist 2-4, log rationale
7. **Novelty gate** — Before finalizing shortlist:
   - For each shortlisted direction, extract 1-2 core novelty claims
   - Do a quick search pass (Phase 2 from `$literature-synthesis` Mode B, scoped to top-5 results per claim)
   - Flag any direction with 🔴 high overlap → drop, reframe, or park as "needs deeper verification"
   - If concerns need deeper investigation, recommend a full `$literature-synthesis` Mode B novelty check
8. **End with next actions** — Best pilot, best publishable angle, cheapest experiment

See [detailed expansion axes, file discipline rules, and convergence protocol](references/DETAIL.md) for complete reference.

## Structured Ideation Frameworks

For deeper ideation, use the 10 cognitive frameworks in [references/ideation-frameworks.md](references/ideation-frameworks.md):

| Situation | Best Framework |
|---|---|
| Have a problem, need solutions | Problem-First |
| Have a technique, need applications | Solution-First, Cross-Pollination |
| Field feels stuck | What Changed, Tension Hunting |
| Want to find novelty | Abstraction Ladder, Boundary Probing |
| Want rigor check on existing idea | Simplicity Test, Explain It |
| Want breadth | Cross-Pollination, Stakeholder Rotation |

## Default Output Shape

**Quick mode:** Seed restatement → Assumptions → 8-15 directions → Short grouping
**File-backed:** Seed → Why file-backed → Workspace path → Direction buckets → Shortlist → Next actions
