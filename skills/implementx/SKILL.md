---
name: implementx
description: |
  Personal lifecycle — execute ALL waves in one breath. Main thread schedules lanes only; subagents write compact lane-notes.
  Sets drive_until_done true. REVIEW_GATE hard block off under lifecycle_profile my-light.
  Use for /implementx after /planx.
routing_layer: L1
routing_owner: owner
routing_gate: evidence
routing_gate_evidence: "ROADMAP.md and WAVE_STATE.json exist"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /implementx
  - implementx
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [my-lifecycle, implement, multi-agent, one-breath]
---

# implementx

**Zone**: execution+ · **profile**: `my-light`

## One-breath all-waves (HARD)

When invoked, run **every wave** in `WAVE_STATE.json` from current `wave_id` through the last wave **without** stopping at wave boundaries to ask the user.

| CAN continue (no user ping) | MUST stop |
|----------------------------|-----------|
| Next lane in parallel group | Scope/requirement error |
| Next wave after merge checkpoint | P0 security |
| Verification failed, fix obvious | External dependency down |
| Retry with new evidence | User said stop |

**Do not** treat “Wave N complete” as a pause point.

## Main thread (scheduler only)

1. Read `WAVE_STATE.json` + `ROADMAP.md`
2. For each wave (in order): spawn all lanes in `parallel_group` when `execution_mode=parallel`
3. Merge: read `lane-notes/<lane_id>.md` only; chat ≤3 bullets + paths
4. Update wave `status` → `completed`; `current_wave`++; checkpoint `EVIDENCE_INDEX`
5. After final wave → suggest `/verifyx` (or auto-chain if user asked full pipeline)

Target: coordinator visible content ≤35% of turn.

## Subagent contract

```json
{
  "lane_id": "w3-lane-cursor",
  "scope_paths": ["scripts/router-rs/src/cursor_hooks/"],
  "output_path": "artifacts/current/<task_id>/lane-notes/w3-lane-cursor.md",
  "max_lines": 15,
  "forbidden": ["paste full transcript to main chat"]
}
```

Prefer `fork_context=false`, disjoint paths, 3–5 parallel lanes when plan allows.

## GOAL_STATE on start

显式 stdio 启动（**无** Stop `GOAL_CONTINUE` hook 注入，2026-05 连续性拔除）：

```bash
# status=running, drive_until_done=true, lifecycle_profile=my-light
printf '%s\n' '{"id":1,"op":"framework_goal_drive","payload":{"operation":"start","repo_root":"<repo>","task_id":"<task_id>","goal":"<from GOAL_STATE>","drive_until_done":true,"status":"running","lifecycle_profile":"my-light"}}' | router-rs --stdio-json
```

## Next

`/verifyx` — evidence + ship in one command.
