---
name: verifyx
description: |
  Personal lifecycle — verify + ship in one command. Evidence index, tests, closeout, goal complete.
  Use after /implementx. Merges legacy verify-work and ship checklists.
routing_layer: L1
routing_owner: owner
routing_gate: evidence
routing_gate_evidence: "WAVE_STATE.json global_status=completed"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /verifyx
  - verifyx
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [my-lifecycle, verify, ship, evidence]
---

# verifyx

**Zone**: execution+ · **profile**: `my-light` · **no separate ship command**

## Checklist (single pass)

### 1. Verify (from `gsd-verify-work`)

- Run `GOAL_STATE.validation_commands` and ROADMAP §6 commands
- Append each run to `EVIDENCE_INDEX.json` (`exit_code`, `command`)
- `VERIFY_REPORT.md` summary on disk

### 2. Ship (from `gsd-ship`, no RFV loop required)

- Git clean / intentional uncommitted documented
- `framework_closeout_evaluate` → `artifacts/closeout/<task_id>.json`
- `GOAL_STATE` → `status: completed`, `drive_until_done: false`

### 3. Chat

≤5 lines: PASS/FAIL, closeout path, blocker if any. No command dumps.

## Pre-conditions

- `WAVE_STATE.global_status` = `completed` (or implement waived with user ack)

## Canonical evidence protocol

See `skills/_archived/gsd-lifecycle/verify-work/evidence-protocol.md` (paths and schema unchanged).
