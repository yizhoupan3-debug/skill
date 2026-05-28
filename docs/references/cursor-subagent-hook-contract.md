# Cursor subagent hook contract (v1)

Machine-readable source: [`configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json`](../../configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json).

## Events

| Event | Role |
|-------|------|
| `subagentStart` | Open a review cycle when lane + independent fork qualify |
| `subagentStop` | Settle one matching cycle key |
| `postToolUse` | May settle when host returns Task without `subagentStop` |

## Cycle keys

- **`id:<stable_id>`** — from `subagent_id` / `subagentId` / `agent_id` / `task_id` / `run_id` family; required for `review-lite` vec.
- **`id:<legacy>`** — bare JSON field `"id"` only → **strict** multiset (not lite).
- **`lane:<type>`** — strict multiset only; **never** `review-lite`.

## `fork_context`

Independent reviewer evidence when parsed as logical **`false`** (JSON boolean `false`, integer `0`, or string `"false"`). Explicit **`true`** never counts.

## Modes

| Mode | Env | Pending |
|------|-----|---------|
| **strict** (default) | unset or `strict` | `review_subagent_pending_cycle_keys` multiset |
| **review-lite** | `ROUTER_RS_CURSOR_REVIEW_GATE_MODE=lite` | `review_lite_pending_cycle_keys` for `id:`; no multiset for `id:` |

Fallback: lite + non-`id:` key → strict path (`review_lite_fallback_strict` log).

## Satisfaction (Stop)

- **strict** and **lite**: `phase >= 3` and **both** `review_lite_pending_cycle_keys` and `review_subagent_pending_cycle_keys` empty (orphan lite pending blocks Stop even under strict env).

Implementation: `review_subagent_evidence_satisfied` in `cursor_hooks/handlers.rs`.
