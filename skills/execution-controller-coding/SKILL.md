---
name: execution-controller-coding
description: |
  内核级执行控制器 (Kernel-level Execution Controller)：负责高负载、跨文件、长运行周期任务的自动化编排、状态恢复、skill 路由、subagent 派发与结果集成。
  适用于“系统指挥中心 / SCR / 状态持久化 / 并行 sidecar / 心跳监控 / 回滚 / 跨文件执行 / 主线程极简”。每轮对话开始 / first-turn / conversation start，复杂执行任务必须优先检查此控制层。
routing_layer: L0
routing_owner: "@kernel-controller"
routing_gate: 高负载, 跨文件, 长运行周期, 系统指挥中心, SCR架构, 协同式确定性治理, 内核级控制器, 状态持久化, 纳米级心跳
routing_priority: P0
session_start: required
short_description: Orchestrate complex execution with aggressive routing, state, delegation, and compressed context
framework_roles:
  - orchestrator
  - supervisor
  - integrator
framework_phase: runtime-orchestration
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: true
metadata:
  version: "2.2.0"
  platforms: [codex]
  tags:
    - orchestrator
    - automation
    - routing
    - subagent
    - checkpoint
    - heartbeat
    - rollback
    - state-machine
risk: high
source: local
source_priority: 40
allowed_tools:
  - shell
  - git
  - python
  - node
  - cargo
approval_required_tools:
  - git push
  - gui automation
  - destructive shell
filesystem_scope:
  - repo
  - .supervisor_state.json
  - artifacts
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---

# execution-controller-coding

`execution-controller-coding` is the **kernel-level execution controller** for Codex. It owns the runtime phase where work is no longer just planning, but has become a multi-surface execution problem requiring orchestration, state discipline, bounded delegation, deterministic integration, and strict context compression.

This skill is the **system command layer**, not a domain implementation skill.

## When to use

- 每轮对话开始 / first-turn / conversation start，任务已经进入复杂执行阶段
- 任务是 **高负载 / 跨文件 / 长运行周期 / 多阶段集成 / 多验证面**
- 需要统一调度多个专业 skills，而不是单一 owner 即可解决
- 需要显式维护 `.supervisor_state.json`、checkpoint、heartbeat、watchdog、rollback
- 需要在主线程保持 10% 高阶决策，同时把 90% 细节压到 sidecars、状态文件、artifact 或日志里
- 任务存在可并行 sidecars，且最终集成必须集中控制

## Do not use

- 单一领域单文件实现，直接交给对应专业 skill 即可
- 如果主目标是 APP 全局优化、前后端契约拉齐、UI 升级与测试闭环联动，优先 use [`$execution-controller-app`](/Users/joe/Documents/skill/skills/execution-controller-app/SKILL.md)
- 只是前期战略成形，还没进入执行主导阶段 → use [`$idea-to-plan`](/Users/joe/Documents/skill/skills/idea-to-plan/SKILL.md)
- 根因未知、当前第一任务是找 bug，而不是编排执行 → use [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md)
- 只是小范围直接编码或轻量问答

## Primary operating principle

This controller should behave like a **true master-control layer**:

1. aggressively route work to the narrowest valid skills
2. derive bounded parallel sidecars before runtime branching, then attempt real spawning when runtime policy permits
3. keep the main conversation short, decision-heavy, and free of raw detail
4. sink implementation detail into state files, artifacts, sidecar outputs, and compact checkpoints
5. treat runtime limits as a **degradation condition**, not as a reason to abandon orchestration

## Main-thread compression contract

The main thread should contain only:

- objective and current phase
- why work is being routed or rerouted
- top blockers and decisions
- integration result and next step

The main thread should **not** become a dump for:

- long raw logs
- large code excerpts
- verbose sidecar traces
- duplicated evidence already stored elsewhere

## Input contract

Before dispatching work, normalize the task into:

- terminal goal
- current phase
- scope and forbidden scope
- constraints
- target files or surfaces
- acceptance criteria
- evidence required for completion
- blockers and assumptions

If these are missing, derive them before dispatch.

## Result validation contract

Any delegated or subordinate execution slice must return:

- scope completed
- artifacts changed or produced
- evidence collected
- blockers / risks / follow-ups
- whether the slice is integration-ready
- recommended next step

The main thread integrates summaries, not raw process logs.

## Automatic skill-routing policy

Default behavior is **not** “do everything locally first”. Default behavior is:

1. identify the narrowest valid owner skills
2. route research, implementation, verification, and audit to the correct specialized skills
3. keep only orchestration, integration, and final reroute judgment local

When multiple skills are needed, the controller should compose them into a pipeline instead of expanding the main-thread explanation.

## Runtime-policy adaptation

Once complexity justifies delegation, derive the sidecar plan before checking the runtime branch.

If runtime policy permits delegation:

- attempt real spawning from the pre-built sidecar plan
- prefer sidecars for bounded search, implementation, verification, and artifact inspection
- keep critical-path synthesis local

If runtime policy does **not** permit subagent spawning:

- stay in **local supervisor mode** rather than plain serial mode
- preserve the same pre-built orchestration structure, checkpoints, and state discipline
- run bounded slices sequentially but still summarize them as if they were sidecar returns
- keep details in `.supervisor_state.json`, artifacts, and compact execution notes

The inability to spawn subagents is a **runtime constraint**, not a reason to drop master-control behavior.

## Automation lane

Default to an **automated orchestration loop**, not ad-hoc manual juggling.

### Required automation moves

1. Restore or initialize `.supervisor_state.json`.
2. Normalize the current objective into an execution contract.
3. Check [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) before splitting work.
4. Auto-build a bounded sidecar plan before runtime branching.
5. Attempt real spawning when the runtime allows it; otherwise keep the same plan in local-supervisor mode.
6. Auto-record checkpoints on major phase exits.
7. Auto-run result validation before marking a slice complete.
8. Auto-reroute when the dominant problem changes from execution to debugging, planning, or audit.

### Persistent state schema

Maintain `.supervisor_state.json` with at least:

- task summary
- execution contract
- active phase
- delegation_plan_created
- spawn_attempted
- spawn_block_reason
- fallback_mode
- delegated sidecars
- local-supervisor queue when delegation is unavailable
- completed slices
- open blockers
- artifacts produced
- verification status
- next actions
- rollback point

### Checkpoint rules

Checkpoint at:

- initial restore or initialization
- post-plan / pre-dispatch
- after each completed sidecar integration
- after each local-supervisor slice when delegation is unavailable
- before risky mutations or merges
- before final sign-off

### Heartbeat format

For long-running work, emit a compact heartbeat:

`[progress%] | [elapsed] | [parallel-or-local-queue] | [phase] | [status] | [next]`

Default interval: every 20 minutes for truly long tasks.

### Watchdog and rollback

If progress stalls beyond reasonable expectations:

1. identify the blocker type
2. stop non-productive sidecars or local queue items
3. revert to the last good checkpoint if needed
4. reroute the blocker to the correct owner skill
5. continue from the last valid state instead of restarting blindly

## Subagent policy

Use [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) as the gate for all meaningful parallelization.

### Keep local

- execution contract definition
- critical-path decisions
- state management and checkpointing
- final integration and reroute judgment
- final completion status

### Good sidecars

- bounded repo or docs investigation
- one isolated implementation slice with a clear write set
- one test/verification slice
- one artifact review or runtime evidence collection slice
- one migration or data processing slice with explicit boundaries

### Do not delegate

- the immediate blocker if local resolution is faster
- overlapping write scopes without a clear merge plan
- final synthesis across all slices
- vague “go solve the whole thing” requests

### Preferred sidecar split

- **Explorer**: evidence gathering, repo inspection, source analysis
- **Worker**: bounded implementation or validation slice
- **Main thread**: orchestration, integration, reroute, final status

## Standard pipelines

### Standard delivery chain

`Contract -> Skill Routing -> Delegation Gate -> Dispatch -> Execute -> Validate -> Integrate -> Audit -> Final Check`

### Failure recovery chain

`Error -> Classify -> Reroute or Debug -> Fix -> Re-validate -> Resume`

### Planning handoff chain

`idea-to-plan -> execution-controller-coding -> domain owner -> execution-audit-codex`

## Skill synergy matrix

- [`$idea-to-plan`](/Users/joe/Documents/skill/skills/idea-to-plan/SKILL.md) for pre-execution strategy formation
- [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) for deciding bounded parallelism
- [`$execution-audit-codex`](/Users/joe/Documents/skill/skills/execution-audit-codex/SKILL.md) for strict sign-off
- domain implementation skills for actual build work
- [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md) when the blocker becomes unknown-failure diagnosis

## Hard constraints

- Do not let the main thread become a raw log dump.
- For all subagent dispatches and CLI test runs (like `cargo test`, `npm build`, `git status`) that generate massive output, default to prefixing commands with `rtk` (Rust Token Killer) to aggressively save tokens and preserve context window.
- Do not dispatch sidecars without explicit boundaries.
- Do not merge results without validation.
- Do not lose resumability for long tasks.
- Do not keep executing under the wrong owner once the dominant problem changes.
- Do not let runtime policy limitations collapse the controller into ordinary assistant behavior.

## Trigger examples

- “这个任务要你当系统指挥中心来编排。”
- “这是高负载跨文件执行，帮我做统一调度和状态管理。”
- “需要 SCR、checkpoint、heartbeat、并行 sidecar 和最终集成。”
- “主线程尽量简短，把细节都下沉。”
