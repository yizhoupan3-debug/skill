---
last_verified: "2026-06-09"
plate: B3
---

# B3 — runtime-core

## 职责

运行时编排：`framework_runtime`（stdio op registry/dispatch、`live_execute`）、任务物化、`session_supervisor` 原生进程管理（P8 去 tmux）、`framework_goal_drive` / task-state 解析。

主实现仍在 `core/router-rs/src/framework_runtime/`，逻辑上属 B3 板块。

## 启动 / 配置

| 能力 | 入口 |
|------|------|
| Goal 驱动 | stdio `framework_goal_drive`；MCP `goal_state_manage` |
| 任务状态解析 | `framework task-state-resolve` |
| Step ledger | `framework step-ledger` → `artifacts/current/<task_id>/STEP_LEDGER.jsonl` |
| Session supervisor | `router-rs session-supervisor …`（子命令见 `--help`） |

**任务目录真源**：`artifacts/current/<task_id>/`（`GOAL_STATE.json`、`RFV_LOOP_STATE.json`、`WAVE_STATE.json`）。

Env 与 flock：见 [`../harness_architecture/02-data-flows.md`](../harness_architecture/02-data-flows.md) §3.1；`ROUTER_RS_TASK_LEDGER_FLOCK` 关闭时 `framework doctor` 会 WARN。

## 排障

| 现象 | 处理 |
|------|------|
| Stop 后任务未完成 | `/implementx` + `framework_goal_drive` stdio（非 hook `GOAL_CONTINUE`） |
| `TASK_STATE_AGGREGATE_SYNC_FAILED` | 分文件权威；跑 `task-state-aggregate-sync` 修复投影 |
| subagent 泄漏 / spawn 错误 | `session_supervisor` mark_blocked；smoke：`subagent_spawn_error_shutdown`；真进程：`subagent_spawn_real_process_smoke`（`ROUTER_RS_SESSION_SUPERVISOR_REAL_PROCESS_SMOKE=1`） |
| workflow phase 串线 | `workflow_state_isolation` 契约；核对 agent `phase:` 标签 |
| live_execute 超时 | `live_execute.rs` 配置与 attach 产物路径 |

## 相关路径

- `core/router-rs/src/framework_runtime/`
- `core/router-rs/src/session_supervisor/`
- `artifacts/current/<task_id>/`
- `docs/task_state_unified_resolve.md`
- `docs/runtime_unified_spec.md`
