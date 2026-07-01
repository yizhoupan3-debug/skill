---

description: Research workspace CLI — claims, hypotheses, runs, logs, barrier escalation, smoke tests. Backed by core/research-harness.
metadata:
  platforms:
  - supported
  tags:
  - research
  - workspace
  - claims
  - hypotheses
  - logs
  - barrier
  version: '1.0.0'
name: research-workspace
scene: research
risk: low
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: optional
short_description: 科研工作区 CLI — claim/假设/实验记录/smoke test/barrier 升级
source: local
trigger_hints:
- 研究工作区
- 科研工作区
- research workspace
- 研究方向（初始化）
- 初始化研究
- 研究初始化
- 开启研究
- claim 管理
- 新颖性声明
- novelty gate
- 假设管理
- 实验假设
- 实验记录
- 实验反思
- 科研回顾
- 研究日志
- research log
- 自动研究
- loop 研究
- barrier research
- 瓶颈研究
- 突破方向
- 自动突破
- autoresearch
---
# Research Workspace — Skill Entry

## 命名注意

本 skill 位于 `skills/research/research-harness/` 目录但路由 slug 为 `research-workspace`，与 Rust crate (`core/research-harness/`) 区分：
- **本 skill** → 文档层，描述 CLI 用法和工作区协议；路由名 `$research（research-workspace lane）`
- **Rust crate** → `core/research-harness/`，包含 CLI 二进制和库代码
- **CLI 二进制** → `cargo run -p research-harness --bin autoresearch -- <subcommand>`


This skill wraps the `autoresearch` CLI binary — a Rust CLI for research workspace
lifecycle management backed by `core/research-harness`. It serves as the **bridge between loop engineering and
systematic research**: when a loop-auto cycle hits a hard barrier
(`consecutive_failures ≥ threshold`), the `barrier_escalation` lane runs
systematic research and returns candidates.

由 `$research`（统一科研前门）`execution` lane 调用，也可直接调用。

## When to use

- The user wants to **initialize a new research direction**: question, project, mode.
- The user needs **claim/novelty management**: draft, compare, set gates.
- The user wants **hypothesis tracking**: add, list, status, conclude.
- The user needs **run recording** with environment fingerprinting and git provenance.
- The user wants **experiment reflection** including claim drift detection.
- The user says "突破不了"/"瓶颈"/"stuck" in an auto-experimentation context.
- The user wants **structured research logging** (text layer + SQLite FTS5).
- The user needs **smoke tests** for quick experiment probes.

## Do not use

- The user needs a **literature survey, theory landscape, or math-background inquiry** (discovery phase) → use `$research` (discovery lane).
- The user wants to **design experiments, ablations, benchmarks, or math modeling** (execution phase) → use `$research` (execution lane).
- The object is a **manuscript, submission, reviewer response** → use `$research` (paper-workbench lane).
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
| `claim_planning` | `plan-search / brief-first-claim / gate-from-research` | Search planning and novelty gate |
| `run_annotation` | `annotate-run / audit-reuse` | Run reuse metadata and audit |
| `sync` | `sync` | Sync to artifact |
| `smoke_test` | 通过 `research_smoke` MCP 工具调用 | Quick directional experiment probes via templates/ |
| `log` | (library API) | Layered logging — SQLite FTS5 (`research_harness::log`) |
| `barrier_escalation` | `barrier <problem>` | **Loop bridge**: systematic research on hard barriers |

### Backend

CLI lanes call `cargo run -p research-harness --bin autoresearch -- <subcommand>` to
invoke the CLI in `core/research-harness/src/bin/autoresearch.rs`.
Smoke tests and research verification are accessible via MCP tools (`research_smoke`, `research_verification_*`).

Workspace data:
- State: `<workspace>/research-state.yaml` (schema v4)
- Runs: `<workspace>/run-ledger.jsonl`
- Logs: `artifacts/research-log/` (text layer) + SQLite FTS5
- Barrier reports: `artifacts/research-barrier/<barrier-id>/`

### Cross-skill handoff

```
research-workspace → $research (discovery lane)    Deep literature / barrier escalation
research-workspace → $research (execution lane)    Experiment design / verification
research-workspace → $research (paper-workbench)   Manuscript-level output
research-workspace → loop runner                   BARRIER_REPORT.json → resume loop
```

### Barrier escalation lane (loop bridge)

When a loop-auto cycle hits `consecutive_failures ≥ threshold`:

1. Construct barrier description (loop_id + run_id + action_id + failure context)
2. Call `cargo run -p research-harness --bin autoresearch -- barrier <description>`
3. Research workspace init with barrier problem as question
4. Literature review via `$research` (discovery lane)
5. Hypothesis generation (draft-claims)
6. Feasibility scan per hypothesis
7. Output BARRIER_REPORT.json → `artifacts/research-barrier/<barrier-id>/`
8. Loop runner consumes report → selects candidate → resumes

Detailed barrier escalation contract and BARRIER_REPORT.json schema are defined
in the barrier escalation implementation at `core/research-harness/src/`. The
report follows a structured JSON format with sections for problem description,
applied directions, evidence, and candidate solutions.

### Logging layer

Layered logging specification:
- Text layer: `artifacts/research-log/YYYY-MM/YYYY-MM-DD_direction-name.md`
- Compressed DB: SQLite FTS5 (`exploration_logs`, `exploration_decisions`, `exploration_insights`, `barrier_reports`)

### Smoke test

General-purpose experiment smoke engine:
- Template dir: `templates/` (any executable)
- Execution: `cargo run -p research-harness --bin autoresearch -- smoke-test --template <name> --params '[{"key":"val"}]'`
- Params injected as `EXPERIMENT_<KEY>=<VALUE>` env vars
- Structured JSON output, LRU+TTL cache, parallel subprocess execution

### Verification and failure contract

- All research outputs must include an evidence map: known, unknown, what must be checked.
- If a lane cannot be verified, return a blocker: missing input, unavailable source, unrun command.
- Do not convert verification failures into confident research conclusions.
- For tool or data failures, preserve the smallest useful error summary.

## Hard constraints

- Do not start manuscript work; hand it to `$research` (paper-workbench lane).
- Do not turn "research" into unsourced speculation.
- Barrier escalation must write BARRIER_REPORT.json — human judgment is not a substitute.
- All external_research results must carry freshness metadata.
- Claim drift detection runs before every `record-run`.

## Cross-references

- **Barrier escalation**: `core/research-harness/src/bin/autoresearch.rs` (barrier subcommand)
- **Loop architecture**: loop-auto profile, barrier escalation via `autoresearch barrier`
- **统一科研前门**: `../SKILL.md`
- **Discovery lane**: `../lanes/discovery.md`
- **Execution lane**: `../lanes/execution.md`
- **Paper manuscript**: `../paper-workbench/SKILL.md`
- **Academic sources (raw HTTP fallback)**: `../references/academic-sources.md`
