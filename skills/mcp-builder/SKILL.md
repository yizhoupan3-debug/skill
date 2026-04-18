---
name: mcp-builder
description: |
  Design, build, review, and improve MCP servers and agent-facing tool interfaces.
  Use for transport choice, tool schema design, auth/errors/pagination,
  and wrapping external services as MCP tools.
risk: medium
source: community-adapted
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - mcp
  - builder
  - mcp builder
  - 给模型设计可用工具界面
  - 给人类写 SDK 文档
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - mcp
    - builder
---

- **Dual-Dimension Audit (Pre: Schema/Contract, Post: Tool-Discoverability/Execution Results)** → `$execution-audit-codex` [Overlay]
# mcp-builder

## Overview

设计和实现高质量的 MCP server。高质量的判断标准：
- agent 能不能发现正确工具
- agent 能不能稳定调用
- 返回结果是否足够聚焦、可组合、可恢复
- 错误信息是否能帮助 agent 自我修正

重点是"给模型设计可用工具界面"，不是"给人类写 SDK 文档"。

## Core Principles

1. **Tool UX > Endpoint Count** — 不是一比一映射 API，而是平衡覆盖能力和高频 workflow 工具
2. **Discoverability First** — 名称清晰、动词明确、前缀一致、描述写出使用时机
3. **Keep Responses Focused** — 结果裁剪、分页、过滤、结构化返回
4. **Actionable Errors** — 告诉模型哪里错了、哪个参数有问题、下一步怎么改

## Workflow Summary

1. Understand target system → 2. Define tool surface → 3. Choose transport → 4. Design contracts → 5. Build shared infra → 6. Implement conservatively → 7. Review for agent usability

## Output Expectations

处理请求时优先产出：工具面设计 → transport 选择理由 → schema 设计 → 命名规范 → 公共基础层 → 风险边界 → 测试方案

See [detailed workflow steps, design rules, and language guidance](references/DETAIL.md) for complete reference.

- **Superior Quality Audit**: For high-availability MCP servers, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Quick Checklist

- 目标系统和使用场景是否明确
- tool surface 是否经过筛选而不是机械映射
- transport 是否合理
- tool name 是否足够可发现
- input / output schema 是否清晰
- 是否有分页、过滤和错误恢复
- 是否标明了只读 / destructive 属性
- 是否准备了基础评测方式

## Codex Notes

- 优先做最小但正确的 MCP server，再扩展工具数量
- OpenAI / ChatGPT Apps 生态注意和 `chatgpt-apps`、`openai-docs` skill 配合
- 对外部 API 封装，优先保留稳定 ID、分页参数、过滤参数

## When to use

- The user wants to build, scaffold, or debug an MCP (Model Context Protocol) server
- The task involves MCP tools, resources, prompts, or server implementation
- "强制进行 MCP server 深度审计 / 检查 Tool Schema 定义与实际执行结果。"
- "Use $execution-audit-codex to audit this MCP server for tool-discoverability idealism."
- The user says "MCP server", "MCP 工具", "Model Context Protocol"
- The user wants to create a tool server that AI agents can call

## Do not use

- The task is building a ChatGPT Custom GPT or plugin → use `$chatgpt-apps`
- The task is general API design without MCP context → use `$api-design`
- The task is prompt engineering without server implementation → use `$prompt-engineer`
