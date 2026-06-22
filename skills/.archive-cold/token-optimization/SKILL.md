---
name: token-optimization
description: |
  Optimize token usage through prompt caching, output compression, and context window management.
  Preserves task completion quality while reducing token consumption and API costs.
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [token-optimization, prompt-caching, cost-optimization, context-management]
risk: low
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
user-invocable: false
disable-model-invocation: true
status: deprecated
trigger_hints:
  - token 优化
  - 减少 token
  - prompt caching
  - 降低成本
  - 上下文压缩
  - token budget
  - context window
  - cost optimization
---

> **DEPRECATED** — Claude 原生已支持 prompt caching、context compaction 和 token 优化。本 skill 不再维护，内容保留作为参考。

# Token Optimization

## 概述

Token 优化核心理念：保持任务完成质量前提下减少 token 消耗。通过系统性策略降低 API 成本，同时维持输出效果。
本 skill 提供 prompt caching、输出压缩、上下文窗口管理三类策略，以及可量化的验证方法。

## 何时使用

- 长 session 中 token 消耗显著超出基线，需要系统性压缩
- 需要为 Anthropic API 设计或调整 prompt caching 断点
- CLAUDE.md / system prompt 冗余，需要精简以降低每次调用的固定开销
- 批量任务（如 agent swarm、深度研究）需要在固定预算内最大化吞吐
- 用户明确要求成本优化或 token 预算控制

## 何时不要用

- 一次性短交互（<200 tokens），优化收益不足以覆盖调整成本
- 任务本身已接近收敛，token 消耗主要来自必要输出而非浪费
- 正在进行紧急调试，优化工作应推迟到稳定期
- 用户明确要求完整详细输出（如教学、文档生成场景）

## Prompt Caching 策略

### 自动缓存模式（推荐）

使用 Anthropic API 顶层 `cache_control` 参数，系统自动管理缓存断点。适用于：
- 系统指令（system prompt）较长且在多轮中保持不变
- 工具定义（tool schemas）固定不变
- 大段参考文档或代码上下文

### 显式缓存断点

在长提示中精确控制缓存位置，适用于需要频繁重用的系统指令或上下文。
将**不变内容前置**（系统指令、工具定义、参考文档），**可变内容后置**（用户问题、动态上下文）。

```text
┌─────────────────────────────────┐  ← 缓存命中区间（cost × 0.1）
│  系统指令 + 工具定义 + 参考文档    │
├─────────────────────────────────┤  ← 缓存断点
│  动态上下文（会话历史、用户输入）  │
└─────────────────────────────────┘
```

### 最小缓存阈值

| 模型     | 最低缓存 tokens |
|----------|----------------|
| Sonnet   | 1,024          |
| Opus     | 4,096          |
| Haiku    | 4,096          |

低于阈值的内容无法触发缓存，合并小段内容以跨过阈值。

### 性能收益

- 缓存命中：延迟降低约 3.3x，成本降低约 90%
- 首次请求（冷缓存）无额外开销——`cache_control` 仅在后续命中时生效

## 输出压缩策略

### 结构化缩写

用表格或 JSON 替代长段落。对比：

```text
# 差：30+ tokens
"第一个选项是使用缓存，这样可以降低成本，第二个选项是不使用缓存，但是速度快..."

# 好：15 tokens
| 方案 | 成本 | 延迟 |
|------|------|------|
| 缓存 | 低   | 中   |
| 直连 | 高   | 低   |
```

### 去除解释性文本

直接给结论和代码，省略过渡语。常见冗余模式：
- "我来解释一下..." / "Let me explain..." → 删除
- "首先...其次...最后..." → 改为编号列表
- "根据我的理解..." → 直接陈述

### 分阶段输出

先摘要再按需展开：
1. 第一轮：输出 1-2 句核心结论
2. 仅当用户要求展开时，提供详细分析
3. 代码示例单独输出，不要嵌入长解释中

### 避免重复

不在多轮对话中重复已给信息。引用前文用简短标记（如"见上面第 2 点"），而非完整复述。

## 上下文窗口管理

### Token 预算分配

长 session 中按阶段分配 token 预算：

| 阶段     | 预算占比 | 说明                           |
|----------|---------|--------------------------------|
| 探索     | ~20%    | 读文件、搜索、理解需求           |
| 实现     | ~50%    | 编码、编辑、工具调用             |
| 验证     | ~30%    | 测试、审查、修复                 |

超预算时优先压缩探索阶段的 token（合并多次 Read 调用，精确搜索而非广撒网）。

### 状态保护

利用框架的 `session_checkpoint` 机制在上下文压缩前保存关键状态：
- 当前任务进展摘要
- 下一步行动列表
- 已验证的文件变更清单

### CLAUDE.md 精简原则

- 前 200 行最关键——模型注意力随距离衰减
- 每条指令应有明确的 **触发条件** 和 **预期行为**，避免模糊描述
- 定期审查并删除过时内容；避免同一指令在多处重复
- 用 `<!-- comment -->` 标记临时禁用的规则，而非删除后遗忘

## Skill 设计中的 Token 约束

- **简洁输出约束**：SKILL.md 中明确指定输出格式（如"返回 JSON，字段为 X/Y/Z"），避免模型自由发挥
- **触发条件精确化**：使用精确的 trigger_hints 避免误触发——误触发 = 无效 token 消耗
- **工具调用最小化**：batch 操作优于多次独立调用；合并相关 Read 请求；一次 Bash 完成多步操作
- **子 agent 精简 prompt**：spawn 子 agent 时，prompt 只包含必要上下文，不要复制整个 session 历史

## 具体优化技巧（带示例）

### 技巧 1：合并搜索调用

```text
# 差：3 次调用
Read src/a.rs  → Read src/b.rs  → Read src/c.rs

# 好：1 次调用
Bash: grep -rn "目标模式" src/ | head -20
```

### 技巧 2：精确工具选择

```text
# 差：Read 整个文件再找目标
Read large_file.rs (2000 lines)

# 好：先搜索定位
Bash: grep -n "function_name" large_file.rs → Read offset=行号 limit=30
```

### 技巧 3：system prompt 分层

```text
# 高频不变内容（缓存层）：框架指令、工具定义、长期记忆
# 低频变化内容（动态层）：当前任务上下文、临时指令
# 每轮变化内容（流动层）：用户输入、工具输出
```

### 技巧 4：输出格式控制

在 SKILL.md 或 prompt 中显式指定：
```text
输出格式：仅返回以下结构，不要添加解释性文本。
{
  "result": "...",
  "confidence": 0-1,
  "next_action": "..."
}
```

## 度量与验证

### 单次任务追踪

记录每个任务的 input / output / cached token 数量。关注三个比率：
- **缓存命中率** = cached_tokens / total_input_tokens（目标 >60%）
- **输出效率** = useful_output_tokens / total_output_tokens（目标 >80%）
- **工具调用比** = tool_calls / task_steps（越低越好，目标 <3）

### 预算基线

建立典型任务的 token 消耗基线：

| 任务类型     | 典型 input | 典型 output | 可优化空间 |
|-------------|-----------|-------------|-----------|
| 代码修改     | 15k       | 3k          | 20-30%    |
| 深度审查     | 40k       | 8k          | 15-25%    |
| 新建项目     | 25k       | 10k         | 10-20%    |
| 简单问答     | 5k        | 1k          | 5-10%     |

### 优化迭代流程

1. 记录当前基线（至少 5 个同类任务的平均值）
2. 应用本文档中的优化策略
3. 对比优化前后的 token 消耗
4. 如果某策略导致输出质量下降（用户需多次追问），回退该策略
5. 将有效策略固化到 SKILL.md 或框架配置中

## References

- Prompt caching 文档：Anthropic API `cache_control` 参数说明
- Quality Gate 循环的 token 控制：`docs/spec.md` §Quality Gate
- 子 agent 编排的 token 最小化：`skills/agent-swarm-orchestration/SKILL.md`
- 路由元数据中的 token 预算标记：`skills/SKILL_ROUTING_RUNTIME.json`
