---
name: agent-swarm-orchestration
description: |
  Design and debug multi-agent systems with planners, routers, workers, reviewers, handoffs, memory, and supervisor logic.
  Use when building agent teams, planner-coder-reviewer loops, task routing systems, swarm architectures, tool delegation, or multi-agent workflows where the challenge is coordination rather than a single prompt. 适用于“多 agent 协作”“agent 编排”“swarm”“orchestrator”“router”“planner-coder-reviewer”“共享记忆”这类请求.
risk: medium
source: community-adapted
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
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
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - agent
    - swarm
    - orchestration
---

- **Dual-Dimension Audit (Pre: Swarm-Graph/Handoff-Logic, Post: Consensus/Task-Completion Results)** → runtime verification gate
# agent-swarm-orchestration

## Overview

这个 skill 用来设计“多个 agent 如何协作”，而不是教单个 agent 怎样写 prompt。

关注点包括：
- 角色划分
- 任务路由
- 状态共享
- 结果验收
- 失败重试
- 人类监督边界

核心原则：
**先把单 agent 做到不混乱，再引入多 agent。**

## When to use

以下情况适合触发：
- 用户要构建 multi-agent system、agent team、swarm、orchestration layer
- 用户要做 planner / coder / reviewer / tester 这类协作链
- 用户要做任务路由、agent handoff、shared memory、consensus、quality gate
- 用户要做 research swarm、support router、自动审查流水线
- 用户要设计 agent supervisor、coordinator、manager-worker 架构

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
- 用户只是要求并行查资料，但没有系统设计需求
- 只是需要一个简单队列 worker 系统，不涉及 agent 协作
- 只是让当前会话直接开 sub-agent 干活，而不是实现一套编排系统

## Primary operating principle

This owner is about **designing multi-agent systems**, not automatically turning the current session into one.

1. separate product-level swarm design from runtime session delegation
2. keep the main thread to architecture, roles, handoffs, and governance
3. when discussing runtime delegation, describe sidecar-ready structure plus local-supervisor fallback
4. avoid implying that current-session spawning is always available

## Main-thread compression contract

The main thread should contain only:

- swarm roles
- handoff graph
- failure / retry / review rules
- shared-state model
- recommendation or design tradeoff

## Runtime-policy adaptation

If the discussion touches current-session execution:

- treat actual spawning as runtime-policy dependent
- preserve local-supervisor fallback as the conceptual downgrade path

## Hard Constraints
- **Superior Quality Audit**: For multi-agent swarm architectures, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples
- "强制进行 Agent 编排深度审计 / 检查协作链路与任务达成结果。"
- "Use the runtime verification gate to audit this agent swarm for orchestration-consensus idealism."

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
