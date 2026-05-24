---
name: planx
description: |
  Personal lifecycle — plan/roadmap (doc-only). Writes ROADMAP.md and WAVE_STATE.json with explicit serial/parallel DAG.
  Use when user explicitly requests plan after /discussx. Does not mutate product code.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_gate_evidence: "REQUIREMENTS.md exists"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /planx
  - planx
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [my-lifecycle, plan, waves]
---

# planx

**Zone**: pre-execution · **profile**: `my-light`

**Entry gate**: user must explicitly invoke `/planx` (or clear plan intent) after `/discussx`; do not enter from agent nudge alone.

**Inputs**: `REQUIREMENTS.md`, `DECISIONS.md`, `OPEN_QUESTIONS.md` (carry unresolved items into plan scope or wave notes).

## Disk outputs

| File | Purpose |
|------|---------|
| `artifacts/current/<task_id>/ROADMAP.md` | Phases, exit criteria, verification commands |
| `artifacts/current/<task_id>/WAVE_STATE.json` | Each wave: `parallel_group`, `depends_on`, `execution_mode`, `lanes[]` |
| `artifacts/current/<task_id>/GOAL_STATE.json` | **`framework_goal_drive` stdio** only — set `status` via ledger-backed ops (`start` / `checkpoint` / `pause`); keep `lifecycle_profile: my-light`, `drive_until_done: false` |

### GOAL_STATE writes (HARD)

Same contract as `skills/discussx/SKILL.md` §GOAL_STATE writes. **Forbidden**: direct file edit of `GOAL_STATE.json`.

## Outputs (schema)

Topology fields (schema id **`my-wave-state-v1`**; field manifest [`configs/framework/WAVE_STATE_FIELDS.json`](../../configs/framework/WAVE_STATE_FIELDS.json)):

| Field | Meaning |
|-------|---------|
| `depends_on` | Prior `wave_key` values (serial edge) |
| `parallel_group` | Lanes in same wave that may run together |
| `execution_mode` | `parallel` \| `serial` |
| `lanes[].scope_paths` | Disjoint write scopes per lane |

## Optional review

At most **one** read-only reviewer on `ROADMAP.md` → compact `lane-notes/` only (no mandatory RFV).

## Next

`/implementx` — executes **all waves** in one breath (see `skills/implementx/SKILL.md`).
