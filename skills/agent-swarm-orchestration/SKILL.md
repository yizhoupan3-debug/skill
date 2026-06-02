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
  version: "2.0.0"
  platforms: [supported]
  tags:
    - agent
    - swarm
    - orchestration

---

- **Dual-Dimension Audit (Pre: Swarm-Graph/Handoff-Logic, Post: Consensus/Task-Completion Results)** → runtime verification gate
# agent-swarm-orchestration

## Overview

这个 skill 是多 agent 的准入门：先判断任务是否应该留在主线程、使用 bounded sidecars，或退回主线程的 local-supervisor queue。

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
- 用户明确要求多 worker 生命周期、协作拆分或 supervisor 集成时，本 gate 负责判断 bounded sidecars 是否足够；不再新增独立 orchestration owner
- 用户要固定 **review → fix → verify** 多轮闭环（可外加与 review **并行**的 **external research** lane，且大 `max_rounds` 时用 `framework_rfv_loop` 写 `RFV_LOOP_STATE.json`）：契约与模板见 harness 参考 [`adversarial-loop/SKILL.md`](../adversarial-loop/SKILL.md)（**非热 skill 路由**）；用户侧入口优先 [`adversarial-loop/SKILL.md`](../adversarial-loop/SKILL.md)（`$adversarial-loop`）或 My 执行区 `/implementx`（`GOAL_STATE.json`、`framework_goal_drive`）；本 gate 仍负责 spawn admission 与 reject reason

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
6. **code / diff 「深度审稿」宿主可见收口**：并入主线程时**默认用 findings 优先级呈现**（与 [`skills/code-review-deep/SKILL.md`](../code-review-deep/SKILL.md) compact 一致）：全局 severity 排序、少叠床架屋；按需再引用各 sidecar 的 lane 标签，不要为了「看起来专业」复述多段 Lens 前言。

## Spawn Admission

### 声明式模式（推荐）

用户描述意图，runtime 自动选择编排模式。无需手动指定 lane 数量或拓扑。

| 用户意图 | 自动选择模式 | 典型 worker 数 |
|----------|-------------|---------------|
| "全面审查这个模块" | `review-parallel` | 3-5（按维度分 reviewer） |
| "调研并实现这个功能" | `research-then-implement` | 2-3（research + implement） |
| "修复所有 CI 失败" | `fix-parallel` | 按失败数自适应 |
| "对比多种方案" | `judge-panel` | 3-5（独立方案 + 评委） |
| "深度审计安全/性能/正确性" | `multi-lens-review` | 3+（每维度独立 reviewer） |
| "迁移这个旧代码" | `migrate-parallel` | 按模块数自适应 |

**声明式流程**：
1. 用户描述目标（自然语言，无需指定拓扑）
2. Gate 识别意图 → 匹配模式 → 自动设定 worker 数和角色
3. Supervisor 按模式 spawn，无需用户确认 lane 分配
4. 集成时 compact 呈现（findings-first，非逐 lane 汇报）

**模式选择原则**：
- 读重任务（review/research/audit）→ 高并行度（3-5 workers）
- 写重任务（implement/migrate）→ 低并行度（2-3 workers）+ disjoint scope
- 混合任务（research-then-implement）→ 串行阶段 + 并行 lane

### 命令式模式（高级）

当声明式模式不适用（用户明确要求特定拓扑、角色、lane 数）时，使用以下 admission 规则。

Allow bounded sidecars when at least one condition is true:

- read-heavy exploration can run independently
- independent hypotheses or domains can be investigated in parallel
- review or verification can run without blocking the supervisor
- write scopes are fully disjoint and lane-local

For these allowed cases, the supervisor should spawn sidecars promptly and keep local ownership of integration and final verification.

Parallel **review / external research** lanes must stay **read-biased**; **verifier** (or supervisor-run commands) owns **executable** pass/fail. **推理深度**见 [推理深度契约](../../docs/references/rfv-loop/reasoning-depth-contract.md)（分工 + `EVIDENCE_INDEX`，非单模型长 CoT）。Without at least one bounded `verify_commands` (or equivalent hook-visible checks), treat as **`verification_missing`** for write-heavy spawns.

Reject spawning with an explicit reason:

- `small_task`
- `shared_context_heavy`
- `write_scope_overlap`
- `next_step_blocked`
- `verification_missing`
- `token_overhead_dominates`

## Sidecar prompt contract

Sidecars should feel like precise lane workers, not vague assistants.

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
- **Superior Quality Audit**: For multi-agent swarm architectures, apply the runtime verification gate to verify against [Superior Quality Bar / verification gate criteria](../SKILL_FRAMEWORK_PROTOCOLS.md#4-runtime-protocol).

## Hooks 集成

以下 hook 事件对 agent 编排最有价值：

### 子代理生命周期
- SubagentStart / SubagentStop：监控子代理启动和终止
- 可用于审计、资源管理、依赖追踪

### 上下文管理
- PreCompact / PostCompact：上下文压缩前后的干预
- PreCompact 时序列化关键状态到文件，PostCompact 时恢复
- 对长 session 的 task 连续性至关重要

### 智能拦截
- Prompt-based hooks：用 prompt 作为 hook 处理器，实现语义级拦截
- Agent-based hooks：用完整 agent 作为 hook 处理器，执行多步验证
- 注意：这些能力的可用性因宿主而异；跨宿主场景需降级为手动流程

### 文件监控
- FileChanged + async hook：文件变更后自动运行测试/lint，不阻塞主流程

### 设计约束
- 保持宿主无关性：Hooks 是宿主特有能力，跨宿主场景需降级为手动流程
- 提取编排模式（依赖管理、并行执行、结果汇总）但保持平台中立

## Trigger examples
- "强制进行 Agent 编排深度审计 / 检查协作链路与任务达成结果。"
- "Use the runtime verification gate to audit this agent swarm for orchestration-consensus idealism."

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
