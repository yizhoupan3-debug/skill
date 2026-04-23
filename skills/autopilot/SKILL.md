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

`autopilot` 继承 OMC `v4.13.2` 的端到端执行流，但在本仓直接落到 Rust supervisor、continuity artifacts 和验证闭环，不再依赖 `.omc`。

显式入口：
- Codex：`$autopilot`
- Claude：`/autopilot`

## Upstream Baseline

- 官方来源：`oh-my-claudecode` `v4.13.2`
- 对应技能：`skills/autopilot/SKILL.md`
- 主流程：Expansion -> Planning -> Execution -> QA -> Validation -> Cleanup

## When to use

- 用户明确要直接推进到底
- 当前任务以实现、修复、验证、收口为主
- 需要把规格、计划、执行、QA、验收串成一条主线
- first-turn 就已经进入执行态

## Do not use

- 只是 brainstorming，还没进执行态
- 纯 review，没有实现或修复动作
- 只是了解 `autopilot`，不是要现在触发

## Canonical owner

- 主 owner：[`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)

## Workflow

1. `Expansion`
   - 先把需求扩成能执行的 spec。
   - 仍然模糊时，先转 [`$deepinterview`](/Users/joe/Documents/skill/skills/deepinterview/SKILL.md)。
2. `Planning`
   - 把 spec 变成计划和验收路径。
3. `Execution`
   - 做真实实现，必要时拆 sidecar。
4. `QA`
   - 构建、测试、修复，直到信号稳定。
5. `Validation`
   - 用 review 面复核结果。
6. `Cleanup`
   - 清掉运行残留，只保留 continuity 和证据。

## Local runtime

- 不再写 `.omc` 状态。
- 运行态由 Rust `session-supervisor`、background state、resume 承接。
- 任务真相写到 `artifacts/current/<task_id>/...`、`SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`、`EVIDENCE_INDEX.json`、`TRACE_METADATA.json`、`.supervisor_state.json`。

## Instructions

1. 六段流程不要跳，尤其不要省掉 spec、QA、validation。
2. 模糊需求先去 [`$deepinterview`](/Users/joe/Documents/skill/skills/deepinterview/SKILL.md)，不要硬猜。
3. 根因未知先去 [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md)。
4. 只有 task、acceptance、next actions 已够具体时，才留在 [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md) 继续跑。
5. `stale` 先 refresh，`inconsistent` 先 repair，`completed` 就新开 bounded task。
6. 清晰、低风险、可逆的本地步骤自动继续。
7. 并行拆分走 [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)。
8. 强验收时加 [`$execution-audit`](/Users/joe/Documents/skill/skills/execution-audit/SKILL.md)。
9. 没有验证证据，不宣布完成。
10. 中断、限流、挂起都要保留恢复锚点并优先续跑。
11. 同一错误连续重复，升级为根因问题，不机械重试。

## Constraints

- 这是官方能力的本地化，不是自创新协议。
- 用本仓 skill、artifact contract、host entrypoint 解释行为。
- 不把宿主私有行为写成 framework 真相。
- 用户看到的是原生 `autopilot`，不是外部兼容层。
