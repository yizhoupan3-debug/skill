# RFC: Merge continuity append streams (P2)

| Status | Draft (documentation only — no implementation in `continuity-evidence-subtraction-2026-05`) |
|--------|---------------------------------------------------------------------------------------------|

## Problem

Three parallel append paths increase lock surface and operator confusion:

- `EVIDENCE_INDEX.json` — verification ledger (L1 exit codes)
- `TRACE_EVENTS.jsonl` — process diagnostics
- `STEP_LEDGER.jsonl` — step recovery

## Proposal (future)

Single append-only stream with typed `kind` rows; `framework snapshot` and digest read models migrate to filtered views.

## Non-goals (this RFC)

- No schema change in current execute waves.
- No removal of `REVIEW_GATE` or RFV machinery.

## Prerequisites before implementation

1. Consumer inventory (`framework snapshot`, trace replay, step-ledger CLI).
2. Lock-order doc update (`docs/hook_lock_order.md`).
3. Dual-write migration window with `schema-drift` baseline.

See [`harness_policy_map.md`](../harness_policy_map.md) item #2.
