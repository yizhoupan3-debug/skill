---
last_verified: "2026-06-16"
depends_on:
  - ../spec.md
---

# Cursor subagent hook contract (v1)

Machine-readable source: [`configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json`](../../configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json).

## Events

| Event | Role |
|-------|------|
| `subagentStart` | Open a review cycle when lane + independent fork qualify |
| `subagentStop` | Settle one matching cycle key |
| `postToolUse` | May settle when host returns Task without `subagentStop` |

## Cycle keys

- **`id:<stable_id>`** — from `subagent_id` / `subagentId` / `agent_id` / `task_id` / `run_id` family; required for **review-lite** vec.
- **`id:<legacy>`** — bare JSON field `"id"` only → **strict** multiset (not lite).
- **`lane:<type>`** — strict multiset only; **never** review-lite.

## `fork_context`

Independent reviewer evidence when parsed as logical **`false`** only (`cursor_review_independent_fork` / `fork_context_from_values` in `review_gate_engine.rs`):

| Form | Accepted as `false` |
|------|---------------------|
| JSON boolean | `false` |
| JSON integer | `0` |
| JSON string (trim + ASCII lower) | `"false"`, `"0"`, `"no"`, `"n"` |

**`true` / shared fork** (boolean `true`, integer `1`, strings `"true"` / `"1"` / `"yes"` / `"y"`, or explicit `fork_context: true`) **never** counts. Field **missing** is not `false` in the parser itself; Cursor may infer `false` on deep lanes when `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` is on (unset default).

## Naming: review-lite vs `review_lite`

| Surface | Spelling |
|---------|----------|
| Env / 文档模式名 | **`review-lite`** — `ROUTER_RS_CURSOR_REVIEW_GATE_MODE=lite` |
| JSON / Rust 字段 | **`review_lite`** — e.g. `modes.review_lite`, `review_lite_pending_cycle_keys` |

## Modes

| Mode | Env | Pending |
|------|-----|---------|
| **strict** (default) | unset or `strict` | `review_subagent_pending_cycle_keys` multiset |
| **review-lite** | `ROUTER_RS_CURSOR_REVIEW_GATE_MODE=lite` | `review_lite_pending_cycle_keys` for `id:`; no multiset for `id:` |

Fallback: lite + non-`id:` key → strict path (`review_lite_fallback_strict` log).

## Satisfaction (Stop)

`review_subagent_evidence_satisfied` requires **`phase >= 3`** and **both** pending structures empty (`review_lite_pending_cycle_keys` and `review_subagent_pending_cycle_keys`). Orphan lite pending blocks Stop even after switching back to strict env. **Bare** legacy `phase≥2` without live subagent settle is insufficient (`wave2_requires_live_evidence` in JSON).

Implementation: `review_subagent_evidence_satisfied` in `cursor_hooks/handlers.rs`. ADR: [`docs/adr/ADR-review-gate-lite.md`](../adr/ADR-review-gate-lite.md).

## Stale recovery

When open subagent count drops to zero, pending keys older than **`ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS`** may be pruned (see JSON `stale_recovery` and [`docs/spec.md`](../spec.md) env 表). Set `0`/`false`/`off`/`no` to disable auto stale hygiene.
