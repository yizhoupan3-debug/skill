---
last_verified: "2026-06-18"
depends_on:
  - ../spec.md
---

# MiMo 宿主操作手册

**闭集 id**: `mimo` · **传输**: native-mimo (hook) · **权威**: `RUNTIME_REGISTRY.json` → `host_projections.mimo`

## 代理身份与画风

- 主代理定位为严谨的科研学者与系统工程专家
- 回复画风专业、客观、谦逊；默认使用简体中文
- 不确定的信息直接说明，不编造

## 能力边界与 Harness 入口

- **任务推进**: `/implementx` + `framework_goal_drive` stdio
- **任务状态**: `artifacts/current/<task_id>/GOAL_STATE.json`
- **门控模式**: hook 事件驱动，与 Claude Code / Cursor 对齐

## Hook 事件矩阵

MiMo 注册 7 个 hook 事件（与 Cursor 对齐）：

| 事件 | 能力 |
|------|------|
| PreToolUse | 路径保护拦截 |
| UserPromptSubmit | review gate 初始化 + context 注入 |
| PostToolUse | 证据收集 + subagent 追踪 |
| Stop | closeout gate + review gate 检查 |
| SessionStart | context 注入 |
| SubagentStart | 子代理启动追踪 |
| SubagentStop | 子代理完成追踪 |

## 安装与文件分布

- **配置文件**: `.mimo/settings.json`（project scope）、`~/.mimo/settings.json`（user scope）
- **Framework 规则**: `AGENTS_MIMO.md`（仓库根）
- **Context 文件**: `AGENTS_MIMO.md`

## MCP 服务器

| 服务器 | 配置路径 |
|--------|----------|
| router-rs-framework | `.mimo/settings.json` → `mcpServers.router-rs-framework` |
| browser-mcp | `.mimo/settings.json` → `mcpServers.browser-mcp` |
| mcp-codegraph | `.mimo/settings.json` → `mcpServers.mcp-codegraph` |
| paperplain | `.mimo/settings.json` → `mcpServers.paperplain` |

## Harness Capabilities

- `hot_runtime_routing` — 热路由
- `l2_continuity_contract` — L2 连续性契约
- `closeout_evidence_hooks` — closeout 证据 hook
- `review_gate_router_observation` — review gate 观测

## Session Supervisor

**不支持**外部进程监管或自动恢复。长时目标依赖会话内连续性产物。

## Fail-open / Fail-closed

MiMo 采用 **fail-closed** 策略：`router-rs` hook 二进制缺失时阻断操作。

## 默认生命周期

与所有闭集宿主一致：`/discussx` → `/planx` → `/implementx` → `/verifyx`
