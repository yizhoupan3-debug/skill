# Cursor 宿主操作手册

**权威能力矩阵**：`configs/framework/RUNTIME_REGISTRY.json` → `host_projections.cursor`  
**接入契约**：`docs/host_adapter_contract.md`  
**官方文档**：[Hooks](https://cursor.com/docs/hooks) · [Rules](https://cursor.com/docs/rules)

## 安装 scope（ADR-003）

| 组件 | Scope | 路径 |
|------|-------|------|
| Framework 叙事（GSD 默认链） | **User only** | `~/.cursor/rules/framework.mdc` |
| Harness hooks | **Project** | `<repo>/.cursor/hooks.json` |
| Review / execution gate 规则 | **Project** | `<repo>/.cursor/rules/*.mdc`（非 framework） |
| Hook state | **Project** | `<repo>/.cursor/hook-state/` |

```bash
export SKILL_FRAMEWORK_ROOT=/path/to/skill   # 或在该仓库内工作
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework host-integration install --to cursor --scope user
```

**不要**在业务仓库跟踪 `framework.mdc`。成功标准 **不是** 与 Codex `AGENTS.md` 字节级一致。

## 默认工作流

全闭集宿主默认 **GSD**：`/gsd-new-project` → `/gsd-discuss-phase` → `/gsd-plan-phase` → `/gsd-execute-phase` → `/gsd-verify-work` → `/gsd-ship`。  
Pre-execution 三命令 **禁止改产品代码**（`skills/gsd/shared/phase-boundaries.md`）。

## Hook 能力（本仓）

- **Agent 面**：`beforeSubmitPrompt`、`stop`、`subagentStart`/`subagentStop`、`postToolUse` 等 → `router-rs cursor hook`
- **REVIEW_GATE 可清点 lane**（仅）：`general-purpose`、`best-of-n-runner`（registry `review_gate.deep_gate_lanes`）
- **Goal drive**：`/gsd-execute-phase`、`/gsd-verify-work`、`/gsd-ship` — **不是** `/gsd-new-project` 等 pre-exec 命令（`/autopilot` 已退役）
- **Fail-closed**：关键事件经 `cursor-router-rs-hook.sh`；`beforeSubmit` 对 plan 文件无可靠 Plan/Agent 模式信号

## Subagent 并发上限（勿与 stdio 默认混淆）

| 语义 | 默认 | 真源 |
|------|------|------|
| Cursor hook **open subagent** 计数上限 | **24** | `MAX_CONCURRENT_SUBAGENTS_LIMIT`；`ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS` 可调低或 `0` 关闭 |
| Goal/stdio **信封** 默认并发 | **8** | `DEFAULT_MAX_CONCURRENT_SUBAGENTS`（`runtime_envelope_ids.rs`） |

`subagentStart` 拒绝文案中的「`max_concurrent_subagents_limit` 契约」指 **24** 上限常量，不是信封里的 8。

## 状态有界 / 内存（hook 子进程）

当前为 **一 hook 一 `router-rs` 进程**（无 warm daemon）：跨调用的堆泄漏风险低；主要控制 **磁盘累积** 与 **单次 hook RSS 尖峰**。

| 机制 | 默认 | 说明 |
|------|------|------|
| `ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS` | `7`（`0` 关闭） | SessionEnd/Start 按 mtime/`updated_at` 清扫陈旧 `.cursor/hook-state/` 自有前缀文件；**不删**当前 `session_key` 与近期并行会话 |
| `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP=1` | 关 | 全目录前缀清扫（多会话互删风险，仅运维 opt-in） |
| SessionStart `init_tracker` | 每会话重置 | `artifacts/current/SESSION_CALL_TRACKER.json` |
| `ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX` | `128` | `per_tool` 键数上限；满时淘汰**调用计数最低**的工具名 |
| `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` | `32` | `review_subagent_pending_cycle_keys` 上限（满则拒绝新 key）+ stale prune |
| `ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS` | `7200`（`0` 关闭自动回收） | open subagent / pending stale 阈值；`0` 不 prune pending |
| SessionStart `SESSION_SUMMARY` | 前缀读 | 预算 ≈ sessionstart max + 512 字节，再出站截断 |
| Stop / signal assistant | tail `4096` 字符 | `hook_common::hook_assistant_tail_window` |
| Terminal 观察缓存 | 进程内 ≤8 目录 | `terminal_observation_cache`（`Arc` + mtime） |

## 常用 env

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_GSD_GOAL_CONTINUE_HOOK=0` | 关闭 Stop 续跑注入（兼容 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0`） |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED=1` | 开启 beforeSubmit pre-goal（绑定 `/gsd-execute-phase`） |
| `ROUTER_RS_CURSOR_HOOK_SILENT=1` | 剥 advisory `additional_context`（含 soft-nag detail）；保留 `router-rs ` 硬短码 |
| `ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS` | hook-state 陈旧清扫天数（见上表） |
| `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` | review pending multiset 上限 |
| `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE=0` | 关闭 Cursor 缺省 `fork_context`→`false` 推断 |
| `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK=1` | 禁止仅凭磁盘 GOAL 放行 pre-goal |
| `ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX` | SESSION_CALL_TRACKER `per_tool` 键上限 |

Stop 硬门控（`REVIEW_GATE` / `AG_FOLLOWUP` / closeout）与 `GSD_GOAL_CONTINUE` **互斥**；无硬门控时 goal 续跑仍注入 `additional_context`。

## 自检

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint verify-cursor-hooks
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework host-integration status
cargo test --manifest-path scripts/router-rs/Cargo.toml host_integration
```

## Unsupported（勿假看齐）

- Claude Code 专用 lane（`review`/`reviewer`/`critic`/`code-review`，见 registry `claude_reviewer_lanes`）**不能**作为本宿主 REVIEW_GATE 清门依据；须用 `general-purpose` 或 `best-of-n-runner`
- 无 tmux `session_supervisor`（Codex CLI 独有）
- Tab / App lifecycle hooks 未接入 harness
