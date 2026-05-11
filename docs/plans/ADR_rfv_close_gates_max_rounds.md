# ADR：RFV `close_gates` 与 `max_rounds` 自动收口路径

**状态**：已采纳（文档与测试对齐实现）  
**日期**：2026-05-11  

## 背景

`RFV_LOOP_STATE.close_gates` 在 `append_round` 写入前对「含本 entry 的预览态」调用 `enforce_rfv_close_gates`（与 `depth_compliance_aggregate` 同源）。历史契约曾写为「仅 supervisor 显式 `close`/`closed` 触发」，而 `rfv_loop.rs` 在 **`closes_due_to_round_cap`**（`round_n >= max_rounds` 且非 block）分支同样调用 gate。

## 决议：**Option B — 以当前 Rust 行为为准，对齐文档**

- **不删除** `closes_due_to_round_cap` 上的 gate 调用：最后一轮在预算耗尽时仍受同一套 opt-in 硬门禁约束，避免「靠刷满轮次绕过 `require_last_round_verify_pass` / `min_depth_score`」的语义洞。
- **不选 Option A**（从实现删 max_rounds 路径的 gate）：未发现必须放宽的安全收益；反而削弱 supervisor 在预算边界上的可审计收口。

## 后果

- 文档真源：`docs/references/rfv-loop/reasoning-depth-contract.md`、`docs/harness_architecture.md`、`docs/rfv_loop_harness.md` 与 `task_state` rustdoc 已写明 **显式 close + max_rounds 自动 closed** 两条路径。
- 单测：`rfv_loop.rs` 覆盖显式 close 与 round-cap 两类 gate 行为。

## 参考

- 实现：`scripts/router-rs/src/rfv_loop.rs`（`closes_due_to_round_cap` + `enforce_rfv_close_gates`）。
- 调研记录：`docs/plans/RESEARCH_harness_depth_longrun_math.md` §2 / Open #1。
