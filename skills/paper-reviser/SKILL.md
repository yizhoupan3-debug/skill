---
name: paper-reviser
description: |
  Specialist revision lane behind `$paper-workbench`. Use when the route is
  already clearly "change the paper now" based on reviewer comments, known
  findings, or a fixed decision to narrow scope. This skill may repair, narrow,
  delete, de-emphasize, or move material to the appendix when that is the
  honest fix. For 顶刊/顶会/top-tier revision, it turns acceptance blockers into
  manuscript changes only after the scientific claim boundary is known.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - $paper-reviser
  - paper-reviser
  - 只进改稿 lane
  - 按现有 findings 直接改稿
  - 直接改稿不要先审
  - 缩口径
  - 按这个维度改
  - 只改摘要
  - 只改图表维度
  - 写 rebuttal
  - 顶刊标准改稿
  - 顶会标准改稿
  - 顶刊顶会改稿
  - top-tier revision
  - revise for top conference
  - revise for top journal
  - 精准修改
  - 大面积重构
  - edit_scope: surgical
  - edit_scope: refactor
metadata:
  version: "3.3.0"
  platforms: [codex]
  tags: [paper, manuscript, revise, reviewer-comments, rebuttal, appendix-routing]
framework_roles:
  - planner
  - executor
  - verifier
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
risk: medium
source: local

---

# Paper Reviser

This skill is the revision specialist lane behind `$paper-workbench`.

It owns the paper-facing execution step: after the problems are known, actually
change the manuscript in the most honest direction.

The execution model is:

- main revision chain = serial
- local specialist checks and cleanup = bounded parallel sidecars
- merge-back and final accept/reject of edits = local

## Edit scope gate

Honor **`edit_scope`** from
[`../paper-workbench/references/edit-scope-gate.md`](../paper-workbench/references/edit-scope-gate.md)
before applying edits:

- **`surgical`**: execute only the listed reviewer items / blockers / slices; do
  not expand into whole-paper restructuring, unsolicited appendix routing, or
  cross-section narrative rewrites.
- **`refactor`**: full allowed-edit decisions (repair, narrow, delete, appendix,
  de-emphasize) across the manuscript as needed.

If the user did not declare scope, default to **`surgical`** until clarified.

## Use this when

- The user explicitly wants edits now, not the front door
- The task is driven by reviewer comments, a review checklist, or a known blocker
- The route is already clearly revise-only
- The paper needs claim downgrade, appendix routing, de-emphasis, or deletion instead of forced repair
- The user wants rebuttal or response-letter work tied to real manuscript edits
- The user has a top-tier readiness finding and wants concrete manuscript
  changes against that blocker

## Do not use

- The user wants one front door for the paper task -> use `$paper-workbench`
- The user is still asking "能不能投" or wants the first review pass -> use `$paper-reviewer`
- The user wants only local wording polish with fixed scientific scope -> use `$paper-writing`
- The user wants only science-level critique without edits -> use `$paper-reviewer` logic mode

## User-facing modes

Use one of only two external modes:

- `按审稿意见改`: default when the user generally wants the manuscript fixed
- `只改这一维`: only when the user explicitly names one dimension or one block

Do not make the user speak in gate language unless they already are.

## Allowed edit decisions

When the strongest honest path is not "repair everything", this skill may:

- repair
- narrow
- delete
- move to appendix
- de-emphasize
- disclose as limitation

These are not edge cases. They are part of the normal contract.

## What this skill should deliver

Default output should stay simple:

1. what was changed in this slice
2. whether the blocker is resolved, partially resolved, or still blocked
3. whether the next step is more revision, re-review, or new evidence

Default user-facing wording contract:

- Prefer author-facing language: `revision done`, `remaining blocker`,
  `next rewrite target`.
- Keep protocol terms internal by default: `gate`, `backjump`, `lane`,
  `manifest`.
- Surface protocol terms only when the user asks for protocol artifacts.

For 顶刊/顶会/top-tier revision, each edit batch should also name which selective
venue risk it reduces: contribution clarity, closest-work separation, decisive
evidence, claim ceiling, reproducibility, figure/table persuasiveness, or
front-door story.

For multi-round revision, each batch must also report:

```text
claim_ledger_delta:
evidence_anchor_delta:
drift_check_result:
```

If the user is running the protocol-backed workflow, follow
[`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md). Treat the protocol as
internal state management, not as the main user interface.

In protocol mode, do not rewrite the whole paper in one undifferentiated pass.
Keep the active blocker serial, and use sidecar lanes only for bounded slices
such as citation fixes, figure/table cleanup, notation audit, mirror cleanup, or
local prose edits after the claim boundary is frozen.

Use the bundled scaffold helper when you need to materialize a parallel batch on
disk:

`python3 /Users/joe/Documents/skill/scripts/paper_lane_scaffold.py ...`

## Internal routing notes

- Use `logic repair` mode when a revision depends on claim-vs-evidence repair
- Use `$citation-management` for citation support changes
- Use `$paper-writing` for local prose rewriting after the claim boundary is fixed
- Use [`../paper-workbench/references/top-tier-paper-standard.md`](../paper-workbench/references/top-tier-paper-standard.md)
  as the acceptance-risk checklist when the user wants top-tier revision

For revision dimension modes, use
[`references/revision-modes.md`](references/revision-modes.md).
- Use `figure-table repair`, `$visual-review`, and `$pdf` for final figure, table, or layout changes
- When multiple local cleanup surfaces are independent, run them as bounded sidecar lanes and merge locally before closing the gate

## Hard rules

- Do not hide evidence that breaks a claim the paper still keeps
- Do not use appendix moves as a substitute for an honest claim downgrade
- Do not parallelize multiple gate-closing decisions at once
- Do not expand a one-slice edit request into a full-paper rewrite
- If a blocker needs new experiments, say so instead of polishing around it
- Do not edit prose that changes claim level unless the claim decision lane
  explicitly approves and records the claim ledger delta
