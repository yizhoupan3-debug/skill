> **Cursor 宿主专属参考**。本文件从 `skills/plan-mode/SKILL.md` 中提取的 Cursor 特化细节，仅适用于 Cursor 宿主环境。其他宿主（如 Claude、Codex）可跳过本文件；跨宿主通用规范见 SKILL.md 正文。

# CreatePlan 输出契约与 Cursor 宿主细节（参考）

本文件汇集 Cursor **CreatePlan** 工具在 `plan-mode` 体系下的宿主特化契约与落盘说明。SKILL.md 正文仅保留通用四元组与 profile 逻辑；以下为 Cursor 侧必须遵守但不必在跨宿主主文档中展开的约束。

---

## 1. CreatePlan YAML 键剥离与手动补写

若宿主 **CreatePlan** 剥离未知 YAML 键（如 `plan_profile`）：

- 生成后须在该 `.plan.md` 的 frontmatter 中**手动补写** `plan_profile: research`（或 `execution`）。
- 可选用文件名 **`*.research.plan.md`** 作为人类可读标签。
- **hook 与契约真源仍以 frontmatter 为准**——本仓库当前 **不**按文件名解析 profile。

---

## 2. 宿主侧计划落盘（Save to workspace）

Cursor 官方说明：计划默认保存在**用户目录**，需**「Save to workspace」** 才进入工作区以便版本管理与团队共享。

- 权威草稿与链接建议存放于 **`.cursor/plans/`**（宿主工作区路径）。
- `docs/plans/` 此前用于索引与指针，已清理。内部 todo 与文件不同步等宿主/社区讨论，参见 [`MIGRATION.md`](../../../MIGRATION.md)。

---

## 3. `.cursor/rules/cursor-plan-output.mdc` 与继承面映射

`.cursor/rules/cursor-plan-output.mdc` 作为 Cursor alwaysApply 规则，对 CreatePlan 产出做硬自检。该 `.mdc` 与本 skill 继承面的关系如下：

- **alwaysApply 自检清单**：以四元组、末条计划/Git 证据收口，以及有前置 research / 等价结论文档 / 外部合成材料时 execution 须有 `## 执行计划继承面` 的指针级硬自检为准。
- **插入位置**：继承面须位于第二个 `---`（YAML frontmatter 闭合行，非正文 Markdown 横线 `---`）**之后**的正文内、先于**任意**正文 Markdown checkbox 或其它分节任务叙述。与 CreatePlan 契约硬条款第 **2** 条一致。
- **减法规则**：继承面字段与行数上限以 SKILL.md **执行计划继承面（research→execution）** 一节为真源。`cursor-plan-output.mdc` **不**镜像继承面全文表格，以免双真源膨胀。

---

## 4. Cursor Plan Build 与 lifecycle goal 门控

Cursor Plan Build **不**自动武装 lifecycle goal 门控。连续执行仅由用户显式启动。

- Pre-goal 提示见 **`ROUTER_RS_PRE_GOAL_ENABLED`**。

---

## 5. CreatePlan 输出契约（完整）

> 以下为从 SKILL.md **CreatePlan 输出契约（Cursor）** 整节下沉的完整内容。

**适用范围**：宿主通过 **CreatePlan** 新建或更新、落盘为 **`.plan.md`** 的计划（常见路径：工作区 `.cursor/plans/`；以 Cursor 实际写入为准）。**Skill 路由不会改写磁盘上的 plan 文件**；合规依赖主线程在调用 CreatePlan **之后**对照本节自检，必要时编辑该 `.plan.md` 补齐。

**Profile 分岔**：`plan_profile: research` 时须同时满足 SKILL.md **Plan profile（`plan_profile`）** 与下表 **`research` 列**；**缺省**或 **`execution`** 时满足 **`execution` 列**。

### 硬条款

0. **`overview` 必含 profile 声明**：
   - **`plan_profile: research`**：`overview` 须含与 **Plan profile** 等价的「调研期零实现面改动」声明；如声明窄例外（回写结论文档 / plan 等路径，须在**同一 `overview` 单句**中列出允许路径集合），且末条 `Verify` 仍按 `research` 列约束 `git status --porcelain`。
   - **`plan_profile: execution`（缺省）**：`overview` 须有一句标明本计划允许按 todos 修改实现面资产，且末条用计划/Git 状态证据收口。

1. **每条** frontmatter `todos[].content` 须在**同一条字符串**内可见 **四元组**（动作、范围 1–3 路径、Done when、Verify），与 SKILL.md **Todo 可执行性** 一致；禁止「content 只有阶段名、细节全在正文」。

2. **`execution` 正文与前置调研**：若有前置 `plan_profile: research` 或外部合成材料，在第二个 `---`（YAML frontmatter 闭合行）**之后**的正文内须有 **`## 执行计划继承面`**，且须位于**任意正文 Markdown checkbox 清单或其它分节任务叙述之前**（**不得**写入 YAML frontmatter）；字段见 SKILL.md **执行计划继承面（research→execution）**；无前置调研的 execution 可省略本节，或在该小节内写一行 **`继承指针：无（无前置调研）`**。

3. **`todos` 最后一条**（依 profile）：

   | | **`execution`（缺省）** | **`research`** |
   |---|-------------------------|------------------|
   | **语义** | 计划 vs 实际 + **Git 状态证据收口** | **调研合成** + 工作区无意外改动 |
   | **`Done when`** | 可客观判定：已对照计划正文与 todos 逐项；未执行项有写明原因或 defer | 调研问题与结论等逐条有结论文或 **open gap**；与各前置 todo 一致 |
   | **`Verify`** | 须显式包含 Git 状态证据（如 `git status --short --branch`、`git diff --stat`）；宿主支持时可包含 **`/gitx plan`** | **不得**将 **`/gitx plan`** 作为必需项；须含 **`git status --porcelain`** 约束与正文对照表述 |

   两条 profile 下末条均须含完整四元组（`execution` 动作可写「对照计划与实现并记录 Git 状态证据」；`research` 动作可写「对照调研问题矩阵与合成结论并完成调研收口」）。

4. 若正文含 Markdown checkbox 清单：**id / 顺序 / 验收**与 YAML `todos` 对齐。

5. **条件分支**（A/B/C）：每条分支独立 todo + **仅当** / **`Blocked by: <todo-id>`**；禁止单条「执行整条链」替代逐步验收。

### 不合规 vs 合规（摘要）

- **不合规（`execution`）**：`overview` 不含「允许按 todos 修改实现面 + 计划/Git 证据收口」声明；`content: "实现功能"`；末条无 Git 状态证据。
- **不合规（`research`）**：`overview` 未含调研期零实现面改动硬声明；todo 主线为改代码/加测试；末条 `Verify` 仍强制 `/gitx plan` 作为唯一收口。
- **合规（`execution` 末条）**：`overview` 已含 execution 一句式声明；`content` 内 `Done:` / `Verify:` 齐全，且 `Verify:` 含 `git status --short --branch`、`git diff --stat` 等 Git 状态证据；宿主支持时可附带 `/gitx plan`。
- **合规（`research` 末条）**：`overview` 已含 research 零改动声明（如使用窄例外亦已单句声明）；`Verify:` **不含** `/gitx plan`，且含 `git status --porcelain` 与正文对照表述。

---

## 6. `.cursor/rules/session-close-summary.mdc` 引用

计划执行完成后的用户可见聊天收尾，语气与「几句带过」见 [`.cursor/rules/session-close-summary.mdc`](../../../.cursor/rules/session-close-summary.mdc)；与仓库根 **`AGENTS.md`** 的 Closeout 「证据优先落工件、聊天不默认堆证据」一致。

---

## Related

- `skills/plan-mode/SKILL.md` — 跨宿主通用 plan-mode 规范（四元组、profile、继承面完整定义）。
- `.cursor/rules/cursor-plan-output.mdc` — Cursor alwaysApply 下对 CreatePlan 产出的硬自检清单。
- `.cursor/rules/session-close-summary.mdc` — Cursor 收尾回复风格约束。
- `docs/plans/` 已清理（见 [`MIGRATION.md`](../../../MIGRATION.md)）。
- [`skills/gitx/SKILL.md`](../../gitx/SKILL.md) — `/gitx` / `/gitx plan` 收口契约。
