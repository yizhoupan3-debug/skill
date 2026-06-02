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
routing_priority: P2
session_start: "n/a"
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

# Token Optimization

## 概述

Token 优化核心理念：保持任务完成质量前提下减少 token 消耗。通过系统性策略降低 API 成本，同时维持输出效果。

## Prompt Caching 最佳实践

**自动缓存模式（推荐）**：使用 Anthropic API 顶层 `cache_control` 参数，系统自动管理缓存断点。

**显式缓存断点**：在长提示中精确控制缓存位置，适用于需要频繁重用的系统指令或上下文。

**最小缓存阈值**：
- Sonnet：1024 tokens
- Opus/Haiku：4096 tokens

**性能收益**：
- 缓存命中延迟降低 3.3x
- 成本降低 90%

## 输出压缩策略

- **结构化缩写**：用表格/JSON 替代长段落描述
- **去除解释性文本**：直接给结论和代码，省略"我来解释一下"等过渡语
- **分阶段输出**：先摘要再按需展开，避免一次性输出大量内容
- **避免重复**：不在多轮对话中重复已给信息，引用前文而非复述

## 上下文窗口管理

- **Token 预算分配**：长 session 中按阶段分配 token 预算（探索 20%、实现 50%、验证 30%）
- **状态保护**：利用 PreCompact/PostCompact hooks 在上下文压缩前保护关键状态
- **CLAUDE.md 精简**：前 200 行最关键，避免冗余指令；定期审查并删除过时内容

## Skill 设计中的 Token 约束

- **简洁输出约束**：SKILL.md 中明确指定输出格式，避免模型自由发挥产生冗余
- **触发条件精确化**：使用精确的 trigger_hints 避免误触发，减少不必要的 token 消耗
- **工具调用最小化**：batch 操作优于多次独立调用，减少工具调用开销

## 度量与监控

- **单次任务追踪**：记录每个任务的 input/output/cached token 数量
- **预算基线**：建立典型任务的 token 消耗基线，识别异常消耗
- **优化迭代**：基于度量数据持续优化提示词和工作流
