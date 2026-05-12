---
name: Harness 弱模型调研后续（execution）
overview: |
  本文件为执行计划（plan_profile: execution）。允许按下方 todos 修改文档与 `scripts/router-rs` 测试；将 [docs/plans/RESEARCH_harness_weak_model_top_tier.md](RESEARCH_harness_weak_model_top_tier.md) 中已关闭/仍 open 项与索引、回归测试对齐；末条以计划 vs 实际 + Git 状态证据收口，宿主支持时可使用 /gitx plan。
plan_profile: execution
todos:
  - id: sync-depth-open-table
    content: |
      动作：修订 DEPTH 调研稿 Open 表与 §2.2 措辞，使与现行契约/实现一致。
      范围：`docs/plans/RESEARCH_harness_depth_longrun_math.md`
      Done when：Open #1 标为已关闭（ADR Option B + 契约 L30）；Open #2 标为已由 PostTool 子串扩展解决或仍注「长尾 python 路径」单点；读者不再误以为 max_rounds 与 close_gates 仍矛盾。
      Verify：`rg -n "max_rounds|Option B|close_gates" docs/plans/RESEARCH_harness_depth_longrun_math.md | head -n 35`
    status: pending
  - id: docs-index-and-mapping
    content: |
      动作：为弱模型/Token 调研链增加只读导航，不扩写第二套架构长文。
      范围：`docs/README.md`（RFV/连续性主题表一行）；`docs/harness_architecture.md`（§6 映射表一行）
      Done when：README 可点击链到 `plans/RESEARCH_harness_weak_model_top_tier.md`；harness_architecture §6 有「弱模型/上下文预算」→ 该合成稿 + `plans/context_token_audit_deep_dive.md` 的指针。
      Verify：`rg -n "RESEARCH_harness_weak_model_top_tier|context_token_audit_deep_dive" docs/README.md docs/harness_architecture.md`
    status: pending
  - id: backlog-cursor-cap-pointer
    content: |
      动作：在路线图 backlog 登记 Cursor `additional_context` 无总 cap 的工程跟进位，避免仅留在 TOKEN 文内。
      范围：`docs/plans/harness_improvement_backlog.md`
      Done when：P1 或 P2 下新增 ≤8 行 bullet，指向 `context_token_audit_deep_dive.md` 与 [`cursor_hooks/`](../../scripts/router-rs/src/cursor_hooks/mod.rs) `merge_additional_context`，标明「可选 env cap / 产品决策」。
      Verify：`rg -n "merge_additional_context|cursor_hooks|CONTEXT_MAX|cursor.*cap" docs/plans/harness_improvement_backlog.md`
    status: pending
  - id: codex-sessionstart-truncation-test
    content: |
      动作：为 Codex `codex_compact_contexts` + `truncate_codex_additional_context` 增加回归单测，固化「多段合并后再截断」时前缀保留顺序（对齐 research Open #6 的优先级可测子集）。
      范围：`scripts/router-rs/src/codex_hooks.rs`（`mod tests`）
      Done when：新 `#[test]` 在 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX=256` 下用 ≥3 段可区分字符串，断言截断结果 `len<=256`、以 `...` 结尾、且保留首段关键标记（或等价顺序断言）。
      Verify：`cargo test --manifest-path scripts/router-rs/Cargo.toml codex_compact_contexts_preserves_join_order_under_small_budget -- --exact --nocapture`（实现时函数名须与此一致或同步改本行）；`cargo clippy --manifest-path scripts/router-rs/Cargo.toml -- -D warnings`（若 CI 同标准）
    status: pending
  - id: execution-closeout-gitx
    content: |
      动作：对照本文件 frontmatter `todos` 与正文 §继承面逐项验收；记录 Git 状态证据。
      范围：仓库根；本文件 `docs/plans/EXECUTION_harness_weak_model_followups.plan.md`
      Done when：前序 todos 均已达 Done when；无未解释搁置项。
      Verify：`git status --short --branch` 与 `git diff --stat` 已记录；宿主支持时可执行 **`/gitx plan`**（与 `/gitx` 同契约，见 `skills/gitx/SKILL.md`）。
    status: pending
isProject: false
---

# Harness 弱模型调研后续 — 执行计划

## 执行计划继承面

| 字段 | 内容 |
|------|------|
| **继承指针** | [docs/plans/RESEARCH_harness_weak_model_top_tier.md](RESEARCH_harness_weak_model_top_tier.md) §1–§7；[docs/plans/RESEARCH_plan_execution_handoff_first_principles.md](RESEARCH_plan_execution_handoff_first_principles.md) |
| **Goal** | 将调研结论落到**可导航文档 + 最小回归测试 + backlog 指针**，减少 DEPTH Open 表与现行实现的叙事漂移。 |
| **Non-goals** | 本计划**不**实现 Cursor 侧可选总 cap（仅 backlog 登记）；不改动 RFV/close_gates 状态机语义；不大改 `AGENTS.md`。 |
| **不变量** | `close_gates` 在显式 close 与 `max_rounds` 耗尽两路径均已校验（ADR Option B）；digest 深度 rollup 与 nudge 闸断不对称仍为已知产品设计点。 |
| **已否决方案** | 在本执行包内不引入「关 nudge 即关 digest 深度行」的 breaking 行为变更。 |
| **问题矩阵映射** | sync-depth→research §2#1；docs-index→§3 弱模型导航；backlog-cursor→§7 P0 Cursor cap；codex-test→§2#4/Open #6；gitx→全文收口。 |
| **外部准入表** | **无**（本 execution 仅仓库内 delta）。 |

---

## 正文说明

- **依赖**：前置调研已写入 `RESEARCH_harness_weak_model_top_tier.md`；本计划不重复其 Executive verdict。
- **风险**：`codex_sessionstart_truncation_test` 若与现有 `additional_context_truncates_on_newline_preference_under_small_budget` 断言重叠，应**合并或改名**为互补场景，避免双测同一路径。
- **Clippy**：若仓库惯例为 PR 前全量 `cargo clippy`，末条 `/gitx plan` 前至少对 `router-rs` 包执行一次 `-D warnings`。
