# Orchestration Contract — External Evidence

> implementx 编排硬约束的权威证据溯源。SKILL.md 为执行真源，本文件仅做审计 trail。

## 1. Anthropic 官方工程博客

### Building Effective Agents (2024-12)

- **orchestrator-workers pattern**：中心 LLM 动态拆解任务 → 分发 worker → 合成结果
- 适合"无法预知子任务数量"的复杂任务（编码正是此类）
- 三条核心原则：**simplicity / transparency / ACI (agent-computer interface)**
- 来源：https://www.anthropic.com/research/building-effective-agents

### How we built our multi-agent research system (2025-06)

- Lead agent + specialized subagents 平行；**3-5 个 subagent** 并行（cut time by 90%）
- token 成本：agent ≈ 4× chat；multi-agent ≈ 15× chat → 任务价值须匹配
- **subagent prompt 五元组**：objective / output_format / tools / task_boundaries / guidance
- **artifact system**：subagent 写文件 → coordinator 只看 lightweight references（避免 game-of-telephone）
- **扩展 thinking**：lead agent 先规划再 spawn
- 来源：https://www.anthropic.com/engineering/multi-agent-research-system

### Effective context engineering (2025)

- subagent 用 tens of thousands tokens 深度工作，**只回传 1,000-2,000 token 摘要**
- **separation of concerns**：详细搜索 context 留在 subagent 内
- compaction / note-taking / multi-agent 三种 context 治理手段
- 来源：https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents

### Scaling Managed Agents: Decoupling brain from hands (2025)

- **brain（harness + Claude）/ hands（sandbox）/ session（event log）** 三层解耦
- session 持久化在 harness 外 → harness crash 后可重启恢复
- hands 是 tool（`execute(name, input) → string`）→ 可替换
- 来源：https://www.anthropic.com/engineering/managed-agents

## 2. Claude Code 官方文档

### Subagents

- 每个 subagent 独立 context window；不共享 conversation
- **subagent 不能 spawn subagent**（单层 delegation）
- `fork_context: false` 为默认
- Explore / Plan 内置只读 agent
- disallowedTools 收窄能力
- 来源：https://code.claude.com/docs/en/sub-agents

### Dynamic Workflows

- JS 脚本编排 subagent；中间结果留在 script variables（不进 Claude context）
- 最多 16 并发 / 1000 agent per run
- `/deep-research` 为内置 workflow 示例
- `ultracode`：自动为每个实质性任务 plan workflow
- 来源：https://code.claude.com/docs/en/workflows

### Agent Teams

- 多 Claude Code 实例独立工作；shared task list + inter-agent messaging
- 推荐 3-5 teammate；5-6 tasks per teammate
- subagent 定义可复用为 teammate 角色
- hooks：TeammateIdle / TaskCreated / TaskCompleted
- 来源：https://code.claude.com/docs/en/agent-teams

## 3. GitHub 社区案例

### maestro-orchestrate（活跃社区项目，400+★）

- 39 specialists × 4 宿主；Express + Standard 双轨
- 4-phase：Design → Plan → Execute → Complete
- quality gate 阻塞；session state 持久化
- 来源：https://github.com/josstei/maestro-orchestrate

### code-audit-system

- CVE 审计 skill；MainAgent fast pre-scan → 立即 dispatch all subagents
- **incremental intelligence injection**：coordinator 中途向 running subagent 注入发现
- workspace/agent-*/background.md 隔离；state 文件做 checkpoint/recovery
- 来源：https://github.com/UserB1ank/code-audit-system

### claude-subagent-system

- 6 阶段 pipeline；**ROLE_TOKEN_BUDGETS** 按角色限 token
- authority hierarchy：orchestrator > specialist
- quality gate 阻断
- 来源：https://github.com/cemigo114/claude-subagent-system

### ArtChiTech-framework

- 7-phase + 5-level scale 自动检测复杂度
- subagent-driven development（SDD）；two-stage review
- git worktree 隔离；deviation rules
- 来源：https://github.com/manuelturpin/ArtChiTech-framework

### code-creation-workflow

- 7-phase orchestrator（~170 lines）
- **phase reference 按需加载**（只加载 2-3 个 reference 文件 → 节省 token）
- parallel reviewers（2× code-reviewer）；TDD guard hook
- 来源：https://github.com/sumrae412/code-creation-workflow

## 4. 社区生产经验（GitHub anthropic-sdk-python Discussion #1313）

- **shared mutable state = 敌人**：v1 shared JSON → race conditions；v3 single-writer memory steward → 稳定
- 关键洞察：**"Give each agent its own write domain and a read-only view of others' domains."**
- **failure cascade**：下游 agent 等上游死锁 → 需 heartbeat / timeout
- **capability narrowing**：child capabilities ⊆ parent capabilities
- 来源：https://github.com/anthropics/anthropic-sdk-python/discussions/1313

## 5. 映射到 implementx 硬约束

| 外部经验 | implementx 硬约束 |
|----------|------------------|
| 3-5 parallel subagent | §2 Lane Split-or-Explain: ≥3 lane / wave |
| orchestrator 不碰代码 | §0 核心身份: coordinator 禁止写产品代码 |
| artifact system | §7 Artifact System: subagent 写文件, coordinator 只读 lane-notes |
| subagent 不能 spawn subagent | §3.3: 单层 delegation |
| capability narrowing | §3.3: fork_context=false, disallowedTools |
| 五元组 handoff | §3.1: objective/scope/output/tools/verification |
| lane return schema | §3.2: changed_files/evidence/verification/risk/next_action |
| incremental injection | §6: research 发现可追加到后续 lane prompt |
| token budget | §3.4: 按角色限 token |
| no silent degradation | §4.2: fallback 必须写 lane-notes |
| quality gate 阻塞 | §3.2: verification.result=fail → retry 或 abort |
