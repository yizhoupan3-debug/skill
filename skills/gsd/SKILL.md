---
name: gsd
description: |
  GSD (Global Skill Development) lifecycle commands for end-to-end project management.
  Use when the user invokes /gsd, /gsd-new-project, /gsd-plan-phase, /gsd-execute-phase,
  /gsd-verify-work, /gsd-discuss-phase, or /gsd-ship. Provides exploration, planning,
  execution, verification, architecture decisions, and delivery with adversarial review.
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
  version: "0.1.0"
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

1. **Adversarial First**: Review from day one, not just before ship
2. **Evidence-Driven**: Every verification must produce EVIDENCE_INDEX entries
3. **One-Breath Execution**: Don't ask user at every step, execute through waves
4. **Multi-Agent**: Subagent-dense, main thread lightweight (≤40% context)
5. **Multi-Host**: Works on Desktop MCP, CLI, Codex, Cursor

## State Files

- `GOAL_STATE.json` - Macro goal contract
- `RFV_LOOP_STATE.json` - Multi-round adversarial loop ledger
- `EVIDENCE_INDEX.json` - Verification command execution records
- `WAVE_STATE.json` - Wave execution state
- `SHIPPING_STATE.json` - Delivery gate state

## Quick Start

1. `/gsd-new-project <project description>` - Start with deep exploration
2. `/gsd-plan-phase` - Create roadmap and wave plan
3. `/gsd-execute-phase` - Execute all phases (one breath)
4. `/gsd-verify-work` - Verify results
5. `/gsd-discuss-phase` - Make architecture decisions
6. `/gsd-ship` - Deliver with adversarial review

See individual command SKILLs for detailed usage.
