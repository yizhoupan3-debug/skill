---
last_verified: "2026-06-22"
depends_on:
  - _common.md
  - ../adr/010-ideal-architecture-v10.md
parent: _common.md
---

# OpenCode 宿主操作手册

**闭集 id**: `opencode` · **传输**: native-opencode（Rust hook（hook.sh → router-rs-cli）+ MCP 双通道） · **权威**: `RUNTIME_REGISTRY.json` → `host_projections.opencode`

**共通内容**（代理身份与画风、Skill 路由、默认生命周期、Python 环境、进程管理与性能调优）见 [`_common.md`](_common.md)。

---

## 能力边界与 Harness 入口

- **任务推进**: `/implementx` + `framework_goal_drive` stdio
- **任务状态**: `artifacts/current/<task_id>/GOAL_STATE.json`
- **门控模式**: shell hook（hook.sh → router-rs-cli）+ MCP 工具层双通道。`tool.execute.before` 经 hook.sh 路由到 Rust `OpenCodeDispatcher`，等价 PreToolUse；`session.idle` 等价 Stop；closeout/review 在 MCP 工具层实现

## Shell Hook 系统

OpenCode 通过 `hook.sh` 统一调用 `router-rs-cli host hook --event=<event> --repo-root <cwd> opencode`（与 Claude/Cursor/Codex 共用同一 shell hook 入口）：

| OpenCode 事件 | 等价于 | hook.sh 事件名 |
|---|---|---|
| `tool.execute.before` | PreToolUse | `tool.execute.before` |
| `tool.execute.after` | PostToolUse | `tool.execute.after` |
| `session.idle` | Stop | `session.idle` |
| `session.created` | SessionStart | `session.created` |

Rust 侧 `OpenCodeDispatcher` 实现 `HostHookDispatcher` trait，处理 PreToolUse 路径保护、Stop 统一流水线等。

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
- `permission.asked` / `permission.replied` hook 可拦截权限请求

## 自定义 Agent 管理

- 通过 `.opencode/agents/*.md` 文件自动发现
- 通过 `opencode.json` 的 `agents` 字段显式声明
- Harness 投影通过 `.opencode/` 目录注入框架 agent

## Fail-closed 安全策略

OpenCode 采用与 Claude / Cursor / Codex 一致的 **fail-closed** 策略：hook 脚本层对 critical events（`tool.execute.before`、`tool.execute.after`、`session.idle`、`session.created`）在 router-rs 不可用时返回 `decision:block`。

**设计理由**：OpenCode 的 hook 通过 `hook.sh` → `router-rs-cli` 调用 Rust `OpenCodeDispatcher`，与其他三宿主共享同一 fail-closed 路径。MCP 工具层（`framework_snapshot`、`skill_route` 等）独立于 hook 系统，hook 缺失不影响 MCP 功能。

## 与其他宿主的架构对齐

OpenCode 的 hook 处理层在 **Rust 侧**（`host-projection` crate 的 `OpenCodeDispatcher`），与其他三宿主（Claude/Cursor/Codex）完全一致：

| 维度 | Claude/Cursor/Codex | OpenCode |
|------|---------------------|----------|
| Hook 入口 | `hook.sh` → `router-rs-cli` | `hook.sh` → `router-rs-cli` |
| Hook 运行时 | Rust（`host-projection` crate） | Rust（`host-projection` crate） |
| PreToolUse | `HostHookDispatcher::handle_pre_tool_use()` | 同左（`OpenCodeDispatcher` 覆盖） |
| Stop | `run_unified_stop()` 13步流水线 | 同左 |
| fail 模式 | fail-closed | fail-closed |
| Provider trait | 完整实现 | 完整实现 |
| `has_native_hook` | `true` | `true` |
| `harness_capabilities` | FULL | FULL |

OpenCode 是 **hook 体系宿主**，hook 处理层在 Rust 侧，与其他三宿主完全对齐。

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

- **shell hook + MCP 双通道**：通过 hook.sh → router-rs-cli 提供 hook，同时通过 `opencode.json` → MCP 提供框架工具
- **权限策略**：fail-closed（hook 脚本层对 critical events fail-closed，与其他宿主一致）
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
| PreToolUse 守卫 | `tool.execute.before` | [`hosts/hook_dispatch.rs`](../../core/host-projection/src/hosts/hook_dispatch.rs) — 统一 `HostHookDispatcher::dispatch()`，所有 4 宿主共享同一 path | 路径保护、框架数据源保护 |
| PostToolUse 证据 | `tool.execute.after` | 同上 | `EVIDENCE_INDEX.json` 自动记录 |
| UserPromptSubmit 上下文 | `beforeSubmitPrompt` | 同上 | review gate nudge、goal context、paper prose |
| Stop closeout | `session.idle` | [`hosts/stop_dispatch.rs`](../../core/host-projection/src/hosts/stop_dispatch.rs) — 统一 Stop 管线 | review gate 状态检查、closeout advisory |
| SessionStart | `session.created` | `hosts/hook_dispatch.rs` | 框架上下文注入 |
| SubagentStart/Stop | `subagent.start/stop` | 同上 | 子代理生命周期遥测 |
| Permission hooks | `permission.asked/replied` | 同上 | 权限拦截 |
| Shell env | `shell.env` | 同上 | 环境变量注入 |
| File edited | `file.edited` | 同上 | 文件变更追踪 |

**环境变量**：
- `ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE=1` — 紧急禁用 review gate
- `ROUTER_RS_OPERATOR_INJECT=0` — 禁用 SessionStart/UPS 上下文注入
- `ROUTER_RS_REVIEW_GATE_DISABLE=1` — 全局禁用 review gate（所有宿主）

**Hook 状态目录**: `.opencode/hook-state/`（review gate 状态、session 状态）

**锁机制**: `file_state_lock::HookStateConfig`（flock-based，与 Codex 共享抽象）
