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
  version: "0.2.0"
  platforms: [supported]
  tags: [my-lifecycle, verify, ship, evidence]
---

# verifyx

**Zone**: execution+ · **profile**: `my-light` · **no separate ship command**

## Checklist (single pass)

### 1. Verify

- Run `GOAL_STATE.validation_commands` and ROADMAP global verification commands
- Append each run to `EVIDENCE_INDEX.json` (`exit_code`, `command`)
- `VERIFY_REPORT.md` summary on disk

### 2. Ship

- Git clean / intentional uncommitted documented
- `framework_closeout_evaluate` → `artifacts/closeout/<task_id>.json` (**embed** evidence rows / verify summary before purge)
- `GOAL_STATE` → `status: completed`, `drive_until_done: false`
- Closeout fields: `gsd_artifacts_purged: true`, `task_dir_removed: true`

### 3. Post-verify task-dir purge (**mandatory**, every my-lifecycle task)

**Order**: closeout JSON written → then delete.

```bash
TASK_ID=<task_id>
# After closeout evaluate succeeded:
rm -rf "artifacts/current/${TASK_ID}"
```

Removes all four-phase traces under `artifacts/current/<task_id>/` (including `REQUIREMENTS.md`, `DECISIONS.md`, `OPEN_QUESTIONS.md`, `ROADMAP.md`, `WAVE_STATE.json`, `GOAL_STATE.json`, `EVIDENCE_INDEX.json`, `VERIFY_REPORT.md`, `lane-notes/`, `SCHEMA_DRIFT_BASELINE.json`).

**Only ship artifact**: `artifacts/closeout/<task_id>.json`.

Neutralize pointers if they reference this task: `active_task.json`, `focus_task.json`, `task_registry.json`, `.supervisor_state.json`.

### 4. Chat

≤5 lines: PASS/FAIL, closeout path, purge done. No command dumps.

## Pre-conditions

- `WAVE_STATE.global_status` = `completed` (or implement waived with user ack)

## Canonical evidence protocol

See `skills/verifyx/references/evidence-protocol.md`.

## Schema drift

Headings contract: `configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md`. Run `schema-drift baseline` + `check` **before** task-dir purge.
