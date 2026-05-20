---
name: gsd
description: |
  GSD (Global Skill Development) lifecycle commands for end-to-end project management.
  Pre-execution (/gsd-new-project, /gsd-discuss-phase, /gsd-plan-phase): doc-only per phase-boundaries.md.
  Execution+ (/gsd-execute-phase, /gsd-verify-work, /gsd-ship): implementation and evidence.
  Use when the user invokes /gsd, /gsd-new-project, /gsd-plan-phase, /gsd-execute-phase,
  /gsd-verify-work, /gsd-discuss-phase, or /gsd-ship.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
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

# GSD - Global Skill Development (legacy)

**Personal default is My lifecycle** — `/my-discuss` → `/my-plan` → `/my-implement` → `/my-verify`. This tree is **legacy-gsd** (cold manifest / framework CI only); `user-invocable: false` so slash commands do not appear in hosts.

Repo-native GSD augments [official get-shit-done](https://github.com/gsd-build/get-shit-done) semantics — see `references/OFFICIAL_GSD_ALIGNMENT.md`.

**Hard contract**: `shared/phase-boundaries.md` — pre-execution must not mutate product code.

## Commands (official order at harness level)

| Command | Zone | Description |
|---------|------|-------------|
| /gsd-new-project | Pre-exec | Exploration, REQUIREMENTS, risks, GOAL_STATE (`planned`, `drive_until_done: false`) |
| /gsd-discuss-phase | Pre-exec | ADRs / architecture decisions (upstream: per-phase CONTEXT before plan) |
| /gsd-plan-phase | Pre-exec | ROADMAP.md, WAVE_STATE (`planned`) |
| /gsd-execute-phase | Execution | Waves, multi-agent, may set `drive_until_done: true` |
| /gsd-verify-work | Execution | Evidence-driven verification |
| /gsd-ship | Execution | Delivery gate + adversarial review |

## Core Principles

1. **Adversarial First**: Review from day one, not just before ship
2. **Evidence-Driven**: Every verification must produce EVIDENCE_INDEX entries
3. **One-Breath Execution**: Don't ask user at every step during execute-phase only
4. **Multi-Agent**: Subagent-dense, main thread lightweight (≤40% context)
5. **Multi-Host**: Works on Desktop MCP, CLI, Codex, Cursor

## State Files

- `GOAL_STATE.json` - Macro goal contract (planning until execute-phase)
- `RFV_LOOP_STATE.json` - Multi-round adversarial loop ledger
- `EVIDENCE_INDEX.json` - Verification command execution records
- `WAVE_STATE.json` - Wave execution state
- `SHIPPING_STATE.json` - Delivery gate state

## Quick Start

1. `/gsd-new-project <description>` — core docs only, **no product code**
2. `/gsd-discuss-phase` — ADRs / decisions (doc-only)
3. `/gsd-plan-phase` — ROADMAP + wave plan (doc-only)
4. `/gsd-execute-phase` — implementation (explicit entry to coding)
5. `/gsd-verify-work` — verify with evidence
6. `/gsd-ship` — deliver with adversarial review

For multi-phase work, repeat steps 2–5 per phase (see alignment doc).

See individual command SKILLs for detailed usage.
