---
last_verified: "2026-06-09"
depends_on:
  - ../harness_architecture/index.md
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

Independent reviewer evidence when parsed as logical **`false`** only ([`review_independent_fork`](../../core/core-policy/src/review_gate_engine.rs) / `fork_context_from_values`):

| Form | Accepted as `false` |
|------|---------------------|
| JSON boolean | `false` |
| JSON integer | `0` |
| JSON string (trim + ASCII lower) | `"false"`, `"0"`, `"no"`, `"n"` |

**`true` / shared fork** (boolean `true`, integer `1`, strings `"true"` / `"1"` / `"yes"` / `"y"`, or explicit `fork_context: true`) **never** counts. Field **missing** is not `false` in the parser itself; Cursor may infer `false` on deep lanes only when `ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` or legacy `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` is **explicitly enabled** (unset = off).

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

**Claude canonical (2026-06)**: Stop clearance = `review_gate_satisfied` ⇔ `review_override` **or** `independent_reviewer_seen` (`reviewer_lanes` + `fork_context=false` via PostTool/subagentStart). Same rule for Claude Code / Codex / Cursor hook hosts. See [`host_adapter_contract.md`](../host_adapter_contract.md) §0.1.

**Advisory-only**: `review_gate_blocks_stop` only decides whether to project **`router-rs REVIEW_GATE incomplete`** nudge; it **does not** hard-block Stop (`permission: deny` / `decision: block`).

**Multiset / review-lite (Cursor telemetry only)**: `review_subagent_pending_cycle_keys` and `review_lite_pending_cycle_keys` track subagent cycle hygiene, phase bump, and operator hints—they are **not** separate Stop clearance conditions. Unsettled pending may still trigger **advisory** nudges for operator visibility; clearance still requires Claude-canonical evidence. Orphan lite pending behavior unchanged for telemetry. **Bare** `phase≥2` without `independent_reviewer_seen` does not clear the gate (`wave2_requires_live_evidence` in JSON).

Implementation: `core-policy::review_gate_engine` (canonical) + `cursor_hooks/handlers.rs` (transport). ADR: [`docs/adr/ADR-review-gate-lite.md`](../adr/ADR-review-gate-lite.md).

## Stale recovery

When open subagent count drops to zero, pending keys older than **`ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS`** may be pruned (see JSON `stale_recovery` and [`docs/harness_architecture/03-hook-and-switches.md`](../harness_architecture/03-hook-and-switches.md) env 表). Set `0`/`false`/`off`/`no` to disable auto stale hygiene.
