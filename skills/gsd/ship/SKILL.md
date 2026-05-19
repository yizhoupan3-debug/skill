---
name: gsd-ship
description: |
  Final delivery gate with adversarial review and multi-worktree merge.
  Use when ready to ship. Operator verification checklist lives in gsd-verify-work;
  ship adds git/worktree/RFV/closeout gates only.
routing_layer: L1
routing_owner: owner
routing_gate: evidence
routing_gate_evidence: "WAVE_STATE.json global_status=completed, EVIDENCE_INDEX has entries"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /gsd-ship
  - gsd ship
  - deliver
  - release
  - merge to main
metadata:
  version: "0.2.0"
  platforms: [supported]
  tags: [gsd, delivery, ship, adversarial-review, worktree]
---

# gsd-ship

Final delivery gate: **verify first**, then git/worktree merge, then RFV adversarial closeout.

## Philosophy

Ship only when verify-work evidence is green, git is clean, worktree review passed, and RFV loop closed.

## Pre-Conditions

1. `/gsd-verify-work` completed for this task (`VERIFY_REPORT.md` or equivalent PASS)
2. `WAVE_STATE.json` → `global_status=completed`
3. `EVIDENCE_INDEX.json` has `artifacts[]` with at least one successful verification row (`exit_code: 0`)
4. Task directory writable

## Operator verification (canonical — do not duplicate here)

**Single source of truth:** [../verify-work/SKILL.md](../verify-work/SKILL.md) and [../verify-work/evidence-protocol.md](../verify-work/evidence-protocol.md).

Before ship you MUST:

1. Re-run or confirm verify-work commands for **this repo** (see task `GOAL_STATE.validation_commands` or `ROADMAP.md` §6).
2. Ensure every command is logged to `artifacts/current/<task_id>/EVIDENCE_INDEX.json` (`entries` + `artifacts`).

**Harness repo default verify bundle** (example — prefer task ROADMAP):

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml
cargo test --test policy_contracts
! rg -n '"/autopilot"' skills/SKILL_ROUTING_RUNTIME.json configs/framework/RUNTIME_REGISTRY.json
```

Do **not** copy the full test/clippy/coverage matrix into this file; extend verify-work if new gates are needed.

## Ship-only gates

### Gate A: Git clean

```bash
git status --short --branch
git diff --stat
```

Branch naming and commit hygiene per root `AGENTS.md` Git section.

### Gate B: Multi-worktree review

See [worktree-flow.md](worktree-flow.md).

```bash
git worktree add ../worktree-review-<task_id> <branch-name>
cd ../worktree-review-<task_id>
# Re-run verify bundle from verify-work (not a second checklist here)
git checkout main && git merge --no-ff <branch-name>
git worktree remove ../worktree-review-<task_id>
```

### Gate C: Adversarial RFV (mandatory before merge)

| Lens | Focus |
|------|-------|
| Correctness | Logic, edge cases, error handling |
| Security | Injection, auth, sensitive data |
| Performance | Complexity, resource leaks |
| Maintainability | Readability, coupling, test coverage |
| Reliability | Error recovery, idempotency |
| Supply Chain | Dependencies, licenses |

```bash
printf '%s\n' '{"id":1,"op":"framework_rfv_loop","payload":{"operation":"start","repo_root":"<path>","goal":"Adversarial review before ship","max_rounds":3,"allow_external_research":true,"review_scope":"<changed-files>","fix_scope":"<scope>","verify_commands":["cargo test --manifest-path scripts/router-rs/Cargo.toml"],"stop_when":["all lenses covered","P0/P1 findings zero","3 rounds complete"]}}' | router-rs --stdio-json
```

## SHIPPING_STATE.json

Track **ship-only** gates (`git_clean`, `worktree_review`, `rfv`); mirror verify pass/fail from `EVIDENCE_INDEX` / `VERIFY_REPORT.md` — do not duplicate verify command lists.

```json
{
  "schema_version": "gsd-shipping-state-v1",
  "task_id": "<task_id>",
  "verify_reference": "artifacts/current/<task_id>/VERIFY_REPORT.md",
  "gates": {
    "verify_work": { "status": "pass", "evidence_index": "artifacts/current/<task_id>/EVIDENCE_INDEX.json" },
    "git_clean": { "status": "pending" },
    "worktree_review": { "status": "pending" },
    "rfv_adversarial": { "status": "pending" }
  },
  "overall_status": "pending"
}
```

## Ship complete

When verify-work PASS + gates A–C green:

```bash
printf '%s\n' '{"id":2,"op":"framework_autopilot_goal","payload":{"operation":"complete","repo_root":"<path>","task_id":"<task_id>"}}' | router-rs --stdio-json
printf '%s\n' '{"id":3,"op":"framework_rfv_loop","payload":{"operation":"close","repo_root":"<path>"}}' | router-rs --stdio-json
```

Write `SESSION_SUMMARY.md` + final `EVIDENCE_INDEX` row with `kind: gsd-ship`.

## Anti-Patterns

- Duplicating verify-work's cargo/clippy/coverage tables in ship
- Merging without verify evidence on disk
- Skipping RFV adversarial loop
- Claiming ship complete without `framework_autopilot_goal complete`

## Delivery checklist (ship-only)

```
□ verify-work PASS (see VERIFY_REPORT.md)
□ Git clean
□ Worktree review + merge
□ RFV 3 rounds, P0/P1 zero
□ SHIPPING_STATE.json updated
□ GOAL_STATE completed via stdio
□ SESSION_SUMMARY written
```
