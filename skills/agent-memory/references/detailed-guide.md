# agent-memory — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Memory Ladder

### Level 1: File-Based Memory

默认首选。适合大多数个人项目和中小型 agent。

推荐结构：

```text
memory/
  MEMORY.md
  decisions.md
  preferences.md
  sessions/
    2026-03-19.md
    2026-03-20.md
```

适用场景：
- 先快速落地
- 零依赖
- 希望人类可读、可审计、可手改

关键规则：
- `MEMORY.md` 只放长期稳定信息
- 每日日志放原始观察，不直接污染长期记忆
- 定期把会话记录提炼进长期文件

### Level 2: Local Semantic Memory

当关键词搜索不够用时，再升级到本地语义检索。

典型方案：
- SQLite + embeddings
- 本地索引 + 余弦相似度

适用场景：
- 需要“意思相近”召回
- 记忆条目开始变多
- 需要基础排序和过滤，但还不想引入外部服务

### Level 3: Production Memory Stack

只有在以下情况才考虑：
- 记忆规模很大
- 需要多租户
- 需要复杂过滤
- 需要高并发读写
- 需要独立部署的 memory service

常见方案：
- ChromaDB
- pgvector
- Pinecone / Weaviate / Milvus

## Core Workflow

### 1. Define Memory Scope

先回答四个问题：
- 记什么：偏好、决策、事实、任务状态、失败教训、外部知识
- 什么时候写入：每次会话结束、任务完成后、显式“remember this”、定时整理
- 什么时候召回：任务开始前、用户提到相关实体时、规划阶段、报错排查前
- 谁能改：agent 自动写、人类审核后写、或两者混合

没有边界的 memory system 很快会变成噪音堆。

### 2. Separate Short-Term and Long-Term Memory

强制区分两类内容：
- 短期记忆：当前任务过程、临时假设、当天上下文
- 长期记忆：稳定偏好、项目事实、重要决策、可复用经验

不要把所有聊天记录直接塞进长期记忆。

### 3. Design a Memory Schema

每条记忆至少要有：
- `content`
- `category`
- `source`
- `created_at`
- `confidence`

建议的 category：
- `project`
- `preference`
- `decision`
- `lesson`
- `entity`
- `runbook`

### 4. Build Retrieval Before Fancy Storage

先确保“能准确找回来”，再追求复杂存储。

检索最少应支持：
- 关键词搜索
- 按 category 过滤
- 最近优先或相似度排序
- 限制返回条数，避免把 prompt 塞爆

### 5. Add Consolidation

consolidation 是记忆系统的核心，不是可选项。

做法：
- 每次会话把原始记录写进 `sessions/`
- 定期提炼：删噪音、合并重复、升级稳定事实
- 把长期有效的信息写回 `MEMORY.md` 或结构化存储

如果没有 consolidation，memory 只会越来越脏。

### 6. Inject Retrieved Context Safely

把召回内容注入 agent 时遵守：
- 只注入最相关的少量记忆
- 标注来源和可信度
- 区分“已确认事实”和“历史推测”
- 不要把整库原文拼进 prompt

目标是帮助推理，不是制造上下文污染。

## Implementation Guidance

### File-Based First

若用户没有明确要求向量检索，默认：
1. 创建 `memory/` 目录
2. 新建 `MEMORY.md` 和一个或多个主题文件
3. 约定写入规则、提炼规则和读取时机
4. 只在证明确实不够用时再升级

### Semantic Search Upgrade

只有在这些信号出现时再升级：
- 用户抱怨“搜不到相关旧经验”
- 记忆条目太多，关键词命中差
- 同义表达很多
- 需要跨文档关联召回

升级时优先本地化方案，除非用户明确要 SaaS。

### Memory Quality Gates

写入前检查：
- 这条内容是否值得长期保存
- 是否与现有内容重复
- 是否包含敏感信息
- 是否只是短期噪音

召回前检查：
- 是否与当前任务相关
- 是否可信
- 是否已经过时

## Privacy and Safety

默认不要把以下内容写入长期记忆：
- API keys
- access tokens
- passwords
- 私人身份信息
- 高敏内部数据原文

如必须记录，优先记“位置”和“处理规则”，不要记明文。

## Output Expectations

处理这类请求时，优先产出以下内容：
- 记忆分层方案
- 存储结构或目录结构
- 读写时机
- consolidation 规则
- 检索与注入策略
- 最小可运行实现

如果用户要直接落代码，应实现：
- memory schema
- 写入接口
- 检索接口
- 基础去重或过滤
- 至少一个验证方式

## Codex Notes

- 优先改当前仓库能直接使用的方案，不要默认上外部服务
- 优先选择可调试、可审计、可迁移的记忆结构
- 若当前任务只是“写一个记忆规范”，不必过度工程化
- 若需要 embeddings 或向量库，说明成本、依赖和维护代价

## Quick Checklist

- 是否明确了记忆边界
- 是否区分了短期与长期记忆
- 是否先从最轻方案开始
- 是否设计了 consolidation
- 是否控制了召回体积
- 是否避免存敏感信息
- 是否有最小验证路径

