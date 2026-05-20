# AGENTS 操作面与环境变量（详表）

真源：`AGENTS.md` 只保留不变量与指针；本文件承载 **可选 env / 注入 / closeout 分层** 完整说明。

## 连续性降噪

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1` | **开启** PostTool 向 `EVIDENCE_INDEX` 追加（unset 默认关） |
| `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=1` | **开启** Stop 自动检查点（unset 默认关） |
| `ROUTER_RS_DEPTH_COMPLIANCE_HINT=1` | **开启** digest `深度信号`（unset 默认关；`DEPTH_SCORE_MODE=strict` 亦开启） |
| `ROUTER_RS_GSD_GOAL_CONTINUE_HOOK=0` | 关闭 Cursor `GOAL_STATE` 续跑提示（兼容 `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0`） |
| `ROUTER_RS_RFV_LOOP_HOOK=0` | 关闭 `RFV_LOOP_STATE` 多轮 RFV 提示 |
| `ROUTER_RS_CURSOR_HOOK_SILENT=1` | 压制非必要 hook 文案（硬阻塞仍可见） |

## Goal 投影（两条路径，勿混读）

- **Codex SessionStart digest**：嵌入 `additional_context`；预算 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX`（256–8192，默认 640 字节）。
- **Cursor 短码**：`GSD_GOAL_CONTINUE`、`RFV_LOOP_CONTINUE` 等单行 `router-rs …` 注入；Codex `Stop` 不产出同名短码。
- **长文案**：`ROUTER_RS_GOAL_PROMPT_VERBOSE=1`；磁盘 `GOAL_STATE.json` 仍为权威。

## Cursor 专项

| 变量 | 作用 |
|------|------|
| `ROUTER_RS_CURSOR_HOOK_STATE_STALE_SWEEP_DAYS` | hook-state 按龄清扫（默认 7；`0` 关闭） |
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

任务 id 省略时读 `artifacts/current/active_task.json`，否则 `focus_task.json`。基线写入 `artifacts/current/<task_id>/SCHEMA_DRIFT_BASELINE.json`。详见 [`skills/gsd/verify-work/SKILL.md`](../../skills/gsd/verify-work/SKILL.md)。

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
