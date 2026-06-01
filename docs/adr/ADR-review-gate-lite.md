---
last_verified: "2026-06-02"
depends_on:
  - ../harness_architecture.md
---

# ADR: Cursor REVIEW_GATE strict vs review-lite

## Status

Accepted (2026-05-28).

## Context

Cursor `REVIEW_GATE` uses a multiset (`review_subagent_pending_cycle_keys`) to survive dual events (`subagentStart` + `PostToolUse`), missing `subagentStop`, and parallel reviewers. That is correct but heavy. Host payloads with stable `subagent_id` allow a lighter counter path.

**Naming**: operator docs say **review-lite**; JSON/Rust use **`review_lite`** (`modes.review_lite`, `review_lite_pending_cycle_keys`). Machine contract: [`configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json`](../../configs/framework/CURSOR_SUBAGENT_HOOK_CONTRACT.json).

## Decision

1. **`ROUTER_RS_CURSOR_REVIEW_GATE_MODE`**: `strict` (default) | `lite` (**review-lite**). **Process-global env** — do not flip mid-session when multiple Cursor sessions share one `router-rs` process; satisfaction always requires both pending vecs empty.
2. **strict**: multiset semantics for new non-lite keys; **Stop** still requires `review_lite_pending_cycle_keys` empty (orphan lite pending blocks after env switch).
3. **lite**:
   - Only **stable** subagent id fields (`subagent_id` family) use `review_lite_pending_cycle_keys`; bare JSON `"id"` → strict multiset (`review_lite_reject_generic_id`).
   - Qualifying `review_kind` + `cursor_review_independent_fork`.
   - Non-`id:` keys → fallback strict (`review_lite_fallback_strict`).
4. **Satisfaction** (both modes):

```rust
fn review_subagent_evidence_satisfied(state: &ReviewGateState) -> bool {
    state.phase >= 3
        && state.review_lite_pending_cycle_keys.is_empty()
        && state.review_subagent_pending_cycle_keys.is_empty()
}
```

5. **Phase 3 bump** on cycle settle only when **both** pending vecs are empty (id settle must not reach phase 3 while `lane:` fallback remains).

## Env matrix (fixtures under `tests/fixtures/review_gate/env_matrix/`)

| case | MODE | FORK infer | cap | expect |
|------|------|------------|-----|--------|
| em-01 | strict | on | default | multiset |
| em-02 | lite | on | default | id pending vec |
| em-03 | lite | off | default | strict fallback / block |
| em-04 | strict | off | 2 | cap refused |
| em-05 | lite | on | 2 | lite id AtCap + `review_pending_cap_refused` |
| em-06 | strict | on | default | parallel two ids |

## Consequences

- Dual code paths until Cursor stdin is versioned and lite becomes default (O1 sampling gate).
- Tests: `review_lite_*` + existing `review_gate_*` must stay green under unset env.
