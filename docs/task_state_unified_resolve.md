# Task state — unified read model (`ResolvedTaskView`)

**状态**：已落地 **只读** 聚合（`task_state`）、**阶段 1**（Cursor continuity frame）、**阶段 2**（`task_write_lock` 串行化 GOAL / RFV / session 批量写 / `EVIDENCE_INDEX` 追加）、**阶段 2.5**（`task_command`：命名 envelope 分发 + `framework task-ledger-dispatch` + stdio `task_ledger_dispatch`）与 **阶段 3**（可选投影 `TASK_STATE.json` + `framework task-state-aggregate-sync`，`ResolvedTaskView` 对外 schema 仍以分文件为准）。

**与 `docs/harness_architecture.md` 的关系**：该文定义 L1–L5 分层；本文定义 **L2→L3 之间的读模型真源**：如何把多份磁盘账本解析成**单一结构**，供 hook / refresh / 调试共用，避免「每个调用点自己拼路径、自己猜优先级」。

**文档索引**：[`docs/README.md`](README.md)。

---

## 1. 问题陈述

`SESSION_SUMMARY` / `NEXT_ACTIONS` / `EVIDENCE_INDEX` / `GOAL_STATE` / `RFV_LOOP_STATE` / closeout（随 session 写）/ `.cursor/hook-state` 在语义上都与「当前任务」相关，但历史上各自有写入点与读取点。维护成本来自 **隐式优先级**（例如 hydration 扫盘 mtime）与 **跨文件推导**（例如 gate 同时看 GOAL 与 EVIDENCE）。

**目标**：先引入 **唯一读 facade** `ResolvedTaskView`，不改变落盘布局；所有新逻辑优先消费该视图；旧逻辑逐步迁移。

---

## 2. 设计原则

1. **单处解析**：`task_state::resolve_task_view(repo_root, task_id_override)` 为默认入口；禁止在新代码中散落 `join!(artifacts/current, tid, GOAL_STATE.json)`。
2. **显式优于隐式（v1）**：`task_id` 解析顺序为 **`task_id_override` > `active_task.json` > `focus_task.json`**。`read_goal_state_for_hydration` 中非空 `active_task.json` 是硬当前任务指针；active 指向缺失/损坏时返回空，不回退 focus。只有 active 缺失/空时才读 `focus_task.json`；按 mtime 扫 `**/GOAL_STATE.json` 仅可作为诊断/兼容路径，不得触发当前任务 Stop/drive 门控。另：`resolve_task_view` 在 **active 无可读 GOAL、focus 另有可读 GOAL** 时在 `resolution_notes` 写入观测短码 `continuity:active_goal_missing_focus_has_goal`（不改变 hydration）。
3. **控制面互斥**：`TaskControlMode` 在视图中显式分类：`idle` / `autopilot` / `rfv_loop` / `conflict`（GOAL 续跑与 RFV `loop_status=active` 同时成立时标记冲突，并附 `resolution_notes`）。当 `GOAL_STATE.json` 或 `RFV_LOOP_STATE.json` **不可解析**（读盘/JSON 失败）时，`resolve_task_view` 在 `resolution_notes` 填入 `*_read_failed` 短句，`goal_state` / `rfv_loop_state` 字段为 `null`，区别于「文件缺失」。
4. **性能**：本地小 JSON、低频 hook；聚合为内存操作。若未来需要缓存，仅在单 hook 进程内按 `mtime` 短路。
5. **适配器保持薄**：Cursor/Codex 仅 stdin/out；不在本文件写宿主策略长文。

---

## 3. `ResolvedTaskView` 字段（概念）

| 字段 | 含义 |
|------|------|
| `schema_version` | 视图 schema 版本字符串（与磁盘各文件 schema 独立） |
| `task_id` | 本次解析使用的任务 id（若指针全无则为 `null`） |
| `pointers` | `active_task_id` / `focus_task_id` 快照 |
| `goal_state` / `rfv_loop_state` | 原始 JSON 片段（若文件不存在则为 `null`） |
| `evidence` | 对 `EVIDENCE_INDEX.json` 的摘要（是否非空、是否有成功验证行） |
| `control_mode` | 见 §2.3 |
| `resolution_notes` | 非致命提示（如冲突、指针缺失） |

---

## 4. CLI / 调试

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework task-state-resolve --repo-root "$PWD"
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework task-state-resolve --repo-root "$PWD" --task-id "<uuid>"
# 阶段 3：刷新单文件投影（缺省从 active_task.json 取 task_id）
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework task-state-aggregate-sync --repo-root "$PWD"
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework task-state-aggregate-sync --repo-root "$PWD" --task-id "<uuid>"
# 阶段 2.5：`kind` 为 autopilot_goal | rfv_loop | session_artifacts | hook_evidence_append；`payload` 与对应直连 API 相同。
cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework task-ledger-dispatch --input-json '{"schema_version":"router-rs-task-ledger-command-envelope-v1","kind":"autopilot_goal","payload":{"repo_root":"'"$PWD"'","operation":"status"}}'
```

输出 JSON，便于与 `framework snapshot` / SessionStart digest / hook 日志对照。

---

## 5. 迁移阶段（路线图）

| 阶段 | 内容 |
|------|------|
| **0** | `task_state` 模块 + `framework task-state-resolve` + 单测 |
| **1** | `cursor_hooks`：`resolve_cursor_continuity_frame` → hydrate + merge；`build_*_from_state` + frame 缓存 |
| **2** | `task_write_lock`：`artifacts/current/.router-rs.task-ledger.lock` 上 `flock` + `apply_task_ledger_mutation(repo_root, …)`；与 session / evidence **同序**共用 repo 级锁（`EVIDENCE_INDEX` 尚可再持 per-path lock）。跨进程边界是 **flock**，不是单进程 `Mutex` |
| **2.5（当前）** | `task_command`：`TaskLedgerCommand` + `dispatch_task_ledger_command_envelope`；CLI `framework task-ledger-dispatch`；stdio `task_ledger_dispatch` |
| **3（可选）** | 单文件 `TASK_STATE.json`（`task_state_aggregate`）+ `framework task-state-aggregate-sync`；GOAL/RFV/Evidence 变更后 best-effort 刷新；`ResolvedTaskView` / `task-state-resolve` 仍只读分文件 |

---

## 6. 代码真源

- 读模型 / frame：`scripts/router-rs/src/task_state.rs`
- 阶段 3 投影刷新：`scripts/router-rs/src/task_state_aggregate.rs`
- 命名写分发（2.5）：`scripts/router-rs/src/task_command.rs`
- 写串行：`scripts/router-rs/src/task_write_lock.rs`
- GOAL 续跑判定复用：`autopilot_goal::goal_state_requests_continuation`
- RFV 活跃判定：`rfv_loop_state.loop_status == active`（大小写不敏感）

维护：若修改 `task_id` 解析或 `control_mode` 分类规则，**同时**更新本文 §2–§3 与 `task_state.rs` 中单测。
