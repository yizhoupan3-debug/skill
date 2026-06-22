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
│ L1 Routing Layer  routing-engine (pure)      │
│   Entry Point     router-rs (aggregator)     │
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
router-rs → runtime-core → host-projection                         ← router-rs is the multi-layer
                          → core-policy                              entry-point aggregator, not a
                          → routing-engine                           pure L1 layer. It depends on all
                          → framework-kernel                         lower layers and exposes the
                          → framework-runtime, session-supervisor,    unified CLI + MCP interfaces.
                            runtime-storage, trace-runtime (extracted)
                          → tools/codegraph-rs (optional)
                          → browser-mcp (optional)
```

B0 core crates 不依赖 `router-rs`；宿主特有逻辑禁止出现在 B0 中。

## 科研 Harness

Research 子系统提供多轮对抗审稿、文献检索、AIGC 检测能力。详见 [research-harness.md](research-harness.md)。

## Review 通用协议

所有 review 类 skill/workflow 的输出约束与幻觉分类标准。

### 约束：Confirmed-only 输出

最终用户可见输出**只包含 confirmed findings**。confirmed = 事实核查通过（evidence 真实存在且准确）+ 判断通过（是真实问题）。rejected（判断驳回）和 hallucinated（事实核查拦截）不出现在用户输出中。可选统计摘要行：`N confirmed / M rejected / K hallucinated`。

### 幻觉分类标准（hallucination_type）

| 值 | 含义 |
|----|------|
| `none` | 事实全部准确 |
| `code_not_exist` | 引用的源不存在 |
| `evidence_fabricated` | 源存在但证据捏造/复述 |
| `wrong_line` | 源存在但位置错误 |
| `behavior_misrepresented` | 证据正确但行为/现象描述有误 |
| `evidence_out_of_context` | 证据真实但与 finding 无关 |
| `source_moved` | 源已重命名/移动 |
| `partial_hallucination` | 部分准确部分幻觉 |
| `indeterminate` | 无法确认 |

### 降级策略

Factcheck 工具不可用时：单 finding 标记 `indeterminate` 不进 Verify；全阶段失败则所有 finding 标记 indeterminate，最终输出为空（0 confirmed）。关键：factcheck 整体失败时**不降级为"跳过 factcheck 直接进 Verify"**。

## 契约漂移规则

机器可读 Schema、状态流转图、指标定义是开发和测试的第一断言断点。配置规则变更须先改本文件，再实现与回归。
