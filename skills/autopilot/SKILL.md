---
name: autopilot
description: |
  RETIRED: `/autopilot` removed. Use GSD execution: `/gsd-execute-phase` with `GOAL_STATE.json`
  (`framework_autopilot_goal` stdio). Archived copy: `skills/_archived/autopilot/SKILL.md`.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P9
session_start: n/a
user-invocable: false
disable-model-invocation: true
metadata:
  version: "2.0.0-retired"
  status: retired
  replacement: /gsd-execute-phase
---

# autopilot (retired)

**`/autopilot` is no longer a framework entrypoint.** Continuous execution uses the GSD spine:

1. `/gsd-plan-phase` (optional) → `ROADMAP.md`
2. **`/gsd-execute-phase`** → sets `GOAL_STATE` `drive_until_done` and runs implementation
3. `/gsd-verify-work` → `/gsd-ship`

Goal persistence: `framework_autopilot_goal` → `artifacts/current/<task_id>/GOAL_STATE.json`.

Stop continuation hook: **`router-rs GSD_GOAL_CONTINUE`** (not `AUTOPILOT_DRIVE`).

See `skills/gsd/SKILL.md` and `artifacts/current/harness-minimal-gsd/ADR-001-autopilot-retirement.md`.
