---
name: hook-gate-thorough-fix
overview: "本文件为执行计划（plan_profile: execution）。允许按 todos 修改 scripts/router-rs、Cursor/Codex hook 相关文档与测试；目标是一次性消除上轮深度 review 中的 P1 门控缺陷，并同步文档与回归测试。末条以计划 vs 实际 + Git 状态证据收口。"
plan_profile: execution
todos:
  - id: stop-continuity-mutex
    content: "实现 Stop 硬门控与 GSD/RFV 连续性互斥 @ scripts/router-rs/src/cursor_hooks/handlers.rs | Done: review/goal 未满足时 skip_continuity_merge=true | Verify: cargo test cursor_hooks stop_hard_gate stop_review_armed"
    status: completed
  - id: multiset-dedup
    content: "修复 push_review_pending_cycle_key 去重 @ handlers.rs | Done: 双事件同 key 仅一条 pending | Verify: review_gate_dual_event_lane_dedup_single_stop_clears"
    status: completed
  - id: subagent-cap-atomic
    content: "子代理 start 计数与 pending cap 原子化 @ handlers.rs | Done: cap 拒绝不增加 active_subagent_count | Verify: pending_cap_denial_does_not_increment_active_subagent_count"
    status: completed
  - id: main-thread-review-clear
    content: "主线程 compact findings 升 phase3 @ handlers.rs | Done: 无 subagent + [P0]-[P2] 可清门 | Verify: main_thread_compact_review_clears_gate_on_stop"
    status: completed
  - id: hydrate-strict-disk
    content: "Stop hydrate 遵守 PRE_GOAL_STRICT_DISK @ handlers.rs | Done: strict on 时磁盘 GOAL alone 不置 pre_goal | Verify: strict_disk_stop_pre_goal_not_satisfied_from_goal_file_alone"
    status: completed
  - id: fork-missing-infer
    content: "Cursor fork_context 缺省推断 @ review_gate_engine.rs router_env_flags.rs | Done: 缺字段 deep lane 可清门；fork_context:true 仍失败 | Verify: review_subagent_start_missing_fork_infers_false_for_deep_lane"
    status: completed
  - id: pending-orphan-safe
    content: "保守化 pending orphan 清扫 @ handlers.rs | Done: 无 timestamp 不误清 pending | Verify: v1_migrate_pending_preserved_when_no_started_at_timestamp"
    status: completed
  - id: docs-and-doctor
    content: "同步 harness/primer/host_adapter/cursor.md；doctor Codex 重复 hook WARN | Done: 文档与实现一致 | Verify: framework doctor --repo-root"
    status: completed
  - id: closeout-plan-git
    content: "计划 vs 实际 + Git 状态证据收口 | Done: 上列 todo 已实现；测试通过 | Verify: git status --short --branch; git diff --stat"
    status: completed
isProject: false
---

# Cursor Hook 门控彻底修复计划

## 执行计划继承面

| 字段 | 内容 |
|------|------|
| **继承指针** | 上轮对话深度 review（compact findings） |
| **Goal** | 消除 Stop/beforeSubmit 反复拉回与 GSD/REVIEW_GATE 结构性错位 |
| **Non-goals** | 不复刻 Codex decision:block；不改 GSD 预执行 continue:false；不删 closeout 硬拦 |
| **不变量** | 真源 handlers.rs + .cursor/hooks.json；机读短码 router-rs 前缀 |
| **已否决方案** | 全局缺字段当 false（无开关） |
| **问题矩阵映射** | 均已按 todos 落地 |
| **外部准入表** | 无 |

## 计划 vs 实际（收口）

| Todo | 状态 | 备注 |
|------|------|------|
| stop-continuity-mutex | 完成 | `stop_hard_gate_blocks_continuity_merge` + goal 分支同步 skip |
| multiset-dedup | 完成 | 任意 key 去重；dual lane 单测 |
| subagent-cap-atomic | 完成 | pending push 失败不增 open count |
| main-thread-review-clear | 完成 | `maybe_bump_review_phase_for_main_thread_compact_findings` |
| hydrate-strict-disk | 完成 | Stop/beforeSubmit 均遵守 strict disk |
| fork-missing-infer | 完成 | `cursor_review_independent_fork` + env 默认开 |
| pending-orphan-safe | 完成 | 无 timestamp 仅日志不清 pending |
| docs-and-doctor | 完成 | harness §5 / primer / host_adapter / cursor.md；doctor Codex dup WARN |
| closeout-plan-git | 完成 | 本文件 + 下方 Git 证据 |

**Defer（未做，按计划）**：GSD 预执行硬 `continue:false`；Cursor Task schema 自动带 fork_context；closeout 在 REVIEW_GATE_DISABLE 时自动关闭。

**验证（2026-05-20）**：

- `cargo test … cursor_hooks` — 171 passed
- `cargo test … review_gate` — 43 passed
- `cargo run … framework doctor --repo-root` — ok（Codex 用户级 hooks 重复 WARN 为运维提示）
- `framework maint verify-cursor-hooks` — 需无 `CARGO_TARGET_DIR` 指向已构建 binary 时测 fail-closed（maint 已 `env_remove`）

**Git 证据**：见会话末 `git status --short --branch` 与 `git diff --stat`（工作区含本计划外并行改动，提交前请按需拆分）。
