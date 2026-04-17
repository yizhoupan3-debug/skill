---
name: idea-to-plan
description: |
  内核级战略编排器 (L-1)：把模糊意图转成高成熟度计划、Pilot 队列与 Duo-Doc 交付。
  适用于“意图到计划 / 蓝图设计 / 方案探索 / Pilot 验证 / 科研自动化 / outline.md / code_list.md / code_list 语言选型 / 主线程极简”。
  每轮对话开始 / first-turn / conversation start，如任务仍处于模糊规划阶段，必须优先检查此控制层。
routing_layer: L-1
routing_owner: "@strategic-orchestrator"
routing_gate: idea-to-plan, 战略编排, 意图到计划, 科研自动化, 试点验证, Pilot, Duo-Doc, outline.md, code_list.md, 蓝图设计
routing_priority: P0
session_start: required
short_description: Turn ambiguous ideas into evidence-backed plans with branch routing and compressed context
framework_roles:
  - strategic-orchestrator
  - planner
  - pilot-router
framework_phase: pre-execution
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
metadata:
  version: "2.3.0"
  platforms: [codex]
  tags:
    - strategy
    - planning
    - pilot
    - automation
    - subagent
    - duo-doc
    - first-turn-routing
    - context-compression
risk: medium
source: local
source_priority: 40
allowed_tools:
  - shell
  - git
  - python
  - web
approval_required_tools:
  - git push
  - network install
filesystem_scope:
  - repo
  - .supervisor_strategy.json
  - artifacts
network_access: conditional
artifact_outputs:
  - outline.md
  - code_list.md
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---

# idea-to-plan

`idea-to-plan` is the **L-1 strategic orchestrator** for Codex. It takes an ambiguous idea, turns it into an evidence-backed blueprint, and hands execution a concrete path instead of vague enthusiasm.

This skill is for the phase **before implementation becomes primary**.

## When to use

- 用户只有一个方向、问题意识、研究兴趣或产品想法，还没有成熟 plan
- 任务存在多个可能路线，需要比较、筛选、收敛
- 需要把模糊意图转成 `outline.md` 与 `code_list.md`
- 需要并行做 **方案探索 / Pilot 验证 / 可行性扫描 / 查新**
- 需要让后续执行层直接接手，而不是在执行阶段继续猜需求
- 每轮对话开始 / first-turn / conversation start，如果当前任务的核心问题仍然是“到底该怎么做”，优先检查本 skill

## Do not use

- 已经有成熟 plan、PRD、任务拆解，只差实现 → use [`$plan-to-code`](/Users/joe/Documents/skill/skills/plan-to-code/SKILL.md)
- 任务的主问题是高负载执行编排而不是战略成形 → use [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)
- 单点调试、局部改代码、已知根因修复
- 只是想润色一份已有文档，而不是重建方案结构

## Primary operating principle

This skill should behave like a **true strategic control layer**:

1. aggressively fan out bounded research and feasibility work
2. define branch and sidecar structure before checking whether runtime can actually spawn
3. compare branches explicitly instead of thinking in one hidden line
4. keep branch detail out of the main thread
5. synthesize only route choice, decision logic, and handoff in the main thread
6. if subagent spawning is unavailable, preserve the same branch structure in local-supervisor mode

## Main-thread compression contract

The main thread should contain only:

- normalized target
- live branch set and branch status summary
- why one route is promoted or rejected
- handoff summary and next execution owner

Branch notes, evidence tables, pilot outputs, and exploration detail should live in Duo-Doc support artifacts or `.supervisor_strategy.json`.

## Strategic objective

This skill must answer five questions before handoff:

1. **What exactly is the target?**
2. **Which constraints actually matter?**
3. **Which candidate routes are worth testing?**
4. **What evidence is needed before committing?**
5. **What should execution build first, second, and never?**

## Minimum deliverable: Duo-Doc

### Required

- **`outline.md`**
  - motivation
  - context
  - target outcome
  - assumptions
  - chosen strategy
  - rejected alternatives
- **`code_list.md`**
  - language plan
  - code structure
  - data / dependency plan
  - experiment or implementation pipeline
  - milestone slices
  - verification expectations
  - chart / output conventions where relevant

### `code_list.md` language policy

`code_list.md` must make the implementation language explicit instead of leaving it implicit.

- Default implementation languages are **Python**, **Go**, and **Rust**.
- **Mixed-language plans are allowed** when module boundaries and interface contracts are clear.
- **Do not default to R**. Use **R** only when the assignment, rubric, course starter code, required library ecosystem, or evaluation script explicitly requires it.
- If the task only says “you may use R” but does not require it, keep the default language set as **Python / Go / Rust** and justify the chosen one.

Language selection order:

1. **Explicit requirement wins**: assignment, professor, rubric, starter repo, deployment target, and evaluation script override defaults.
2. **Otherwise choose the narrowest fit** from **Python / Go / Rust** instead of leaving the stack ambiguous.
3. **Only choose R with explicit justification**; “allowed” is not the same as “recommended”.

Recommended defaults:

- **Python**: research pipelines, data processing, rapid pilots, notebooks, orchestration, experiment glue code, ML, scripting
- **Go**: services, CLI tools, agent backends, concurrency-heavy control paths, networking, deployment-oriented runtime pieces
- **Rust**: performance-critical kernels, parsers, systems components, memory-safety-sensitive modules, low-level tooling

When `code_list.md` proposes a mixed-language implementation, it must also specify:

- primary language and why it is primary
- per-module language ownership
- interface boundary between languages
- data exchange format or FFI / IPC contract where relevant
- build / test commands for each language slice
- simplification fallback if the mixed stack becomes unnecessary

Prefer a single primary language unless the constraints clearly justify a polyglot design.

Minimum required language fields in `code_list.md`:

- `primary_language`
- `secondary_languages` or `none`
- `language_selection_reason`
- `module_language_map`
- `runtime_or_toolchain_notes`
- `verification_commands`

### Optional support artifacts when complexity justifies

- `pilot_matrix.md` — candidate branches, success criteria, cost, and kill rules
- `decision_log.md` — key branch decisions and why they were made
- `.supervisor_strategy.json` — persistent orchestration state

## duo_doc_schema

A valid Duo-Doc must make execution-ready answers explicit:

- goal
- constraints
- assumptions
- option comparison
- selected route
- rejected routes
- milestones
- validation plan
- dependencies and data sources
- language plan
- stop / kill rules where pilots exist

## Runtime-policy adaptation

Once the branch matrix is valid, derive the delegation plan before checking the runtime branch.

If runtime policy permits delegation:

- attempt real spawning from the pre-built branch and pilot plan
- prefer sidecars for bounded research, repo inspection, feasibility checks, and pilot design
- keep final route choice and Duo-Doc synthesis local

If runtime policy does **not** permit subagent spawning:

- switch to **local-supervisor planning mode**
- keep the same pre-built branch matrix and pilot queue structure
- execute bounded branches sequentially while recording them as branch outputs
- preserve compressed main-thread reporting instead of narrating every branch in chat

The inability to spawn subagents is a runtime constraint, not a reason to abandon strategic orchestration.

## Automation lane

Default to a **semi-automatic planning pipeline**, not free-form brainstorming.

### Required automation moves

1. Normalize the request into:
   - object
   - action
   - constraints
   - deliverable
2. Generate a bounded option set rather than unlimited ideation.
3. Auto-build a **branch comparison matrix**.
4. Auto-identify what can be answered from research vs what needs a Pilot.
5. Auto-build the delegation plan before the runtime branch.
6. Auto-convert the winning route into execution-ready slices.
7. Auto-refresh `.supervisor_strategy.json` for non-trivial planning sessions.

### Strategy state schema

`.supervisor_strategy.json` should persist:

- normalized goal
- assumptions
- branch list
- branch_budget
- evidence status
- delegation_plan_created
- spawn_attempted
- spawn_block_reason
- fallback_mode
- pilot results
- sidecar_outputs_or_local_branch_queue
- chosen route
- rejected routes
- fallback_owner
- handoff status

### Restore and resume

If a planning session resumes, restore:

- active branch set
- evidence already collected
- invalidated branches
- pending pilot decisions
- remaining synthesis tasks

### Stage checkpoints

Recommended default:

- `checkpoint_on_stage_exit: true`
- checkpoint after branch generation
- checkpoint after evidence routing
- checkpoint after each pilot synthesis
- checkpoint before Duo-Doc handoff

### Heartbeat / watchdog

For long or multi-branch planning work, emit a lightweight heartbeat aligned with [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md):

- branch count
- completed evidence items
- active Pilot count or local queue size
- current bottleneck
- next synthesis step

## Automatic skill-routing policy

Default behavior is:

1. route early ideation to the narrowest upstream research skills
2. route evidence collection to research and retrieval skills
3. route bounded pilots to execution owners
4. keep only synthesis, decision, and Duo-Doc handoff local

The strategic layer should prefer **composed skill pipelines** over long free-form thinking in the main thread.

## Subagent orchestration

Use [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) early when parallel sidecars materially improve plan quality.

### Keep local

- final strategy choice
- cross-branch synthesis
- risk tradeoff judgment
- Duo-Doc writing and final handoff

### Good sidecars

- literature / repo / doc scouting for one branch
- feasibility scan for one technical path
- bounded Pilot design for one candidate route
- evidence extraction from one external source or one codebase slice
- cost/risk matrix drafting for one branch

### Do not delegate

- the final strategic decision
- an unbounded “research everything” request
- tightly coupled writing of the final plan across overlapping scopes
- urgent main-thread clarification that blocks all next steps

### Preferred delegation split

- **Explorer**: collect evidence, prior art, repo patterns, interface assumptions
- **Worker**: run one bounded Pilot or implementation probe when a real artifact is needed
- **Main thread**: decide, synthesize, and write the Duo-Doc

## Pilot contract

Any Pilot launched from this skill must define:

- target hypothesis
- bounded scope
- required artifact or measurement
- success criteria
- kill rule
- maximum cost / time budget
- owner skill for execution

### pilot_spec_schema

- hypothesis
- branch_id
- scope
- artifact_expected
- success_criteria
- kill_rule
- max_cost_or_time
- execution_owner
- evidence_required

### branch budget and merge policy

Recommended defaults:

- `branch_budget`: keep only a bounded set of live branches
- `merge_policy`: converge, kill, or promote each branch explicitly
- `fallback_owner`: [`$plan-writing`](/Users/joe/Documents/skill/skills/plan-writing/SKILL.md) for simple planning, [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md) once execution dominates

If these are not known, do **not** launch the Pilot yet.

## Strategy lifecycle

`Intake -> Normalize -> Branch -> Evidence Route -> Pilot -> Synthesis -> Duo-Doc Validation -> Handoff`

## Duo-Doc validation

Before handoff, verify:

- the selected route is explicit
- rejected routes are justified
- the language plan in `code_list.md` is explicit
- `code_list.md` states whether the plan is single-language or mixed-language
- mixed-language plans have concrete boundaries and entrypoints
- milestones are ordered
- validation expectations are stated
- execution does not need to rediscover the plan

## Handoff closure

### handoff_contract

A valid handoff should specify:

- chosen route
- first execution slice
- required owner skill
- risks requiring watch
- validation target
- what not to build yet

## Boundary map

- vague idea with multiple candidate directions → `idea-to-plan`
- simple brainstorming without convergence pressure → [`$brainstorm-research`](/Users/joe/Documents/skill/skills/brainstorm-research/SKILL.md)
- already-known implementation plan → [`$plan-to-code`](/Users/joe/Documents/skill/skills/plan-to-code/SKILL.md)
- execution-heavy orchestration or pilot production at scale → [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)

## Skill synergy matrix

### Upstream / ideation

- [`$brainstorm-research`](/Users/joe/Documents/skill/skills/brainstorm-research/SKILL.md) for divergence when the problem is still too narrow or underexplored

### Evidence / research

- [`$academic-search`](/Users/joe/Documents/skill/skills/academic-search/SKILL.md)
- [`$literature-synthesis`](/Users/joe/Documents/skill/skills/literature-synthesis/SKILL.md)
- [`$information-retrieval`](/Users/joe/Documents/skill/skills/information-retrieval/SKILL.md)

### Pilot / execution

- [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md) for high-load Pilot execution or cross-file validation
- domain implementation skills for bounded proof tasks

### Review / quality

- [`$execution-audit-codex`](/Users/joe/Documents/skill/skills/execution-audit-codex/SKILL.md) for validating Pilot outputs before strategy commitment
- [`$architect-review`](/Users/joe/Documents/skill/skills/architect-review/SKILL.md) when the dominant uncertainty is structural design

## Hard constraints

- Do not confuse ideation abundance with plan quality.
- Do not hand execution a vague or placeholder-heavy `code_list.md`.
- Do not leave the `code_list.md` language choice implicit, especially when Python / Go / Rust / R are all plausible.
- Do not keep dead branches alive after evidence invalidates them.
- Do not delegate the final strategy judgment.
- Do not narrate every branch in the main thread when state files or Duo-Doc artifacts can hold it.
- If the planning task becomes primarily execution-heavy, reroute to [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md).

## Trigger examples

- “把这个模糊想法整理成可执行蓝图。”
- “我现在只有方向，没有计划，帮我做意图到计划。”
- “先做方案探索和 Pilot，再产出 outline.md / code_list.md。”
- “主线程尽量简短，把探索细节下沉。”
