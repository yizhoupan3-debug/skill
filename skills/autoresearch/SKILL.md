---
name: autoresearch
description: |
  Orchestrate autonomous research through a recoverable two-loop cycle:
  hypothesis, experiment, reflection, synthesis. Check this skill early at
  每轮对话开始 / first-turn / conversation start when the task is a multi-
  hypothesis, cross-session, or autonomous research project rather than a one-off run.
  When the loop writes or rewrites experiment code, proactively check code-acceleration
  and the relevant memory-control owner before expensive runs.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: preferred
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
---

# Autoresearch

Autonomous research orchestration for projects that need repeated hypothesis testing,
explicit state, and evidence-backed synthesis. This skill owns the research control
loop; it does not replace domain execution skills. When implementation enters the
loop, route code-writing slices through the narrowest execution owners, with
proactive acceleration and memory checks before expensive runs. See
`references/workflow-notes.md` for the source-backed operating constraints that
shaped this skill.

## When To Use

- Starting a new research project from a question or claim
- Running autonomous multi-hypothesis experiments
- Managing repeated experiment → reflection → synthesis cycles
- Coordinating a research project across sessions with recoverable state

## Do Not Use

- One-off model training, tuning, or evaluation without a research loop
- Pure literature review or citation gathering
- Brainstorming only, without execution
- Paper review, paper writing, or figure polishing only

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

1. Scope the question and name the expected contribution.
2. Search literature enough to identify the novelty boundary.
3. Extract 3 to 5 core claims and run a novelty check.
4. Write the first protocol before any experiment starts.

### Inner loop

1. Pick one highest-priority hypothesis.
2. Isolate the experiment in its own directory or branch.
3. Before expensive runs, route implementation slices through the relevant execution owners and proactively check acceleration and memory-control paths.
4. Run the experiment, measure the agreed proxy, and sanity-check the result.
5. Record the outcome with what changed, what was observed, and what failed.
6. Update the research state before starting the next cycle.

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
- Keep a single source of truth for project state. Do not let parallel branches edit
  the same state file at once.
- Log command, code version, data version, environment, seed, and metric for every
  meaningful run.
- Record throughput, latency, or peak-memory evidence whenever code changes affect
  execution shape or scale limits.
- Record negative results with the failure mode they rule out.
- Re-read the state and findings before resuming work in a later session.
- If the state is stale or contradictory, reconcile it before launching new runs.
- Treat `research-state.yaml` as the canonical control plane and `research-log.md` as
  the chronological record.
- Keep experiment folders append-only after a run is labeled complete.

## Minimum Run Record

Each meaningful run should capture:

- Hypothesis id and one-line claim
- Protocol version or commit hash
- Data snapshot or generation recipe
- Command or entry point used
- Seed and environment notes
- Primary metric and sanity checks
- Outcome label: confirmatory, exploratory, failed, or ambiguous
- What the result rules in or rules out

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

## Skill Routing

When this skill causes code to be written, rewritten, or scaled up, do not wait for
"it is too slow" or "it OOMed" as the trigger. Proactively route implementation
slices as follows before expensive runs.

| Activity | Route To |
|---|---|
| Training, fine-tuning, or model-level experiments | `ai-research` primary; on Apple Silicon or MPS first check `mac-memory-management`; add `code-acceleration` when a generic hot path remains after runtime policy is settled |
| Data pipelines, preprocessing, evaluation harnesses, inference hot paths, or agent loops | `ai-research`; on Mac add `mac-memory-management` first; add `code-acceleration` when generic dataflow or serializer bottlenecks remain |
| Literature search or novelty checking | `literature-synthesis`, `academic-search` |
| Brainstorming | `brainstorm-research` |
| Reproducibility and experiment provenance | `experiment-reproducibility` |
| Statistical analysis | `statistical-analysis` |
| Paper drafting or revision | `paper-writing`, `paper-reviser` |
| Paper review or logic review | `paper-reviewer`, `paper-logic` |
| Citation cleanup | `citation-management` |
| Data analysis or notebooks | `jupyter-notebook`, `python-pro` |

## Default execution co-routing

- `autoresearch` owns the research loop, not the hot path.
- When a loop step writes or changes code, treat `ai-research` as the primary execution owner for ML or experiment code.
- On Apple Silicon, MPS, unified-memory, or unstable memory-headroom paths, co-check [`$mac-memory-management`](../mac-memory-management/SKILL.md) first as the runtime owner.
- Co-check [`$code-acceleration`](../code-acceleration/SKILL.md) when a generic hot path remains after the Mac runtime layer is no longer the main blocker.
- Keep the orchestrator responsible for direction changes; companion owners must not silently redefine the research question.

## Research Discipline

- Prefer mechanistic hypotheses: `X because Y, predicting Z`.
- Treat unexpected results as a signal to revisit the assumption set.
- Use confirmatory labels only when the protocol was written before the run.
- Keep exploratory and confirmatory results separate in the log.
- A coherent negative result is valid if it rules out a meaningful alternative.
- Do not continue random changes when the loop stalls; stop and reframe.

## Workspace Structure

`{project}/`
`research-state.yaml` central state
`research-log.md` timeline
`findings.md` narrative synthesis
`literature/` source notes
`experiments/{hypothesis-slug}/` protocol, code, results, analysis
`to_human/` handoff drafts
`paper/` manuscript assets

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
