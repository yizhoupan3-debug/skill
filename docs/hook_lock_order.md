# Hook lock order (router-rs)

Cross-process hook subprocesses serialize continuity writes with **POSIX `flock`**, not `std::sync::Mutex`.

## Layers (low → high)

| Level | Lock file | Scope |
|-------|-----------|--------|
| **L1** | `artifacts/current/.router-rs.task-ledger.lock` | All task-ledger writers: `GOAL_STATE`, `RFV_LOOP_STATE`, `STEP_LEDGER`, session artifact batches, tracker |
| **L2** | `artifacts/current/.router-rs.<filename>.lock` | Per-artifact RMW (e.g. `EVIDENCE_INDEX.json`) |
| **L3** | `.cursor/hook-state/review-subagent-<session>.lock` | Cursor review gate state only |

## Rules

1. **Allowed nesting**: L1 → L2 → L3 (repo flock first, then narrower locks).
2. **Forbidden**: Hold **L3** while acquiring **L1** (e.g. Stop checkpoint under session hook lock).
3. **PostTool evidence** (`append_evidence_index_merged_row`): **L2 only** — must not call `apply_task_ledger_mutation` (deadlock avoidance with concurrent ledger writers).
4. **PostTool handler order**: `record_tool_call` (L1) → session lock (L3) → release L3 → evidence (L2) → optional `cargo check` (no lock).

## Host differences

| Host | Session lock behavior |
|------|------------------------|
| **Cursor** | `try_lock_exclusive` + stale recovery; up to ~1.5s retry |
| **Codex** | Unix: **blocking** `flock(LOCK_EX)` on `{state_path}.lock` |

**Cursor L3 stale recovery**（`acquire_state_lock` 重试路径）：读取 lock 元数据 `pid`/`ts` 后，若 **`age_ms > HOOK_STATE_LOCK_STALE_MS`（30_000）** 或 **holder PID 已死**，则 `remove_file` lock 路径以便新 hook 获锁（见 [`handlers.rs`](../scripts/router-rs/src/cursor_hooks/handlers.rs) `acquire_state_lock`）。**SessionEnd**：删除 review gate 状态前须 `acquire_state_lock` → 删 state 文件 → `release_state_lock`（禁止未持锁删 state）。

Do not share one lock file between Cursor and Codex. See `task_write_lock.rs` and `codex_hooks.rs`.

## Env

| Variable | Default | Effect |
|----------|---------|--------|
| `ROUTER_RS_TASK_LEDGER_FLOCK` | on | Off → parallel ledger writes (torn JSON risk); `framework doctor` warns |
| `ROUTER_RS_HOOK_TIMING` | off | stderr `hook_timing` lines |
| `ROUTER_RS_CURSOR_CARGO_CHECK_SYNC` | off | On → blocking `cargo check` in postToolUse (up to 25s) |
| `ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC` | off | On → `sync_all` parent dir after hook-state write |

## Commit 2 behavior index（PR 审查对照）

| 行为 | 实现锚点 | 单测锚点 |
|------|----------|----------|
| Stop 先释放 L3 再跑 continuity | `release_lock_then_finalize_stop` | `stop_releases_l3_before_continuity_checkpoint` |
| Stop soft-nag 仍含 `need=` | `handle_stop` soft 分支 | `review_gate_soft_nag_includes_need_segment`（另见 `review_gate_stop_softens_after_max_nudges_env_cap`） |
| SessionEnd 持锁删 state | `handle_session_end` | `session_end_acquires_lock_before_state_delete` |
| `ROUTER_RS_CURSOR_CARGO_CHECK_SYNC=0` | `maybe_run_cursor_rust_lint` 早退 | `post_tool_skips_cargo_check_when_env_off` |
| L3 30s stale + dead pid | `acquire_state_lock` | `cursor_lock_recovers_from_stale_timestamp` / `cursor_lock_recovers_from_stale_timestamp_when_pid_is_current_process` |
| PostTool ∥ Stop 不死锁 | PostTool 锁序 | `stop_and_post_tool_concurrent_hooks_complete_under_one_second` |

## Related

- `scripts/router-rs/src/task_write_lock.rs` (L1 contract)
- `artifacts/current/hook-perf-deadlock/ARCHITECTURE.md`
- ADR: `docs/adr/hook-daemon.md` (warm process — not implemented by default)
