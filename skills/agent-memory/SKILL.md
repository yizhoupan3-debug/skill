---
name: agent-memory
description: |
  Agent 长期记忆 / 跨会话记忆设计：MEMORY.md、偏好记忆、决策档案、语义召回与 context injection。
  Use when the user wants an agent to remember project context across sessions instead of relying on chat history.
short_description: Design persistent agent memory across sessions
trigger_hints:
  - 长期记忆
  - 跨会话记忆
  - MEMORY.md
  - 语义召回
  - memory layer
risk: medium
source: community-adapted
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - agent
    - memory
allowed_tools:
  - shell
  - python
  - rust
approval_required_tools: []
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - MEMORY.md
  - memory.sqlite3
  - run_summary.json
  - active_task.json
  - .supervisor_state.json
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once

---
- **Dual-Dimension Audit (Pre: Memory-Schema/Logic, Post: Recall-Accuracy/Context-Injection Results)** → runtime verification gate
# agent-memory

## Overview

这个 skill 用来设计和实现 AI agent 的长期记忆能力，而不是普通的临时上下文拼接。

目标通常包括：
- 让 agent 跨会话记住项目背景、用户偏好、关键决策和历史经验
- 让 agent 在执行前先召回相关上下文，而不是每次从零开始
- 把杂乱的会话记录沉淀成结构化长期知识

优先用最轻的方案解决问题：
1. 先用文件型记忆
2. 再考虑本地语义检索
3. 最后才上向量数据库或云服务

## Repo-Local Memory Audit

在本仓库内，memory 闭环必须同时验证三层，不允许只看 `changed_files`：

- `memory_root`: 以 `run-memory-automation` 输出、`run_summary.json`、`snapshot.json` 为准，通常是仓库内 `.codex/memory`。
- `sqlite`: 必须检查 `.codex/memory/memory.sqlite3` 是否存在，以及 `memory_items` 表里的 active 计数；没有 sqlite 时计数只能报 `0`，不能说已持久化。
- `continuity`: 必须检查 `artifacts/current/active_task.json`、`artifacts/current/focus_task.json`、`artifacts/current/task_registry.json`、根 `.supervisor_state.json`，以及任务级 `SESSION_SUMMARY.md`、`NEXT_ACTIONS.json`、`EVIDENCE_INDEX.json`、`TRACE_METADATA.json`。

统一入口：

```bash
scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml \
  codex host-integration run-memory-automation \
  --repo-root "$PWD" \
  --workspace skill \
  --output-dir artifacts/ops/memory_automation/current \
  --query "memory audit"
```

这条命令应当 materialize repo-local memory root、sqlite `memory_items` seed、bootstrap mirror、active continuity 控制面，并在 `continuity_audit` 中明确 `ready` 或 `blocked`。若 `continuity_audit.status=blocked`，必须把 `missing_control_plane_anchors`、`missing_recovery_anchors` 和 `residual_blocker_count` 作为残留 blocker 汇报。

## When to use

以下情况适合触发：
- 用户要求“让 agent 记住这些信息”
- 用户要做 `MEMORY.md`、长期记忆、项目知识库、决策档案
- 用户要做跨会话上下文保留
- 用户要做“按语义召回历史经验”，而不是只靠关键词搜索
- 用户要设计 agent memory architecture、memory consolidation、context injection
- 用户要给 coding assistant、research assistant、客服助手增加记忆层
- 用户要核查 repo-local memory root、sqlite `memory_items`、bootstrap 镜像、artifact migration 或 continuity 控制面是否真的闭环

常见表达：
- “给这个 agent 加记忆”
- “让它下次还能记住项目背景”
- “做一个长期记忆系统”
- “把历史决策沉淀下来”
- “支持语义检索过去的对话/知识”
- “做 MEMORY.md 工作流”

## Do not use

以下情况不要触发，或只部分借鉴：
- 用户只是要保存一个普通文档，不涉及 agent 记忆
- 用户只是要做 RAG 检索，但不需要长期记忆或增量写回
- 用户只是问数据库选型，不是为 agent 设计 memory layer
- 用户只要求当前会话内的短期总结
- 用户只想把几条偏好手工写进 README，这时不需要完整 memory system
- 用户只是在问普通任务状态但不涉及 memory root、sqlite、bootstrap 或 continuity 控制面

## Hard Constraints
- **Superior Quality Audit**: For persistent memory systems, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples
- "强制进行 Agent 记忆深度审计 / 检查召回准确性与上下文注入结果。"
- "Use the runtime verification gate to audit this memory layer for recall-accuracy idealism."

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
