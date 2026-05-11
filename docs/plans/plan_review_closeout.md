# 计划 vs 实际 + Git 状态证据收口记录

本记录按新的 plan-mode 口径：先诊断（status / worktree），再看提交面（diff），再跑与改动面相称的验证，最后对照计划逐项收口。宿主支持时可追加 `/gitx plan`；本文件本身以 Git 状态证据作为跨宿主可执行收口。

## 用户调研计划四项 todo 对照

| Todo | 计划 Done | 实际 | 备注 |
|------|-------------|------|------|
| internalize-plan-quadruple | 四元组内化 + 下一份 plan 合规 | **done** | 新增 [`plan_review_adoption.md`](plan_review_adoption.md) 与合规样例 [`.cursor/plans/plan_review_adoption_execution.plan.md`](../../.cursor/plans/plan_review_adoption_execution.plan.md) |
| plan-reviewer-round | findings + 一轮修订 | **done** | [`plan_review_findings_round1.md`](plan_review_findings_round1.md)；执行计划 YAML 已采纳 Finding 1/2/4 |
| deep-review-playbook | verdict + P1/P2/open question + 双 lane + test_repro_gap + 证据门槛 | **done** | [`REVIEW_plan_review_adoption.md`](REVIEW_plan_review_adoption.md) |
| gitx-plan-closeout | 计划 vs 实际 + Git 状态证据；宿主支持时可追加 `/gitx plan` | **done** | 本节 + 下列命令输出 |

## 执行样例计划（`plan_review_adoption_execution.plan.md`）YAML 对照

| id | status | 说明 |
|----|--------|------|
| t1-internalize | completed | 采纳指南已落盘 |
| t2-reviewer-round | completed | findings 已落盘；frontmatter 已修订 |
| t3-deep-review | completed | REVIEW 已落盘 |
| t4-gitx-closeout | completed | 本文件 |

## Git 诊断（tier 1）

**`git status --short --branch`**（节选，反映本任务相关未跟踪项）：

```text
## cursor/paper-adversarial-skills
?? .cursor/plans/
?? docs/plans/REVIEW_plan_review_adoption.md
?? docs/plans/plan_review_adoption.md
?? docs/plans/plan_review_findings_round1.md
?? docs/plans/plan_review_closeout.md
（另含仓库既有大量 M/??，未纳入本任务提交面）
```

**`git worktree list`**：

```text
~/Documents/skill 9abcc8a [cursor/paper-adversarial-skills]
~/.codex/worktrees/f363/skill 86ab36a (detached HEAD)
```

## 实质 diff（tier 2）

本任务新增/修改的工件主要为**未跟踪**文件：`git diff --stat` 对路径参数无输出（尚未 `git add`）。**范围声明**：本收口说明覆盖 `docs/plans/plan_review_*.md`、`docs/plans/REVIEW_plan_review_adoption.md`、`.cursor/plans/plan_review_adoption_execution.plan.md`，不包含工作区中其它已存在的大块 `M` 文件。

## 验证记录（tier 3）

- **`cargo test --manifest-path scripts/router-rs/Cargo.toml`**：`482 passed`（约 28.5s）。本任务未改 `scripts/router-rs/src`，测试作为**回归向量**确认当前工作区可构建通过。

## defer / 未纳入项

- 用户附件中的 `plan与review提质_611e6884.plan.md`：**按约束未编辑**。
- 未执行 `git add` / `commit` / `push`（用户未授权写入 Git 历史）。

## 宿主侧可选 `/gitx plan`

本记录已给出跨宿主 Git 状态证据。若在支持 slash skill 的宿主里需要完整 `/gitx` 同契约流程，可再手动输入 **`/gitx plan`**。
