# 计划写作核对清单（Todo 可执行性）

**权威叙述**：[`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) 中 **Todo 可执行性** 与 **CreatePlan 输出契约**。本页为可打印/可勾选的短清单；Cursor 草稿计划默认在 [`.cursor/plans/`](../../.cursor/plans/)（若纳入版本控制则与仓库同轨）。Cursor 侧 alwaysApply 重申见 [`.cursor/rules/cursor-plan-output.mdc`](../../.cursor/rules/cursor-plan-output.mdc)。

## CreatePlan（`.plan.md`）

- [ ] **每条** `todos[].content` 在同一条字符串内包含四元组（动作、范围、Done when、Verify），禁止仅阶段名。
- [ ] **`plan_profile`**（与 `name` / `overview` / `todos` 同级）：**缺省或 `execution`** → 标准实现计划；**`research`** → 仅只读调研 todo，见 [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) **Plan profile**。
- [ ] **`overview` profile 声明**（与 SKILL **CreatePlan 输出契约 §0** 对齐）：
  - **`research`**：`overview` 含调研期 **Non-goals** 硬声明——**不修改** tracked 源码 / 配置 / 测试 / CI 工作流 / 依赖锁文件，实现另开 `plan_profile: execution` 计划；如使用「仅回写本 `.plan.md`」窄例外，须在同一 `overview` 单句声明。
  - **`execution`（缺省）**：`overview` 有一句标明**本计划允许**按 todos 修改代码 / 配置 / 测试等实现面资产，并由末条 **`/gitx plan`** 完成 Git 收口。
- [ ] **`overview` 调研范围句**（见 [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) **调研范围（Research scope）与能力联动**）：**默认**须标明仅仓库内只读（不默认对外网络检索）；若用户要求「外部 / 网络 / 官方 cross-check」，须标明 **仓库内 + 外部只读并行**，且至少一条 todo 的 `Verify` 能核对外部来源（URL + 日期/版本）；**不得**用外部摘要替代仓库内检索 todo。
- [ ] **审 plan / 深度 review 可追踪**：需要独立 reviewer findings 时，todos 或 Related 能落到 Workflow 第 3 步及建议 findings 路径（如 `docs/plans/*_findings*.md`）；需要对抗式代码审时，能路由到 [`skills/code-review-deep/SKILL.md`](../../skills/code-review-deep/SKILL.md)。
- [ ] **最后一条** todo（依 profile）：
  - **`execution`（缺省）**：`Verify` 显式含 **`/gitx plan`**；`Done when` 可判定（对照计划逐项、未做项有原因或 defer）；关联 closeout 或末条说明中宜含 **`git diff --stat`** 或显式「本次无代码 diff」（与 [`skills/gitx/SKILL.md`](../../skills/gitx/SKILL.md) 实质性 diff 记录习惯一致）。
  - **`research`**：`Verify` **不含** **`/gitx plan`** 作为必需收口；须含 **`git status --porcelain`** 与对照正文 + YAML `todos`；`Done when` 覆盖调研问题矩阵结论文或 open gap。
- [ ] 调用 CreatePlan 后若不合规：**编辑该 `.plan.md` 补齐**（含补写被剥离的 `plan_profile: research` 与 `overview` profile 声明），不依赖路由或 hook 自动改文件。

## 起草每条 todo 时

- [ ] **动作**：动词 + 对象（改什么 / 断言什么 / 写哪段文档）。
- [ ] **范围**：1–3 个路径（文件或目录），无「整仓模糊」式 owner。
- [ ] **Done when**：可客观勾选（`rg`/测试/段落定位），不依赖主观「更好看」。
- [ ] **Verify**：完整命令或明确人工步骤（含 `--manifest-path` 等若需要）。
- [ ] **Non-goals**（可选）：一行写明本步不碰什么。

## 计划结构

- [ ] **YAML 与正文对齐**：`todos[].content` 与正文 checkbox 的 id、顺序、验收一致；禁止「YAML 只有阶段名、验收只在正文」。
- [ ] **拆分**：一条 todo 一个 PR 级结果；有「且/然后/另外」则拆条。
- [ ] **分支/依赖**：多选一写多条并标 **仅当**；依赖写 **`Blocked by: <id>`** 或指向 § 决策。

## 流程与收口

- [ ] 调研与证据先进计划，计划不代替定位结论（见 `plan-mode` Workflow 第 1 步）。
- [ ] **审 plan 一轮修订可复核**（见 `plan-mode` **弱例与强例**）：合并 reviewer findings 后，`Verify` 宜含对本计划文件的 `git diff .cursor/plans/<本计划>.plan.md | head -n 40`（路径按实际替换）或 closeout 内嵌等价 diff 摘要，避免仅 `rg Finding` 无法证明正文已改。
- [ ] **`execution`**：实现与测试通过后按计划对照实际，用 **`/gitx plan`**（与 **`/gitx`** 同契约，见 `skills/gitx/SKILL.md`）收口。
- [ ] **`research`**：末条完成调研矩阵与合成收口；后续实现另开 **`execution`**（或缺省）计划再 **`/gitx plan`**。
