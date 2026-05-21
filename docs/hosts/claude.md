# Claude Code / Claude Desktop 宿主操作手册

**权威能力矩阵**：`configs/framework/RUNTIME_REGISTRY.json` → `host_projections.claude-code` / `claude-desktop`  
**接入契约**：[host_adapter_contract.md](../host_adapter_contract.md)

**默认 lifecycle（全宿主）**：`/discussx` → `/planx` → `/implementx` → `/verifyx`（`implementx` 一口气跑完 `WAVE_STATE`；`verifyx` 证据+ship）。`/gsd-*` 已于 2026-05 移除。

## Claude Code (`claude-code`)

**官方**：[Hooks reference](https://code.claude.com/docs/en/hooks)

| 组件 | 路径 |
|------|------|
| Hooks（**4 事件**，减法闭集） | `.claude/settings.json` → [`claude-router-rs-hook.sh`](../../configs/framework/claude-router-rs-hook.sh) |
| 项目 env | [`.claude/router-rs-hook.env`](../../.claude/router-rs-hook.env)（模板：[`configs/framework/claude-router-rs-hook.env`](../../configs/framework/claude-router-rs-hook.env)） |
| Framework 规则 | `.claude/rules/framework.md` |
| 项目叙事 | `.claude/CLAUDE.md`（可选；Code 项目规则） |
| Desktop 短指针 | 同路径 **`.claude/CLAUDE.md`**（`install --to claude-desktop` 写入 ≤40 行 MCP 工作流指针，**非** Code 四事件 hook 表） |

### `session_key`（与 Cursor 同类）

`.claude/hook-state/review_gate_*.json` / `.claude/hook-state/hook_state_*.json` 文件名由 **`session_key`** 分流，解析顺序：**显式会话 id**（`session_id` / `sessionId` 等）→ **`ROUTER_RS_CLAUDE_SESSION_NAMESPACE`**（非空时）→ **`cwd` / workspace 路径字段** → **repo 稳定 token**。同仓多会话在无 id 时可能共用状态；并行分流时设 namespace（语义对齐 `ROUTER_RS_CURSOR_SESSION_NAMESPACE`，见 [`harness_architecture.md`](../harness_architecture.md) 环境变量表）。

**Legacy 迁移（2026-05）**：`load_review_gate_disk` 在 hook-state 文件缺失时会 **只读** 旧路径 `.claude/review_gate_<hash>.json` 并 best-effort 写入 hook-state；**PreToolUse 仍 deny** 对 legacy 路径的直接写入。

### 默认注册的 hook 事件

| 事件 | 作用 |
|------|------|
| `PreToolUse` | 可 **deny**；守卫 framework/settings 路径 |
| `UserPromptSubmit` | Review 武装 / **spawn-first** 单行 nudge（`spawn_first_nudge_by_host.claude-code`）；窄范围不武装；my-light UPS 清 sticky `review_required` |
| `PostToolUse` | settings/framework 变更提示；reviewer 证据（无 Cursor 式每工具 tracker 风暴） |
| `Stop` | `REVIEW_GATE` / settings 校验 / touch-state 清门 |

**与 Cursor 的差异**：Claude **无** `subagentStart`/`subagentStop` multiset、无 `sessionStart`/`sessionEnd`、无 shell 生命周期 hook、无 `afterFileEdit` rustfmt。深度审稿用 registry **`review_gate.claude_reviewer_lanes`**（含 `review`/`reviewer`/`critic`/`code-review`；Cursor/Codex 仅 `deep_gate_lanes`）。

### 内存 / release（与 Cursor 对齐）

1. **构建 release**（优先命中 ~8MB 而非 debug ~37MB）：
   ```bash
   CARGO_TARGET_DIR="$PWD/scripts/router-rs/target" \
     cargo build --release --manifest-path scripts/router-rs/Cargo.toml
   ```
2. **Launcher** 探测：仓库 `scripts/router-rs/target/release` → `/tmp/skill-cargo-target/release` → debug → `PATH`（须支持 `claude hook --help`）。
3. **项目 env**：`settings.json` 包装命令在 exec 前 `source` `.claude/router-rs-hook.env`。默认 **`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`**（减 PostTool 写盘）。Cursor 专用变量（`ROUTER_RS_CURSOR_*`、`ROUTER_RS_CURSOR_KILL_STALE_TERMINALS`）**不**适用于 Claude。
4. 可选：`export ROUTER_RS_BIN="$PWD/scripts/router-rs/target/release/router-rs"`。

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework host-integration install --to claude --scope project
```

安装会合并 **上述 4 事件** 进 `.claude/settings.json`；**不会**覆盖已存在的 `.claude/router-rs-hook.env`。

## Claude Desktop (`claude-desktop`)

**传输**：MCP stdio — **无** CLI 级 PreToolUse / SubagentStop hook 表。操作步骤与 MCP 工具序见项目 **`.claude/CLAUDE.md`**（[`docs/hosts/claude-desktop.md`](claude-desktop.md)）；**勿**与 Code 的 `.claude/settings.json` 四事件 hook 混读。

| 能力 | 状态 |
|------|------|
| 热路由 + L2 continuity | ✓ |
| `closeout_evidence_hooks` | **unsupported**（registry exception） |
| `review_gate_router_observation` | **unsupported** |
| 硬门控 REVIEW_GATE | **勿声称**与 Claude Code 相同 |

Desktop 用户须在 MCP 侧 **手动** `record_evidence` / `session_checkpoint`；证据与 goal 仍写在 `artifacts/current/<task_id>/`。

## 默认工作流（两者）

与文首 **默认 lifecycle** 一致：`/discussx` → `/planx` → `/implementx` → `/verifyx`。hook 能力差异 **不改变** 该命令顺序。

## 自检（Code）

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml claude
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework host-integration status
```

## Unsupported（勿假看齐）

- Cursor 式 `subagentStart`/`subagentStop` open 计数与 multiset（Claude 用 Stop + 磁盘 `review_gate_*.json`）
- Cursor hook `GOAL_CONTINUE` / `RFV_LOOP_CONTINUE`（2026-05 已拔除；Claude 续跑用 MCP / 聊天 + `framework_goal_drive` / `framework_rfv_loop` stdio，见 Desktop 手册）
