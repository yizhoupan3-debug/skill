# 调研 plan → 执行 plan：减法与第一性（结论文档）

本文档落实调研计划《调研执行衔接架构》的可验收项；**按用户约束未修改** `.cursor/plans/` 下该调研 plan 源文件本身。结论与 §合成 均写于本路径，便于纳入 `docs/plans/` 与 PR 审阅。

---

## 调研问题与结论

### Q1 第一性：执行计划真正需要从调研计划继承什么？

**已有证据**

- [`skills/plan-mode/SKILL.md`](../../skills/plan-mode/SKILL.md) 已区分 **`research` / `execution`**，并规定调研完成后 **另开** execution 计划写实现 todos 与 **`/gitx plan`** 末条；`research` 正文建议 **§调研问题与结论 / §证据与范围 / §合成**。
- **四元组**（动作、范围、Done、Verify）已是 execution todo 的硬形状，与 [`skills/SKILL_FRAMEWORK_PROTOCOLS.md`](../../skills/SKILL_FRAMEWORK_PROTOCOLS.md) 中 **execution item**（`action`、`scope`、与 finding 关联）在思想上同构：执行侧本质是「可验证动作列表」而非叙事长文。
- **调研范围** 句已可贴入 `overview`，区分仅仓库内 vs 内外并行，减少执行侧默认乱拉外部范式。

**缺口**

- 契约**未命名**「从 research 复制到 execution 的最小字段集」；模型容易把 research 全文当上下文，或在 execution 里重写一遍背景，导致 **继承面模糊**。
- `SKILL_FRAMEWORK_PROTOCOLS` 的 execution item 要求 **`finding_ids`**，而 plan-mode 的 Markdown plan **未强制** research 产出结构化 finding id；**交接时 finding→exec item 映射无单页真源**。

---

### Q2 减法：现有文档里哪些是重复噪声、应收敛或删除指向？

**已有证据**

- 「另开 execution」「末条 `/gitx plan`」「四元组」「CreatePlan 自检」在 `plan-mode`、`cursor-plan-output.mdc`、`plan_todo_checklist.md` 间有**必要重复**（宿主 alwaysApply vs skill 深度说明）。
- [`docs/plans/plan_writing_capability_research_synthesis.md`](plan_writing_capability_research_synthesis.md) §1b 已承认初版偏摘录，并以短链指回 `plan-mode` **调研范围与能力联动** — 属于**减法方向**（合成稿不做第二套 Workflow 长文）。

**缺口**

- **调研范围**、**能力联动表**、**Workflow**、**CreatePlan 契约** 在 `plan-mode` 内相邻多节，新人阅读仍有「表格 + 列表叠床」感；尚未规定「**先读哪 5 行再展开**」的阅读顺序（第一性：入口句）。
- `plan_todo_checklist` 与 `plan-mode` 在「research overview 调研范围句」等处**新增**勾选后，与旧条目的合并顺序未在文档层标明「单一真源优先级：争议以 `plan-mode` 为准」。

**减法清单（建议下游 execution 文档化采纳，本调研不直接改 skill）**

| 动作 | 对象 | 保留理由 |
|------|------|----------|
| **并** | checklist 与 plan-mode 中重复的「末条 profile 分岔」说明 | 保留 checklist 为可打勾短表；长解释只保留 `plan-mode` 一处 |
| **链** | 合成稿中任何未来新增的宿主 URL | 不复制论坛长帖；只保留链接 + 一行结论 |
| **删（语义上）** | execution plan 正文中的大段 research 复述 | 改为「见 `docs/plans/<本调研>.md` §X」+ **继承面**（下节） |

---

### Q3 行为缺口：为何「写完调研就去执行」时执行 plan 吃不上调研？

**已有证据**

- Workflow 第 5 步要求 **delta** 交接，但针对的是「计划修订」对用户审批，**不是** research→execution 的强制机器字段。
- `下游` 仅说「另开 execution」，**未规定** execution 的 `overview` 必须引用上一份 research 的路径或节锚。

**缺口**

- **research → 立即 coding** 路径下，没有**强制停顿点**让作者写出「本 execution 只继承以下条目」；宿主 CreatePlan 也不会自动注入 research 路径。
- 缺少 **单一 handoff 工件或小节名**（固定标题），导致执行 plan 在模型侧像「冷启动」。

**可选 handoff 形态（仅建议，本调研不改代码）**

1. **`## 执行计划继承面`（≤15 行）** 写在 **execution** `.plan.md` 正文最前（在分节 todos 前）：字段固定为 `继承自: <path>#锚点`、**Goal / Non-goals / 不变量 / 已否决方案 / 外部准入表（可空）**；execution 的每条 todo 在 `content` 首行或 Done 中标注 `继承: <research 问题 id>`。依赖：作者纪律；Verify 可用 `rg "继承自|继承:" .cursor/plans/<execution>.plan.md`。
2. **侧车 findings 文件**：research 收口时写 `docs/plans/<topic>_research_closeout.md` 仅含矩阵与结论；execution `overview` 单行链到该文件。依赖：多一文件；适合长调研。

---

### Q4 外部「好案例」如何进 execution 才不变成架构屎山？

**已有证据**

- `plan-mode` 已要求：外部开启时 **§证据与范围** 须 URL + 日期/版本，且 **不得省略** 仓库内检索 todo — 这是 **并行** 与 **可追溯** 的底线。
- 合成稿 §4 已把「外部方法论」与 **A 档规范** 对齐，但未写 **「进入架构设计的门槛」**。

**缺口**

- 未规定 **外部条目数量上限**、**禁止把外部整章架构粘贴进 execution overview**、以及 **「未映射到本仓库路径则不得写进架构 todos」**。

**结论（可验收规则草案）**

- **准入表**：进入 execution 正文的每条外部启发须一行：`来源 URL | 一言用途 | 本仓库锚点路径（或显式「无，不采纳结构仅采纳问题表述」）| 采纳/不采纳`；**默认上限 5 行**（减法）。
- **第一性**：任何「新抽象 / 新子系统 / 新配置层」类 execution todo，Done 须含 **本仓库** `rg` 或 Read 证据；纯外部范式只能出现在 **Non-goals** 或 **已否决**。
- **Non-goals（执行面）**：execution overview 增加一句「不复制外部仓库目录结构 / 不引入调研中未出现的需求」。

---

## 合成（Synthesis）

### 第一性最小交接集（建议 execution 侧强制携带的 ≤6 类信息）

1. **继承指针**：`docs/plans/<本调研>.md` 或 `.cursor/plans/<research>.plan.md` 的路径 + §锚点（一行）。
2. **Goal / Non-goals**：从 research §合成 压缩各 **一行**（不得长于 research 原文复述）。
3. **不变量**：调研中已证明的边界条件（例如「不新增 hook schema」）。
4. **已否决方案**：research 中明确否掉的选项各 **半行**，防止 execution 又捡回来。
5. **问题矩阵 id → execution todo**：每条 P0 todo 至少映射一个 research 问题 id 或 `open gap`。
6. **外部准入表**：见 Q4；若无外部调研则写「无」。

与 **四元组** 的映射：`scope` 继承自矩阵中的路径；`Verify` 不得弱于 research 已写的命令类型（可收紧、不可无故换掉真源）。

**与 `SKILL_FRAMEWORK_PROTOCOLS` 对齐**：若下游采用 execution item 思维，可为每条 item 虚拟 `finding_id = research-<Qn>-<slug>`，便于对照 §3 的映射；**open gap**：plan-mode 尚未要求 Markdown plan 写 finding_id，需另开 **execution 级** 小改动才统一。

### 减法清单（执行侧优先）

- execution **overview** 禁止超过 **30 行**背景；背景一律 **链** research 文档。
- 删除 execution 中与 research **逐字重复** 的 §证据；只保留 **继承面** + 新写的 **实现 delta**。
- checklist 层：同一检查句若已在 `cursor-plan-output` 对 CreatePlan 硬约束，skill 内仅保留 **「为何」一句 + 链接**。

### 外部案例准入门槛（防屎山）

- 无 **本仓库锚点** 的外部模式 → 只能进 **「阅读材料」** 小节，**不得**生成「仿该仓库的模块拆分 todos」。
- 超过 **5** 条外部启发 → 必须拆 **第二份 execution** 或回到 research 再收敛（减法：分批）。
- **禁止**在 execution frontmatter `overview` 内嵌套多段外部长引文；只保留链接与准入表行。

### handoff 形态建议（自然联动优先）

- **推荐**：`## 执行计划继承面` + **继承指针** 一行（成本最低、对「写完调研就执行」最友好）。
- **备选**：独立 `*_research_closeout.md`（适合长调研、多人读）。

---

## Open gaps

- **finding_id 与 plan Markdown**：是否要在 `plan-mode` 增加可选 frontmatter `research_finding_ids`（宿主可能剥离）— 风险与收益需单独 **execution** 小计划评估。
- **Cursor CreatePlan** 是否支持在 execution 模板中预置「继承面」标题 — 依赖产品，本仓库只能规则约束。

---

## 建议下游 execution 计划标题（单条）

**「plan-mode：增加执行计划继承面模板 + research_closeout 链式约定（不含 router-rs 校验）」**

---

## Verify 记录（对应调研 plan todos）

```text
rg -n "下游|调研范围|1b\\.|另开" skills/plan-mode/SKILL.md docs/plans/plan_writing_capability_research_synthesis.md
rg -n "execution item|四元组" skills/SKILL_FRAMEWORK_PROTOCOLS.md docs/plans/plan_todo_checklist.md
rg -n "合成|减法|第一性|准入" docs/plans/RESEARCH_plan_execution_handoff_first_principles.md
```

（执行调研时应在终端保存上述命令输出摘要至 §证据与范围；本文件为结论文档已内嵌命令文本。）
