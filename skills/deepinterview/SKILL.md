---

allowed_tools:
- shell
- git
- python
description: Native repo deep-interview workflow for evidence-first clarification and convergence review.
metadata:
  platforms:
  - supported
  tags:
  - deepinterview
  - review
  - alias
  - convergence
  version: '1.0.0'
name: deepinterview
scene: general
risk: low
routing_gate: none
routing_layer: L1
routing_owner: owner
routing_priority: P2
session_start: n/a
source: runtime
trigger_hints:
- /deepinterview
- 先问清楚
- 深度采访
- 澄清需求
---
# deepinterview

`deepinterview` 是本仓自有的深度澄清流程：单轮单问、持续量化模糊度、达标后再交给执行；状态和 handoff 都走证据、continuity 与 Rust supervisor。

显式入口：`/deepinterview`

## Native Workflow

- 本仓来源：`skills/deepinterview/SKILL.md`
- 主流程：one-question-at-a-time -> target weakest dimension -> score ambiguity each round -> handoff only below threshold
- 外部依赖：无外部插件、无旧插件状态目录、无插件运行态

## When to use

- 用户要严格 review、review 到收敛
- 用户要深度采访、深度核查、深挖根因
- findings 必须优先
- 需要把代码、架构、安全、测试几个 review 面串起来
- 需求还虚，不想让执行阶段浪费在猜需求上

## Do not use

- 单纯写代码，没有 review 目标
- 只是代码风格统一，不需要 findings 驱动
- 只想要轻量建议
- 已经给了明确文件、函数、验收标准，应该直接执行

## Canonical owner

- 主 owner：`deepinterview` 只负责澄清、收敛和 findings-first handoff。
- 达到清晰阈值后，交给当前任务的最窄 skill owner（通过当前会话上下文切换）。
- 若需要多 review 面并行取证，先走 [`agent-swarm-orchestration`](../agent-swarm-orchestration/SKILL.md) 判断 bounded sidecar 边界。

## Workflow

1. 每轮只问一个问题。
2. 每轮优先打当前最弱的 clarity dimension，不是随便追问。
3. brownfield 场景先查仓库证据，再问用户，不让用户替系统补代码上下文。
4. 每轮回答后都要重新判断模糊度和剩余空洞。
5. 只有当需求足够清晰时，才 handoff 给执行。

## Review Lanes After Clarification

- 架构、测试、安全或实现风险：留在 native runtime 中做证据优先 review，必要时在当前上下文做系统化调试查未知失败。
- 多面向 review：用 [`agent-swarm-orchestration`](../agent-swarm-orchestration/SKILL.md) 先决定是否拆 bounded sidecar；不要引用不存在的 reviewer lane。
- 产物或界面 review：按 artifact/evidence gate 进入 [`visual-review`](../visual-review/SKILL.md)、[`pdf`](../pdf/SKILL.md)、[`doc`](../doc/SKILL.md)、[`slides`](../slides/SKILL.md) 或 [`spreadsheets`](../spreadsheets/SKILL.md)。
- 收敛验收：以本轮实际测试、diff、截图、生成产物或明确 blocker 为准。

## Local runtime

- 不再写旧插件状态或旧插件 spec。
- 访谈进度和澄清结果写到 `skills/deepinterview/artifacts/bootstrap/` 及 `session_state.json`。
- 达标后的 handoff 交给当前任务的最窄 skill owner（通过当前会话的上下文切换）。

## Instructions

1. 每轮只问一个问题，不批量追问。
2. 根因未知或需求不清时，先做澄清，不急着给结论。
3. brownfield 场景必须先找仓库证据，再问用户确认。
4. 每轮都要明确当前最弱维度，下一问就打这里。
5. 需要进入 review 时，findings 按严重度排，不把 blocker 和 nit 混在一起。
6. 需要修复时，走 review -> fix -> verify 的循环，直到当前有界范围收敛。
7. 引用具体文件、行为或测试证据，不给空泛评价。

## Constraints

- 这是本仓自有澄清协议，不是外部插件兼容壳。
- 用本仓 review skill 和验证证据解释结论。
- 不在不同 host 上分叉 `deepinterview` 的意义。
- 用户看到的是本仓原生 `deepinterview`，不是外部兼容层。

## Hard constraints

- 每轮只问一个问题——不得批量追问或将多个问题混在同一轮
- 模糊度评分必须在每轮回答后更新，不得跳过评分直接进入下一问
- brownfield 场景必须先查仓库证据再问用户，不得让用户替系统补代码上下文
- handoff 给执行前，模糊度必须低于阈值——需求不清时不得强制推进
- findings 必须按严重度排序（blocker > warning > nit），不得混排
- 引用具体文件/函数/测试证据，不得给空泛评价
