---
name: gsd
description: |
  GSD (Global Skill Development) lifecycle commands for end-to-end project management.
  Pre-execution (/gsd-new-project, /gsd-plan-phase, /gsd-discuss-phase): core docs only, no code fixes.
  /gsd-execute-phase is the first phase allowed to change product code.
  Use when the user invokes /gsd, /gsd-new-project, /gsd-plan-phase, /gsd-execute-phase,
  /gsd-verify-work, /gsd-discuss-phase, or /gsd-ship.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /gsd
  - /gsd-new-project
  - /gsd-plan-phase
  - /gsd-execute-phase
  - /gsd-verify-work
  - /gsd-discuss-phase
  - /gsd-ship
metadata:
  version: "0.2.0"
  platforms: [supported]
  tags: [gsd, lifecycle, project-management, adversarial-review]
---

# GSD - Global Skill Development

GSD provides end-to-end development lifecycle management through 6 commands.

## Commands

| Command | Description | Phase |
|---------|-------------|-------|
| /gsd-new-project | Start new project with deep exploration + adversarial review | Exploration |
| /gsd-plan-phase | Create ROADMAP.md and wave plan | Planning |
| /gsd-execute-phase | Execute all phases in waves with multi-agent | Execution |
| /gsd-verify-work | Verify work with evidence-driven approach | Verification |
| /gsd-discuss-phase | Architecture decisions and ADR documentation | Discussion |
| /gsd-ship | Final delivery gate with adversarial review + multi-worktree | Delivery |

## Core Principles

1. **Docs Before Code**: Pre-execution phases produce core documents only — no product code changes (see [phase-boundaries.md](shared/phase-boundaries.md))
2. **Adversarial First**: Review from day one on **documents**; code adversarial review starts at ship / post-execute
3. **Evidence-Driven**: Every verification must produce EVIDENCE_INDEX entries (execution+ runs commands; pre-execution logs doc/review evidence)
4. **One-Breath Execution**: Don't ask user at every step, execute through waves — **only after** `/gsd-execute-phase` starts
5. **Multi-Agent**: Subagent-dense, main thread lightweight (≤40% context); pre-execution subagents must be read-only
6. **Multi-Host**: Works on Desktop MCP, CLI, Codex, Cursor

## Phase Mutation Policy

| Command | Output | Mutate repo code? |
|---------|--------|-------------------|
| /gsd-new-project | REQUIREMENTS, ARCHITECTURE, risks, GOAL_STATE (draft) | **No** |
| /gsd-plan-phase | ROADMAP, WAVE_STATE (planned) | **No** |
| /gsd-discuss-phase | ADR, decision docs | **No** |
| /gsd-execute-phase | Implementation | **Yes** |
| /gsd-verify-work | Evidence from tests/quality | Read + fix only if verify fails |
| /gsd-ship | Merge, gates, code RFV | **Yes** |

**Hard rule**: If the active command is not execute / verify / ship, do not edit `src/`, tests, configs, or run fix/build to heal the repo.

## State Files

- `GOAL_STATE.json` - Macro goal contract
- `RFV_LOOP_STATE.json` - Multi-round adversarial loop ledger
- `EVIDENCE_INDEX.json` - Verification command execution records
- `WAVE_STATE.json` - Wave execution state
- `SHIPPING_STATE.json` - Delivery gate state

## Quick Start

1. `/gsd-new-project <project description>` - Exploration + **core docs only** (no code)
2. `/gsd-plan-phase` - ROADMAP + wave plan (**no code**)
3. `/gsd-execute-phase` - **First phase allowed to change product code**
4. `/gsd-verify-work` - Verify results
5. `/gsd-discuss-phase` - Make architecture decisions
6. `/gsd-ship` - Deliver with adversarial review

See individual command SKILLs for detailed usage.
