# ADR: Cursor REVIEW_GATE strict vs review-lite

## Status

Accepted (2026-05-28).

## Context

Cursor `REVIEW_GATE` uses a multiset (`review_subagent_pending_cycle_keys`) to survive dual events (`subagentStart` + `PostToolUse`), missing `subagentStop`, and parallel reviewers. That is correct but heavy. Host payloads with stable `subagent_id` allow a lighter counter path.

## Decision

1. **`ROUTER_RS_CURSOR_REVIEW_GATE_MODE`**: `strict` (default) | `lite`.
2. **strict**: unchanged multiset semantics.
3. **lite**:
   - Only cycles with `id:` keys and qualifying `review_kind` + `cursor_review_independent_fork`.
   - Track open `id:` cycles in `review_lite_pending_cycle_keys` (Vec); **never** push multiset for `id:` keys.
   - Non-`id:` keys → fallback strict (`review_lite_fallback_strict`).
4. **Satisfaction**:

```rust
fn review_subagent_evidence_satisfied(state: &ReviewGateState) -> bool {
    if state.phase < 3 { return false; }
    match cursor_review_gate_mode() {
        CursorReviewGateMode::Lite => {
            state.review_lite_pending_cycle_keys.is_empty()
                && state.review_subagent_pending_cycle_keys.is_empty()
        }
        CursorReviewGateMode::Strict => state.review_subagent_pending_cycle_keys.is_empty(),
    }
}
```

## Env matrix (fixtures under `tests/fixtures/review_gate/env_matrix/`)

| case | MODE | FORK infer | cap | expect |
|------|------|------------|-----|--------|
| em-01 | strict | on | default | multiset |
| em-02 | lite | on | default | counter |
| em-03 | lite | off | default | strict fallback / block |
| em-04 | strict | off | 2 | cap refused |
| em-05 | lite | on | 2 | lite cap via strict fallback on lane |
| em-06 | strict | on | default | parallel two ids |

## Consequences

- Dual code paths until Cursor stdin is versioned and lite becomes default (O1 sampling gate).
- Tests: `review_lite_*` + existing `review_gate_*` must stay green under unset env.
