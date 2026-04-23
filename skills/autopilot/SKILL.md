---
name: autopilot
description: |
  Official OMC autopilot workflow, localized onto this repo's native execution lane and Rust supervisor.
  It keeps the original end-to-end execution pipeline while replacing .omc state with local continuity artifacts and rust-session-supervisor.
routing_layer: L1
routing_owner: owner
routing_gate: delegation
session_start: preferred
trigger_hints:
  - $autopilot
  - /autopilot
  - autopilot
  - auto pilot
  - auto-pilot
  - full auto
  - fullsend
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

`autopilot` 现在按 OMC 官方 `v4.13.2` 的 `autopilot` 技能骨架来跑，再做本地 Rust 化和仓内落地。也就是说，核心能力不是我自己想的，而是直接继承官方的“从想法到可验证代码”的全流程自动推进；本仓只把 `.omc` 状态面换成了你仓自己的 Rust supervisor、continuity artifacts 和验证闭环。

## Upstream Baseline

- 官方来源：`oh-my-claudecode` `v4.13.2`
- 对应技能：`skills/autopilot/SKILL.md`
- 继承的原版主流程：Expansion -> Planning -> Execution -> QA -> Validation -> Cleanup

## When to use

- 每轮对话开始 / first-turn / conversation start，而且这轮已经是执行主导
- 用户想直接推进，不想来回确认
- 任务是执行主导：改代码、修问题、补验证、直到收口
- 需要把计划、调试、委派、验证串成一条连续执行链
- 用户已经给了产品想法、项目目标或一段可落地需求，希望直接端到端做完

## Do not use

- 只是泛泛 brainstorming，没有进入执行态
- 任务本质是纯 review，没有实现/修复动作
- 明显需要单一窄 owner，且不需要执行控制器收口
- 用户只是想了解 `autopilot` 是什么或怎么用，而不是现在就触发它

## Canonical owner

- 主 owner：[`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)

## Official Workflow

1. `Expansion`
   - 把简短需求扩成清晰 spec。
   - 如果输入仍然很虚，优先转给 [`$deepinterview`](/Users/joe/Documents/skill/skills/deepinterview/SKILL.md) 先做澄清。
2. `Planning`
   - 把 spec 变成可执行计划和验收路径。
3. `Execution`
   - 进入真实代码实现，必要时并行拆 sidecar。
4. `QA`
   - 构建、测试、修复，直到测试信号稳定。
5. `Validation`
   - 用架构、安全、代码质量、执行验收几条 review 面做复核。
6. `Cleanup`
   - 结束时清理运行态残留，只保留 continuity 和证据。

## Local Replacements

- 不再写 `.omc/state/autopilot-state.json` 这类官方状态文件。
- 运行态改由 Rust `session-supervisor`、background state 和 resume 机制承接。
- 规格、计划、证据改写入：
  - `artifacts/current/<task_id>/bootstrap/`
  - `artifacts/current/<task_id>/evidence/`
  - `SESSION_SUMMARY.md`
  - `NEXT_ACTIONS.json`
  - `EVIDENCE_INDEX.json`
  - `TRACE_METADATA.json`
  - `.supervisor_state.json`

## Instructions

1. 按官方 `autopilot` 的 6 段流程推进，不跳过中间的 spec、计划、QA、validation。
2. 如果任务仍模糊，先走 [`$deepinterview`](/Users/joe/Documents/skill/skills/deepinterview/SKILL.md) 做澄清，不要硬扩需求。
3. 如果根因未知，先走 [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md)，再回到执行主线。
4. 对清晰、低风险、可逆的本地步骤自动继续推进。
5. 需要并行拆分时，使用 [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)。
6. 进入强验收时，加入 [`$execution-audit`](/Users/joe/Documents/skill/skills/execution-audit/SKILL.md)。
7. 没有验证证据，不宣布完成。
8. 如果执行被打断、限流或挂起，必须保留恢复锚点并优先续跑，不把中断当完成。
9. 如果同一错误连续重复，必须上升为根因问题处理，而不是机械重试。

## Constraints

- 这是“复用官方实现再本地化”，不是自创新协议。
- 用本仓共享 skill、artifact contract、host entrypoint 来解释行为。
- 不把宿主私有行为写成 framework 真相。
- 用户看到的是稳定的原生 `autopilot` 能力，不是外部兼容层。
- 必须做到“承接 OMC 核心能力，但实现标准更强”，至少强制根因定位、验证证据、恢复续跑、收敛闭环。
