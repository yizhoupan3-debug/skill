---
name: discussx
description: |
  Personal lifecycle — discuss/requirements (doc-only). Multi-round, user-gated depth; stay in discuss until explicit /planx.
  Use for /discussx or starting a task. Does not mutate product code.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: true
disable-model-invocation: true
trigger_hints:
  - /discussx
  - discussx
metadata:
  version: "0.2.1"
  platforms: [supported]
  tags: [my-lifecycle, discuss, requirements]
---

# discussx

**Zone**: pre-execution · **profile**: `my-light` · **no product code**

## Stay in discuss (HARD)

- **Default**: remain in `/discussx` until the user **explicitly** requests plan (`/planx`, 「进 plan」「可以计划了」等).
- **Forbidden**: auto-advance narrative（默认收尾写「接下来请 /planx」）；把首轮回答当作「需求已齐」；在输入很少时催促进 plan.
- **Each user turn**: treat as **additive** — merge new angles, constraints, and refinements into disk artifacts; do not discard prior context.
- **When user adds constraints**: update disk (`REQUIREMENTS.md`, `DECISIONS.md`, `OPEN_QUESTIONS.md`) **before** visible reply.

## Discussion depth

Run **multi-round** requirement shaping, not one-shot Q&A:

| Focus | Capture |
|-------|---------|
| Goal & success | What done looks like; measurable exit |
| Non-goals & scope edges | What we will not do |
| Constraints | Time, stack, policy, compatibility, style |
| Existing owners & delta | What owns this today; minimal change |
| Risks & open questions | Risks → `REQUIREMENTS.md`; unresolved items → `OPEN_QUESTIONS.md` |
| Validation | How we will prove it works |

- **Minimum**: at least **two** visible synthesis rounds before *offering*（非催促）plan readiness — unless user asks to plan early.
- Optional parallel **read-only** explore lanes when repo context is needed → `lane-notes/<lane_id>.md` (≤15 lines each). Main thread reads paths only.

## Main-thread contract (HARD)

Visible chat **structure** (adapt depth to turn; omit empty sections):

1. **Synthesis** — 3–8 bullets: current understanding (goal, scope, constraints locked so far, still open). Not a full `REQUIREMENTS.md` paste.
2. **Decision** — blocking choices only, one line each (skip if none)
3. **Recommend** — default option + one-line why (per open decision)
4. **Your turn** — invite more angles/constraints; note `/planx` is **user-gated** (do not pressure)

**Forbidden in chat**: full `REQUIREMENTS.md` / `DECISIONS.md` / `OPEN_QUESTIONS.md` paste; exploration dumps; subagent transcripts; 「可以进 plan 了」unless user asked for a readiness check.

## Disk outputs

| File | Purpose |
|------|---------|
| `artifacts/current/<task_id>/REQUIREMENTS.md` | Living requirements — **update every round** |
| `artifacts/current/<task_id>/DECISIONS.md` | Locked choices only |
| `artifacts/current/<task_id>/OPEN_QUESTIONS.md` | **Unresolved items only** — separate file (required once discuss starts) |
| `artifacts/current/<task_id>/GOAL_STATE.json` | Via `framework_goal_drive` stdio — 遵循 [../my-lifecycle-common/GOAL_STATE_CONTRACT.md](../my-lifecycle-common/GOAL_STATE_CONTRACT.md) 中的 GOAL_STATE 写入规范 |

### GOAL_STATE writes

遵循 [../my-lifecycle-common/GOAL_STATE_CONTRACT.md](../my-lifecycle-common/GOAL_STATE_CONTRACT.md) 中的 GOAL_STATE 写入规范。

### OPEN_QUESTIONS.md (HARD)

- **Always** a separate file; do **not** fold open questions into `REQUIREMENTS.md`.
- One item per bullet; tag deferral: `now` | `plan` | `implement`.
- When resolved: record outcome in `DECISIONS.md` or `REQUIREMENTS.md`, then **remove** from `OPEN_QUESTIONS.md`.
- Empty file is OK (`# Open questions` + none yet).

### REQUIREMENTS.md starter headings

Use these `##` sections (planx `ROADMAP.md` must mirror headings per schema-drift):

- `## Goal`
- `## Success criteria`
- `## Non-goals`
- `## Constraints`
- `## Context & existing owners`
- `## Risks`

Initialize on first turn; append/refine each round. Open questions live in `OPEN_QUESTIONS.md`, not here.

## Exit to plan (user-gated)

Only when user explicitly invokes `/planx` or clear plan intent → hand off to `skills/planx/SKILL.md` (may set `GOAL_STATE.status` to `planned`).

## Next

`/planx` — wave DAG in `WAVE_STATE.json`. **Not** the default next step after discuss.
