---
name: autoresearch
description: |
  Orchestrate autonomous research through a recoverable loop of hypothesis,
  experiment, reflection, and synthesis.
  Check this skill early at 每轮对话开始 / first-turn / conversation start for multi-hypothesis, cross-session,
  or autonomous research projects.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: preferred
trigger_hints:
  - autonomous research
  - autonomous research loop
  - multi-hypothesis
  - 多假设
  - 多假设实验
  - experiment loop
  - reflection synthesis
  - 记录反思
  - 综合结论
  - research state
  - experiment orchestration
  - two loop
  - two-loop
  - hypothesis testing
metadata:
  version: "3.1.0"
  platforms: [codex]
  tags:
    - autonomous-research
    - experiment-orchestration
    - two-loop
    - hypothesis-testing
    - research-state
risk: medium
source: community-adapted
runtime_requirements:
  rust:
    - cargo
---

# Autoresearch

Autonomous research orchestration for projects that need repeated hypothesis testing,
explicit state, and evidence-backed synthesis. This skill owns the research control
loop; it does not replace domain execution skills. When implementation enters the
loop, route code-writing slices through the narrowest execution owners, with
proactive acceleration and memory checks before expensive runs. See
`references/workflow-notes.md` for the source-backed operating constraints that
shaped this skill.

The default posture is scientific sensemaking, not tuning. A run is only useful if
it can change a belief about a mechanism, a boundary condition, or a rival
explanation. Parameter sweeps are allowed only after the hypothesis says what the
parameter is supposed to reveal.

## When To Use

- Starting a new research project from a question or claim
- Running autonomous multi-hypothesis experiments
- Managing repeated experiment → reflection → synthesis cycles
- Coordinating a research project across sessions with recoverable state

## Do Not Use

- The user wants one front door for a research-project task instead of directly entering the experiment loop
- One-off model training, tuning, or evaluation without a research loop
- Pure literature review or citation gathering
- Brainstorming only, without execution
- Paper review, paper writing, or figure polishing only

Use `$research-workbench` for ambiguous research-project asks where the first
active lane is not obviously the autonomous experiment loop yet.

## Operating Modes

### Full mode

Use for multi-session or publication-scale research. Maintain persistent state and
artifact folders so the work can resume without replaying the whole thread.

### Quick mode

Use for a focused loop inside one conversation. Keep the state compact, but still
write a protocol, run the experiment, record the result, and choose the next move.

Default to quick mode unless persistence is explicitly needed or the work clearly
spans sessions.

## Core Loop

### Bootstrap

1. Scope the question and name the expected contribution plus the mechanism sketch.
2. Search literature enough to identify the novelty boundary. External web and
   scholarly-API lookup is allowed; keep the captured evidence in the workspace.
3. Extract 3 to 5 core claims and run a novelty check against rival explanations,
   not just matching keywords.
4. Write the first protocol before any experiment starts.

### Inner loop

1. Pick one highest-priority hypothesis with a mechanism and a falsifiable
   prediction.
2. Isolate the experiment in its own directory or branch.
3. Name the simple baseline, ablation, or control before implementation starts.
4. Before expensive runs, route implementation slices through the relevant execution owners and proactively check acceleration and memory-control paths.
5. Run the experiment, measure the agreed proxy, and sanity-check the result.
6. Record the outcome with what changed, what was observed, what failed, what the
   result rules in/out, and what alternative explanations remain.
7. Update the research state before starting the next cycle.

### Outer loop

1. Cluster results into worked, failed, or ambiguous.
2. Explain why the pattern happened, not just what happened.
3. Update findings and literature positioning if the result is surprising.
4. Decide one direction: `DEEPEN`, `BROADEN`, `PIVOT`, or `CONCLUDE`.

### Finalize

1. Consolidate the story into a paper, report, or handoff package.
2. Archive the state so a later session can reproduce the decision path.

## State Discipline

- Write the protocol before the first run. Include hypothesis, prediction, metric,
  success threshold, stop condition, and owner artifact paths.
- Include mechanism, falsifiable prediction, baseline/control, confounders, and
  negative signals before treating a run as a research run.
- Keep a single source of truth for project state. Do not let parallel branches edit
  the same state file at once.
- Log command, code version, data version, environment, seed, and metric for every
  meaningful run.
- Record throughput, latency, or peak-memory evidence whenever code changes affect
  execution shape or scale limits.
- Record negative results with the failure mode they rule out.
- Convert every run into a reusable finding, decision delta, and scope boundary;
  avoid chronological "then I tried..." notes as the primary record.
- If an older run only has a narrative summary, backfill it with `annotate-run`
  before citing it as evidence. The generated `findings-reuse-index.md` is the
  fast lookup surface for reusable results.
- Run `audit-reuse` after migration or before handoff to list run records that
  still have narrative-only evidence.
- Separate metric movement from interpretation: a better number is not a finding
  until the baseline/control and rival explanations have been checked.
- Re-read the state and findings before resuming work in a later session.
- If the state is stale or contradictory, reconcile it before launching new runs.
- Treat `research-state.yaml` as the canonical control plane and `research-log.md` as
  the chronological record.
- Keep experiment folders append-only after a run is labeled complete.
- Prefer the bundled Rust controller `../../scripts/autoresearch-rs` over ad hoc manual edits for
  init, queued hypotheses, run records, reflections, and next-step suggestions.
- Keep external lookup inside the Rust controller path (`research-claim`,
  `research-all`, `gate-from-research`) when
  possible; do not add Python helper scripts for search or state mutation.

## Minimum Run Record

Each meaningful run should capture:

- Hypothesis id and one-line claim
- Mechanism being tested and falsifiable prediction
- Baseline/control or ablation expectation
- Protocol version or commit hash
- Data snapshot or generation recipe
- Command or entry point used
- Seed and environment notes
- Primary metric and sanity checks
- Outcome label: confirmatory, exploratory, failed, or ambiguous
- Reusable finding, decision delta, and applies-to / does-not-apply-to scope
- What the result rules in or rules out
- Alternative explanations and threats to interpretation

## Parallelization Rules

- Parallelize only independent retrieval, reproduction, plotting, or hypothesis
  branches.
- Keep one orchestrator responsible for merges and direction changes.
- Use sidecars only for bounded tasks with a clear output contract.
- Never let two workers mutate the same experiment artifact concurrently.

## Novelty Gate

Before the first inner-loop cycle:

1. Convert the initial hypotheses into 3 to 5 specific novelty claims.
2. Compare the claims against literature and mark overlap level.
3. If the claims are mostly high-overlap, pivot the question.
4. If the claims are mixed, proceed but document the differentiation strategy.
5. If the claims are mostly low-overlap, proceed and log the positioning decision.
6. Treat the gate as a hard execution check: `record-run` must not proceed unless `novelty_gate.status == passed`.
7. If one bounded pilot must run early, use an explicit override with a written reason so the deviation is auditable.

## Hypothesis Lifecycle

- `queued`: candidate branch waiting for activation.
- `active`: the branch currently being prepared or extended.
- `needs_reflection`: a run finished and the branch must be reflected on before the next run.
- `parked`: the branch was pivoted away from, but can be reactivated later.
- `concluded`: the branch is closed and should stay append-only.

Only transition hypotheses through the controller so the state file, Markdown projections, and ledger stay aligned.

## Skill Routing

When this skill causes code to be written, rewritten, or scaled up, do not wait for
"it is too slow" or "it OOMed" as the trigger. Proactively route implementation
slices as follows before expensive runs.

| Activity | Route To |
|---|---|
| Training, fine-tuning, or model-level experiments | `ai-research` primary; on Apple Silicon or MPS first check `mac-memory-management`; add `code-acceleration` when a generic hot path remains after runtime policy is settled |
| Data pipelines, preprocessing, evaluation harnesses, inference hot paths, or agent loops | `ai-research`; on Mac add `mac-memory-management` first; add `code-acceleration` when generic dataflow or serializer bottlenecks remain |
| Literature search or novelty checking | `literature-synthesis` |
| Brainstorming | `brainstorm-research` |
| Reproducibility and experiment provenance | `experiment-reproducibility` |
| Statistical analysis | `statistical-analysis` |
| Paper drafting or revision | `paper-writing`, `paper-reviser` |
| Paper review or logic review | `paper-workbench`, `paper-reviewer` logic mode |
| Citation cleanup | `citation-management` |
| Data analysis or notebooks | `jupyter-notebook`, language-specific execution owner |

## Default execution co-routing

- `autoresearch` owns the research loop, not the hot path.
- When a loop step writes or changes code, treat `ai-research` as the primary execution owner for ML or experiment code.
- On Apple Silicon, MPS, unified-memory, or unstable memory-headroom paths, co-check [`$mac-memory-management`](../mac-memory-management/SKILL.md) first as the runtime owner.
- Co-check [`$code-acceleration`](../code-acceleration/SKILL.md) when a generic hot path remains after the Mac runtime layer is no longer the main blocker.
- Keep the orchestrator responsible for direction changes; companion owners must not silently redefine the research question.

## Research Discipline

- Prefer mechanistic hypotheses: `X because Y, predicting Z`.
- Ask "what would change my mind?" before asking "what parameter should I try?"
- Treat unexpected results as a signal to revisit the assumption set.
- Check the boring explanation first: leakage, data shift, implementation bug,
  missing baseline, or compute artifact.
- Prefer one decisive contrast over many loosely connected sweeps.
- Use confirmatory labels only when the protocol was written before the run.
- Keep exploratory and confirmatory results separate in the log.
- A coherent negative result is valid if it rules out a meaningful alternative.
- Do not call a metric gain a finding until you can say what mechanism it supports
  and which rival explanation it weakens.
- Do not continue random changes when the loop stalls; stop and reframe.

## Workspace Structure

`{project}/`
`research-state.yaml` central state
`run-ledger.jsonl` append-only audit trail for runs, reflections, gate changes, and workspace syncs
`CURRENT_CONTEXT.md` current working view with freshness guardrails
`research-log.md` timeline
`findings.md` narrative synthesis
`BOOTSTRAP_BRIEF.md` scoped research brief
`literature/` source notes
`literature/NOVELTY_GATE.md` overlap and positioning gate
`experiments/{hypothesis-slug}/` protocol, code, results, analysis
`experiments/_templates/` hypothesis, protocol, run, reflection templates
`to_human/` handoff drafts
`paper/` manuscript assets

## Primary Assets

- Rust controller CLI: `scripts/autoresearch-rs`
- Templates: `skills/autoresearch/templates/`
- Resume handoff: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- resume --workspace <project>`
- File resync: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- sync --workspace <project>`
- Claim drafting lane: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- draft-claims --workspace <project>`
- External research lane: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- research-claim --workspace <project> --claim-id C1`; this queries Semantic Scholar/arXiv from Rust and writes `literature/EXTERNAL_RESEARCH.md`
- Batch external research: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- research-all --workspace <project> --max-claims 3`; this searches the top claims without manual per-claim repetition
- Gate recommendation from captured research: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- gate-from-research --workspace <project> --apply`
- Reuse annotation lane: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- annotate-run --workspace <project> --run-id run-001 --finding "..." --decision-delta "..." --reuse-note "..."`; this upgrades old run notes into reusable evidence.
- Reuse audit lane: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- audit-reuse --workspace <project> --apply`; this refreshes `findings-reuse-index.md` and reports missing reusable fields.
- Novelty comparison lane: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- compare-claim --workspace <project> ...`
- Search-plan refresh: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- plan-search --workspace <project>`; this refreshes the managed search view from structured claims instead of creating a new persisted truth surface
- First-claim brief refresh: `cargo run --manifest-path scripts/autoresearch-rs/Cargo.toml -- brief-first-claim --workspace <project>`; the brief lives inside `CURRENT_CONTEXT.md`, and legacy `literature/NOVELTY_BRIEF.md` is treated as removable stale output

## Git Protocol

| Event | Message Pattern |
|---|---|
| Init | `research(init): {project} — {question}` |
| Protocol locked | `research(protocol): {hypothesis}` |
| Results | `research(results): {hypothesis} — {outcome}` |
| Reflection | `research(reflect): {direction} — {reason}` |
| Paper | `research(paper): {title}` |

## Trigger Examples

- `帮我做一个完整的自主研究项目`
- `用 two-loop 方式跑实验并综合结论`
- `管理多个假设的实验流程`
- `自动化研究编排`

## Continuity

For long-running research, set a heartbeat or scheduled resume mechanism and keep the
state files current at each tick. If the session stops, the next run should be able to
resume from the saved state instead of reconstructing the whole conversation.
