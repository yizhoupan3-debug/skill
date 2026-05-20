# Official GSD vs Repo-Native Harness

**Upstream**: [gsd-build/get-shit-done](https://github.com/gsd-build/get-shit-done) — canonical semantics in [docs/FEATURES.md](https://github.com/gsd-build/get-shit-done/blob/main/docs/FEATURES.md).

**Local harness**: `skills/gsd/*` + `artifacts/current/<task_id>/` + router-rs hooks. This document is the **augment-only** contract: we add host integration, RFV, evidence, and multi-host routing; we do **not** replace upstream phase meaning.

## Official core flow (per phase)

| Order | Official command | Purpose | Key artifacts (`.planning/`) |
|------:|------------------|---------|------------------------------|
| 0 | `/gsd-new-project` | Questions → parallel research → requirements + **ROADMAP** + **PROJECT**; **user approval** before proceed; no re-init if `PROJECT.md` exists | `PROJECT.md`, `REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md` |
| 1 | `/gsd-discuss-phase [N]` | Gray-area decisions **before** planning for phase N | `{phase}-CONTEXT.md` |
| 1.5 | `/gsd-ui-phase [N]` | Optional UI/UX spec before plan | UI spec files |
| 2 | `/gsd-plan-phase [N]` | Research → **PLAN.md** (XML tasks) → plan checker loop (≤3) | `{phase}-PLAN.md`, checker reports |
| 3 | `/gsd-execute-phase [N]` | Wave-parallel executors, atomic commits, SUMMARY | `{phase}-*-SUMMARY.md`, commits |
| 4 | `/gsd-verify-work [N]` | User acceptance / UAT | `{phase}-UAT.md` |
| 5 | `/gsd-ship` | PR bridge after verification | PR + STATE updates |

Official **discuss → plan → execute → verify** is **per phase**, after project init.

## Local harness mapping (augment)

| Official intent | Local command | Local artifacts | Augmentation notes |
|-----------------|---------------|-----------------|-------------------|
| Project init | `/gsd-new-project` | `REQUIREMENTS.md`, `ARCHITECTURE.md`, `RISK_REGISTER.md`, `GOAL_STATE.json` (draft) | 5-layer funnel + mandatory RFV; ROADMAP may be deferred to plan-phase |
| Phase context / ADR | `/gsd-discuss-phase` | `STATE.md`, `ADR-*.md` | Harness-level ADR lane; map mentally to `{phase}-CONTEXT.md` when you adopt per-phase numbering |
| Phase planning | `/gsd-plan-phase` | `ROADMAP.md`, `WAVE_STATE.json` | Wave/orchestrator model; upstream `PLAN.md` XML can be mirrored later without dropping ROADMAP |
| Execution | `/gsd-execute-phase` | `WAVE_STATE.json`, code changes, `EVIDENCE_INDEX.json` | Multi-agent waves + stdio goal drive |
| UAT / verify | `/gsd-verify-work` | `EVIDENCE_INDEX.json` | Evidence-driven checklist |
| Ship | `/gsd-ship` | `SHIPPING_STATE.json` | Delivery gate + adversarial review |

**Path mapping**: upstream `.planning/` ↔ local `artifacts/current/<task_id>/`. Do not require `.planning/` in this repo unless a project explicitly adopts upstream layout.

## Hard boundary (local enforcement)

See `skills/gsd/shared/phase-boundaries.md`:

| Zone | Commands | Product code |
|------|----------|--------------|
| Pre-execution | `new-project`, `discuss-phase`, `plan-phase` | **READ-ONLY** |
| Execution+ | `execute-phase`, `verify-work`, `ship` | Allowed per ROADMAP/gates |

**Hooks** (`hook_common.rs`): only execution-zone `/gsd-execute-phase`, `/gsd-verify-work`, `/gsd-ship` arm `goal_required`; pre-execution injects `GSD_PRE_EXECUTION_HOOK_NUDGE` on prompt submit.

## Known gaps (augment backlog, not forks)

| Gap | Official | Local today | Recommended augment |
|-----|----------|-------------|---------------------|
| Discuss timing | Per-phase, before plan | Harness-level command | Document phase id in ADR; optional `[N]` arg in skill |
| Plan artifact | `PLAN.md` + 8-dim checker | `ROADMAP.md` + waves | Add optional `PLAN.md` export per phase without removing ROADMAP |
| Project file | `PROJECT.md` | `REQUIREMENTS.md` + `ARCHITECTURE.md` | Cross-link sections; optional `PROJECT.md` stub |
| UI phase | `/gsd-ui-phase` | Not exposed | Add skill when UI-heavy phases are common |
| Init ROADMAP | Created at new-project | Often at plan-phase | Accept either if user approved; block execute without roadmap |
| Goal drive | N/A (orchestrator-owned) | `framework_autopilot_goal` stdio | `drive_until_done: false` until execute-phase |

## Default lifecycle string (all hosts)

`/gsd-new-project` → `/gsd-discuss-phase` → `/gsd-plan-phase` → `/gsd-execute-phase` → `/gsd-verify-work` → `/gsd-ship`

Repeat discuss → plan → execute → verify **per phase** when following upstream literally; the string above is the harness-level spine.

## Anti-patterns (root cause: “early coding”)

1. Treating `/gsd-new-project` as execution-zone goal drive — **fixed in hooks**: pre-exec no longer sets `goal_required`.
2. `drive_until_done: true` in pre-exec stdio — **forbidden** in `phase-boundaries.md` and `new-project/SKILL.md`.
3. Vague user prompt + default GSD narrative without explicit phase — agent jumps to implementation. **Mitigation**: require explicit `/gsd-execute-phase` before product edits; hooks nudge on pre-exec only.
4. RFV “fix” round touching `src/` during new-project — **forbidden**; doc-only fix scope.

## References

- Upstream features: https://github.com/gsd-build/get-shit-done/blob/main/docs/FEATURES.md
- Local phase contract: `skills/gsd/shared/phase-boundaries.md`
- Stdio (phase-aware): `skills/gsd/shared/stdio-contracts.md`
