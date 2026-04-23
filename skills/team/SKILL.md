---
name: team
description: |
  Official OMC team workflow, localized onto this repo's Rust-first supervisor and delegation lane.
  It keeps the original team orchestration intent while replacing .omc team state with local continuity artifacts, tmux-backed worker management, and Rust-owned resume.
routing_layer: L1
routing_owner: owner
routing_gate: delegation
session_start: preferred
trigger_hints:
  - $team
  - /team
  - team
  - team mode
  - agent team
  - multi agent execution
  - worker orchestration
  - 团队编排
  - 多 agent 执行
  - 多工并行
framework_roles:
  - orchestrator
  - alias
  - supervisor
framework_phase: runtime-orchestration
framework_contracts:
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: true
metadata:
  version: "1.0.0"
  platforms: [codex, claude]
  tags:
    - team
    - alias
    - orchestration
    - delegation
    - supervisor
risk: medium
source: project
allowed_tools:
  - shell
  - git
  - python
  - cargo
approval_required_tools:
  - destructive shell
---

# team

`team` 现在按 OMC 官方 `v4.13.2` 的 `team` 能力意图来跑，再做本地 Rust 化和仓内落地。它不是沿用 `.omc` 团队状态目录的兼容壳，而是把“任务拆分、worker 生命周期、恢复续跑、结果收口”统一接到本仓的 Rust supervisor、continuity artifacts 和宿主原生 worker 管理能力上。

## Upstream Baseline

- 官方来源：`oh-my-claudecode` `v4.13.2`
- 对应能力：`team`
- 继承的核心目标：scoping -> delegation -> execution -> integration -> qa -> cleanup

## When to use

- 每轮对话开始 / first-turn / conversation start，而且用户明确要 `/team` 或 `$team`
- 当前任务需要多个 bounded worker / sidecar 并行推进
- 需要把拆分、执行、集成、验收压进同一条 supervisor 主线
- 需要 worker 生命周期和恢复锚点都可追踪，而不是一次性并发后失控
- 用户想保留多 agent / team 的执行体验，但底层必须是本仓的 Rust-first runtime

## Do not use

- 任务其实是单线程小修，不需要 team orchestration
- 只是设计多 agent 架构，不是要在当前仓库里进入 team 执行态
- 只是要决定是否拆 sidecar，但还没进入 team 运行主线
- 多个 worker 会互相重叠写同一份 continuity 主文件

## Canonical owner

- 主 owner：[`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)

## Delegation Gate

- team split gate：[`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)

## Official Workflow

1. `Scoping`
   - 先确定什么必须留在 supervisor 主线程，什么可以拆成 bounded worker。
2. `Delegation`
   - 先定义 worker 边界、输入输出和 forbidden scope，再决定是否真实派发。
3. `Execution`
   - worker 按 lane 执行，主线程保留 orchestration 和关键判断。
4. `Integration`
   - 回收各 worker 输出，统一合并结果和证据。
5. `QA`
   - 对集成结果做构建、测试、回归和必要修复。
6. `Cleanup`
   - 清理运行态残留，只保留 continuity、证据和可恢复锚点。

## Local Replacements

- 不再写 `.omc/state/team*.json` 或其他 OMC team 状态目录。
- worker 生命周期改由 Rust `session-supervisor`、宿主 tmux worker 管理和 resume 机制承接。
- continuity 仍写入：
  - `artifacts/current/<task_id>/bootstrap/`
  - `artifacts/current/<task_id>/evidence/`
  - `SESSION_SUMMARY.md`
  - `NEXT_ACTIONS.json`
  - `EVIDENCE_INDEX.json`
  - `TRACE_METADATA.json`
  - `.supervisor_state.json`
- shared continuity 只允许 supervisor 主线程持有，worker 只返回 lane-local 输出或 delta。

## Instructions

1. 先定主线程与 worker 的边界，再决定是否真实派发。
2. shared continuity 只允许 supervisor 写，worker 不准直接共写主 continuity 文件。
3. 如果 runtime policy 不允许派发，保留同样的 team 结构并退化成 local-supervisor 队列，而不是放弃 team 逻辑。
4. 需要拆分时，优先走 [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) 做 bounded split。
5. 集成后必须补验证证据，没有验证证据不宣布 team 收口完成。
6. 如果 worker 失败或中断，必须保留恢复锚点并优先续跑，不把中断当完成。

## Constraints

- 这是“复用官方实现再本地化”，不是继续依赖 `.omc` 团队状态。
- 用本仓共享 skill、artifact contract、host worker 管理和 Rust supervisor 来解释行为。
- 不把 Claude / Codex 某个宿主的私有 team 行为写成 framework 真相。
- 用户看到的是稳定的原生 `team` 能力，不是外部兼容层。
- 必须做到“承接 OMC 核心能力，但实现标准更强”，至少强制 worker 边界、恢复续跑、supervisor 单写 continuity、验证收口。
