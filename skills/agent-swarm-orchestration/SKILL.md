---
description: Decide whether work should stay local, use bounded sidecars, or escalate to workflow orchestration. Also design and debug multi-agent systems when the real problem is coordination, handoff, worker boundar
metadata:
  platforms:
  - supported
  tags:
  - agent
  - team
  - orchestration
  version: '4.0.0'
name: agent-swarm-orchestration
risk: medium
routing_gate: delegation
routing_layer: L0
routing_owner: gate
routing_priority: P1
session_start: required
source: project
trigger_hints:
- /workflow
- agent orchestrator
- agent supervisor
- agent 编排
- multi-agent workflow
- planner-coder-reviewer
- shared agent memory
- swarm architecture
- task routing system
- 多 agent 协作
- 多 agent 执行
---
## Quick Ref
- **Purpose**: 多 agent 编排选择——local / sidecar / team 模式，判定 spawn admission 与团队执行面
- **Key Rules**: 默认保守不自动升格 team；3 模式优先级 explicit_team > auto_multi_phase > sidecar > local；spawn 需满足 admission 条件；**team 为多 agent 编排的真源，替代已移除的 workflow**
- **Trigger**: "多 agent 协作"、"team 编排"、"swarm"、"planner-coder-reviewer"
<!-- full content below; load on demand -->

- **Dual-Dimension Audit (Pre: Swarm-Graph/Handoff-Logic, Post: Consensus/Task-Completion Results)** → runtime verification gate
# agent-swarm-orchestration

## Overview

这个 skill 是多 agent 编排的准入门：先选择编排 **模式**（local / sidecar / team），再判断 spawn admission 与宿主执行面。

关注点包括：
- spawn admission
- 角色划分
- 团队所有权
- 状态共享
- agent 间通信
- 结果验收
- 失败重试
- 人类监督边界

核心原则：
**默认保守**：不自动升格 team。满足 team 触发后，编排真源为 `session-supervisor`（Rust team 层）与 `artifacts/teams/` 文件系统；否则先做 spawn admission，在清晰并行 lane 时优先 bounded sidecars。

## Orchestration mode selection

Gate **必须先**输出一行机读决策（可附人读半句），再 spawn：

```text
orchestration: { mode, trigger, reason }
```

| `mode` | 何时 | 执行面 |
|--------|------|--------|
| `local` | 小任务、紧耦合、拒绝规则命中 | 主线程 |
| `sidecar` | 有清晰并行 lane，**未**触发 team | Task / subagent（声明式表） |
| `team` | team 触发 + 有 supervisor agent | `session-supervisor` team API（`artifacts/teams/`）；成员 agent 通过 `team_send_message` / `team_read_messages` 通信 |

**优先级（HARD）**：`explicit_team` > `auto_multi_phase` > 声明式 `sidecar` > `local`。

## Team triggers

### Explicit（强制 team）

用户当轮出现任一即 `trigger: explicit`：

- 关键词：`/team`、`team 编排`、`团队执行`、`多 agent 团队`、`用 team`

### Auto multi-phase（自动 team）

未显式提 team，但任务描述满足 **任一** → `trigger: auto_multi_phase`：

- **≥3 个命名阶段**（Phase 1/2/3、第一步/第二步…、或清单式阶段标题）
- **管道型流程**（含 `→`、Scan→Verify、先…再…再…）
- **串行 + 并行混合**（例如「多维度串行扫描后批量验证」）

不满足以上且不满足 explicit → **不得** team，走 sidecar 声明式表或 local。

## Main-thread contract (team)

当 `mode` 为 `team` 时（HARD）：

1. **主线程为 supervisor**：admission、创建 team、添加成员、推进阶段、异常升级、最终集成；**禁止**在聊天中展开子 agent 全文或大量中间 findings。
2. **每阶段结束**：写 `artifacts/teams/<team_id>/team.json`（更新状态 + 阶段计数）。
3. **Merge / Synthesize**：supervisor 主线程纯代码，**不**为 Merge/Synthesize spawn agent。
4. **可见收口**：findings-first（对齐 [`code-review-deep`](../code-review-deep/SKILL.md) compact）；admission 一行 + 每阶段一行进度。
5. **Agent 间通信**：通过 `team_send_message`（文件系统消息总线）传递，supervisor 监控消息队列。中间态进 `artifacts/teams/<team_id>/messages/`，不进主 context。
6. **生命周期清理**：成员完成时 `team_remove_member` → 自动写 `agent_unregister`；所有成员完成或 team 终止时 `team_complete`。
7. **Workflow 已彻底移除**：不再使用 JS 编排脚本。所有多 agent 协作统一使用 team 模型。

Sidecar 模式的压缩契约仍见下文 **Main-thread compression contract**。

## Host capabilities (orchestration)

所有宿主通过 team API / MCP 工具 / stdio 三种方式之一接入 team 编排能力，具体宿主→接入方式映射见 `RUNTIME_REGISTRY.json`。

所有宿主共享统一 team 文件系统（`artifacts/teams/`），互操作不受限。

## Orchestration boundary

| 入口 | 职责 |
|------|------|
| **本 skill** | 模式选择、team 触发、spawn admission、team/sidecar 编排形态 |
| **owner skill** | 具体任务执行（实现、验证等由最窄 skill owner 负责） |

## 深度对抗 review：选型表（HARD）

| 场景 | 推荐入口 | 执行面 | 对抗 Verify |
|------|----------|--------|-------------|
| 多 agent **团队审查**（Scan→Merge→Verify→Synthesize） | **team 模式** + `artifacts/teams/` | 所有宿主: `team_native` | 单 agent 批量验证 + `BATCH_VERDICT_SCHEMA` |
| **Hook 可数**深度 review、PR/全仓、spawn-first gate | **`$code-review-deep`** | `deep-reviewer` / `general-purpose` lane +（非 interactive）REVIEW_GATE | skill 层 findings-only + 多 lens |
| 窄范围单文件 review | 主线程或 sidecar | 无 team | 可选 |

**注意**：team 路径的 Verify 产物在 `artifacts/teams/<team_id>/messages/`；code-review-deep 产物在 review lane-notes。同一任务只选**一条** audit 主路径，除非用户显式要求两阶段（先 team audit 再 owner skill 修复）。Workflow（JS 编排）已彻底移除。

## When to use

以下情况适合触发：
- 当前任务需要判断是否允许 subagent / sidecar / worker delegation
- 任务是 read-heavy exploration，且多个方向可以独立并行
- 任务有多个独立假设、独立模块或独立验证维度
- review / verification 可以和主线程实现并行，且不会阻塞下一步
- 深度 / 全面 / 全仓 / 跨模块 review 明显包含多个独立审查维度时，先进入 subagent admission；适合则开启 reviewer sidecars
- 写入范围完全 disjoint，worker 只产出 lane-local delta
- 用户要构建 multi-agent system、agent **team**、swarm、orchestration layer
- 用户要做 planner / coder / reviewer / tester 这类**子代理协作链**
- 用户要做任务路由、agent handoff、shared memory、consensus、quality gate
- 用户要做 research swarm、support router、自动审查流水线
- 用户要设计 agent supervisor、coordinator、manager-worker 架构
- 用户显式要求 **team** / **多 agent 团队**，或任务自带多阶段审计/审查管道
- 用户明确要求多 worker 生命周期、协作拆分或 supervisor 集成时，本 gate 负责判断 bounded sidecars 或 team 是否足够
- 用户要固定 **review → fix → verify** 多轮闭环（可外加与 review **并行**的 **external research** lane，且大 `max_rounds` 时用 `framework_quality_gate` 写 `QUALITY_GATE_STATE.json`）：契约与模板通过 `framework_quality_gate` 运行时管理；用户侧入口优先 My 执行区（`GOAL_STATE.json`、`framework_goal_drive`）；本 gate 仍负责 spawn admission 与 reject reason

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

Parallel **review / external research** lanes must stay **read-biased**; **verifier** (or supervisor-run commands) owns **executable** pass/fail. **推理深度**见 [推理深度契约](../../docs/architecture.md)（分工 + `EVIDENCE_INDEX`，非单模型长 CoT）。Without at least one bounded `verify_commands` (or equivalent hook-visible checks), treat as **`verification_missing`** for write-heavy spawns.

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
- Do not create a new agent role, mailbox, graph, or state artifact unless an existing **team member** or **sidecar** lane contract cannot express the need.
- **所有多 agent 协作通过 team（Rust 层 `session-supervisor` team API）+ sidecar（bounded worker）实现**。Workflow（JS 编排）已彻底移除。
- 使用 `team_send_message` / `team_read_messages` 实现 agent 间通信。
- 使用 `agent_register` / `agent_unregister` 跟踪 agent 生命周期。
- 所有 agent 完成时必须调用 `agent_unregister` 或 `team_remove_member`，确保资源释放。
- Do not let workers write outside their assigned lane-local scope.
- Supervisor owns integration and final verification.
- **Superior Quality Audit**: For multi-agent swarm architectures, apply the runtime verification gate to verify against [Superior Quality Bar / verification gate criteria](../SKILL_FRAMEWORK_PROTOCOLS.md#1-runtime-protocol).

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

| 文档 | 用途 |
|------|------|
| [references/orchestration-mode.md](./references/orchestration-mode.md) | 模式表(team/sidecar/local)、触发、优先级、拒绝规则 |
| [references/team-protocol.md](./references/team-protocol.md) | Team API 契约、agent 通信协议、生命周期 |
| [references/detailed-guide.md](./references/detailed-guide.md) | 拓扑、handoff、spawn 细节 |
| `core/session-supervisor/src/team_manager.rs` | Rust 层 team 编排实现真源 |
| `core/session-supervisor/src/process.rs` | Agent 生命周期跟踪实现 |
| `./framework-health.md` | Agent 健康检查命令 |
