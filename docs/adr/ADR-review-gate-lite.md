---
last_verified: "2026-06-09"
depends_on:
  - ../harness_architecture/index.md
  - ../host_adapter_contract.md
---

# ADR: Cursor REVIEW_GATE strict vs review-lite

## Status

Accepted (2026-05-28).

**2026-06 supersede (operator):** Cursor pending vecs (`review_subagent_pending_cycle_keys`, `review_lite_pending_cycle_keys`) are **telemetry / phase-bump only**—they do **not** define Stop clear-gate. Clear gate (all hook hosts, Claude canonical): `independent_reviewer_seen` **or** override per [`host_adapter_contract.md`](../host_adapter_contract.md) §0.1; Stop `REVIEW_GATE` is advisory-only globally.

## Context

Cursor `REVIEW_GATE` uses a multiset (`review_subagent_pending_cycle_keys`) to survive dual events (`subagentStart` + `PostToolUse`), missing `subagentStop`, and parallel reviewers. That is correct but heavy. Host payloads with stable `subagent_id` allow a lighter counter path.

**Naming**: operator docs say **review-lite**; JSON/Rust use **`review_lite`** (`modes.review_lite`, `review_lite_pending_cycle_keys`). Machine contract: [`configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json`](../../configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json).

## Decision

1. **`ROUTER_RS_CURSOR_REVIEW_GATE_MODE`**: `strict` (default) | `lite` (**review-lite**). **Process-global env** — do not flip mid-session when multiple Cursor sessions share one `router-rs` process. Pending vecs track cycle hygiene for **phase-bump / operator nudge** only (see Status supersede); they do **not** define Stop clear-gate.
2. **strict**: multiset semantics for new non-lite keys; orphan `review_lite_pending_cycle_keys` after env switch may still trigger advisory nudge until settled — **not** a hard Stop block.
3. **lite**:
   - Only **stable** subagent id fields (`subagent_id` family) use `review_lite_pending_cycle_keys`; bare JSON `"id"` → strict multiset (`review_lite_reject_generic_id`).
   - Qualifying `review_kind` + `review_independent_fork`.
   - Non-`id:` keys → fallback strict (`review_lite_fallback_strict`).
4. **Stop clear-gate** (both modes, Claude canonical): `review_gate_satisfied` ⇔ override **or** `independent_reviewer_seen`. **Advisory nudge** (Cursor): `review_stop_followup_needed` may inject `REVIEW_GATE incomplete` when pending vecs unsettled or canonical gate unsatisfied — advisory-only, never hard-block Stop.

```rust
fn review_stop_followup_needed(state: &ReviewGateState) -> bool {
    if review_hard_armed(state) && !pending_both_empty(state) {
        return true;
    }
    review_gate_blocks_stop(ReviewGateFacts { /* independent_reviewer_seen */ })
}
```

5. **Phase bump** on cycle settle tracks wave-2 telemetry; phase alone does **not** clear Stop (compact bump still requires live cycle evidence).

## Env matrix (fixtures under `tests/fixtures/review_gate/env_matrix/`)

| case | MODE | FORK infer | cap | expect |
|------|------|------------|-----|--------|
| em-01 | strict | explicit on | default | multiset |
| em-02 | lite | explicit on | default | id pending vec |
| em-03 | lite | unset/off | default | strict fallback / block |
| em-04 | strict | unset/off | 2 | cap refused |
| em-05 | lite | explicit on | 2 | lite id AtCap + `review_pending_cap_refused` |
| em-06 | strict | explicit on | default | parallel two ids |

## Consequences

- Dual code paths until Cursor stdin is versioned and lite becomes default (O1 sampling gate).
- Tests: `review_lite_*` + existing `review_gate_*` must stay green under unset env.
