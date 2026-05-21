# AGENTS 操作面与环境变量（详表）

真源：`AGENTS.md` 只保留不变量与指针；本文件承载 **可选 env / 注入 / closeout 分层** 完整说明。

## 连续性退出（2026-05：仅 stdio + 手动画板）

**续跑与 digest 不再经 hook 注入。** Stop / SessionStart **不**产出 `GOAL_CONTINUE`、`RFV_LOOP_CONTINUE` 或 `framework_runtime::continuity_digest` 段落；操作员用 **`framework_goal_drive` / `framework_rfv_loop` stdio** 与 **`artifacts/current/<task_id>/`** 手动画板（`GOAL_STATE.json`、`RFV_LOOP_STATE.json`、`EVIDENCE_INDEX` 等）。

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1` | **opt-in** PostTool → `EVIDENCE_INDEX`（unset 默认关） |
| `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=1` | **已无操作**（保留 env 名兼容） |
| `ROUTER_RS_DEPTH_COMPLIANCE_HINT=1` | 遗留测试/stdio；**不**驱动 SessionStart digest |
| `ROUTER_RS_GOAL_CONTINUE_HOOK` | **已无操作**（历史名；hook 路径已删） |
| `ROUTER_RS_RFV_LOOP_HOOK` | **已无操作**（历史名；hook 路径已删） |
| `ROUTER_RS_OPERATOR_INJECT=0` | 关闭 SessionStart advisory 等（**不含**已移除的 goal/RFV 续跑行） |
| `ROUTER_RS_CURSOR_HOOK_SILENT=1` | 压制非必要 hook 文案（硬阻塞仍可见） |

## Goal / RFV（stdio + 手动画板）

- **权威磁盘**：`artifacts/current/<task_id>/GOAL_STATE.json`、`RFV_LOOP_STATE.json`。
- **显式控制面**：`framework_goal_drive`、`framework_rfv_loop`（stdio-json）；My 执行区用 `/implementx`、`/verifyx` 驱动，**非**宿主 Stop 自动续跑。
- SessionStart：**仅** `Repo:` / 可选 `SESSION_SUMMARY` 前缀（Cursor `summary` 模式）；预算见 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` / `ROUTER_RS_CURSOR_SESSIONSTART_CONTEXT_MAX_CHARS`。

## Codex CLI 专项

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1` | 关闭 Codex `CODEX_REVIEW_GATE` 硬拦；UPS/PostTool 亦清 hook-state |
| `ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY=0` | 允许无 `session_id` 的 legacy payload（默认 **on** = 缺则 lifecycle block） |
| `ROUTER_RS_CODEX_HOOK_STATE_SALT` | unstable fallback 文件名盐（与 repo+cwd+payload session 组合；生产建议保持 strict session on） |
| `ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE=0` | 深度 lane 缺 `fork_context` 时不推断 independent 证据 |
| `ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS=1` | `stop_hook_active` 重放时跳过 review gate |
| `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE=0` | 关闭 spawn-first 单行（Cursor beforeSubmit、Codex UPS、Claude UserPromptSubmit）；文案来自 `spawn_first_nudge_by_host.<host>` 或全局回退 |
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` | UPS/SessionStart `additionalContext` UTF-8 字节上限 |

## Cursor 专项

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1` | 应急关闭 Cursor REVIEW_GATE 全链 |
| `ROUTER_RS_REVIEW_SPAWN_FIRST_NUDGE=0` | 关闭 spawn-first 单行 nudge（Cursor beforeSubmit + Codex UserPromptSubmit；**零注入**；清门阈值不变） |
| `ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES` | Stop REVIEW_GATE 硬行次数上限（默认 8；超 cap 降为 soft_nag） |
| `ROUTER_RS_CURSOR_REVIEW_PENDING_CYCLE_MAX` | review `pending_cycle_keys` 上限（默认 32） |
| `ROUTER_RS_SESSION_CALL_TRACKER_TOOL_KEYS_MAX` | `SESSION_CALL_TRACKER` `per_tool` 键上限（默认 128） |
| `ROUTER_RS_CURSOR_KILL_STALE_TERMINALS=0` | 关闭 SessionEnd 终端回收 |
| `ROUTER_RS_CURSOR_TERMINAL_KILL_MODE=legacy` | 旧「全仓库 active terminal」清扫 |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED=1` | 开启 beforeSubmit pre-goal |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES` | 自动放行次数（默认 8；`0` 关闭） |
| `ROUTER_RS_CURSOR_PRE_GOAL_STRICT_DISK` | **默认 strict**（unset 禁止仅凭磁盘 GOAL 满足 pre-goal）；legacy：`0`/`false`/`off`/`no` |
| `ROUTER_RS_CURSOR_HOOK_LEGACY_SUBTRACTED_EVENTS=1` | 5 个已从默认 `hooks.json` 移除的事件在未注册时仍 dispatch 完整 handler |
| `ROUTER_RS_CURSOR_HOOK_STATE_FAIL_OPEN=1` | hook-state 写失败时 beforeSubmit 仍 `continue:true`（应急） |
| `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK=1` | 论文强对抗审稿注入（文案 `PAPER_ADVERSARIAL_HOOK.txt`） |
| `ROUTER_RS_OPERATOR_INJECT=0` | 关闭 operator 注入总闸（含 paper adversarial） |

## Schema drift（verify / CI）

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- schema-drift contract
cargo run --manifest-path scripts/router-rs/Cargo.toml -- schema-drift baseline --repo-root "$PWD"
cargo run --manifest-path scripts/router-rs/Cargo.toml -- schema-drift check --repo-root "$PWD"
```

任务 id 省略时读 `artifacts/current/active_task.json`，否则 `focus_task.json`。基线写入 `artifacts/current/<task_id>/SCHEMA_DRIFT_BASELINE.json`。详见 [`skills/verifyx/SKILL.md`](../../skills/verifyx/SKILL.md) 与 [`configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md`](../../configs/framework/SCHEMA_DRIFT_HEADINGS_CONTRACT.md)。

## Closeout 分层

- **软**（本地、未设 `ROUTER_RS_CLOSEOUT_ENFORCEMENT`、非 CI）：完成态可不附带 `closeout_record`。
- **硬**（CI 或变量已设且非 `0`/`false`/`off`/`no`，**含空字符串**）：须提供合法 record。
- **显式关闭硬门禁**：`ROUTER_RS_CLOSEOUT_ENFORCEMENT=0`。

## 深度硬门禁（opt-in）

`GOAL_STATE.completion_gates`、`RFV_LOOP_STATE.close_gates` 默认关闭。见 `docs/references/rfv-loop/reasoning-depth-contract.md`、`docs/harness_architecture.md` §4/§8（`ROUTER_RS_DEPTH_SCORE_MODE`）。

## 总闸与 nudge

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_OPERATOR_INJECT=0` | SessionStart advisory 等（见 harness §2.1） |
| `ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` | 关闭 `HARNESS_OPERATOR_NUDGES.json` 文案 |

完整矩阵：`docs/harness_architecture.md` §5、`docs/operator_profiles.md`。
