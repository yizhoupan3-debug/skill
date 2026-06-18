---
last_verified: "2026-06-16"
depends_on:
  - ../spec.md
---

# Opencode 宿主操作手册

**闭集 id**: `opencode` · **传输**: native-opencode（JS/TS 插件 hook + MCP 双通道） · **权威**: `RUNTIME_REGISTRY.json` → `host_projections.opencode`

## 代理身份与画风

- 主代理定位为严谨的科研学者与系统工程专家
- 回复画风专业、客观、谦逊；默认使用简体中文
- 不确定的信息直接说明，不编造

## 能力边界与 Harness 入口

- **任务推进**: `/implementx` + `framework_goal_drive` stdio
- **任务状态**: `artifacts/current/<task_id>/GOAL_STATE.json`
- **门控模式**: 插件 hook + MCP 工具层双通道。`tool.execute.before` 可 throw 阻断工具执行（等价 PreToolUse）；`session.idle` 等价 Stop；closeout/review 在 MCP 工具层实现

## 插件 Hook 系统

OpenCode 通过 JS/TS 插件系统提供完整 hook 生命周期：

| OpenCode Hook | 等价于 | 能力 |
|---|---|---|
| `tool.execute.before` | PreToolUse | 拦截工具调用，可修改参数或 throw 阻断 |
| `tool.execute.after` | PostToolUse | 工具执行后处理 |
| `session.idle` | Stop | 会话空闲时触发 |
| `permission.asked` / `permission.replied` | Permission hooks | 权限拦截 |
| `shell.env` | 环境注入 | 注入 shell 环境变量 |

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

## 默认生命周期

- `/discussx` → `/planx` → `/implementx` → `/verifyx`
- 默认 `my-light`（advisory closeout）

## 自检诊断

```bash
router-rs framework doctor
router-rs framework host-integration status
```

## Python 环境

- 使用 uv-only、默认 3.12；仓库级 `uv.lock`

## Fail-open / Fail-closed 设计意图

OpenCode 采用 **fail-open** 策略：当 `router-rs` hook 二进制缺失或不可读时，`tool.execute.before` 事件静默通过（不阻断工具执行）。这与 Claude / Codex CLI 的 fail-closed 策略不同。

**设计理由**：OpenCode 的插件 hook 系统通过 JS/TS 运行时执行，hook 失败不应阻断核心编辑器功能。MCP 工具层（`framework_snapshot`、`skill_route` 等）独立于 hook 系统，hook 缺失不影响 MCP 功能。

**对比**：
| 宿主 | 策略 | Hook 缺失时行为 |
|------|------|----------------|
| Claude | fail-closed | Stop 返回 `decision:block` |
| Codex | fail-closed | 各事件返回 `decision:block` |
| Cursor | fail-closed | 各事件返回 `decision:block` |
| OpenCode | fail-open | 静默通过，MCP 工具层不受影响 |

## 架构对比：TS/JS 插件 vs Rust Native Hook

OpenCode 的 hook 处理层在 **TS/JS 插件系统**中执行，而非 Rust 侧。这与 cursor/claude-code/codex 三宿主的 Rust hook 分发有本质差异：

| 维度 | cursor/claude-code/codex | opencode |
|------|--------------------------|----------|
| Hook 运行时 | Rust（`host-projection` crate） | JS/TS 插件运行时 |
| PreToolUse | `PreToolUse` 事件 | `tool.execute.before` 插件事件 |
| PostToolUse | `PostToolUse` 事件 | `tool.execute.after` 插件事件 |
| Stop | `Stop` 事件 | `session.idle` 插件事件 |
| 权限守卫 | Rust 侧实现 | `permission.asked` / `permission.replied` 插件事件 |
| Rust 侧 dispatch | ✅ 数千行 | ❌ 不需要（插件层处理） |
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

## AGENTS_OPENCODE.md 说明

`AGENTS_OPENCODE.md`（仓库根目录）是 OpenCode 宿主 delta 文件，与 `AGENTS.md`（跨宿主内核）配合使用：

- **双文件注入**：`AGENTS.md`（跨宿主内核）+ `AGENTS_OPENCODE.md`（宿主 delta）
- **内容**：MCP-native 架构说明、permission 规则替代 PreToolUse、session/review 状态管理路径
- **与其他宿主的差异**：opencode 的 `has_hard_gate_hooks` 为 `false`（无 Rust 侧 hard gate），closeout evidence hooks 不支持

## 安装与文件分布

- **Hooks 行为配置**: `.opencode/opencode.json`（项目级）
- **Framework 规则**: 通过 `.opencode/opencode.json` 的 `rules` 字段注入
- **MCP 配置**: `.opencode/opencode.json` 的 `mcpServers` 字段
- **Framework projection manifest**: `.opencode/.framework-projection.json`
- **安装命令**:
  ```bash
  ./scripts/install-opencode.sh
  # 或仅全局：./scripts/install-opencode.sh --scope user
  ```
- **跨仓库**: `./scripts/opencode-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"`

## Skill 存放与路由

- **Skill 存放**: 统一在项目根目录 `skills/` 文件夹
- **热路由入口**: `skills/SKILL_ROUTING_RUNTIME.json`
- **冷表清单**: `skills/SKILL_MANIFEST.json`
- **查找原则**: 通过路由表精确匹配，不模糊猜测

## 进程管理与性能调优

1. **构建 Release 二进制**:
   ```bash
   CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
     cargo build --release --manifest-path core/router-rs/Cargo.toml
   ```
2. **Launcher 探测顺序**: 仓库 `core/router-rs/target/release` → `/tmp/skill-cargo-target/release` → debug → `PATH`

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
