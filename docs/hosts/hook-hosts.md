---
last_verified: "2026-06-18"
depends_on:
  - _common.md
  - ../spec.md
parent: _common.md
---

# Hook 宿主手册 (Hook Hosts)

本文档覆盖 **Claude** / **Cursor** / **Codex** / **MiMo** 四宿主。**OpenCode** 因架构差异（JS/TS 插件系统 + fail-open）单独见 [`opencode.md`](opencode.md)。

**共通内容**（代理身份与画风、Skill 路由、默认生命周期、Python 环境、进程管理与性能调优）见 [`_common.md`](_common.md)。

---

## 总览

| 属性 | Claude | Cursor | Codex | MiMo |
|------|--------|--------|-------|------|
| 闭集 id | `claude` | `cursor` | `codex` | `mimo` |
| 传输 | claude-hooks | cursor-hooks | codex-hooks | native-mimo (hook) |
| Hook 事件数 | 4（减法闭集） | 7（减法闭集） | 事件驱动（review gate + evidence） | 7（与 Cursor 对齐） |
| Fail 策略 | fail-closed | fail-closed | fail-closed | fail-closed |
| 注册表源 | `RUNTIME_REGISTRY.json` → `host_projections.claude` | `RUNTIME_REGISTRY.json` → `host_projections.cursor` | `RUNTIME_REGISTRY.json` → `host_projections.codex` | `RUNTIME_REGISTRY.json` → `host_projections.mimo` |
| 多代理支持 | 原生 Task（无 hook 门控） | `subagentStart`/`subagentStop` hook 门控 | agent 自觉驱动（无 hook 门控） | `SubagentStart`/`SubagentStop` 追踪 |
| Session Supervisor | 不支持 | 不支持 | **支持**（launch / resume / terminate） | 不支持 |

---

## Hook 事件矩阵

### Claude — 4 事件

**默认注册 4 事件**（减法闭集）：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`。项目 env：[`.claude/router-rs-hook.env`](../../.claude/router-rs-hook.env)（模板 [`configs/framework/claude-router-rs-hook.env`](../../configs/framework/claude-router-rs-hook.env)）；launcher **release 优先**（[`claude-router-rs-hook.sh`](../../configs/framework/claude-router-rs-hook.sh)）。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| PreTool / Stop 守卫、settings 变更提示 | 宿主 hooks 调用 `router-rs claude hook --event=PreToolUse\|Stop\|…` | [`claude_hooks.rs`](../../core/host-projection/src/hosts/claude_hooks.rs) | `.claude/hook-state/review_gate_*.json`、`.claude/hook-state/hook_state_*.json`（Cursor 指纹 payload 静默忽略）；出站 Claude hook JSON |
| **Claude Stop × `.claude` 状态 JSON** | Stop | `claude_hooks::run_stop` | `hook-state/review_gate_*.json` / `hook_state_*.json` 缺失不单独拦截；**已存在但不可读或损坏**：**fail-closed**，`stopReason` 含 `CLAUDE_HOOK_STATE_UNREADABLE` |
| 投影规则与 hook 绑定 | `router-rs framework host-integration install --to claude` | [`host_integration/mod.rs`](../../core/host-projection/src/host_integration/mod.rs) | `.claude/rules/framework.md`、`.claude/settings.json`（四事件 hook）、`.claude/.framework-projection.json`（project scope） |
| **Paper prose L4** | `UserPromptSubmit` 写作/润色语境 | `paper_prose_hook.rs` | `PAPER_PROSE_QUALITY_HOOK`（**默认开**：`ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK`）；`ROUTER_RS_CLAUDE_PAPER_ADVERSARIAL_HOOK=1` opt-in |

### Cursor — 7 事件

**默认注册 7 事件**（2026-05-20 减法闭集）：`beforeSubmitPrompt`、`stop`、`sessionStart`、`sessionEnd`、`postToolUse`、`subagentStart`、`subagentStop`。已移除：`afterAgentResponse`、`beforeShellExecution`/`afterShellExecution`、`afterFileEdit`、`preCompact`（恢复见 [`MIGRATION.md`](../../MIGRATION.md)）。`postToolUse` 对非门控工具走 **fast-path**（[`post_tool_use_needs_work`](../../core/host-projection/src/hosts/cursor_hooks/handlers.rs)）。

项目 env：[`.cursor/router-rs-hook.env`](../../.cursor/router-rs-hook.env)。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| Review / subagent 门控、beforeSubmit/Stop | `router-rs cursor hook <event>` | `cursor_hooks::execute_cursor_hook` → `CursorHookHost::dispatch` → `dispatch_cursor_hook_event` | `.cursor/hook-state/review-subagent-*.json`；**`ROUTER_RS_CURSOR_REVIEW_GATE_MODE`**=`strict`（默认 multiset）或 `lite`（仅 `id:` pending）；`framework doctor` 打印 mode；Stop advisory 提示上限 **`ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES`** |
| Stop / beforeSubmit 出站 | Same | [`cursor_hooks/`](../../core/host-projection/src/hosts/cursor_hooks/mod.rs) | **my-light Stop 早退**：仅 `CLOSEOUT_FOLLOWUP` + `SESSION_CLOSE_STYLE`（无 `REVIEW_GATE` / `AG_FOLLOWUP`）；非 my-light 保留完整 Stop 链；**不**合并 `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` |
| **Paper prose L4** | beforeSubmit 命中 `has_paper_prose_edit_context` | `paper_prose_hook.rs` | 合并 `PAPER_PROSE_QUALITY_HOOK`（**默认开**：`ROUTER_RS_CURSOR_PAPER_PROSE_HOOK`，`0` 关）；对抗审稿 opt-in：`ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK=1` |
| **SessionStart** | 会话启动 | `cursor_hooks`（`handle_session_start`） | **仅** `Repo:` 单行（`ROUTER_RS_OPERATOR_INJECT=0` 时为空）；**无** digest / 无 pointer hint |
| **运维自检** | 手工排障 | `router-rs framework doctor --repo-root <repo>` | **metadata-only** `generated-artifacts-status`；`ROUTER_RS_TASK_LEDGER_FLOCK` 关闭时打印 WARN |

### Codex

细则见 [`spec.md`](../spec.md) §13、「主数据流」与 `.codex/hooks.json`。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| PostTool 证据、`CODEX_REVIEW_GATE` | 配置项指向 `router-rs codex hook …` | `codex hook`（[`codex_hooks/mod.rs`](../../core/host-projection/src/hosts/codex_hooks/mod.rs)） | **opt-in** `EVIDENCE_INDEX` 追加；SessionStart **不**注入 continuity digest / `GOAL_CONTINUE`；wave-2：PostTool 深度 lane → `phase≥2`，Stop compact/rg_clear 清门；`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` 关闭 review nudge |
| **Paper prose L4** | `UserPromptSubmit` 写作/润色语境 | `paper_prose_hook.rs` | `PAPER_PROSE_QUALITY_HOOK`（**默认开**：`ROUTER_RS_CODEX_PAPER_PROSE_HOOK`）；`ROUTER_RS_CODEX_PAPER_ADVERSARIAL_HOOK=1` opt-in |
| **Codex hook stdout** | 任一 hook 进程退出 0 | `dispatch_codex_command` → `codex_hook_stdout_payload` | **始终**打印单行紧凑 JSON；无附带输出时为 **`{}`** |
| **Codex Stop × `.codex/hook-state`** | Stop 事件 | `handle_codex_stop` | 状态文件缺失：不据此拦截；状态不可读（损坏 JSON / IO）：**fail-closed**，`followup_message` 含 `CODEX_HOOK_STATE_UNREADABLE` |
| 宿主入口对齐 | `router-rs framework sync-entrypoints --host-id codex` | shared `host_entrypoint_sync` + Codex provider | 生成 `.codex/hooks.json`、**`AGENTS_CODEX.md`**、`.codex/README.md` 及 **`host_entrypoints_sync_manifest`**；跨宿主内核 **[`AGENTS.md`](../../AGENTS.md)** 人工维护、不由 sync 覆盖 |

### MiMo

**注册 7 事件**（与 Cursor 对齐，减法闭集）：`PreToolUse`、`PostToolUse`、`UserPromptSubmit`、`Stop`、`SessionStart`、`SubagentStart`、`SubagentStop`。Rust 侧 [`mimo_hooks.rs`](../../core/host-projection/src/hosts/mimo_hooks.rs) 仅 `PreToolUse` + `Stop` 有显式 dispatch；其余事件通过 `router-rs host mimo mimo-hook <event>` 统一 CLI 路径处理。Hook 状态目录为 `.mimo/hook-state/`。

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| PreToolUse 路径保护 | 工具调用匹配 `Bash\|Write\|Edit\|NotebookEdit` | `mimo_hooks::run_mimo_hook` → `PreToolUse` | 未授权工具调用 → `continue:false`（硬阻断）；未物化 GOAL_STATE 同理 |
| PostToolUse 证据收集 | 工具调用匹配 `Bash\|Write\|Edit\|Agent` | `mimo_hooks::run_mimo_hook` → `PostToolUse` | `EVIDENCE_INDEX.json` 自动记录（opt-in） |
| UserPromptSubmit 上下文 | 任意用户提交 | `mimo_hooks::run_mimo_hook` → `UserPromptSubmit` | review gate 检测 + context 注入 |
| Stop closeout | 会话 stop | `mimo_hooks::run_mimo_hook` → `Stop` | closeout gate + review gate 状态检查 |
| SessionStart | 会话启动 | `mimo_hooks::run_mimo_hook` → `SessionStart` | context 注入 |
| SubagentStart/Stop | 子代理生命周期 | `mimo_hooks::run_mimo_hook` → `SubagentStart/Stop` | 子代理追踪 |

**统一原则**：宿主配置命令须 **短命 + 超时**；语义在 Rust，不在 shell 脚本分支。

---

## Fail-open / Fail-closed 比较

| 宿主 | 策略 | Hook 缺失时行为 | 设计理由 |
|------|------|----------------|----------|
| Claude | **fail-closed** | Stop 返回 `decision:block` | 4 事件深度嵌入会话，`Stop` 可阻断提交。二进制损坏 → 安全关键路径断裂 → 避免无审查不可逆操作 |
| Cursor | **fail-closed** | critical 事件返回 `continue:false` / `permission:deny` | 7 事件紧密嵌入生命周期。`stop` 可阻断提交、`beforeSubmitPrompt` 可注入 nudge。critical 事件缺 binary 阻止不可逆操作 |
| Codex | **fail-closed** | 各事件返回 `decision:block` | `.codex/hooks.json` 解析顺序：`ROUTER_RS_BIN` → 仓库 `target/{release,debug}` → `command -v router-rs`；解析失败直接阻断 |
| MiMo | **fail-closed** | PreToolUse 与 Stop 返回 `decision:block` | 7 事件紧密嵌入，PreToolUse 可硬阻断工具调用，Stop 驱动 closeout gate。二进制损坏 → 安全路径断裂 |

---

## 安装与文件分布

| 关注点 | Claude | Cursor | Codex | MiMo |
|--------|--------|--------|-------|------|
| **Hooks 配置** | `.claude/settings.json` | `.cursor/hooks.json` | `.codex/hooks.json` | `.mimo/settings.json` |
| **环境变量文件** | `.claude/router-rs-hook.env` | `.cursor/router-rs-hook.env` | — | `.mimo/router-rs-hook.env`（可选） |
| **Framework rules** | `.claude/rules/framework.md` (project)；`~/.claude/rules/framework.md` (user) | `~/.cursor/rules/framework.mdc` (user)；`.cursor/rules/*.mdc` (project) | `.codex/prompts/framework.md` (project) | `AGENTS_MIMO.md` (仓库根，双文件注入) |
| **Project 叙事** | `.claude/CLAUDE.md` | `.cursor/commands/*.md`、`.cursor/agents/deep-reviewer.md` | — | — |
| **AGENTS delta** | `AGENTS_CLAUDE.md` (仓库根) | `AGENTS_CURSOR.md` (仓库根) | `AGENTS_CODEX.md` (仓库根) | `AGENTS_MIMO.md` (仓库根) |
| **Hook state 目录** | `.claude/hook-state/` | `.cursor/hook-state/` | `.codex/hook-state/` | `.mimo/hook-state/` |
| **Projection manifest** | `.claude/.framework-projection.json` | — | — | `.mimo/.framework-projection.json` |

### 安装命令

**Claude**（须含 user scope 刷新 `~/.claude/rules/framework.md`）：
```bash
./scripts/install-claude.sh
# 或仅全局：./scripts/install-claude.sh --scope user
```
其它仓库：`./scripts/claude-bootstrap-framework.sh --framework-root "$SKILL_FRAMEWORK_ROOT"` + `install-claude.sh --scope user`。

**Cursor**：
```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to cursor --scope user
```

**Codex**（修改了 router-rs 嵌入的 AGENTS 文本、hook 模板或需重新材料化时）：
```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
```

**MiMo**：
```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- \
  framework host-integration install --to mimo --scope user
# 亦可直接：./scripts/install-claude.sh
```

---

## 各宿主差异速查

### Claude

- **能力边界**：4 核心 hook 事件；深度 Review 默认 `lifecycle_profile: my-light` 不注入 spawn-first；非 my-light 时 spawn-first 配对审稿，见 [`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)
- **Review Gate**：全局 advisory-only（仅 `followup_message` nudge）；my-light 下 Stop 上 `REVIEW_GATE` / `AG_FOLLOWUP` 关闭，仅保留 `CLOSEOUT_FOLLOWUP` + `SESSION_CLOSE_STYLE`
- **自检命令**：
  ```bash
  cargo test --manifest-path core/router-rs/Cargo.toml claude
  ```
  通用：`cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status`

### Cursor

- **能力边界**：7 事件 hook；Stop `REVIEW_GATE` 全局 advisory-only（对齐 [`AGENTS_CURSOR.md`](../../AGENTS_CURSOR.md)）；`lifecycle_profile: my-light` suppress `REVIEW_GATE` / spawn-first nudge
- **自检命令**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-cursor-hooks
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status
  cargo test --manifest-path core/router-rs/Cargo.toml host_integration
  ```
- **Cursor 独有环境变量**：
  - `ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE` — 自动注入子代理继承主会话模型
  - `ROUTER_RS_CURSOR_REVIEW_GATE_MODE` — `strict`（默认 multiset）或 `lite`
  - `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` — Stop advisory 提示上限
  - `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` — 缺省 `fork_context` 推断
  - `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` — 磁盘 GOAL_STATE 严格模式
  - `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN` — 应急 fail-open
  - `ROUTER_RS_CURSOR_SESSION_NAMESPACE` — 多聊天隔离

#### Cursor 排障

**症状**：对话像被掐断、无法提交、子代理 `permission: deny`、Stop 后需手动 `/implementx` 续跑。

| 现象 | 常见根因 | 处理 |
|------|----------|------|
| Stop 后任务未完成 | **无** hook `GOAL_CONTINUE`（2026-05 已删） | `/implementx` + `framework_goal_drive` stdio + `artifacts/current/<task_id>/` |
| Stop 后出现 `router-rs REVIEW_GATE` / `AG_FOLLOWUP` | 非 **my-light** 且 review 未清门（advisory nudge，非硬拦） | 先 spawn `fork_context=false` 深度 lane；或 `rg_clear` / 拆开 review 与 `/implementx` |
| `beforeSubmit` 无法继续（`continue:false`） | hook-state 锁/持久化失败 | 查 `.cursor/hook-state` 权限；应急 `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1` |
| 子代理 `permission: deny`（open count） | 重复 `subagentStart` 或 session 分片 | 看 `review-subagent-*.json` 的 `active_subagent_count` vs pending；升级后旧 state 可删或等新会话 |
| PostTool 卡 ~20s | L1/L3 争用或 armed 全路径 L3 | 默认已修 L3→L1 逆序；仍慢则 w2 压测后可将 gate timeout 提到 25（见 `.cursor/hooks.json`） |
| 双聊天互相影响 | 同 `cwd` 共桶 | 各聊天设 **`ROUTER_RS_CURSOR_SESSION_NAMESPACE`**（见 `.cursor/router-rs-hook.env` 注释） |
| `CLOSEOUT_FOLLOWUP`（my-light） | 无磁盘 goal 仍声称完成 | 仅 hydration 有 `GOAL_STATE` 时触发；口语「完成了」不应再拦 |

**其他注意事项**：
- **PostToolUse timeout**：门控事件默认 **20s**（`hooks.json`）；`postToolUse` 超时会导致 review multiset 不完整 → Stop 循环。慢盘先查 hook-state 体积与锁 stderr（`hook-state lock held`）。
- **router-rs 缺失**：critical 事件 **fail-closed**（`continue:false` / 工具拒绝）；确保 `core/router-rs/target/release/router-rs` 存在或 `ROUTER_RS_BIN` 指向二进制。
- **SESSION_CLOSE_STYLE**：每轮 Stop 可能注入软提示；不需要时设 `ROUTER_RS_OPERATOR_INJECT=0`。
- **session_key 升级**：修复后 hook-state 文件名 hash 可能变化；首会话门控状态重置，可用 `rg_clear` 或删 `.cursor/hook-state/review-subagent-*.json`（仅本机调试）。
- **`fork_context` 缺省**：默认 **`ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` 开启**时可推断 `false`；关闭后仅布尔 `false` 计独立证据。显式 `fork_context: true` 永不算。
- **磁盘 `GOAL_STATE` 与 pre-goal**：默认 strict；legacy 宽松设 `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK=0|false|off|no`。
- **`cursor-router-rs-hook.sh` exit code**：critical 事件（beforeSubmit/Stop/postToolUse/subagentStart/subagentStop）在 `router-rs` 缺失时 **fail-closed**；其余 **fail-open**。
- **fail-closed 出站字段（按事件）**：beforeSubmit / PostTool（review-armed 锁失败）/ Stop（部分路径）→ `"continue": false`；subagentStart（限额/锁失败）→ `"permission": "deny"`。launcher 缺 binary 时 PostTool 亦 `continue:false`。
- **仿宿主续跑行**：聊天区无 `router-rs ` 前缀的仿机读行勿当 hook 真源；以 hook stdout JSON 为准。
- **清门粘贴**：勿把 **`RG_FOLLOWUP`…** 当清门令牌；请用 **`rg_clear`**、拒因 token，或自然语言 override。

### Codex

- **能力边界**：事件驱动（review gate + evidence）；SessionStart **不**注入 continuity digest / `GOAL_CONTINUE`
- **独有 Session Supervisor**：原生进程生命周期管理（`launch` / `resume` / `terminate` / `mark_blocked` / `resume_due`）
- **独有环境变量**：
  - `ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` — **默认开**（`unset` = 开）。PostTool 深度 lane 且省略 `fork_context` 时可计为独立 reviewer 证据。设 `0`/`false`/`off`/`no` 则要求 JSON 显式 `fork_context: false`
  - `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY` — 规定会话交互过程中要求稳定的 Session Key
  - `ROUTER_RS_CODEX_HOOK_STATE_SALT` — hook 状态存取盐（salt）
  - `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` — 关闭 review nudge
- **会话周期**：UserPromptSubmit 重新武装 (re-arm) 机制 — 每次 UPS 后重设拦截门控
- **自检命令**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint verify-codex-hooks
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework skills validate
  ```

#### 多代理编排

Codex CLI **积极鼓励多代理并行执行**。与 Cursor 通过 `subagentStart`/`subagentStop` hook 做硬门控不同，Codex 端的多代理行为由文档契约与 agent 自觉驱动。

**并行执行指引**：
- `/implementx` 且 `execution_mode=parallel` 时，主线程**应主动 spawn 子代理**并行执行各 lane，主线程仅担任 scheduler（coordinator visible content ≤35% of turn）
- 深度 review：非 my-light 时默认 spawn-first 配对审稿（`fork_context=false` 只读 reviewer）；my-light 下仍可按需 spawn
- ≥2 独立子问题时默认并行；通常 3–5 个 `fork_context=false` lane
- 窄范围（单文件、`small_task`）：可不 spawn，但不应以此为默认习惯

**子代理契约**：
```json
{
  "lane_id": "w3-lane-codex",
  "scope_paths": ["core/host-projection/src/hosts/codex_hooks/"],
  "output_path": "artifacts/current/<task_id>/lane-notes/w3-lane-codex.md",
  "max_lines": 15,
  "forbidden": ["paste full transcript to main chat"]
}
```

| 约束 | 说明 |
|------|------|
| 写入 disjoint | 各 lane 仅写 `scope_paths` 内文件，不修改共享 continuity artifact |
| `fork_context` | 深度 reviewer 必须显式 `fork_context: false`；env `ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`（默认开）可推断 |
| review 只读 | 默认 review-only，禁止默认改代码（除非用户显式要求） |

**与 Cursor / Claude 的关键差异**：

| 维度 | Cursor | Claude | Codex CLI |
|------|--------|-------------|-----------|
| 子代理生命周期 hook | `subagentStart` / `subagentStop` | 无（原生 `Task`） | **无**（agent 自觉） |
| 专用 gate 文件 | `execution-subagent-gate.mdc` + `review-subagent-gate.mdc` | 无 | **无**（本文档为真源） |
| 模型继承规则 | 禁默认 Sonnet/Claude | N/A | **继承主会话模型**，不显式指定 |
| 并行 lane 数 | 3–5 | 按需 | **3–5**（同 Cursor） |

### MiMo

- **Transport 架构**：Hook 层（`.mimo/settings.json` → `router-rs host mimo mimo-hook <event>`）处理 PreToolUse/PostToolUse/UPS 等 7 事件；MCP Bridge 层（`mcpServers` → `router-rs-framework`）提供 `framework_snapshot`、`skill_route`、`goal_state_manage` 等工具
- **能力边界**：7 事件注册表；`PreToolUse` 守卫依赖 `GOAL_STATE` 存在性；Rust handler 仅 `PreToolUse` + `Stop` 有显式 dispatch；其余事件统一 CLI 路径处理
- **Harness Capabilities**（`MimoHostProvider`）：
  - `hot_runtime_routing` — 热路由
  - `l2_continuity_contract` — L2 连续性契约
  - `closeout_evidence_hooks` — closeout 证据 hook
  - `review_gate_router_observation` — review gate 观测
- **Review Gate**：
  - **始终 advisory-only**（与 Claude canonical 实现一致）：仅 `followup_message` nudge，**不** `permission: deny` / 硬拦 Stop
  - my-light 下完全 suppress：Stop 上无 `REVIEW_GATE` / `AG_FOLLOWUP` 输出，仅保留 `CLOSEOUT_FOLLOWUP` + `SESSION_CLOSE_STYLE`
  - 无 Cursor 的 `REVIEW_GATE_STOP_MAX_NUDGES` 上限机制，无 Codex 的 `subagentStart`/`subagentStop` 硬门控
- **Goal/RFV**：通过 `framework_goal_drive` / `framework_rfv_loop` stdio + `artifacts/current/<task_id>/` 手动管理（**无** hook `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE` / continuity digest 注入）
- **MCP 服务器**：通过 `.mimo/settings.json` 的 `mcpServers` 字段注册。包括：
  - `router-rs-framework` — Framework snapshot / skill routing / goal / closeout gating
  - `browser-mcp` — Browser automation / session worker / background tasks
  - `mcp-codegraph` — Code knowledge graph (search, callers, callees)
  - `paperplain` — Academic paper metadata fetch/search
  - 由 `host-integration install` 自动写入 `.mimo/settings.json`，通过 `.framework-projection.json` 标记 managed keys
- **Session Supervisor**：**不支持**。长时目标依赖会话内连续性产物（`artifacts/current/` 目录）
- **自检命令**：
  ```bash
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework host-integration status
  cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor
  cargo test --manifest-path core/router-rs/Cargo.toml host_integration
  ```
