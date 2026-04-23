---
name: deepinterview
description: |
  Official OMC deep-interview workflow, localized onto this repo's evidence-first clarification lane.
  It keeps the original Socratic ambiguity-gated interview model while replacing .omc persistence with local continuity artifacts and Rust-backed resume.
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - $deepinterview
  - deepinterview
  - deep-interview
  - deep interview
  - ouroboros
  - interview me
  - don't assume
  - 深度采访
  - 深度核查
  - 全面review
  - review到收敛
framework_roles:
  - detector
  - alias
  - verifier
framework_phase: review
framework_contracts:
  emits_findings: true
  emits_verification_results: true
metadata:
  version: "1.0.0"
  platforms: [codex, claude]
  tags:
    - deepinterview
    - review
    - alias
    - convergence
risk: low
source: project
allowed_tools:
  - shell
  - git
  - python
approval_required_tools: []
---

# deepinterview

`deepinterview` 继承 OMC `v4.13.2` 的 `deep-interview` 流程：单轮单问、持续量化模糊度、达标后再交给执行；本仓把 `.omc` 状态和 slash pipeline 换成证据、continuity 与 Rust supervisor。

显式入口：
- Codex：`$deepinterview`
- Claude：`/deepinterview`

## Upstream Baseline

- 官方来源：`oh-my-claudecode` `v4.13.2`
- 对应技能：`skills/deep-interview/SKILL.md`
- 主流程：one-question-at-a-time -> target weakest dimension -> score ambiguity each round -> handoff only below threshold

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

- 主 owner：[`$code-review`](/Users/joe/Documents/skill/skills/code-review/SKILL.md)

## Workflow

1. 每轮只问一个问题。
2. 每轮优先打当前最弱的 clarity dimension，不是随便追问。
3. brownfield 场景先查仓库证据，再问用户，不让用户替系统补代码上下文。
4. 每轮回答后都要重新判断模糊度和剩余空洞。
5. 只有当需求足够清晰时，才 handoff 给执行。

## Review Lanes After Clarification

- 架构面：[`$architect-review`](/Users/joe/Documents/skill/skills/architect-review/SKILL.md)
- 安全面：[`$security-audit`](/Users/joe/Documents/skill/skills/security-audit/SKILL.md)
- 测试面：[`$test-engineering`](/Users/joe/Documents/skill/skills/test-engineering/SKILL.md)
- 收敛验收：[`$execution-audit`](/Users/joe/Documents/skill/skills/execution-audit/SKILL.md)

## Local runtime

- 不再写 `.omc/state/deep-interview*.json` 或 `.omc/specs/deep-interview-*.md`。
- 访谈进度和澄清结果写到 `artifacts/current/<task_id>/bootstrap/`、`SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`、`EVIDENCE_INDEX.json`、`TRACE_METADATA.json`、`.supervisor_state.json`。
- 达标后的 handoff 交给本仓 `autopilot` 和 Rust supervisor。

## Instructions

1. 每轮只问一个问题，不批量追问。
2. 根因未知或需求不清时，先做澄清，不急着给结论。
3. brownfield 场景必须先找仓库证据，再问用户确认。
4. 每轮都要明确当前最弱维度，下一问就打这里。
5. 需要进入 review 时，findings 按严重度排，不把 blocker 和 nit 混在一起。
6. 需要修复时，走 review -> fix -> verify 的循环，直到当前有界范围收敛。
7. 引用具体文件、行为或测试证据，不给空泛评价。

## Constraints

- 这是官方能力的本地化，不是自创新协议。
- 用本仓 review skill 和验证证据解释结论。
- 不在 Claude 和 Codex 上分叉 `deepinterview` 的意义。
- 用户看到的是原生 `deepinterview`，不是外部兼容层。
