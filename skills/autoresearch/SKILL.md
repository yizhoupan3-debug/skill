---
name: autoresearch
description: |
  Research workspace CLI frontend — init, draft claims, external research,
  hypothesis management, run records, novelty gating, reflection, barrier
  escalation (loop bridge), layered logging, smoke tests, and sync.
  Backed by core/research-harness unified library.
  Use for requests like "帮我初始化一个研究方向", "做 claim 管理",
  "自动研究这个瓶颈", or "记录实验反思".
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: optional
user-invocable: true
disable-model-invocation: false
short_description: Research workspace CLI — claims, hypotheses, runs, logs, barrier escalation
trigger_hints:
  # workspace
  - 研究工作区
  - 科研工作区
  - research workspace
  - 研究方向（初始化）
  - 初始化研究
  - 研究初始化
  - 开启研究
  # claim / novelty
  - claim 管理
  - 新颖性声明
  - novelty gate
  - draft claims
  - 声明管理
  # hypotheses
  - 假设管理
  - 实验假设
  - hypothesis
  # experiments / runs
  - 实验记录
  - 实验反思
  - 科研回顾
  - 实验回顾
  - record run
  # logging
  - 研究日志
  - research log
  - 科研日志
  - 记录探索
  - 研究方向记录
  # barrier escalation (loop bridge)
  - 自动研究
  - loop 研究
  - barrier research
  - 瓶颈研究
  - 突破方向
  - 自动突破
  - 突破不了
  # misc
  - 实验记录
  - 科研回顾
trigger_hints_long: references/trigger-hints-long.md
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [research, workspace, claims, hypotheses, logs, barrier]
risk: low
source: local

---

# Autoresearch

This skill wraps the `autoresearch` CLI binary — a Rust CLI for research workspace
lifecycle management backed by `core/research-harness` — and exposes it through
the routing system. It also serves as the **bridge between loop engineering and
systematic research**: when a loop-auto cycle hits a hard barrier
(`consecutive_failures ≥ threshold`), the `barrier_escalation` lane runs
systematic research and returns candidates.

## When to use

- The user wants to **initialize a new research direction**: question, project, mode.
- The user needs **claim/novelty management**: draft, compare, set gates.
- The user wants **hypothesis tracking**: add, list, status, conclude.
- The user needs **run recording** with environment fingerprinting and git provenance.
- The user wants **experiment reflection** including claim drift detection.
- The user says "突破不了"/"瓶颈"/"stuck" in an auto-experimentation context.
- The user wants **structured research logging** (text layer + SQLite FTS5).
- The user needs **smoke tests** for academic source freshness.

## Do not use

- The user needs a **literature survey, theory landscape, or math-background inquiry** (discovery phase) → use `$research-discovery`.
- The user wants to **design experiments, ablations, benchmarks, or math modeling** (execution phase) → use `$research-execution`.
- The object is a **manuscript, submission, reviewer response** → use `$paper-workbench`.
- The user only asks which **statistical test** to use → use `$statistical-analysis`.
- The user only asks for a **formal proof, derivation, or pure-math task** → use `$math-derivation`.
- The user only asks for **citation metadata cleanup** → use `$citation-management`.
- The user asks for **ordinary coding** → answer in the current coding context.

## Operating contract

### Lane classification

| Lane | CLI subcommand | Description |
|------|---------------|-------------|
| `workspace_init` | `init` | Initialize new research direction |
| `workspace_resume` | `status / next / resume` | Resume / view current workspace |
| `claim_drafting` | `draft-claims / compare-claim / set-novelty-gate` | Claim lifecycle |
| `external_research` | `research-claim / research-all` | External academic retrieval |
| `hypothesis_tracking` | `add-hypothesis / list-hypotheses` | Hypothesis CRUD |
| `run_recording` | `record-run` | Experiment recording (env + git) |
| `reflection` | `reflect` | Experiment reflection + drift detection |
| `log` | `log:record / log:search / log:insight / log:connect` | Layered logging (text + SQLite); bridges `research-log-rs` CLI |
| `smoke_test` | `smoke-test` | Freshness guard |
| `barrier_escalation` | `barrier <problem>` | **Loop bridge**: systematic research on hard barriers |
| `sync` | `sync` | Sync to artifact |

### Backend

All lanes call `cargo run -p research-harness --bin autoresearch -- <subcommand>` to
invoke the CLI in `core/research-harness/src/bin/autoresearch.rs`.

Workspace data:
- State: `<workspace>/research-state.yaml` (schema v4)
- Runs: `<workspace>/run-ledger.jsonl`
- Logs: `artifacts/research-log/` (text layer) + SQLite FTS5
- Barrier reports: `artifacts/research-barrier/<barrier-id>/`

### Cross-skill handoff

```
autoresearch → $research-discovery    Deep literature / barrier escalation
autoresearch → $research-execution    Experiment design / verification
autoresearch → $paper-workbench       Manuscript-level output
autoresearch → loop runner            BARRIER_REPORT.json → resume loop
```

### Barrier escalation lane (loop bridge)

When a loop-auto cycle hits `consecutive_failures ≥ threshold`:

1. Construct barrier description (loop_id + run_id + action_id + failure context)
2. Call `cargo run -p research-harness --bin autoresearch -- barrier <description>`
3. Research workspace init with barrier problem as question
4. Literature review via `$research-discovery`
5. Hypothesis generation (draft-claims)
6. Feasibility scan per hypothesis
7. Output BARRIER_REPORT.json → `artifacts/research-barrier/<barrier-id>/`
8. Loop runner consumes report → selects candidate → resumes

See `docs/spec/research-harness.md` §19.4.3 for the detailed barrier escalation
contract and BARRIER_REPORT.json schema.

### Logging layer

See `docs/spec/research-harness.md` §19.5 for the layered logging specification:
- Text layer: `artifacts/research-log/YYYY-MM/YYYY-MM-DD_direction-name.md`
- Compressed DB: SQLite FTS5 (`exploration_logs`, `exploration_decisions`, `exploration_insights`, `barrier_reports`)

### Smoke test

See `docs/spec/research-harness.md` §19.6 for smoke test specification:
- Registry: `artifacts/research-log/smoke-tests.json`
- Execution: `cargo run -p research-harness --bin autoresearch -- smoke-test [--source <src>] [--barrier <id>]`
- Freshness metadata on every external_research result

### Verification and failure contract

- All research outputs must include an evidence map: known, unknown, what must be checked.
- If a lane cannot be verified, return a blocker: missing input, unavailable source, unrun command.
- Do not convert verification failures into confident research conclusions.
- For tool or data failures, preserve the smallest useful error summary.

## Hard constraints

- Do not start manuscript work; hand it to `$paper-workbench`.
- Do not turn "research" into unsourced speculation.
- Barrier escalation must write BARRIER_REPORT.json — human judgment is not a substitute.
- All external_research results must carry freshness metadata.
- Claim drift detection runs before every `record-run`.

## Cross-references

- **Research harness specification**: `docs/spec/research-harness.md` (full §19)
- **Loop architecture**: spec.md §七层模型 (loop-auto profile, barrier escalation)
- **Discovery front door**: `skills/research-discovery/SKILL.md`
- **Execution back door**: `skills/research-execution/SKILL.md`
- **Paper manuscript**: `skills/paper-workbench/SKILL.md`
- **Academic sources (raw HTTP fallback)**: `skills/research-discovery/references/academic-sources.md`
