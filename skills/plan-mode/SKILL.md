---

description: 'Cross-host Plan / 策划文档闸门：先用本地证据起草可执行计划，再产出可验收 todo；默认轻量五行证据；高风险时 audit plan。 `plan_profile: execution`（缺省）末条须做计划 vs 实际 + Git 状态证据收口（宿主支持时使用 `/gitx plan`）；`plan_profile: 跨宿主 Plan 闸门，产出可验收 todo，非 lifecycle 专用。'
metadata:
  platforms:
  - supported
  tags:
  - plan
  - workflow
  - gate
  - closeout
  version: '2.0.0'
name: plan-mode
scene: general
risk: low
routing_gate: none
routing_layer: L1
routing_owner: owner
routing_priority: P2
session_start: preferred
source: project
trigger_hints:
- CreatePlan
- Cursor Plan
- Plan 模式
- gitx plan 收口
- research-only plan
- 可验收 todo
- 策划文档闸门
- 纯调研
- 计划对照实际
- 调研计划
---
# plan-mode

把「写计划」当成**证据先行、可验收、可对照收口**的产物，而不是一次性 prose。默认不要把小任务拖进审计级流程；只有跨模块、高风险、用户明确要求或宿主 gate 需要时，才升级到完整 audit plan。计划草稿落在宿主工作区（如 `.cursor/plans/`）；Task 状态机驱动下计划真源在 **`artifacts/current/<task_id>/ROADMAP.md`**。终稿统一归档到 `artifacts/current/<task_id>/`。

## When to use

- 用户要在 **Plan 模式** 下先把范围、风险、验证路径钉死，再允许大规模改动。
- 用户提到 **策划文档闸门**、**可验收 todo**，或明确要求审计级计划。
- 用户明确要走：**计划获批 → 实现 + 测试通过 → 计划 vs 实际 + Git 状态证据** 的收口；宿主支持时可用 `/gitx plan`（`/gitx plan` 与 `/gitx` 等价，见 [`skills/gitx/SKILL.md`](../gitx/SKILL.md)）。
- **每轮对话开始 / first-turn / conversation start**：任务看起来像「先出高质量计划/蓝图」且后续实现依赖该计划的验收标准。
- 用户要 **纯调研 plan**：只产出深度、多角度 **只读**调研 todo，**不包含**实现 / 改代码 / 改配置 / 改测试（见 `plan_profile: research`）。

## Do not use

- 用户只要直接实现、明确禁止前置规划或策划文档。
- 纯 **skill 路由系统 / routing registry / manifest** 治理与 miss repair → `skill-framework-developer`。
- 单一极小改动（单文件几行）且计划成本明显高于收益 → 直接最小 delta + 验证即可。

## Plan profile（`plan_profile`）

### Plan depth（轻量 / execution / audit）

| 档位 | 适用 | 最小要求 |
|------|------|----------|
| **轻量 plan** | 小中型、低风险、单 owner 任务 | 五行证据（Goal / Non-goals / 已读证据 / 最大风险 / 首选验证）+ 可验收 todo；不强制继承面或审 plan。 |
| **execution plan** | 需要落盘、跨文件或需对照实现的任务 | `plan_profile: execution`（或缺省）；todos 写四元组；末条做计划 vs 实际 + Git 状态证据收口。 |
| **audit plan** | 跨模块、高风险、安全/供应链、用户明确要求审计划/深度 review | execution plan + 可选 review-only findings + 更严格证据门槛；是否启用 subagent 仍受 `AGENTS.md` 执行梯子约束。 |

**与继承面**：**轻量**指不强制完整 audit 叙事与继承面占位；若本文件为 **`plan_profile: execution`（或缺省）** 且满足宿主工具产出契约硬条款第 2 条触发条件，仍**须**含 **`## 执行计划继承面`**。

与 `name` / `overview` / `todos` **同级**的 frontmatter 字段 **`plan_profile`** 区分计划类型：

- **`execution`**（**缺省**）：标准实现计划；末条须按计划 vs 实际 + **Git 状态证据**收口，宿主支持时使用 **`/gitx plan`**。若 todo 触及 Python 依赖/CI/本机 PATH，须引用 **`$python-env-management`**，禁止 operator `pip`。
- **`research`**：**调研专用**；todos **仅**只读调研与结论合成；**禁止**以「实现 / 改行为 / 加测试 / 改 CI」为单条主线；**末条不得**把 **`/gitx plan`** 当作本 profile 的必需验证。

若宿主工具 **剥离** 未知 YAML 键：生成后**手动补写** `plan_profile: research`。可选用文件名 `*.research.plan.md` 作为人类可读标签；**hook 与契约真源仍以 frontmatter 为准**。Cursor 宿主专属处理：见 [references/cursor-createplan-contract.md](references/cursor-createplan-contract.md)。

详细对照表、overview 模板、todo 收口要求与正文结构：见 [references/research-profile-guide.md](references/research-profile-guide.md)。

## 调研范围与能力联动

在 `overview` 中用**一句**标明调研是否触网（仓库内只读 vs 内外并行）。范围模板、能力与工件联动表、弱/强示例：见 [references/research-scope-and-examples.md](references/research-scope-and-examples.md)。

## 执行计划继承面（research→execution）

当存在**前置** `plan_profile: research` 文档或等价调研结论文档时，`execution` `.plan.md` 在 frontmatter 闭合后正文内须有 **`## 执行计划继承面`**（≤15 行），先于任意正文 checkbox 清单。无前置调研可省略或写 `继承指针：无`。

| 字段 | 要求 |
|------|------|
| **继承指针** | 一行：路径真实存在或可检 |
| **Goal / Non-goals** | 各**一行**，从 research §合成 压缩，禁止长段粘贴 |
| **不变量** | 调研已钉死的边界（若无写「无」） |
| **已否决方案** | 每项半行；若无写「无」 |
| **问题矩阵映射** | 每条 P0/P1 级 execution todo 对应至少一个 research 问题 id 或 `open gap` |
| **外部准入表** | 无外部调研写 **`无`**；有则每行：`URL | 用途 | 本仓库锚点路径 | 采纳或否`；**默认不超过 5 行** |

**与四元组**：`scope` 路径应能从继承指针或矩阵映射追溯到仓库内证据；`Verify` 不得无故弱于 research 已给出的验证类型。Cursor 宿主专属继承面映射：见 [references/cursor-createplan-contract.md](references/cursor-createplan-contract.md)。

## Workflow（四步）

1. **本地证据先进计划**（见 [能力与工件联动表](references/research-scope-and-examples.md)）：在写结构化计划前，完成域内必要的深读、检索或代码定位；计划应收敛已有证据，而不是用计划代替定位结论。
2. **Todo 必须可验收**：每条 todo 在同一条可见文案里写全 **四元组**（见 **Todo 可执行性**）；宿主工具生成的 `.plan.md` 还须满足宿主工具的产出契约。
3. **可选 review 只找问题**：仅当用户明确要求 review plan / 审计划 / 深度 review 时，review lane 只读计划与证据，输出 findings / risks / missing tests（**默认 compact**）；不改代码、不自动修复。详见 [`skills/code-review-deep/SKILL.md`](../code-review-deep/SKILL.md)。
4. **收口（依 `plan_profile`）**：
   - **`research`**：完成调研合成与问题矩阵收口（见 [references/research-profile-guide.md](references/research-profile-guide.md)）；**不**把 `/gitx plan` 作为本 profile 的必需验证。
   - **`execution`**：获批且实现与测试通过后做计划 vs 实际逐项对照并记录 Git 状态证据。宿主支持时可用 `/gitx plan`。
   - **用户可见回复**：默认宏观短回复 — 做了什么 + 效果；是否完整完成；缺口（仅当非「是」时）；下一步推荐（可选）。语气与 `AGENTS.md` → Closeout 一致。Cursor 宿主收尾约束：见 [references/cursor-createplan-contract.md](references/cursor-createplan-contract.md)。

## Todo 可执行性（四元组、对齐与依赖）

### 必备四元组（每条 todo 自检）

| 维度 | 要求 |
|------|------|
| **动作** | 动词 + 对象；`research` 下应为「读 / 对照 / 归纳」等 |
| **范围** | 主要路径 1–3 个；忌「全仓优化」式模糊 |
| **完成定义（Done when）** | 可勾选、可客观判定 |
| **验证手段（Verify）** | 完整命令或明确人工步骤 |

可选第五行 **Non-goals**：一行写明本步**不**改什么。

### 单条模板（复制用）

```text
[id] <动词> <对象> @ <path1>[, <path2>]
Done: <条件1>; <条件2>
Verify: <命令 或 人工检查步骤>
Non-goals: <可选>
```

弱例与强例（含 gitx 对齐、审 plan 修订、深度 review 防空壳等场景）：见 [references/research-scope-and-examples.md](references/research-scope-and-examples.md)。

### YAML 正文对齐与拆分依赖

- **禁止**只在正文写验收、frontmatter `todos[].content` 仅写阶段名；**id / 顺序 / 验收标准**在 YAML `todos` 与正文 checklist 之间保持一致。
- **一条 todo ≈ 一个可合并 PR 级结果**（或更小）。出现「且 / 然后 / 另外」时拆成多条。
- **分支 / 多选一**：为每条分支写独立 todo + **仅当** / **`Blocked by: <todo-id>`**。忌单条「执行整条 P0 链」。

**宿主工具产出契约**：宿主工具生成或更新的 `.plan.md` 文件须满足四元组、profile 分岔、继承面与末条收口等核心规范。**Skill 路由不会改写磁盘上的 plan 文件**。Cursor 宿主专属契约：见 [references/cursor-createplan-contract.md](references/cursor-createplan-contract.md)。

## Continuity 与工件

分层与 hook 以 [`docs/architecture.md`](../../docs/architecture.md) 为准；计划落盘于宿主工作区 plans 目录；宿主 Plan Build **不**自动武装 lifecycle goal 门控，连续执行由用户显式启动。

## 宿主差异

| 宿主特性 | 说明 | 详细文档 |
|----------|------|----------|
| Cursor **CreatePlan** 输出契约 | 硬条款、不合规/合规示例、Save to workspace、Plan Build 与 goal 门控、`.mdc` 自检清单 | [references/cursor-createplan-contract.md](references/cursor-createplan-contract.md) |
| Cursor **review 硬路径** + **session-close-summary** | `.cursor/hook-state` 清门；收尾回复语气约束 | 本文件 **能力与工件联动表** + [references/cursor-createplan-contract.md](references/cursor-createplan-contract.md) |
| 通用 **宿主工作区路径** | 计划草稿落在宿主工作区 | 本文件 **Continuity 与工件** |

## Related

- `skills/SKILL_FRAMEWORK_PROTOCOLS.md` — 讨论 → 规划 → 执行 → 验证形状。`skills/gitx/SKILL.md` — `/gitx plan` 收口。
- `skills/code-review-deep/SKILL.md` — 深度代码审。终稿统一归档到 `artifacts/current/<task_id>/`。
- [references/research-profile-guide.md](references/research-profile-guide.md) — 对照表、overview 模板、todo 收口。
- [references/research-scope-and-examples.md](references/research-scope-and-examples.md) — 调研范围、能力联动与弱/强示例。
- [references/cursor-createplan-contract.md](references/cursor-createplan-contract.md) — Cursor 宿主专属全量契约。
