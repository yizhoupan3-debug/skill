---
name: planx
description: |
  Personal lifecycle — plan/roadmap (doc-only). Writes ROADMAP.md and WAVE_STATE.json with explicit serial/parallel DAG.
  Use when user explicitly requests plan after /discussx. Does not mutate product code.
routing_layer: L1
routing_owner: owner
routing_gate: evidence
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

## Outputs

- `ROADMAP.md` — phases, exit criteria, verification commands
- `WAVE_STATE.json` — each wave: `parallel_group`, `depends_on`, `execution_mode`, `lanes[]`

Topology fields (extend `gsd-wave-state-v1` in place):

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
