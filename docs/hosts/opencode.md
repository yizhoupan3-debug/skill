---
last_verified: "2026-06-22"
depends_on:
  - _common.md
  - ../adr/010-ideal-architecture-v10.md
parent: _common.md
---

# OpenCode 宿主操作手册

**闭集 id**: `opencode` · **传输**: native-opencode（JS/TS 插件 hook + MCP 双通道） · **权威**: `RUNTIME_REGISTRY.json` → `host_projections.opencode`

**共通内容**（代理身份与画风、Skill 路由、默认生命周期、Python 环境、进程管理与性能调优）见 [`_common.md`](_common.md)。

---

## 能力边界与 Harness 入口

- **任务推进**: `/implementx` + `framework_goal_drive` stdio
- **任务状态**: `artifacts/current/<task_id>/GOAL_STATE.json`
- **门控模式**: 插件 hook + MCP 工具层双通道。`tool.execute.before` 可 throw 阻断工具执行（等价 PreToolUse）；`session.idle` 等价 Stop；closeout/review 在 MCP 工具层实现

## 插件 Hook 系统

OpenCode 通过 JS/TS 插件系统提供完整 hook 生命周期。插件调用 `router-rs-cli host hook --event=<event> --repo-root <cwd> opencode`（与其它宿主共用 `hook.sh` 的统一命令格式）：

| OpenCode Hook | 等价于 | router-rs 命令 |
|---|---|---|
| `tool.execute.before` | PreToolUse | `router-rs-cli host hook --event=PreToolUse --repo-root <cwd> opencode` |
| `tool.execute.after` | PostToolUse | `router-rs-cli host hook --event=PostToolUse --repo-root <cwd> opencode` |
| `session.idle` | Stop | `router-rs-cli host hook --event=Stop --repo-root <cwd> opencode` |
| `permission.asked` / `permission.replied` | Permission hooks | 权限拦截 |
| `shell.env` | 环境注入 | 注入 `SKILL_FRAMEWORK_ROOT` / `OPENCODE_PROJECT_ROOT` |

插件加载顺序：全局配置 → 项目配置 → `~/.config/opencode/plugins/` → `.opencode/plugins/`

## opencode.json 配置结构

- 项目级: `./opencode.json`；用户级: `~/.config/opencode/opencode.json`
- MCP 注册字段: `mcp`；Agent 注册字段: `agents`
- 目录自动发现: `.opencode/agents/*.md`、`.opencode/commands/*.md`

## 自定义 Home 目录

- `OPENCODE_HOME` 环境变量可覆盖默认 `~/.opencode` 路径
- `--opencode-home` CLI 标志优先级最高（> `--home` 共享参数 > `OPENCODE_HOME` > 默认值）
- 仅影响框架投影定位，不影响 opencode 自身运行时配置

## 权限与安全模型

- 三类: Allow / Ask / Deny；权限分类: read, write, run, browser
- 框架 MCP server 通常注册为 project scope 的 `mcp`
- `permission.asked` / `permission.replied` 插件 hook 可拦截权限请求

## 自定义 Agent 管理

- 通过 `.opencode/agents/*.md` 文件自动发现
- 通过 `opencode.json` 的 `agents` 字段显式声明
- Harness 投影通过 `.opencode/` 目录注入框架 agent

## Fail-open / Fail-closed 设计意图

OpenCode 采用**分层安全策略**：插件层 **fail-open**（JS/TS 插件 hook 失败不阻断编辑器），hook 脚本层对 critical events（`tool.execute.before`、`tool.execute.after`、`session.idle`、`session.created`）仍 **fail-closed**（router-rs 不可用时返回 `decision:block`）。这与 Claude / Cursor / Codex 的纯 fail-closed 策略不同（见 [`hook-hosts.md`](hook-hosts.md) §Fail-open / Fail-closed 比较）。

**设计理由**：OpenCode 的插件 hook 系统通过 JS/TS 运行时执行，hook 失败不应阻断核心编辑器功能。MCP 工具层（`framework_snapshot`、`skill_route` 等）独立于 hook 系统，hook 缺失不影响 MCP 功能。

## 架构对比：TS/JS 插件 vs Rust Native Hook

OpenCode 的 hook 处理层在 **TS/JS 插件系统**中执行，而非 Rust 侧。这与 cursor/claude/codex 三宿主的 Rust hook 分发有本质差异：

| 维度 | cursor/claude/codex | opencode |
|------|--------------------------|----------|
| Hook 运行时 | Rust（`host-projection` crate） | JS/TS 插件运行时 |
| PreToolUse | `PreToolUse` 事件 | `tool.execute.before` 插件事件 |
| PostToolUse | `PostToolUse` 事件 | `tool.execute.after` 插件事件 |
| Stop | `Stop` 事件 | `session.idle` 插件事件 |
| 权限守卫 | Rust 侧实现 | `permission.asked` / `permission.replied` 插件事件 |
| Rust 侧 dispatch | 数千行 | 不需要（插件层处理） |
| Provider trait | 完整实现 | 完整实现（v7 已对齐） |
| `has_native_hook` | `true` | `true` |
| `harness_capabilities` | FULL | FULL |

OpenCode 是 **hook 体系宿主**，只是 hook 处理层在插件系统而非 Rust。Provider trait、harness capabilities 和注册表元数据与其他三宿主完全一致。

## Session/Review 状态管理

OpenCode 通过 `.opencode/hook-state/` 目录管理会话和审核状态，磁盘格式与 cursor/claude/codex 兼容：

```
.opencode/hook-state/
  session_key.json       — session 哈希 + metadata
  review_gate.json       — review 状态 (armed/disarmed/override)
  touch_state.json       — settings/framework 变更追踪
```

- **Session key**: 复用 `core-policy::session_key::extract_session_key()`，与其他宿主共享实现
- **Review gate**: 复用 `HookReviewDiskCore`（core-policy），支持 `armed` / `disarmed` / `override` 三态
- **文件锁**: 使用 `file_state_lock::HookStateConfig`（flock-based），与 Codex 共享抽象

## Provider Trait 实现

OpenCode 的 `OpencodeHostProvider` 实现以下 trait 方法：

| 方法 | 实现值 | 说明 |
|------|--------|------|
| `host_id()` | `"opencode"` | 注册表 ID |
| `aliases()` | `["opencode"]` | CLI 别名 |
| `driver_binary()` | `"opencode"` | 宿主 CLI 二进制名 |
| `transport_type()` | `"native-opencode"` | 与注册表一致 |
| `extract_observation_surfaces()` | 自定义适配 | 适配 opencode hook 输出的 JSON 结构 |
| `has_native_hook()` | `true` | 声明为完整 hook 体系宿主 |

## OpenCode 宿主行为差异

`AGENTS.md` 是唯一的策略真源文件，OpenCode 宿主行为差异内嵌于 `AGENTS.md` § 宿主行为差异 / OpenCode：

- **MCP-native 架构**：通过 JS/TS 插件系统提供 hook，同时通过 `opencode.json` → MCP 提供框架工具
- **权限策略**：fail-open（插件层；hook 脚本层对 critical events 仍 fail-closed）
- **与其他宿主的差异**：opencode 的 `has_hard_gate_hooks` 为 `false`（无 Rust 侧 hard gate），closeout evidence hooks 通过 MCP 工具层实现

## 安装与文件分布

- **Hooks 行为配置**: `.opencode/opencode.json`（项目级）
- **Framework 规则**: 通过 `.opencode/opencode.json` 的 `rules` 字段注入
- **MCP 配置**: `.opencode/opencode.json` 的 `mcpServers` 字段
- **Framework projection manifest**: `.opencode/.framework-projection.json`
- **安装命令**:
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
    framework host-integration install --to opencode --repo-root "$PWD"
  ```
- **跨仓库**: 先设 `SKILL_FRAMEWORK_ROOT`，再用 `framework host-integration install --to opencode`（参见 [`operations/index.md`](../operations/index.md) §跨项目引导）。

## Hook 事件矩阵详细

| 关注点 | 典型触发 | router-rs 路径 | 主要产出 |
|--------|----------|----------------|---------|
| PreToolUse 守卫 | `tool.execute.before` | `opencode_hooks.rs` | 路径保护、框架数据源保护 |
| PostToolUse 证据 | `tool.execute.after` | `opencode_hooks.rs` | `EVIDENCE_INDEX.json` 自动记录 |
| UserPromptSubmit 上下文 | `beforeSubmitPrompt` | `opencode_hooks.rs` | review gate nudge、goal context、paper prose |
| Stop closeout | `session.idle` | `opencode_hooks.rs` | review gate 状态检查、closeout advisory |
| SessionStart | `session.created` | `opencode_hooks.rs` | 框架上下文注入 |
| SubagentStart/Stop | `subagent.start/stop` | `opencode_hooks.rs` | 子代理生命周期遥测 |
| Permission hooks | `permission.asked/replied` | `opencode_hooks.rs` | 权限拦截 |
| Shell env | `shell.env` | `opencode_hooks.rs` | 环境变量注入 |
| File edited | `file.edited` | `opencode_hooks.rs` | 文件变更追踪 |

**环境变量**：
- `ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE=1` — 紧急禁用 review gate
- `ROUTER_RS_OPERATOR_INJECT=0` — 禁用 SessionStart/UPS 上下文注入
- `ROUTER_RS_REVIEW_GATE_DISABLE=1` — 全局禁用 review gate（所有宿主）

**Hook 状态目录**: `.opencode/hook-state/`（review gate 状态、session 状态）

**锁机制**: `file_state_lock::HookStateConfig`（flock-based，与 Codex 共享抽象）
