---
name: Plan与Review采纳执行样例
plan_profile: execution
overview: 本文件为执行计划（plan_profile: execution）。允许按下方 todos 修改文档类资产（docs/plans、.cursor/plans）以落地调研计划的四项 todo（四元组内化、审 plan 一轮、深度 review 样例、Git 证据收口记录）；不修改用户附件中的调研 plan 文件；末条以计划 vs 实际 + Git 状态证据收口，宿主支持时可使用 /gitx plan。
todos:
  - id: t1-internalize
    content: "撰写 Plan/Review 采纳指南并链到 checklist @ docs/plans/plan_review_adoption.md, docs/plans/plan_todo_checklist.md | Done: plan_review_adoption.md 含五行证据模板、待核验启发式、深度 review 最小输出与工件索引 | Verify: test -f docs/plans/plan_review_adoption.md && rg -n '四元组|Git 状态证据|待核验启发式' docs/plans/plan_review_adoption.md"
    status: completed
  - id: t2-reviewer-round
    content: "本地主线程模拟独立 reviewer 视角只读审执行计划并落盘 findings；主线程一轮修订本文件 YAML/正文 @ docs/plans/plan_review_findings_round1.md, .cursor/plans/plan_review_adoption_execution.plan.md | Done: findings≥3 条且指向具体 todo/节；本计划采纳 Finding 1/2/4 于各条 todo 文案，Finding 3/5 标 defer；修订可用 git diff 复核 | Verify: test -f docs/plans/plan_review_findings_round1.md && rg -n '本地主线程模拟独立视角|Finding' docs/plans/plan_review_findings_round1.md && git diff .cursor/plans/plan_review_adoption_execution.plan.md | head -n 40"
    status: completed
  - id: t3-deep-review
    content: "对 router-rs 切片做 code-review-deep 形交付 @ scripts/router-rs/src/closeout_enforcement.rs, scripts/router-rs/src/rfv_loop.rs, docs/plans/REVIEW_plan_review_adoption.md | Done: REVIEW 含 verdict、P1/P2/open question（≥1 条含 evaluate_closeout_record 或 ALLOWED_VERIFY_RESULTS 等符号锚点）、test_repro_gap、Evidence checked、Tests searched、Residual risk；Lane_correctness 与 Lane_security 两节 | Verify: test -f docs/plans/REVIEW_plan_review_adoption.md && rg -n 'verdict|Lane_correctness|Lane_security|test_repro_gap|Evidence checked|Tests searched|Residual risk|evaluate_closeout_record|ALLOWED_VERIFY_RESULTS' docs/plans/REVIEW_plan_review_adoption.md"
    status: completed
  - id: t4-gitx-closeout
    content: "对照本计划与实际改动并记录 Git 状态证据 @ .cursor/plans/plan_review_adoption_execution.plan.md, docs/plans/plan_review_closeout.md | Done: plan_review_closeout.md 含逐项 done/defer、git status --short --branch、git worktree list --porcelain、git diff --stat（或声明仅文档无代码 diff）；宿主支持时记录 /gitx plan 结果 | Verify: rg -n 'git diff --stat|git status|worktree|对照|/gitx plan' docs/plans/plan_review_closeout.md"
    status: completed
isProject: false
---

# 证据（写计划前）

已读证据：`skills/plan-mode/SKILL.md` Todo 四元组与 CreatePlan 契约；`.cursor/rules/cursor-plan-output.mdc`；`docs/plans/plan_todo_checklist.md`；`skills/gitx/SKILL.md` §深度 review checklist 与 `/gitx plan` 等价说明。

Goal: 产出可对照验收的文档与样例计划，落实「Plan 更好、Review 更能发现问题」调研结论。  
Non-goals: 不修改用户附件中的 `plan与review提质_611e6884.plan.md`；不改 router-rs 行为代码。  
最大风险: 工件散落在 `docs/plans/` 与 `.cursor/plans/` 两处导致检索遗漏。  
首选验证: `rg -n 'plan_review' docs/plans .cursor/plans`

## 执行计划继承面

继承指针：无（无前置调研；用户附件中的调研 plan 不纳入本仓库修改范围）
Goal：产出可对照验收的 Plan/Review 规范样例。
Non-goals：不修改 router-rs 行为代码；不修改用户附件计划。
外部准入表：无（外部研究结论仅以待核验启发式保留）。

## 正文 checklist（与 YAML `todos` id 对齐）

- [x] **t1-internalize**：`docs/plans/plan_review_adoption.md` 已存在且含模板与链接。
- [x] **t2-reviewer-round**：`plan_review_findings_round1.md` 已存在；本文件 frontmatter 已按 findings 修订一轮（Finding 1/2/4 采纳，3/5 defer）。
- [x] **t3-deep-review**：`REVIEW_plan_review_adoption.md` 已存在且结构完整。
- [x] **t4-gitx-closeout**：`plan_review_closeout.md` 含 Git 诊断与计划对照。
