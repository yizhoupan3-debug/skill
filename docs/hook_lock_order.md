---
last_verified: "2026-06-02"
depends_on:
  - harness_architecture.md
---

# Hook lock order (router-rs)

Cross-process hook subprocesses serialize continuity writes with **POSIX `flock`**, not `std::sync::Mutex`.

## Layers (low → high)

| Level | Lock file | Scope |
|-------|-----------|--------|
| **L1** | `artifacts/current/.router-rs.task-ledger.lock` | All task-ledger writers: `GOAL_STATE`, `RFV_LOOP_STATE`, `STEP_LEDGER`, session artifact batches, tracker |
| **L2** | `artifacts/current/.router-rs.<filename>.lock` | Per-artifact RMW (e.g. `EVIDENCE_INDEX.json`) |
| **L3** | 宿主 session-scoped hook-state lock（如 Cursor: `.cursor/hook-state/review-subagent-<session>.lock`） | 宿主 review gate state |

## Rules

1. **Allowed nesting**: L1 → L2 → L3 (repo flock first, then narrower locks).
2. **Forbidden**: Hold **L3** while acquiring **L1** (e.g. Stop finalization that touches task-ledger under session hook lock).
3. **PostTool evidence** (`append_evidence_index_merged_row`): **L2 only while holding the evidence path lock** — must not call `apply_task_ledger_mutation` or acquire **L1** during the L2 RMW block (deadlock avoidance with concurrent ledger writers). After **L2 is released**, an independent **L1** `append_transaction` to `TASK_LEDGER.jsonl` is allowed (audit trail; not nested under L2).
4. **PostTool handler order**: `record_tool_call` (L1) → session lock (L3) → release L3 → evidence (L2 RMW, then optional L1 ledger append) → optional `cargo check` (no lock).

## Host differences

| Host | Session lock behavior |
|------|------------------------|
| **Cursor** | `try_lock_exclusive` + stale recovery; up to ~1.5s retry |
| **Codex** | Unix: **blocking** `flock(LOCK_EX)` on `{state_path}.lock` |

L3 行为因宿主而异，详见各宿主手册（[Cursor](hosts/cursor.md) · [Codex](hosts/codex-cli.md)）。以下为 Cursor 实现摘要：

**Cursor L3 stale recovery**（`acquire_state_lock` 重试路径）：**仅当 holder PID 已死**时 `remove_file` lock 路径；`age_ms > 30s` 且 PID 仍存活时**只重试、不删路径**（避免双 inode 双 flock）。孤儿 lock 文件（无持有者）靠 `try_lock_exclusive` 直接成功。**7d age sweep**（`sweep_stale_hook_state_by_age`）：`.lock` **仅**在 holder PID 已死（或 lock 缺失/不可读）时可删；存活 holder 即使 `age>30s` 也不 unlink；关联 json 仍按 mtime/`updated_at` 判 7d 陈旧。**SessionEnd 清扫顺序**：当前 `session_key` state（持锁删除）→ `sweep_hook_state_tmp_orphans` → `SESSION_CALL_TRACKER.tmp` → **`sweep_stale_hook_state_by_age`**（默认 7d）→ 可选 `LEGACY_FULL_SWEEP` 全目录。锁不可用则 stderr `session_end_state_delete_skipped=lock_unavailable` 且保留 state 文件。

Do not share one lock file between Cursor and Codex. See `task_write_lock.rs` and `hosts/codex_hooks/mod.rs`.

## Env

| Variable | Default | Effect | 宿主 |
|----------|---------|--------|------|
| `ROUTER_RS_TASK_LEDGER_FLOCK` | on | Off → parallel ledger writes (torn JSON risk); `framework doctor` warns | 全宿主 |
| `ROUTER_RS_HOOK_TIMING` | off | stderr `hook_timing` lines | 全宿主 |
| `ROUTER_RS_CURSOR_CARGO_CHECK_SYNC` | off | On → blocking `cargo check` in postToolUse (up to 25s) | Cursor |
| `ROUTER_RS_CURSOR_HOOK_STATE_DIR_SYNC` | off | On → `sync_all` parent dir after hook-state write | Cursor |

## Commit 2 behavior index（PR 审查对照）

| 行为 | 实现锚点 | 单测锚点 |
|------|----------|----------|
| Stop 先释放 L3 再 finalize（**无**自动 checkpoint，2026-05） | `release_lock_then_finalize_stop` | `stop_releases_l3_before_continuity_checkpoint` |
| Stop soft-nag 仍含 `need=` | `handle_stop` soft 分支 | `review_gate_soft_nag_includes_need_segment`（另见 `review_gate_stop_softens_after_max_nudges_env_cap`） |
| SessionEnd 持锁删 state | `handle_session_end` | `session_end_acquires_lock_before_state_delete` |
| `ROUTER_RS_CURSOR_CARGO_CHECK_SYNC=0` | `maybe_run_cursor_rust_lint` 早退 | `post_tool_skips_cargo_check_when_env_off` |
| L3 dead pid removes lock | `acquire_state_lock` | `cursor_lock_recovers_from_stale_timestamp` |
| L3 orphan lock (alive pid metadata) | `acquire_state_lock` | `cursor_lock_recovers_orphan_lock_file_without_remove_when_holder_alive` |
| Age sweep keeps alive-holder lock | `hook_state_lock_removable_for_sweep` | `stale_sweep_preserves_alive_holder_lock_when_json_fresh` |
| SessionEnd skip delete if no lock | `handle_session_end` | `session_end_skips_state_delete_when_lock_unavailable` |
| PostTool ∥ Stop 不死锁 | PostTool 锁序 | `stop_and_post_tool_concurrent_hooks_complete_under_one_second` |

## Related

- `core/router-rs/src/task_write_lock.rs` (L1 contract)
- `artifacts/current/hook-perf-deadlock/ARCHITECTURE.md`
- ADR: `docs/adr/hook-daemon.md` (warm process — not implemented by default)
