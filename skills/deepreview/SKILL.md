---
name: deepreview
description: |
  Shared review alias for the repo's native review lane.
  It routes into the repo's own code-review, architecture, security, test, and convergence lanes.
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - $deepreview
  - /deepreview
  - deepreview
  - 深度审查
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
    - deepreview
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

# deepreview

`deepreview` 是共享审查入口，代表本仓自己的严格 review 通道。用户用这个名字，就是直接进入 findings 优先的 review 体系。

## When to use

- 用户要全面 review、严格 review、review 到收敛
- 需要 findings 优先，而不是先讲实现细节
- 需要把代码、架构、安全、测试几个 review 面串起来

## Do not use

- 任务只是单纯写代码，没有 review 目标
- 只是代码风格统一，不需要 findings 驱动
- 只想要轻量建议，不想进入严格审查流程

## Canonical owner

- 主 owner：[`$code-review`](/Users/joe/Documents/skill/skills/code-review/SKILL.md)

## Review lanes

- 架构面：[`$architect-review`](/Users/joe/Documents/skill/skills/architect-review/SKILL.md)
- 安全面：[`$security-audit`](/Users/joe/Documents/skill/skills/security-audit/SKILL.md)
- 测试面：[`$test-engineering`](/Users/joe/Documents/skill/skills/test-engineering/SKILL.md)
- 收敛验收：[`$execution-audit-codex`](/Users/joe/Documents/skill/skills/execution-audit-codex/SKILL.md)

## Instructions

1. 先给 findings，再给简短结论。
2. findings 按严重度排，不把 blocker 和 nit 混在一起。
3. 需要修复时，走 review -> fix -> verify 的循环，直到当前有界范围收敛。
4. 引用具体文件、行为或测试证据，不给空泛评价。

## Constraints

- 用本仓共享 review skill 和验证证据来解释结论
- 不在 Claude 和 Codex 上分叉 `deepreview` 的意义
- 用户看到的是稳定的原生 `deepreview` 能力，不是外部兼容层
