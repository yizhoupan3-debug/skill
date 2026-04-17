# agent-swarm-orchestration — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## First Decision: Do You Even Need Multiple Agents?

只有满足下列至少一项，才建议多 agent：
- 任务天然能拆成独立子任务
- 不同角色需要明显不同的上下文和目标
- 需要并行执行来降低总耗时
- 需要一个 agent 专门做审查或验收
- 单 agent 上下文已经太杂，质量明显下降

如果只是一个人能顺着做完的简单流程，优先单 agent。

## Topology Selection

### 1. Pipeline

结构：

```text
Input -> Planner -> Coder -> Reviewer -> Tester -> Result
```

适合：
- 线性流程
- 阶段边界清晰
- 每一步产物都能被下一步消费

不要用于：
- 高度分叉的任务
- 需要频繁回流修改的复杂协作

### 2. Manager-Worker

结构：

```text
Manager
  |- Worker A
  |- Worker B
  |- Worker C
```

适合：
- 任务可拆成多个并行块
- 需要中心化分配和汇总
- 需要统一质量标准

### 3. Router-Specialist

结构：

```text
Router -> Specialist chosen by task type
```

适合：
- 请求类型差异大
- 每类请求有明显专长 agent
- 希望先分类再执行

### 4. Hybrid

常见组合：
- Router -> Specialist pipeline
- Manager -> workers -> reviewer

只有当简单拓扑不够时才上 hybrid。

## Core Workflow

### 1. Define Roles Clearly

每个 agent 必须有清晰边界：
- 它负责什么
- 不负责什么
- 输入是什么
- 输出是什么
- 成功标准是什么

坏例子：
- “Agent A 负责大部分实现，Agent B 也可以顺手改”

好例子：
- Planner 只拆任务和定义接口
- Coder 只按接口实现
- Reviewer 只找风险和回归
- Tester 只验证行为和覆盖率

### 2. Define Handoff Contracts

agent 之间不能只传自然语言大段描述，最好有结构化交接。

每次 handoff 至少包含：
- task id
- role
- goal
- constraints
- artifacts
- acceptance criteria

没有 handoff contract，系统很快会漂。

### 3. Add Shared State Carefully

共享状态的目标是减少重复劳动，不是制造全局混乱。

可共享内容：
- 事实
- 决策
- 当前任务状态
- 产物路径

不要共享太多：
- 未验证猜测
- 长篇原始上下文
- 与当前角色无关的噪音

### 4. Add Quality Gates

每个关键阶段后都要有 gate。

常见 gate：
- schema / contract 是否满足
- 代码是否可编译
- 测试是否通过
- 审查是否批准
- 风险是否在阈值内

**未经检查的输出不要直接传给下一阶段。**

### 5. Add Retry Policy

重试不是“无限再试一次”。

建议规则：
- 每阶段最多重试 2 到 3 次
- 每次重试都必须携带明确反馈
- 连续失败后要升级到 manager 或 human review

如果只会重复同一个提示词，重试通常没有意义。

### 6. Keep Human-in-the-Loop

这些点最好有人类确认：
- 拓扑是否合理
- 高风险变更是否继续
- 多个方案怎么选
- 角色划分是否偏离目标
- 最终是否发布

多 agent 系统不是为了去掉人类，而是为了把人类放在更高价值的位置。

## Canonical Roles

常见角色模板：

- `planner`
  负责拆解任务、定义接口、安排依赖

- `router`
  负责把请求送到正确 specialist

- `coder`
  负责实现，不擅自扩 scope

- `reviewer`
  负责找 bug、回归、设计风险

- `tester`
  负责验证行为、测试、基线

- `supervisor`
  负责汇总状态、仲裁冲突、决定是否重试或升级

不要同时让一个 agent 扮演过多高冲突角色。

## Shared Memory Guidance

如果系统需要共享记忆，优先只共享：
- 已确认事实
- 中间产物索引
- 已采纳决策
- review findings

共享记忆要满足：
- 可追溯
- 可裁剪
- 可过期

没有过期机制的 shared memory 会持续污染路由与判断。

## Failure Modes

常见失败模式：
- 角色重叠，互相改写
- router 误分流
- manager 给的任务过大
- reviewer 只说空话，不给可执行反馈
- tester 只跑表面验证
- shared memory 太大，导致所有 agent 被噪音拖慢
- retry 没有新增信息，形成死循环

设计时优先防这些问题，而不是先追求“有多少 agent”。

## Output Expectations

处理这类请求时，优先产出：
- 推荐拓扑和理由
- 角色定义
- handoff contract
- shared state 方案
- quality gates
- retry / escalation 规则
- 最小实现路径

如果用户要代码，应至少实现：
- orchestrator 或 router 主体
- 角色接口
- 任务状态模型
- handoff 数据结构
- 一条可运行的端到端流程

## Codex Notes

- 若用户只是希望当前会话并行处理，不要把“实现多 agent 系统”和“立刻开很多 sub-agent”混为一谈
- 在当前 Codex 环境里，运行时是否真的执行 delegation 取决于当前 session policy；但设计层应保持 sidecar-ready 结构，并在不可派发时退化为 local-supervisor 模式
- 这个 skill 更适合设计或实现多 agent 产品、框架、流水线代码
- 优先从 2 到 4 个角色开始，不要一上来设计十几个 agent

## Quick Checklist

- 是否真的需要多 agent
- 是否为每个角色定义了边界
- 是否定义了 handoff contract
- 是否有 quality gates
- 是否有 retry 上限和升级机制
- 是否控制了 shared memory 污染
- 是否保留了 human-in-the-loop

