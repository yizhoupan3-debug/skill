# GSD Phase Boundaries (Hard Contract)

**Version**: 0.2.0  
**Applies to**: all GSD commands and subagents

## Lifecycle Split

| Zone | Commands | May mutate product code? | May run fix/build/test on repo? |
|------|----------|---------------------------|----------------------------------|
| **Pre-execution** | `/gsd-new-project`, `/gsd-plan-phase`, `/gsd-discuss-phase` | **NO** | **NO** |
| **Execution+** | `/gsd-execute-phase`, `/gsd-verify-work`, `/gsd-ship` | YES | YES (per ROADMAP / gates) |

Until `/gsd-execute-phase` starts, treat the repository as **read-only for implementation**.

## Pre-Execution: Allowed Writes

Only write under `artifacts/current/<task_id>/` (and explicitly listed doc paths):

| Artifact | Phase |
|----------|-------|
| REQUIREMENTS.md | new-project |
| ARCHITECTURE.md | new-project |
| RISK_REGISTER.md | new-project |
| GOAL_STATE.json | new-project (contract only; see below) |
| EVIDENCE_INDEX.json | new-project (exploration / review evidence) |
| ROADMAP.md | plan-phase |
| WAVE_STATE.json | plan-phase (`global_status`: `planned` only) |
| ADR-*.md, STATE.md | discuss-phase |
| RFV_LOOP_STATE.json | any pre-execution RFV |

Optional continuity: `SESSION_SUMMARY.md` in the same task directory.

## Pre-Execution: Forbidden

- Editing `src/`, `tests/`, `lib/`, `apps/`, `packages/`, migrations, CI workflows, lockfiles, or project config used by builds
- `cargo fix`, `npm run fix`, format-on-save commits, dependency bumps, scaffold generation
- Spawning implementation / fix-CI / best-of-n-runner agents
- RFV **code** fix rounds (`fix_scope` pointing at product source)
- Starting autopilot with `drive_until_done: true`
- Running verification commands from ROADMAP **for the purpose of fixing the repo** (planning may *name* commands; do not execute them until execute-phase)

## RFV in Pre-Execution

RFV loop semantics before execution:

1. **Review**: read-only (`code-review-deep`, explore subagents with `readonly: true`)
2. **Fix**: revise **documents only** (REQUIREMENTS.md, ARCHITECTURE.md, ADR, ROADMAP, risk register)
3. **Verify**: re-read updated docs; checklist pass/fail — **not** `cargo test` / `cargo clippy` on product code

Record `fix_summary` as documentation changes, e.g. "Updated REQUIREMENTS.md §3 constraints".

## GOAL_STATE During new-project

`GOAL_STATE.json` is a **planning contract**, not an execution trigger:

- `status`: `planned` or `draft` until execute-phase sets `running`
- `drive_until_done`: **false** until execute-phase
- `validation_commands`: listed for later; **do not run** during new-project / plan-phase

## Enforcement Checklist (agent self-check)

Before any tool call in pre-execution:

- [ ] Target path is under `artifacts/current/<task_id>/` or an allowed doc
- [ ] Not a StrReplace/Write on product source
- [ ] Subagent `readonly: true` if reviewing repo
- [ ] RFV round closes with doc diff, not code diff

If user asks to "fix the build" during new-project or plan-phase: **refuse implementation**, capture as ROADMAP task or risk, proceed with docs.

## Entry to Execution

`/gsd-execute-phase` may start only when:

1. Core docs exist (REQUIREMENTS.md, ROADMAP.md, WAVE_STATE.json planned)
2. User or workflow explicitly invokes execute-phase
3. GOAL_STATE may transition to `running` and `drive_until_done: true` here
