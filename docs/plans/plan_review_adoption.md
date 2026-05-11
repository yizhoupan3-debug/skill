# Plan 与深度 Review：采纳摘要（可打印）

**权威真源**：[`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md)（Todo 四元组、CreatePlan 契约、审 plan 流程）；[`.cursor/rules/cursor-plan-output.mdc`](../../.cursor/rules/cursor-plan-output.mdc)；[`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)；[`docs/references/rfv-loop/reasoning-depth-contract.md`](../../docs/references/rfv-loop/reasoning-depth-contract.md) §A–B。

与本页互补的**短勾选清单**：[`plan_todo_checklist.md`](plan_todo_checklist.md)。

## 写计划前：五行证据（Goal / Context / Constraints / Done / Verify 的压缩版）

复制到 `.plan.md` 正文最前（在分节标题之前）：

```text
Goal: <本计划要改变的行为或交付物>
Non-goals: <明确不做的 1 行>
已读证据: <路径或命令输出的一行摘要，证明已定位真源>
最大风险: <1 条>
首选验证: <一条完整命令，含 --manifest-path 等>
```

## 外部研究结论（待核验启发式）

以下为未在本次仓库内补齐 URL/日期/来源的启发式，不作为已证实事实写入硬契约；需要外部化时须回到 `code-review-deep` 的 external calibration 形态补 URL、抓取日期、contradiction sweep 与 unknowns。

- **计划绑定验证**：先有可执行验证的计划再实现；计划失败时用验证反馈修正计划。
- **AI 审查上限**：自动化审查与人类 PR 评论对齐度可能有限；应用 **lane 结构 + verdict-first + test/repro gap**，并保留人类与工具链终裁。
- **对抗 / 多角色**：Promote 前可反驳过滤、分阶段门控、独立 critic，可能提高「真问题」密度；若作为外部 claim 使用，必须补来源。

## 深度 Review 最小输出（对齐 code-review-deep）

1. 一行 **verdict**（`ship with caveats` / `revise before merge` / `blocked`）。
2. **P0–P2**，每条带 **路径 + 符号或锚点**。
3. **test_repro_gap**：最小缺失测试或复现步骤。
4. 涉及第三方时：**Claims + contradiction sweep**（可简写为要点 + 链接）。

**lane 结构**（只读、artifact-disjoint）：至少拆 **correctness** 与 **security**（或 `review-dimensions.md` 中其它正交组合）。只有用户显式授权 subagent/并行，或宿主 review gate 允许/要求时，才把这些 lane 分配给并行 subagent；否则主线程本地按 lane 结构审。

## 本仓库执行顺序（与 plan-mode 六步一致）

调研与定位 → 结构化计划（四元组 todos）→ 可选只读审 plan → 合并必要修订 → 实现与验证 → 计划 vs 实际 + Git 状态证据收口；宿主支持时可用 **`/gitx plan`**（与 **`/gitx`** 同契约，见 [`skills/gitx/SKILL.md`](../../skills/gitx/SKILL.md)）。

## 本次落地工件索引（由 `plan_review_adoption_execution.plan.md` 驱动）

| 工件 | 路径 |
|------|------|
| 合规计划样例（含 YAML todos） | [`.cursor/plans/plan_review_adoption_execution.plan.md`](../../.cursor/plans/plan_review_adoption_execution.plan.md) |
| 审 plan findings（本地主线程模拟独立视角） | [`plan_review_findings_round1.md`](plan_review_findings_round1.md) |
| 深度 review 试跑 | [`REVIEW_plan_review_adoption.md`](REVIEW_plan_review_adoption.md) |
| 计划 vs 实际 + Git 诊断记录 | [`plan_review_closeout.md`](plan_review_closeout.md)（含等价 `/gitx plan` 命令摘要） |
