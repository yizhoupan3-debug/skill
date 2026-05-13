---
name: Harness 弱模型调研后续（execution）
overview: |
  本文件为执行计划（plan_profile: execution）。允许按下方 todos 修改文档与索引（必要时窄幅改 `router-rs` 外的契约叙述）；将弱模型/DEPTH 调研链与 backlog、harness 映射对齐现行实现；不在本计划内改 Cursor hooks 默认出站语义（真源见 harness_architecture §4.2）。末条以计划 vs 实际 + Git 状态证据收口，宿主支持时可使用 /gitx plan。
plan_profile: execution
todos:
  - id: depth-open-table-audit
    content: |
      动作：对 DEPTH 调研稿 Open 表 / §2.2 / §7 执行决议做一致性复核（必要时仅加脚注）；不要求再把 #1/#2 当作待修订项。
      范围：`docs/plans/RESEARCH_harness_depth_longrun_math.md`
      Done when：`rg` 确认 Option B、Open #1 已关闭、Open #2 已解决等措辞与 ADR / `rfv_loop.rs` 叙事一致；若磁盘已一致则正文注明 **no-op 复核** 即可。
      Verify：`rg -n "max_rounds|Option B|close_gates|已关闭|已解决" docs/plans/RESEARCH_harness_depth_longrun_math.md | head -n 40`
    status: completed
  - id: docs-index-and-mapping
    content: |
      动作：为弱模型/Token 调研链增加只读导航，不扩写第二套架构长文。
      范围：`docs/README.md`（RFV/连续性主题表）；`docs/harness_architecture.md`（**§8 文件映射表**一行）
      Done when：README 可点击链到 `plans/RESEARCH_harness_weak_model_top_tier.md`；harness_architecture §8 有「弱模型/上下文预算」→ `RESEARCH_harness_weak_model_top_tier.md` + `context_token_audit_deep_dive.md` 的指针。
      Verify：`rg -n "RESEARCH_harness_weak_model_top_tier|context_token_audit_deep_dive" docs/README.md docs/harness_architecture.md`
    status: completed
  - id: backlog-cursor-cap-pointer
    content: |
      动作：在路线图 backlog 用「合并链路 vs 出站截断」两阶段模型替换「Cursor 全无 cap」单一叙事；指向 harness §4.2、`ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`、`frag_04_review_gate_runtime` / context_token 审计 H8。
      范围：`docs/plans/harness_improvement_backlog.md`（改写 **P2-5**，≤8 行量级）
      Done when：P2-5 强调出站前缀截断与较晚并入段易丢失；读者可调 `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`（边界见 router_env_flags）；**不**再暗示库里不存在出站字节上限。
      Verify：`rg -n "merge_additional_context|ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS|apply_cursor_hook_output_policy|context_token_audit" docs/plans/harness_improvement_backlog.md docs/harness_architecture.md`（与 harness_improvement_backlog.md P2-5 Verify 同形）
    status: completed
  - id: codex-sessionstart-fixture-deferred
    content: |
      动作：**Superseded**：Open #6 的可测子集已由既有单测覆盖；本计划不新增重复 `#[test]`。
      范围：`scripts/router-rs/src/codex_hooks.rs`（只读核对）；`docs/plans/RESEARCH_harness_depth_longrun_math.md` Open #6 表述
      Done when：确认存在 `codex_compact_contexts_preserves_join_order_under_small_budget`；全 digest+nudge+goal 合成 fixture **defer**（另开 execution 再立项）。
      Verify：`rg -n "codex_compact_contexts_preserves_join_order_under_small_budget" scripts/router-rs/src/codex_hooks.rs`；可选 `cargo test --manifest-path scripts/router-rs/Cargo.toml codex_compact_contexts_preserves_join_order_under_small_budget`
    status: completed
  - id: execution-closeout-gitx
    content: |
      动作：对照本文件 frontmatter `todos` 与正文 §继承面逐项验收；记录 Git 状态证据。
      范围：仓库根；本文件 `docs/plans/EXECUTION_harness_weak_model_followups.plan.md`
      Done when：前序 todos 均已达 Done when；无未解释搁置项。
      Verify：`git status --short --branch` 与 `git diff --stat` 已记录；宿主支持时可执行 **`/gitx plan`**（与 `/gitx` 同契约，见 `skills/gitx/SKILL.md`）；无 Cursor 宿主或非交互环境时可跳过 `/gitx plan`，以 `git status`/`git diff` 为最低证据。
    status: completed
isProject: false
---

# Harness 弱模型调研后续 — 执行计划

## 执行计划继承面

| 字段 | 内容 |
|------|------|
| **继承指针** | [docs/plans/RESEARCH_harness_weak_model_top_tier.md](RESEARCH_harness_weak_model_top_tier.md) §1–§7；[docs/plans/RESEARCH_plan_execution_handoff_first_principles.md](RESEARCH_plan_execution_handoff_first_principles.md) |
| **Goal** | 将调研结论落到**可导航文档 + backlog/harness 指针**，减少 DEPTH Open 表与现行实现的叙事漂移。 |
| **Non-goals** | 不改 Cursor hooks 默认出站裁剪语义（已有 `ROUTER_RS_CURSOR_HOOK_OUTBOUND_CONTEXT_MAX_CHARS`，见 harness_architecture §4.2）；不改动 RFV/close_gates 状态机语义；不大改 `AGENTS.md`。 |
| **不变量** | `close_gates` 在显式 close 与 `max_rounds` 耗尽两路径均已校验（ADR Option B）；digest 深度 rollup 与 nudge 闸断不对称仍为已知产品设计点。 |
| **已否决方案** | 在本执行包内不引入「关 nudge 即关 digest 深度行」的 breaking 行为变更。 |
| **问题矩阵映射** | depth-open-table-audit→DEPTH Open 表/§7；docs-index→弱模型导航；backlog-cursor-cap-pointer→Cursor 两阶段上下文/backlog P2-5；codex-sessionstart-fixture-deferred→Open #6 残余 defer；execution-closeout-gitx→Git/计划对照收口。 |
| **外部准入表** | **无**（本 execution 仅仓库内 delta）。 |

---

## 正文说明

- **DEPTH Open 表（`depth-open-table-audit`）**：no-op 复核 — `RESEARCH_harness_depth_longrun_math.md` 已与 Option B、Open #1 已关闭、Open #2 已解决等现行叙事一致，本轮不对该文件做正文修订。
- **依赖**：前置调研已写入 `RESEARCH_harness_weak_model_top_tier.md`；本计划不重复其 Executive verdict。
- **风险**：Codex Open #6 全路径 fixture 仍 defer；与现有 `codex_compact_contexts_preserves_join_order_under_small_budget` 勿重复造轮。
- **Clippy**：若仓库惯例为 PR 前全量 `cargo clippy`，末条 `/gitx plan` 前至少对 `router-rs` 包执行一次 `-D warnings`。
- **验收宿主**：无 Cursor 或非交互环境时，末条 Verify 中的 **`/gitx plan` 可跳过**，以 `git status` / `git diff` 为最低证据（与 `harness_improvement_backlog` P2-4 表述一致）。
