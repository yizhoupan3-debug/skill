---
name: discussx
description: |
  Personal lifecycle — discuss/requirements (doc-only). Main chat shows only decisions needed and recommended options.
  Use for /discussx or starting a task. Does not mutate product code.
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /discussx
  - discussx
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [my-lifecycle, discuss, requirements]
---

# discussx

**Zone**: pre-execution · **profile**: `my-light` · **no product code**

## Main-thread contract (HARD)

Visible chat **only**:

1. **Decision** — what you must choose (one line each)
2. **Recommend** — default option + one-line why
3. **Your call** — optional single question

**Forbidden in chat**: full `REQUIREMENTS.md` paste, exploration dumps, subagent transcripts.

## Disk outputs

| File | Purpose |
|------|---------|
| `artifacts/current/<task_id>/REQUIREMENTS.md` | Requirements |
| `artifacts/current/<task_id>/DECISIONS.md` | Locked choices |
| `artifacts/current/<task_id>/GOAL_STATE.json` | `lifecycle_profile: my-light`, `status: planned` |

## Subagents

Optional parallel **read-only** explore lanes → `lane-notes/<lane_id>.md` (≤15 lines). Main thread reads paths only.

## Next

`/planx` — wave DAG in `WAVE_STATE.json`.
