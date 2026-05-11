# 本地主线程模拟独立视角：对 `plan_review_adoption_execution.plan.md` 的只读 findings

**角色**：本地主线程模拟独立 reviewer 视角；未实际启用 subagent。**范围**：仅审 `.cursor/plans/plan_review_adoption_execution.plan.md` 初稿与已有真源引用是否自洽。

## Finding 1（采纳）— t4 Verify 与 gitx checklist 对齐不足

**观察**：末条 todo 要求宿主 `/gitx plan`，但未要求把 **`git diff` / `--stat` 摘要**写入 `plan_review_closeout.md`，与 `skills/gitx/SKILL.md` 中「Substantive diff」记录习惯有缝。

**建议**：在 **t4-gitx-closeout** 的 `Done when` 中显式加入「`plan_review_closeout.md` 含 `git diff --stat`（或声明本次无代码 diff）」；`Verify` 中保留 `/gitx plan` 并附带 `git status --short --branch`。

## Finding 2（采纳）— t2 Verify 缺少「修订可复核」硬证据

**观察**：「一轮修订」的验证仅 `rg Finding`，无法证明主线程确实合并进 **本计划文件**。

**建议**：在 **t2-reviewer-round** 的 `Verify` 增加 `git diff .cursor/plans/plan_review_adoption_execution.plan.md | head -n 40`（或等价：closeout 内嵌 diff 摘要）。

## Finding 3（defer）— 为 todos 增加 `Blocked by` 示例

**观察**：本样例为线性四步，未演示 `Blocked by:` 分支依赖。

**建议**：若后续扩展样例，可加一条「仅当 findings 阻塞时」的占位 todo；**本轮 defer**（不阻塞当前四步闭环）。

## Finding 4（采纳）— t3 防「空壳 review」

**观察**：深度 review 文件易被模板化而无实质符号锚点。

**建议**：在 **t3-deep-review** 的 `Done when` 要求 **P0–P2 至少一条**引用 `closeout_enforcement.rs` 或 `rfv_loop.rs` 中的**函数名或常量名**；`Verify` 用 `rg` 命中这些符号之一。

## Finding 5（defer）— isProject 与多工作区

**观察**：frontmatter `isProject: false` 在跨工作区镜像时可能需文档说明。

**建议**：在 `plan_review_adoption.md` 用一句话说明即可；**本轮 defer**（非验收阻塞）。
