---
name: execution-controller-coding
description: |
  内核级执行控制器 (Kernel-level Execution Controller)：负责高负载、跨文件、长运行周期任务的自动化编排、状态恢复、skill 路由、subagent 派发与结果集成。
  它也是仓库内 get-shit-done / gsd 执行姿态的主 owner：自动推进安全本地步骤，压缩主线程，直到有真实验证证据。
  适用于“系统指挥中心 / SCR / 状态持久化 / 并行 sidecar / 心跳监控 / 回滚 / 跨文件执行 / 主线程极简”。每轮对话开始 / first-turn / conversation start，复杂执行任务必须优先检查此控制层。
routing_layer: L0
routing_owner: "@kernel-controller"
routing_gate: delegation
routing_priority: P0
session_start: required
short_description: Orchestrate complex execution with aggressive routing, state, delegation, and compressed context
trigger_hints:
  - 高负载
  - 跨文件
  - 长运行周期
  - 系统指挥中心
  - SCR架构
  - 协同式确定性治理
  - 内核级控制器
  - 状态持久化
  - 纳米级心跳
  - gsd
  - get shit done
  - 推进到底
  - 直接干完
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
  version: "2.5.0"
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
    - gsd
    - get-shit-done
risk: high
source: local
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
  - TRACE_METADATA.json
bridge_behavior: mobile_complete_once
---

# execution-controller-coding

`execution-controller-coding` is the kernel-level owner for complex execution once the task is already in build, fix, integrate, or verify mode. It is an orchestration skill, not a domain implementation skill.

## When to use

- 每轮对话开始 / first-turn / conversation start，任务已经进入执行主导阶段
- 高负载、跨文件、长运行、多阶段集成、多验证面并存
- 需要显式维护 `.supervisor_state.json` 和 continuity artifacts
- 并行 lane 存在共享状态冲突风险，需要由单一控制器收口 continuity 写入
- 主线程必须只保留决策与集成，细节需要下沉到 sidecars、state、artifacts
- 用户明确要求 `gsd` / `get shit done` / “推进到底” / “别停”

## Do not use

- 单文件或单领域任务，直接交给窄 owner
- 任务还在战略成形期 → [`$idea-to-plan`](/Users/joe/Documents/skill/skills/idea-to-plan/SKILL.md)
- 根因未知且第一任务是取证 → [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md)
- APP 全局协调优先 → [`$execution-controller-app`](/Users/joe/Documents/skill/skills/execution-controller-app/SKILL.md)

## GSD posture

- `gsd` 不是独立运行时，而是这个 controller 的强执行姿态。
- 默认自动继续清晰、低风险、可逆的本地 edit/test/verify 链路，不做无意义权限交还。
- 强执行不等于盲猜：如果不同解释会明显改变方案、风险或改动面，必须先显式化。
- 强执行不等于做大：默认走最小可行路线，不为“以后也许会用到”提前加层、加开关、加抽象。
- 强执行不等于大 diff：改动面要能逐条追溯到当前目标或必要 fallout。
- 主线程只报告决策、证据和 blocker，不复述大量过程。
- 没有验证证据就不宣告完成。
- 卡住时优先换招，不优先请求人工代劳。

## Output Contract

签收前至少保持这几份产物一致：

- `SESSION_SUMMARY.md`
- `NEXT_ACTIONS.json`
- `EVIDENCE_INDEX.json`
- `TRACE_METADATA.json`
- `.supervisor_state.json`

非平凡执行在开工前还要先定清：

- success criteria：什么现象算完成
- verification path：用什么测试/命令/证据验收
- minimum route：先走哪条最小实现路径

连续性写入规则：

- 这些文件是 **supervisor-only** 全局写面
- sidecar / worker / local-supervisor queue item 不得直接改写这些共享文件
- 并行 lane 只能写 lane-local artifacts 或 delta payload，等待 integrator 合并
- 只有集成步骤才能刷新全局 continuity artifacts

主线程只保留：

- objective / current phase
- active assumptions that materially affect execution
- reroute or delegation decision
- top blockers
- integration result
- next step

## Reroute Rules

- 未知根因优先：reroute to `$systematic-debugging`
- 方案仍模糊：reroute to `$idea-to-plan`
- 已有 checklist / phase queue / execution blueprint，但 serial/parallel 边界、scope、acceptance、或 update contract 仍不稳定：reroute to `$checklist-normalizer`
- 进入强验收：add `$execution-audit-codex`
- 能并行且边界清晰：先过 `$subagent-delegation`

## References

- [references/runtime-playbook.md](references/runtime-playbook.md)
