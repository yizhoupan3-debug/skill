---
name: agent-swarm-orchestration
description: |
  Decide whether work should stay local, use bounded sidecars, or escalate to team orchestration.
  Also design and debug multi-agent systems when the real problem is coordination, handoff, worker boundaries, or supervisor logic. 适用于“多 agent 协作”“agent 编排”“swarm”“orchestrator”“router”“planner-coder-reviewer”“共享记忆”这类请求.
risk: medium
source: community-adapted
routing_layer: L0
routing_owner: gate
routing_gate: delegation
routing_priority: P1
session_start: required
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 多 agent 协作
  - agent 编排
  - swarm architecture
  - agent orchestrator
  - task routing system
  - planner-coder-reviewer
  - shared agent memory
  - agent supervisor
  - multi-agent workflow
  - 多 agent 执行
metadata:
  version: "1.1.1"
  platforms: [codex]
  tags:
    - agent
    - swarm
    - orchestration

---

- **Dual-Dimension Audit (Pre: Swarm-Graph/Handoff-Logic, Post: Consensus/Task-Completion Results)** → runtime verification gate
# agent-swarm-orchestration

## Overview

这个 skill 是多 agent 的准入门：先判断任务是否应该留在主线程、使用 bounded sidecars，还是升级到 `$team` / `/team` 的完整生命周期编排。

关注点包括：
- spawn admission
- 角色划分
- 任务路由
- 状态共享
- 结果验收
- 失败重试
- 人类监督边界

核心原则：
**默认先做 spawn admission；当任务存在清晰、独立、可验证的并行 lane 时，优先启动 bounded sidecars。只有边界、验证或关键路径不清晰时才拒绝。**

## When to use

以下情况适合触发：
- 当前任务需要判断是否允许 subagent / sidecar / worker delegation
- 任务是 read-heavy exploration，且多个方向可以独立并行
- 任务有多个独立假设、独立模块或独立验证维度
- review / verification 可以和主线程实现并行，且不会阻塞下一步
- 深度 / 全面 / 全仓 / 跨模块 review 明显包含多个独立审查维度时，先进入 subagent admission；适合则开启 reviewer sidecars
- 写入范围完全 disjoint，worker 只产出 lane-local delta
- 用户要构建 multi-agent system、agent team、swarm、orchestration layer
- 用户要做 planner / coder / reviewer / tester 这类协作链
- 用户要做任务路由、agent handoff、shared memory、consensus、quality gate
- 用户要做 research swarm、support router、自动审查流水线
- 用户要设计 agent supervisor、coordinator、manager-worker 架构
- 用户明确要求 full team orchestration 而不是普通 bounded sidecars；显式 `$team` / `/team` 入口只归 `team` 命令本身，不由本 gate 抢占
- 用户要固定 **review → fix → verify** 多轮闭环（可外加与 review **并行**的 **external research** lane，且大 `max_rounds` 时用 `framework_rfv_loop` 写 `RFV_LOOP_STATE.json`）：用 [`review-fix-verify-loop`](../review-fix-verify-loop/SKILL.md)（`$review-fix-verify-loop`）承载契约与模板；本 gate 仍负责 spawn admission 与 reject reason

常见表达：
- “做一个多 agent 协作框架”
- “让几个 agent 分工合作”
- “实现 planner-coder-reviewer 流水线”
- “做任务路由器和 orchestrator”
- “做 agent supervisor”
- “设计 swarm architecture”

## Do not use

以下情况不要触发：
- 普通单 agent 编码任务
- 用户只是想让你更认真一点，不是真的要多 agent
- 小而紧耦合的改动，主线程直接做更快
- 子任务结果会阻塞主线程的下一步判断
- 多个 worker 会改同一文件、同一模块或共享隐含上下文
- 没有明确验证方式，或 worker 只能自报完成
- token / 协调成本明显高于串行执行
- 只是需要一个简单队列 worker 系统，不涉及 agent 协作
- 只是让当前会话直接开 sub-agent 干活，而不是实现一套编排系统

## Primary operating principle

This gate is about **admitting delegation when bounded parallelism beats local execution**, not automatically turning the current session into a full team.

1. spawn bounded sidecars by default when read-heavy, review, verification, or independent implementation lanes are clear
2. prefer read-only sidecars before write-capable workers
3. allow write delegation only for disjoint, lane-local scopes
4. for broad reviews, split independent reviewer lanes when the lane boundaries are clear
5. fall back to local-supervisor queue when spawning is blocked or not worth it

## Spawn Admission

Allow bounded sidecars when at least one condition is true:

- read-heavy exploration can run independently
- independent hypotheses or domains can be investigated in parallel
- review or verification can run without blocking the supervisor
- write scopes are fully disjoint and lane-local

For these allowed cases, the supervisor should spawn sidecars promptly and keep local ownership of integration and final verification.

Reject spawning with an explicit reason:

- `small_task`
- `shared_context_heavy`
- `write_scope_overlap`
- `next_step_blocked`
- `verification_missing`
- `token_overhead_dominates`

## Codex sidecar prompt contract

Codex sidecars should feel like precise lane workers, not vague assistants.

Use `fork_context=false` by default and pass only:

- repo path and relevant files / diff / command target
- lane goal and why it can run independently
- exact bounded scope and forbidden scope
- expected output shape
- verification or evidence requirement
- reminder that the sidecar is not alone in the codebase and must not revert unrelated edits

Prefer spawning multiple independent read-only explorers in the same round when the task has parallel research, audit, risk, or verification lanes. For write-capable workers, assign disjoint ownership up front and require a final answer with changed files, evidence, verification, risk, and next action.

Do not hand a sidecar the immediate blocker. The supervisor should continue useful non-overlapping work locally while sidecars run, then integrate rather than redoing their work.

Worker summaries should stay compressed to:

- `changed_files`
- `evidence`
- `verification`
- `risk`
- `next_action`

## Main-thread compression contract

The main thread should contain only:

- admission decision
- reject reason or allowed lane split
- file / scope ownership
- verification evidence
- supervisor next action

## Runtime-policy adaptation

If the discussion touches current-session execution:

- treat actual spawning as runtime-policy dependent
- preserve local-supervisor fallback as the conceptual downgrade path
- never delegate the immediate blocker on the critical path

## Hard Constraints
- Do not create a new agent role, mailbox, graph, or state artifact unless an existing `team` / lane contract cannot express the need.
- Do not let workers write outside their assigned lane-local scope.
- Supervisor owns integration and final verification.
- **Superior Quality Audit**: For multi-agent swarm architectures, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples
- "强制进行 Agent 编排深度审计 / 检查协作链路与任务达成结果。"
- "Use the runtime verification gate to audit this agent swarm for orchestration-consensus idealism."

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).

For explicit `$team` / `/team` alias handling, see [references/team-mode.md](./references/team-mode.md).
