# Opencode Agent Policy

跨宿主协议见 [`AGENTS.md`](AGENTS.md)。**双文件注入**：须与 `AGENTS.md` 同时生效，勿单独使用本文件。本文仅 **OpenCode**（`opencode`）transport delta。手册 [`docs/hosts/opencode.md`](docs/hosts/opencode.md) · [`docs/spec.md`](docs/spec.md) §0.1。

---

## Transport 要点

- **插件 hook + MCP 双通道**：OpenCode 通过 JS/TS 插件系统提供 hook（`tool.execute.before`、`tool.execute.after`、`session.idle` 等），同时通过 `opencode.json` → MCP 提供框架工具。插件目录：`~/.config/opencode/plugins/` + `.opencode/plugins/`。
- **安装**：`framework host-integration install --to opencode --repo-root "$PWD"`。
- **Task 子代理**：`WAVE_STATE` 中 `execution_mode=parallel` 时应 spawn；输出 `artifacts/current/<task_id>/lane-notes/<lane_id>.md`。
- **Review / closeout**：清门 **Claude canonical**；Stop review **advisory-only**（MCP `ADVISORY`）；非 my-light 时 MCP 可对**未满足 closeout 证据** hard-block（与 review 分层）。`ROUTER_RS_CLOSEOUT_ENFORCEMENT` 见 [`docs/references/AGENTS_OPERATOR_SURFACE.md`](docs/references/AGENTS_OPERATOR_SURFACE.md)。
- **联网**：`browser-mcp` MCP 提供浏览器自动化（`host-integration install` 自动注册）。

---

## 架构：TS/JS 插件 vs Rust Native Hook

OpenCode 的 hook 处理层在 **TS/JS 插件系统**中执行，而非 Rust 侧。这与 cursor/claude-code/codex 三宿主的 Rust hook 分发有本质差异：

| 维度 | cursor/claude-code/codex | opencode |
|------|--------------------------|----------|
| Hook 运行时 | Rust（`host-projection` crate） | JS/TS 插件运行时 |
| PreToolUse | `PreToolUse` 事件 | `tool.execute.before` 插件事件 |
| PostToolUse | `PostToolUse` 事件 | `tool.execute.after` 插件事件 |
| Stop | `Stop` 事件 | `session.idle` 插件事件 |
| 权限守卫 | Rust 侧实现 | `permission.asked` / `permission.replied` 插件事件 |
| Rust 侧 dispatch | ✅ 数千行 | ❌ 不需要（插件层处理） |
| Provider trait | 完整实现 | 完整实现（v6.5 已对齐） |
| `has_native_hook` | `true` | `true` |
| `harness_capabilities` | FULL | FULL |

OpenCode 是 **hook 体系宿主**，只是 hook 处理层在插件系统而非 Rust。Provider trait、harness capabilities 和注册表元数据与其他三宿主完全一致。

---

## Hook 事件矩阵

OpenCode 通过 JS/TS 插件系统提供完整 hook 生命周期：

| OpenCode Hook | 等价于 | 能力 |
|---|---|---|
| `tool.execute.before` | PreToolUse | 拦截工具调用，可修改参数或 throw 阻断 |
| `tool.execute.after` | PostToolUse | 工具执行后处理 |
| `session.idle` | Stop | 会话空闲时触发 |
| `permission.asked` / `permission.replied` | Permission hooks | 权限拦截 |
| `shell.env` | 环境注入 | 注入 shell 环境变量 |

插件加载顺序：全局配置 → 项目配置 → `~/.config/opencode/plugins/` → `.opencode/plugins/`

### 详细事件路径

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

---

## opencode.json 配置结构

- 项目级: `./opencode.json`；用户级: `~/.config/opencode/opencode.json`
- MCP 注册字段: `mcp`；Agent 注册字段: `agents`
- 目录自动发现: `.opencode/agents/*.md`、`.opencode/commands/*.md`

### 关键环境变量

| 环境变量 | 作用 | 默认值 |
|----------|------|--------|
| `ROUTER_RS_OPENCODE_REVIEW_GATE_DISABLE` | 紧急禁用 review gate | `0`（启用） |
| `ROUTER_RS_OPERATOR_INJECT` | 禁用 SessionStart/UPS 上下文注入 | `1`（启用） |
| `ROUTER_RS_REVIEW_GATE_DISABLE` | 全局禁用 review gate（所有宿主） | `0`（启用） |
| `OPENCODE_HOME` | 覆盖默认 `~/.opencode` 路径 | `~/.opencode` |

---

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

---

## 权限与安全模型

- 三类: Allow / Ask / Deny；权限分类: read, write, run, browser
- 框架 MCP server 通常注册为 project scope 的 `mcp`
- `permission.asked` / `permission.replied` 插件 hook 可拦截权限请求
- OpenCode 采用 **fail-open** 策略：hook 缺失时不阻断工具执行（与 Claude/Codex/Cursor 的 fail-closed 不同）

---

## 默认生命周期

$$\text{Discuss} \longrightarrow \text{Plan} \longrightarrow \text{Implement} \longrightarrow \text{Verify}$$

1. **`/discussx`**：初始需求对齐与技术预研
2. **`/planx`**：规划阶段，生成 `ROADMAP.md` 与 `WAVE_STATE.json`
3. **`/implementx`**：执行阶段，配合 `framework_goal_drive` stdio
4. **`/verifyx`**：验证与清理收尾

默认 `lifecycle_profile: my-light`（advisory closeout，无 spawn-first nudge）

---

## 自定义 Home 目录

- `OPENCODE_HOME` 环境变量可覆盖默认 `~/.opencode` 路径
- `--opencode-home` CLI 标志优先级最高（> `--home` 共享参数 > `OPENCODE_HOME` > 默认值）
- 仅影响框架投影定位，不影响 opencode 自身运行时配置

---

## 自定义 Agent 管理

- 通过 `.opencode/agents/*.md` 文件自动发现
- 通过 `opencode.json` 的 `agents` 字段显式声明
- Harness 投影通过 `.opencode/` 目录注入框架 agent

---

## Python 环境

- 使用 uv-only、默认 3.12；仓库级 `uv.lock`
- 禁止使用 `pip`

---

## 进程管理与性能调优

1. **构建 Release 二进制**:
   ```bash
   CARGO_TARGET_DIR="$PWD/core/router-rs/target" \
     cargo build --release --manifest-path core/router-rs/Cargo.toml
   ```
2. **Launcher 探测顺序**: 仓库 `core/router-rs/target/release` → `/tmp/skill-cargo-target/release` → debug → `PATH`

---

## Skill 存放与路由

- **Skill 存放**: 统一在项目根目录 `skills/` 文件夹
- **热路由入口**: `skills/SKILL_ROUTING_RUNTIME.json`
- **冷表清单**: `skills/SKILL_MANIFEST.json`
- **查找原则**: 通过路由表精确匹配，不模糊猜测

---

## 自检诊断

```bash
router-rs framework doctor
router-rs framework host-integration status
```

---

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

---

## 与其他宿主的差异对比

| 维度 | claude-code | cursor | codex | opencode | mimo |
|------|-------------|--------|-------|----------|------|
| Hook 运行时 | Rust | Rust | Rust | JS/TS 插件 | Rust |
| Hard gate hooks | ✓ | ✓ | ✓ | ✗（fail-open） | ✓ |
| Closeout evidence hooks | ✓ | ✓ | ✓ | ✗ | ✓ |
| Review gate | Rust 侧 | Rust 侧 | Rust 侧 | MCP 工具层 | Rust 侧 |
| Session supervisor | mcp_bridge | ✗ | codex_driver | ✗ | ✗ |
| Worktree 支持 | ✓ | ✓ | ✓ | ✗ | ✗ |
| Provider trait 完整 | ✓ | ✓ | ✓ | ✓ | ✓ |
| `harness_capabilities` | FULL | FULL | FULL | FULL | FULL |
