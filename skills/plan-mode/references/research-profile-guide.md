> **调研 profile 深度参考**。本文件从 `skills/plan-mode/SKILL.md` 提取的 `plan_profile: research` 与 `execution` 详细定义、overview 模板、todo 收口要求及对照表。主文件仅保留摘要与指针；完整规范以本文件为准。

# 调研与执行计划 Profile 详细指南

---

## 1. 两类计划对照表

| 维度 | **`research`** | **`execution`（缺省）** |
|------|----------------|--------------------------|
| **目的** | 只读调研、问题矩阵、合成结论 | 落地实现：改代码 / 配置 / 测试 / 文档 |
| **允许动作（todos）** | 读、`rg` / 检索、对照文档、只读 code review、外部资料只读拉取、结论文字合成 | 写代码、加测试、改配置 / CI / 锁文件、生成产物、迁移与重构 |
| **禁止项** | 以「实现 / 改行为 / 加测试 / 改 CI / 改依赖锁」为单条主线；隐式触达 tracked 实现面资产 | 末条以「只读调研收口」替代计划/Git 证据收口；把跨宿主不可用的 `/gitx plan` 当成唯一验证 |
| **末条收口** | 调研合成 + 工作区无意外改动（`git status --porcelain` 为空，或输出仅含 `overview` 已声明允许回写的路径，须与窄例外单句一致） | 计划 vs 实际 + **Git 状态证据**（如 `git status --short --branch`、`git diff --stat`；宿主支持时可用 `/gitx plan`） |
| **下游** | 完成后**另开** `plan_profile: execution`（或缺省）写实现 todos | 通常即为终态；如再分阶段，可拆为多份顺序 `execution` 计划 |

**与 lifecycle 执行区衔接**：`plan_profile: research` **不得**与宏任务写入在同一回合、同一武装语义下混用；调研收口后**另开** execution（或缺省）计划，或用户显式启动 goal。execution 的 `Done when` / Horizon 可与 `GOAL_STATE.json` 契约对齐。宿主同轮混写行为见 [`AGENTS.md`](../../../AGENTS.md)。

---

## 2. `research`：overview 必填声明模板

`plan_profile: research` 时，`overview` 必须包含等价于以下语义的不可歧义表述（措辞可微调，语义不可缺）：

- 本文件为**调研计划**；调研执行期**不修改** tracked 的**源码 / 配置 / 测试 / CI 工作流 / 依赖锁文件**等实现面资产。
- 任何**实现 / 改行为 / 加测试**仅出现在**另开的** `plan_profile: execution`（或缺省）计划中。
- **窄例外（可选，须显式声明）**：若执行期需要**仅**回写结论文档 / plan 以记录结论，须在 `overview` **单句**列出允许路径集合（例如 `<host-plans-dir>/<本文件>.plan.md`），且末条 `Verify` 仍约束 `git status --porcelain` 为空或仅含上述已声明路径。
- 若用户明确要求**不声明窄例外**（不回写任何落盘结论文档 / plan），则**不**在 `overview` 写允许回写路径，结论只留在对话；不得默认隐式改任何未声明路径。

**最小模板（可裁剪复制）**：

```text
本文件为调研计划（plan_profile: research）。调研执行期不修改 tracked 源码 / 配置 / 测试 / CI / 依赖锁等实现面资产；后续实现另开 plan_profile: execution（或缺省）计划。
[可选窄例外] 仅允许在末条 Verify 约束内回写 overview 已声明的路径（示例：`<host-plans-dir>/<本文件>.plan.md`）；不触达其它路径。
```

---

## 3. `execution`：overview 一句式模板

`plan_profile: execution`（或缺省）时，`overview` 须有一句标明**本计划允许**按 todos 修改实现面资产（代码 / 配置 / 测试等），且末条做计划 vs 实际 + Git 状态证据收口；宿主支持时可使用 **`/gitx plan`**：

```text
本文件为执行计划（plan_profile: execution / 缺省）。允许按下方 todos 修改代码 / 配置 / 测试等实现面资产；末条以计划 vs 实际 + Git 状态证据完成收口（宿主支持时使用 /gitx plan）。
```

---

## 4. `research`：正文建议结构

- **`## 调研问题与结论`**（或等价标题）：每个子问题对应结论文或显式 **`open gap`**（未答须写原因或外部依赖）。
- **`## 证据与范围`**：已读路径、命令/检索摘要、外部引用；避免无来源断言。
- **`## 合成（Synthesis）`**：跨 todo 的一致结论与矛盾消解说明。

---

## 5. `research`：每条 todo 与末条收口

- **动作**：以读、搜、`rg`、对照文档、只读 code review、外部资料只读拉取为主；**Verify** 须为只读命令或可勾选的人工对照（不得依赖「改文件后跑测试通过」作为唯一手段）。
- **profile 级 Non-goals**：在 `overview` 或每条 todo 写明 **不改**仓库内 tracked **源码 / 配置 / 测试**；若执行中须回写结论文档 / plan 以写入结论，须在**末条** `Verify` 中显式允许的路径集合与 `overview` 窄例外单句一致（示例：`<host-plans-dir>/<本文件>.plan.md`）。
- **末条 todo（调研收口）**：`Done when`：§调研问题与结论 逐条有结论文或 open gap；与各前置 todo 结论交叉一致、无自相矛盾。`Verify`：**不**含 **`/gitx plan`**；须含 **`git status --porcelain`** 为空 **或** 输出仅含已声明允许的 plan 路径，并含「对照正文指定节与 YAML `todos` 逐项」的可客观勾选表述。

**末条示例（`research`，按实际文件名替换路径）**：

```text
对照调研问题矩阵与合成结论并完成调研收口 @ <host-plans-dir>/<本文件>.plan.md
| Done: §调研问题与结论 逐条有结论或 open gap；与前置 todos 无矛盾
| Verify: git status --porcelain 为空或仅列出 overview 已声明允许的路径；人工逐项对照 §调研问题与结论 与 frontmatter todos（不得要求 /gitx plan）
```

---

## 6. 下游说明

调研 profile 完成后，**另开**一份 **`plan_profile: execution`**（或缺省）计划写实现类 todos，并用计划 vs 实际 + Git 状态证据作为末条收口；宿主支持时可使用 **`/gitx plan`**。避免同一文件混用「一半调研一半实现」。有前置调研时，新开 execution 须按主文件 **执行计划继承面** 写入继承指针与准入表，再写实现 todos。

---

## Related

- `skills/plan-mode/SKILL.md` — 跨宿主通用 plan-mode 规范（主文件）。
- [references/research-scope-and-examples.md](research-scope-and-examples.md) — 调研范围、能力联动与弱/强示例。
