# Paper Gate Protocol

This file defines the shared gate-chain contract used by `$paper-workbench`,
`$paper-reviewer`, and `$paper-reviser`.

`$paper-workbench` is the default front door. The protocol still keeps the
internal main chain and sidecar lanes explicit so no capability is lost when the
front door is unified.

Use this protocol only when disk-backed, repeatable, multi-turn review state is
actually useful. For normal interactive paper review, keep the protocol
internal and lead with verdict, blockers, external calibration, and next move.
Missing target venue should create a provisional review bar, not a hard stop.

## 1. Root Artifact Layout

All runtime artifacts live in the manuscript workspace root, not in the
skill-library repo.

- `paper_ref/`
- `paper_review_v<N>/`
- `paper_review_v<N>/lanes/`

`paper_ref/` is the reusable target-journal-first benchmark pool:

- `paper_ref/TARGET_CONTRACT.md`
- `paper_ref/ref_pool_manifest_v<N>.md`
- `paper_ref/pdfs/001_<slug>.pdf` through `paper_ref/pdfs/020_<slug>.pdf`

`paper_review_v<N>/` is the overall review round folder. It contains only
non-overwriting gate checklist files:

- `g00_target_contract_r1.md`
- `g02_core_evidence_r1.md`
- `g02_core_evidence_r2.md`
- `g11_figure_gate_r1.md`

`paper_review_v<N>/lanes/` is the bounded sidecar workspace for parallel-only
work that supports one active main gate:

- `lanes/g02_batch_a/lane_manifest.md`
- `lanes/g05_refs_a/lane_manifest.md`
- `lanes/g11_figures_a/lane_manifest.md`
- `lanes/g14_layout_a/lane_manifest.md`

Rules:

1. Start a new `paper_review_v<N>` only for a new whole-paper review cycle.
2. Continue the current unfinished `paper_review_v<N>` when the user is still
   working through the same cycle.
3. Every turn must create exactly one new actionable gate file.
4. Never overwrite an older `gate_r<M>.md`.
5. If the current gate passes, create the next gate's `r1` file.
6. If the current gate fails, or a later quality gate backjumps upstream, create
   the same or earlier gate's next round file.
7. Parallel lane artifacts may be appended under `lanes/`, but they do not
   replace the one-main-gate-file rule.

## 2. Shared Fields

| Field | Meaning |
|---|---|
| `target_contract` | Locked target venue, article type, audience, page/word budget, disclosure requirements, and comparison bar |
| `benchmark_ref_pool` | Local `paper_ref/` pool state: manifest version, retained PDFs, search date, and coverage gaps |
| `object_map` | Inventory of review units by abstract dimension rather than by paper section |
| `review_scope` | `full_chain` by default, or `single_gate` when the user explicitly names one dimension / gate |
| `requested_gate_scope` | Explicit requested gate such as `G3` or `g11_figure_gate` when single-gate mode is used |
| `assumed_frozen_inputs` | Upstream gates treated as assumed rather than fully proven when a user asks for a late single-gate audit without a completed full chain |
| `review_worker_mode` | `fresh_isolated_subagent` for review passes so each pass starts from a clean reviewer stance |
| `context_isolation_policy` | No prior review-chat context is carried unless it is restated in the markdown packet |
| `transport_contract` | `md_only` when passing state across turns, ticks, or workers |
| `transport_docs` | Markdown packet files allowed to carry the review state |
| `automation_wrapper` | Wrapper mode such as `heartbeat_5m_full_chain` for autonomous review execution |
| `automation_tick_goal` | What one heartbeat tick is allowed to complete, normally one gate-round advancement |
| `parallel_group_id` | Stable id for one bounded sidecar batch attached to the current main gate |
| `lane_id` | Stable id for one sidecar lane inside that parallel batch |
| `lane_kind` | `evidence_extract`, `citation_verify`, `figure_audit`, `table_audit`, `notation_audit`, `layout_audit`, `mirror_cleanup`, or `prose_local` |
| `lane_scope` | Concrete slice owned by that lane, such as `figure:F3-F6` or `citation_cluster:C5-C11` |
| `lane_owner` | Specialist skill or worker responsible for that lane |
| `lane_status` | `queued`, `running`, `merged`, `blocked`, or `dropped` |
| `lane_outputs` | Artifacts produced by the lane before merge-back |
| `merge_back_rule` | How the main thread is allowed to consume lane outputs without changing frozen upstream decisions |
| `gate_id` | `G0` through `G14` |
| `gate_order` | Stable integer order for freeze / backjump rules |
| `gate_kind` | `setup`, `decision`, or `quality` |
| `unit_type` | Example: `claim`, `figure`, `table`, `citation_cluster`, `front_door_text`, `notation_set`, `layout_surface` |
| `unit_id` | Stable identifier inside the gate, such as `claim:C2` or `figure:F4` |
| `anchor_evidence` | Concrete evidence used to justify the current gate judgment |
| `selected_decision` | Gate outcome selected for the current unit or gate |
| `claim_floor` | Lowest honest claim that still survives without new support |
| `claim_ceiling` | Highest honest claim currently supportable for the target article |
| `selected_claim_level` | The chosen claim level after G3 |
| `math_closure_required` | Whether the surviving claim requires formal closure in G4 |
| `overmath_risk` | Whether the draft is carrying math beyond what the surviving claim needs |
| `appendix_routing` | Which content stays in main text vs appendix vs gets removed |
| `backjump_gate_on_regression` | Earlier gate to revisit when a quality gate finds an upstream break |
| `freeze_after_pass` | Whether this gate is frozen unless a later backjump is explicitly opened |

## 3. Gate Order

| Gate | Slug | Kind | Legal output |
|---|---|---|---|
| `G0` | `g00_target_contract` | `setup` | `pass` / `fail` |
| `G1` | `g01_fatal_eligibility` | `decision` | `ideal` / `hide` / `abandon` |
| `G2` | `g02_core_evidence` | `decision` | `ideal` / `hide` / `abandon` |
| `G3` | `g03_claim_ceiling` | `decision` | `ideal` / `hide` / `abandon` |
| `G4` | `g04_math_closure` | `decision` | `ideal` / `hide` / `abandon` |
| `G5` | `g05_reference_support` | `decision` | `ideal` / `hide` / `abandon` |
| `G6` | `g06_main_vs_appendix` | `decision` | `ideal` / `hide` / `abandon` |
| `G7` | `g07_narrative_spine` | `quality` | `ideal_only` |
| `G8` | `g08_front_door_text` | `quality` | `ideal_only` |
| `G9` | `g09_mirror_consistency` | `quality` | `ideal_only` |
| `G10` | `g10_notation_consistency` | `quality` | `ideal_only` |
| `G11` | `g11_figure_gate` | `quality` | `ideal_only` |
| `G12` | `g12_table_gate` | `quality` | `ideal_only` |
| `G13` | `g13_language_naturalness` | `quality` | `ideal_only` |
| `G14` | `g14_rendered_layout` | `quality` | `ideal_only` |

Definitions:

- `ideal`: keep the object or claim in the main surviving paper at the target bar.
- `hide`: keep only in a strategically reduced form, such as narrowed framing,
  appendix placement, limitation framing, or de-emphasis.
- `abandon`: remove the object or claim from the surviving manuscript path.
- `ideal_only`: no new disposition is allowed; the gate either reaches the ideal
  bar or it backjumps upstream.

## 4. Object Map Default Categories

The shared `object_map` should inventory units by abstract dimension, for
example:

- `claim`
- `core_result`
- `ablation`
- `theorem_or_derivation`
- `citation_cluster`
- `main_text_block`
- `front_door_text`
- `notation_set`
- `figure`
- `table`
- `caption`
- `layout_surface`

Do not organize the review primarily as "Section 1, Section 2, Section 3".

## 5. Gate File Template

Each newly created `gate_r<M>.md` must be an executable checklist with these
sections in this order:

1. `Goal`
2. `Frozen Inputs`
3. `Review Objects`
4. `Hard Bar`
5. `Checklist`
6. `Decision Slot`
7. `Backjump Rule`
8. `Pass Line`
9. `Next File If Pass`
10. `Next File If Fail`

Rules:

- `Decision Slot` must be `pass/fail` for `G0`, `ideal/hide/abandon` for
  decision gates, and `ideal_only` for quality gates.
- `Backjump Rule` must be concrete for quality gates. If no backjump exists yet,
  write the earliest upstream gate that would need reopening if a regression is
  found.
- `Frozen Inputs` should name the already-passed gates and artifacts that cannot
  be silently rewritten.
- `Review Objects` should name stable `unit_type:unit_id` entries from the
  `object_map`.

## 6. Freeze and Backjump Rules

1. Once a gate passes, it is frozen by default.
2. Later gates may not silently rewrite earlier conclusions.
3. If a quality gate exposes an upstream contradiction, it must set
   `backjump_gate_on_regression` and send execution back to the earliest broken
   gate.
4. Quality gates cannot invent a new `hide` or `abandon` decision; they can only
   demand a backjump.
5. Decision gates are the only place where strategic narrowing, appendix moves,
   or abandonment are chosen.
6. Sidecar lanes may collect evidence or propose local edits, but they may not
   independently freeze a gate, advance the chain, or override the main-thread
   decision.

## 7. Scope Modes

Two review scopes are valid:

- `full_chain`: default when the user does not explicitly name a gate or review
  dimension
- `single_gate`: only when the user explicitly names a gate or dimension such as
  `claim ceiling`, `math closure`, `reference support`, `figure gate`, or `G3`

Rules:

1. Unspecified review requests default to `full_chain`.
2. Explicit gate or dimension requests use `single_gate`.
3. In `single_gate`, review only the requested gate for that turn.
4. If the requested gate depends on upstream gates that were not actually passed,
   record them as `assumed_frozen_inputs` instead of silently backfilling the
   whole chain.
5. `single_gate` still creates exactly one new non-overwriting gate file.

## 8. Review Isolation Contract

To avoid reviewer softening across repeated rounds, every review pass should use:

- `review_worker_mode = fresh_isolated_subagent`
- `context_isolation_policy = no_prior_review_context_except_md_packet`
- `transport_contract = md_only`

Rules:

1. Every gate review or re-review pass starts from a fresh reviewer worker.
2. The worker may read only the manuscript artifacts plus the declared markdown packet.
3. Prior chat, prior free-form summaries, and earlier reviewer prose are not
   authoritative unless copied into the markdown packet.
4. If runtime cannot actually spawn a subagent, emulate the same isolation by
   reloading only the markdown packet from disk and treating earlier thread
   context as non-authoritative.

Recommended markdown packet:

- `paper_ref/TARGET_CONTRACT.md`
- latest `paper_ref/ref_pool_manifest_v<N>.md`
- current active `paper_review_v<N>/<gate_slug>_r<M>.md`
- upstream gate files named in `Frozen Inputs`
- any manuscript-path note required to locate the paper artifacts

## 9. Main Chain vs Parallel Lanes

The paper workflow is intentionally hybrid:

- main chain = serial
- sidecar lanes = bounded parallel
- merge-back = local and serial

Why:

- `G0-G6` decide what the paper is honestly allowed to claim
- later quality surfaces depend on those earlier decisions being frozen
- parallelism is useful for evidence collection and local inspection, not for
  replacing one main judgment with many conflicting judgments

### 9.1 Main chain rules

The following remain serial:

- choosing the active gate
- selecting `ideal / hide / abandon`
- freezing a gate
- opening a backjump
- creating the next main gate file

### 9.2 Allowed parallel lane families

Parallel lanes are allowed only when they are bounded and feed one active gate.

Recommended families:

- `G0`: target-venue-near paper collection and local PDF inventory
- `G2`: table/figure/result extraction, strongest-baseline checks, ablation inventory
- `G5`: citation existence checks, claim-to-citation precision checks, venue calibration sweeps
- `G7-G9`: mirror-surface diffing across abstract / intro / conclusion / captions / rebuttal
- `G10`: notation and abbreviation consistency scans
- `G11-G12`: per-figure and per-table audits at final scale
- `G13`: local prose smoothing only after claim boundaries are frozen
- `G14`: layout, float, and page-economy checks

### 9.3 Forbidden parallel patterns

Do not parallelize:

- multiple decision gates at once
- many reviewers each deciding claim ceiling independently
- sidecar lanes that directly mutate the main gate file
- lane-local choices of `hide` or `abandon` without main-thread confirmation
- free-form sidecar chat state as merge truth

## 10. Lane Manifest Contract

Each parallel batch should create one manifest:

- `paper_review_v<N>/lanes/<parallel_group_id>/lane_manifest.md`

The manifest should include:

1. `Main Gate`
2. `Batch Goal`
3. `Frozen Inputs`
4. `Lane Table`
5. `Merge Back Rule`
6. `Stop Condition`

The `Lane Table` should track:

- `lane_id`
- `lane_kind`
- `lane_scope`
- `lane_owner`
- `status`
- `output_artifact`
- `blocked_by`

Rules:

- one manifest per bounded batch
- each lane owns a disjoint slice
- lane output is advisory until merged by the main thread
- lane artifacts are append-only or replace-only within the lane root, not the main gate root

Bundled scaffold helper:

```bash
python3 /Users/joe/Documents/skill/scripts/paper_lane_scaffold.py \
  --workspace /path/to/manuscript \
  --review-dir paper_review_v3 \
  --batch-id g11_figures_a \
  --main-gate G11 \
  --batch-goal "Audit final-scale figures before gate closeout." \
  --frozen-input "G2 passed" \
  --frozen-input "G3 selected_claim_level locked" \
  --lane "fig_a|figure_audit|figure:F1-F2|figure-table-mode" \
  --lane "fig_b|figure_audit|figure:F3-F4|figure-table-mode"
```

Or let the script auto-fill a default batch for supported gates:

```bash
python3 /Users/joe/Documents/skill/scripts/paper_lane_scaffold.py \
  --workspace /path/to/manuscript \
  --review-dir paper_review_v3 \
  --main-gate G11 \
  --preset-by-gate
```

## 11. Merge-Back Contract

Merge-back is always local to the main thread.

The main thread may:

- accept a lane result as evidence
- reject a lane result
- request one rerun for a blocked lane
- drop a lane if its slice is no longer relevant after a decision change

The main thread may not:

- let a lane silently redefine frozen inputs
- merge contradictory lane outputs without explicit adjudication
- advance the main chain before merge-critical lanes are resolved or waived

Default merge policy:

- decision gates: merge evidence first, decide second
- quality gates: merge local audit findings, then emit one pass or backjump decision

## 12. Automation Wrapper Contract

For autonomous full-chain review mode, use:

- `automation_wrapper = heartbeat_5m_full_chain`
- `automation_tick_goal = advance_at_most_one_main_gate_or_one_parallel_batch`

Per heartbeat tick:

1. Read the markdown packet only.
2. Resolve the active gate from the latest gate files.
3. Choose one of two legal actions:
   - advance the main gate once
   - launch or merge one bounded parallel batch for the active gate
4. If a parallel batch is launched, write or update only the lane manifest and
   lane-local artifacts.
5. If the main gate is advanced, write exactly one new non-overwriting gate
   markdown file.
6. Exit without carrying hidden state into the next tick.

The heartbeat wrapper is part of the skill contract; it does not authorize
overwriting old markdown files or carrying free-form hidden state between ticks.
