---
name: autopilot
description: |
  Shared execution alias for the repo's native execution lane.
  It routes into the repo's own execution-controller, planning, debugging, delegation, and verification stack.
routing_layer: L1
routing_owner: owner
routing_gate: delegation
session_start: preferred
trigger_hints:
  - $autopilot
  - /autopilot
  - autopilot
  - 自动推进
  - 一路执行到底
  - 持续跑到收敛
framework_roles:
  - orchestrator
  - alias
framework_phase: runtime-orchestration
framework_contracts:
  emits_execution_items: true
  emits_verification_results: true
metadata:
  version: "1.0.0"
  platforms: [codex, claude]
  tags:
    - autopilot
    - execution
    - alias
    - orchestrator
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

# autopilot

`autopilot` 是共享执行入口，代表本仓自己的连续执行通道。用户用这个名字，就是直接进入本仓的执行控制链。

## When to use

- 每轮对话开始 / first-turn / conversation start，而且这轮已经是执行主导
- 用户想直接推进，不想来回确认
- 任务是执行主导：改代码、修问题、补验证、直到收口
- 需要把计划、调试、委派、验证串成一条连续执行链

## Do not use

- 只是泛泛 brainstorming，没有进入执行态
- 任务本质是纯 review，没有实现/修复动作
- 明显需要单一窄 owner，且不需要执行控制器收口

## Canonical owner

- 主 owner：[`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)

## Reroute rules

- 任务仍模糊：先走 [`$idea-to-plan`](/Users/joe/Documents/skill/skills/idea-to-plan/SKILL.md)
- 根因未知：先走 [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md)
- 可以并行拆 sidecar：加入 [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)
- 进入强验收：加入 [`$execution-audit-codex`](/Users/joe/Documents/skill/skills/execution-audit-codex/SKILL.md)

## Instructions

1. 先定最小成功标准和验证路径。
2. 优先做最小可闭环实现，不为了“自动化”额外扩大设计。
3. 对清晰、低风险、可逆的本地步骤自动继续推进。
4. 需要切换策略时，按上面的 reroute 规则切，不要硬顶。
5. 没有验证证据，不宣布完成。

## Constraints

- 用本仓共享 skill、artifact contract、host entrypoint 来解释行为
- 不把宿主私有行为写成 framework 真相
- 用户看到的是稳定的原生 `autopilot` 能力，不是外部兼容层
