---
name: team
description: |
  Repo-native team workflow for Rust-first supervisor-led delegation and worker lifecycle management.
  It owns team orchestration through local continuity artifacts, tmux-backed worker management, and Rust-owned resume.
routing_layer: L1
routing_owner: owner
routing_gate: delegation
session_start: preferred
trigger_hints:
  - $team
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

`team` 是本仓自有的团队编排流程，直接落到 Rust supervisor、continuity artifacts 和宿主原生 worker 管理，不依赖外部 Claude 插件或旧团队状态目录。

显式入口：
- Codex：`$team`
- Claude：`/team`

## Native Workflow

- 本仓来源：`skills/team/SKILL.md` + `configs/framework/RUNTIME_REGISTRY.json`
- 主流程：scoping -> delegation -> execution -> integration -> qa -> cleanup
- 外部依赖：无外部 Claude 插件、无旧插件状态目录、无插件运行态

## When to use

- 每轮对话开始 / first-turn / conversation start，如果用户明确要进入 `$team` / `/team`，先按 team 入口检查
- 用户明确要 `$team` / `/team`
- 当前任务天然就是 supervisor-led 的多 worker 编排
- worker 生命周期、integration、QA、cleanup、resume/recovery 都是主流程
- 需要多个 bounded worker 持续并行推进，且 supervisor 不能失去控制

## Do not use

- 单线程小修
- 只是设计多 agent 架构，不是现在进入 team 执行态
- 只是判断要不要拆 bounded subagents，这时优先 [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)
- 多个 worker 会共写同一份主 continuity

## Canonical owner

- 主 owner：[`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)

## Delegation Gate

- team split gate：[`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)

## Official Workflow

- 生命周期和恢复状态真相在 `configs/framework/RUNTIME_REGISTRY.json` 与共享 control-plane contracts。
- 当前轮的可执行投影以 Rust `alias.state_machine` 和 `alias.entry_contract` 为准。

## Local runtime

- 不再写旧团队状态目录。
- worker 生命周期由 Rust `session-supervisor`、宿主 worker 管理、resume 机制承接。
- continuity 真相仍写到 `artifacts/current/<task_id>/...`、`SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`、`EVIDENCE_INDEX.json`、`TRACE_METADATA.json`、`.supervisor_state.json`。
- shared continuity 只允许 supervisor 主线程持有；worker 只返回 lane-local 输出或 delta。

## Instructions

1. 先定 supervisor 与 worker 边界，再决定是否派发。
2. shared continuity 只允许 supervisor 写。
3. runtime policy 不允许派发时，退化成 local-supervisor 队列，不放弃 team 逻辑。
4. bounded subagent lane 交给 [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)；需要 supervisor 持续主导时留在 `team` 主线。
5. 集成后必须补验证证据，没有证据不宣布收口。
6. worker 失败或中断时，保留恢复锚点并优先续跑。

## Constraints

- 这是本仓自有编排协议，不是外部插件兼容壳。
- 用本仓 skill、artifact contract、host worker 管理、Rust supervisor 来解释行为。
- 不把 Claude / Codex 的私有 team 行为写成 framework 真相。
- 用户看到的是本仓原生 `team`，不是外部兼容层。
