---
last_verified: "2026-06-22"
version: "unified-v9"
---

# 框架统一规约

## 七层模型

```
┌─────────────────────────────────────────────┐
│ L6 Feature Layer  research-harness, paper   │
├─────────────────────────────────────────────┤
│ L5 Hook Layer     hook-layer (OnceLock fn)  │
├─────────────────────────────────────────────┤
│ L4 Runtime Layer  runtime-core              │
│  ├─ Behavior      loop-engine, goal, context │
│  ├─ Orchestrate   session, multi-agent      │
│  ├─ Infra         transport, config, telemetry│
│  └─ Exit Gate     quality-gate, closeout    │
├─────────────────────────────────────────────┤
│ L3 Tool Layer     tool-layer (ToolRegistry) │
├─────────────────────────────────────────────┤
│ L2 Skill Layer    skill-layer (SKILL.md)    │
├─────────────────────────────────────────────┤
│ L1 Routing Layer  routing-engine, router-rs │
├─────────────────────────────────────────────┤
│ L0 Host Layer     host-projection (thin)    │
└─────────────────────────────────────────────┘
```

| 层 | 职责 | 允许 | 禁止 |
|----|------|------|------|
| L0 宿主层 | argv/stdin/stdout 协议转换 | 轻量适配 | 业务/路由 |
| L1 路由层 | 意图匹配、skill/tool 路由 | 策略矩阵 | 直接执行工具 |
| L2 Skill 层 | SKILL.md 注入、技能生命周期 | 契约验证 | 第二套连续性目录 |
| L3 工具层 | ToolRegistry、统一注册 | MCP/原生工具 | 宿主逻辑 |
| L4 运行层 | 层间编排、session 管理 | 行为/基建/退出 | Feature 侵入 |
| L5 Hook 层 | 函数指针注册、事件分发 | 49 slots/review | 业务逻辑 |
| L6 Feature 层 | 领域特化插件 | research-harness | 核心运行时修改 |

## 架构原则

| 原则 | 含义 |
|------|------|
| **单一权威真源** | `RUNTIME_REGISTRY.json` 为宿主闭集唯一权威 |
| **L0/L4 解耦** | 宿主差异仅存于 L0 适配壳 |
| **二元编排** | 仅 `subagent` + `workflow`；team 已废弃 |
| **纯 Rust 隔离** | PID + SQLite |
| **Fail-closed** | 未知均默认拒绝 |
| **函数指针注册表** | hooks 通过 OnceLock 函数指针注册，非 trait |
| **MCP 统一** | 4 个 MCP server 四宿主统一注册 |
| **用户级配置** | MCP/hooks/settings 在 `~/.config/`，不在项目目录 |

## 工具路由 vs Skill 路由

两条独立管线，共享 hook 事件流：

| 维度 | Skill 路由 | 工具路由 |
|------|-----------|---------|
| 入口 | `route_task()` | PreToolUse/PostToolUse hook |
| 数据源 | `SKILL_ROUTING_RUNTIME.json` | Rust 层分类 |
| 分类 | `kind`: skill / framework_command | `ToolOrigin`: NativeHost / McpServer / Unknown |

**关键类型**：`ToolOrigin`、`classify_tool_origin()`、`parse_mcp_tool_fqn()`、`dangerous_mcp_tool_reason()`

## 依赖关系约束

```
router-rs → runtime-core → host-projection
                          → core-policy
                          → routing-engine
                          → framework-kernel
                          → framework-runtime, session-supervisor,
                            runtime-storage, trace-runtime (extracted)
                          → tools/codegraph-rs (optional)
                          → browser-mcp (optional)
```

B0 core crates 不依赖 `router-rs`；宿主特有逻辑禁止出现在 B0 中。

## 科研 Harness

Research 子系统提供多轮对抗审稿、文献检索、AIGC 检测能力。详见 [spec/research-harness.md](spec/research-harness.md)。

## 契约漂移规则

机器可读 Schema、状态流转图、指标定义是开发和测试的第一断言断点。配置规则变更须先改本文件，再实现与回归。
