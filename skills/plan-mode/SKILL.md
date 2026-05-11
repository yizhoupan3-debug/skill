---
name: plan-mode
description: |
  Cursor Plan / 策划文档闸门 owner：先用本地证据起草可执行计划，再产出可验收 todo；`plan_profile: execution`（缺省）末条用 `/gitx plan` 对照计划收口，`plan_profile: research` 为纯调研计划（只读 todos，末条不含 /gitx plan）。
  `overview` 须按 profile 显式声明实现面边界：`research` 含调研期零实现面改动硬声明（仅可选窄例外回写本 .plan.md），`execution` 标明允许按 todos 修改并由末条 `/gitx plan` 收口。
  Use at 每轮对话开始 / first-turn / conversation start when the user wants Cursor Plan mode、Plan 模式、策划文档闸门、可验收 todo、
  或明确要走「计划→实现→验证→对照 git 收口」而不是直接堆代码。
  Aligns execution-item / verification shapes with `skills/SKILL_FRAMEWORK_PROTOCOLS.md`；continuity 分层见 `docs/harness_architecture.md`。
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - Cursor Plan
  - Plan 模式
  - 策划文档闸门
  - 可验收 todo
  - gitx plan 收口
  - 计划对照实际
  - CreatePlan
  - 调研计划
  - 纯调研
  - research-only plan
metadata:
  version: "1.6.0"
  platforms: [codex, cursor]
  tags: [plan, cursor-plan, workflow, gate, closeout]
---

# plan-mode

把「写计划」当成**可闸门、可验收、可对照 Git 收口**的产物，而不是一次性 prose。计划文件建议落在 **`.cursor/plans/`**（Cursor 默认）并在仓库内用 **`docs/plans/`** 做可读镜像或摘要指针，避免只在 IDE 私有目录里丢稿。

## When to use

- 用户要在 **Cursor Plan** / **Plan 模式** 下先把范围、风险、验证路径钉死，再允许大规模改动。
- 用户提到 **策划文档闸门**、**可验收 todo**、要用 **独立上下文 subagent** 审计划初稿。
- 用户明确要走：**计划获批 → 实现 + 测试通过 → 固定用 `/gitx plan` 做计划 vs 实际** 的收口（`/gitx plan` 与 `/gitx` 等价，见 `skills/gitx/SKILL.md`）。
- **每轮对话开始 / first-turn / conversation start**：任务看起来像「先出高质量计划/蓝图」且后续实现依赖该计划的验收标准。
- 用户要 **纯调研 plan**：只产出深度、多角度 **只读**调研 todo，**不包含**实现 / 改代码 / 改配置 / 改测试（见 frontmatter **`plan_profile: research`** 与下文 **调研计划**）。

## Do not use

- 用户只要直接实现、明确禁止前置规划或策划文档。
- 纯 **skill 路由系统 / routing registry / manifest** 治理与 miss repair → `skill-framework-developer`（本 skill 不把框架路由当主触发域）。
- 单一极小改动（单文件几行）且计划成本明显高于收益 → 直接最小 delta + 验证即可。

## Plan profile（`plan_profile`）

与 `name` / `overview` / `todos` **同级**的 frontmatter 字段 **`plan_profile`** 区分计划类型：

| 取值 | 含义 |
|------|------|
| **`execution`**（**缺省**） | 标准实现计划：可含改代码、加测试、改配置等 todo；**CreatePlan 末条**须按 **Git 收口**写 **`/gitx plan`**（见 **CreatePlan 输出契约**）。 |
| **`research`** | **调研专用**：todos **仅**只读调研与结论合成；**禁止**以「实现 / 改行为 / 加测试 / 改 CI」为单条主线；**末条不得**把 **`/gitx plan`** 当作本 profile 的必需验证。 |

若宿主 **CreatePlan** 剥离未知 YAML 键：生成后在本文件 **手动补写** `plan_profile: research`。可选用文件名 **`*.research.plan.md`** 作为人类可读标签；**hook 与契约真源仍以 frontmatter 为准**（本仓库当前 **不**按文件名解析 profile）。

### 两类计划对照表

| 维度 | **`research`** | **`execution`（缺省）** |
|------|----------------|--------------------------|
| **目的** | 只读调研、问题矩阵、合成结论 | 落地实现：改代码 / 配置 / 测试 / 文档 |
| **允许动作（todos）** | 读、`rg` / 检索、对照文档、只读 code review、外部资料只读拉取、结论文字合成 | 写代码、加测试、改配置 / CI / 锁文件、生成产物、迁移与重构 |
| **禁止项** | 以「实现 / 改行为 / 加测试 / 改 CI / 改依赖锁」为单条主线；隐式触达 tracked 实现面资产 | 末条以「只读调研收口」替代 Git 收口；遗漏 `/gitx plan` 写入末条 |
| **末条收口** | 调研合成 + 工作区无意外改动（`git status --porcelain` 为空，或仅本 `.plan.md`，须 overview 已声明该窄例外） | 计划 vs 实际 + **`/gitx plan`**（与 `/gitx` 同契约，见 [`skills/gitx/SKILL.md`](../gitx/SKILL.md)） |
| **下游** | 完成后**另开** `plan_profile: execution`（或缺省）写实现 todos | 通常即为终态；如再分阶段，可拆为多份顺序 `execution` 计划 |

### `research`：overview 必填声明模板

`plan_profile: research` 时，`overview` 必须包含等价于以下语义的不可歧义表述（措辞可微调，语义不可缺）：

- 本文件为**调研计划**；调研执行期**不修改** tracked 的**源码 / 配置 / 测试 / CI 工作流 / 依赖锁文件**等实现面资产。
- 任何**实现 / 改行为 / 加测试**仅出现在**另开的** `plan_profile: execution`（或缺省）计划中。
- **窄例外（可选，须显式声明）**：若执行期需要**仅**回写本 `.plan.md` 以记录结论，须在 `overview` 单句声明该例外，且末条 `Verify` 仍约束 `git status --porcelain` 为空或仅含该路径。
- 若用户明确要求「连 plan 文件也不改」，则**不**声明该例外，结论只留在对话；不得默认隐式改任何其它路径。

**最小模板（可裁剪复制）**：

```text
本文件为调研计划（plan_profile: research）。调研执行期不修改 tracked 源码 / 配置 / 测试 / CI / 依赖锁等实现面资产；后续实现另开 plan_profile: execution（或缺省）计划。
[可选窄例外] 仅允许在末条 Verify 约束内回写本 .plan.md 的调研结论；不触达其它路径。
```

### `execution`：overview 一句式模板

`plan_profile: execution`（或缺省）时，`overview` 须有一句标明**本计划允许**按 todos 修改实现面资产（代码 / 配置 / 测试等），且末条仍以 **`/gitx plan`** 做 Git 收口：

```text
本文件为执行计划（plan_profile: execution / 缺省）。允许按下方 todos 修改代码 / 配置 / 测试等实现面资产；末条以 /gitx plan 对照计划 vs 实际并完成 Git 收口。
```

## 执行计划继承面（research→execution）

当存在**前置** `plan_profile: research` 文档（或等价调研结论文档）时，`execution` `.plan.md` **正文**须在分节 todos **之前**增加固定标题 **`## 执行计划继承面`**（≤15 行），避免执行计划从零复述调研或把外部范例整段搬进 overview。第一性原则与减法规则见 [`docs/plans/RESEARCH_plan_execution_handoff_first_principles.md`](../../docs/plans/RESEARCH_plan_execution_handoff_first_principles.md)。

### 继承面建议字段（每行一项，可写「无 / 不适用」）

| 字段 | 要求 |
|------|------|
| **继承指针** | 一行：`docs/plans/<file>.md#锚` 或 `.cursor/plans/<research>.plan.md`（路径真实存在或可检） |
| **Goal / Non-goals** | 各**一行**，从 research §合成 压缩，禁止长段粘贴 research 正文 |
| **不变量** | 调研已钉死的边界（若无写「无」） |
| **已否决方案** | 每项半行；若无写「无」 |
| **问题矩阵映射** | 每条 P0/P1 级 execution todo 对应至少一个 research 问题 id 或 `open gap`（可在 todo `Done` 内写 `继承: Qn`） |
| **外部准入表** | 若无外部调研写 **`无`**；若有则每行：`URL | 用途 | 本仓库锚点路径 | 采纳或否`；**默认不超过 5 行**，超出则拆第二份 execution 或回到 research |

**与四元组**：`scope` 路径应能从继承指针或矩阵映射追溯到仓库内证据；`Verify` 不得无故弱于 research 已给出的验证类型。

**与 `.cursor/rules/cursor-plan-output.mdc`**：alwaysApply 仍以四元组与末条 **`/gitx plan`** 为硬自检；**不**在该 `.mdc` 内重复展开继承面全文（减法：本节为真源；`cursor-plan-output` 不镜像以免双真源膨胀）。

### `research`：正文建议结构

- **`## 调研问题与结论`**（或等价标题）：每个子问题对应结论文或显式 **`open gap`**（未答须写原因或外部依赖）。
- **`## 证据与范围`**：已读路径、命令/检索摘要、外部引用；避免无来源断言。
- **`## 合成（Synthesis）`**：跨 todo 的一致结论与矛盾消解说明。

### `research`：每条 todo 与末条收口

- **动作**：以读、搜、`rg`、对照文档、只读 code review、外部资料只读拉取为主；**Verify** 须为只读命令或可勾选的人工对照（不得依赖「改文件后跑测试通过」作为唯一手段）。
- **profile 级 Non-goals**：在 `overview` 或每条 todo 写明 **不改**仓库内 tracked **源码 / 配置 / 测试**；若执行中仅更新本 `.plan.md` 以写入结论，须在**末条** `Verify` 中显式允许的路径集合内说明（通常仅 `.cursor/plans/<本文件>.plan.md`）。
- **末条 todo（调研收口）**：`Done when`：§调研问题与结论 逐条有结论文或 open gap；与各前置 todo 结论交叉一致、无自相矛盾。`Verify`：**不**含 **`/gitx plan`**；须含 **`git status --porcelain`** 为空 **或** 输出仅含已声明允许的 plan 路径，并含「对照正文指定节与 YAML `todos` 逐项」的可客观勾选表述。

**末条示例（`research`，按实际文件名替换路径）**：

```text
对照调研问题矩阵与合成结论并完成调研收口 @ .cursor/plans/<本文件>.plan.md
| Done: §调研问题与结论 逐条有结论或 open gap；与前置 todos 无矛盾
| Verify: git status --porcelain 为空或仅列出本文件路径；人工逐项对照 §调研问题与结论 与 frontmatter todos（不得要求 /gitx plan）
```

**下游**：调研 profile 完成后，**另开**一份 **`plan_profile: execution`**（或缺省）计划写实现类 todos 与 **`/gitx plan`** 末条；避免同一文件混用「一半调研一半实现」。新开 execution 时须按上节 **`## 执行计划继承面`** 写入继承指针与准入表（或显式「无」），再写实现 todos。

## 调研范围（Research scope）与能力联动

与 **`plan_profile`**、零实现面声明**并列**：在 `overview` 中用**一句**标明调研是否触网，避免默认静默拉取外部资料或反过来只做网页摘要却未读仓库。

### Overview 可加贴的调研范围句（复制用）

**默认（仅仓库内只读）** — 不默认发起 WebSearch / WebFetch；本地证据以 `rg`、Read、`cargo test`/`clippy`（只读验收）、以及按需 **`router-rs framework snapshot`** / `contract-summary` 等为准：

```text
调研范围：仅仓库内只读（rg/读文件/仓库内命令与连续性工件）；不默认发起对外网络检索。
```

**用户明确要求「外部 / 网络 / 官方 cross-check」等（内部 + 外部并行）** — 仍须保留至少一条**仓库内**检索/读文件类 todo；外部仅限 **只读** 拉取（WebSearch、WebFetch、只读 MCP）；**§证据与范围** 须写清 URL、抓取日期或文档版本：

```text
调研范围：仓库内只读 + 外部只读（与仓库内检索并行）；外部来源须在 §证据与范围 列 URL 与日期/版本；禁止未经批准的网络写操作。
```

### 能力与工件联动表

| 能力 | 适用 profile | 最小证据 | 指针 |
|------|----------------|----------|------|
| 本地代码与配置调研 | `research` / `execution`（起草前） | 路径级 `rg` 命中或 Read 锚点 | 见 **Workflow** 第 1 步 |
| 连续性 / 框架只读视图 | 按需 | `router-rs framework snapshot` 或文档约定命令输出摘要 | `docs/harness_architecture.md`；勿在 plan 正文发明第二套账本 |
| **可选审 plan** | 仅当用户明确要求 review plan / 审计划 | review-only findings（问题、风险、缺失验证），不改代码 | 可落盘 `docs/plans/<topic>_findings*.md` |
| 对抗式 / 全切片 **深度代码审** | 用户要 hostile / security / 整 PR 级 review 时 | review-only verdict + P0–P2 带路径与符号锚点；只找问题，不改代码 | [`skills/code-review-deep/SKILL.md`](../code-review-deep/SKILL.md) |
| Cursor **review** 硬路径（宿主） | 深度 review 类任务 | 以仓库根 **`AGENTS.md`** → **Execution Ladder** 与 **`.cursor/hook-state`** 为准；清门只用宿主注入的 **`router-rs …`** 单行短码 | 不在 plan 正文自拟长段机读块 |
| 调研收口 | `research` | `git status --porcelain` + 正文矩阵对照 | **Plan profile** 末条 |
| Git 计划收口 | `execution`（或下游计划） | **`/gitx plan`**（与 **`/gitx`** 同契约） | [`skills/gitx/SKILL.md`](../gitx/SKILL.md) |

**Build 与 goal（可选）**：若 Build 时带入 `.cursor/plans/*.plan.md` 且需与 **`/autopilot`** goal 门控对齐，见 **Continuity 与工件** 中 **`ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE`**（默认关闭，语义见 **`AGENTS.md`**）。

### 宿主侧计划落盘（与协作）

Cursor 官方说明：计划默认保存在**用户目录**，需 **「Save to workspace」** 才进入工作区以便版本管理与团队共享；内部 todo 与文件不同步等宿主/社区讨论，见 [`docs/plans/plan_writing_capability_research_synthesis.md`](../../docs/plans/plan_writing_capability_research_synthesis.md) §3。

## Workflow（四步）

1. **本地证据先进计划**（见上节 **能力与工件联动表**「本地代码与配置调研」）：在写结构化计划前，完成域内必要的深读、检索或代码定位；计划应收敛已有证据，而不是用计划代替定位结论。
2. **Todo 必须可验收**：每条 todo 在同一条可见文案里写全 **四元组**（见下文 **Todo 可执行性**）；**通过 Cursor CreatePlan 生成的 `.plan.md` 还须满足下文 CreatePlan 输出契约**。与 `skills/SKILL_FRAMEWORK_PROTOCOLS.md` 的 execution item / verification 思想对齐（不必冗长复制 schema）。
3. **可选 review 只找问题**：仅当用户明确要求 review plan / 审计划 / 深度 review 时，review lane 只读计划与证据，输出 findings / risks / missing tests；不改代码、不自动修复。主线程再决定是否把问题转成 plan delta。
4. **收口（依 `plan_profile`）**：
   - **`research`**：完成 **调研合成与问题矩阵收口**（见 **Plan profile** 末条要求）；**不**把 **`/gitx plan`** 作为本 profile 的必需验证。
   - **缺省 / `execution`**：**获批且实现与测试通过后** 用 **`/gitx plan` 固定收口**：按计划 vs 实际逐项对照（scope、验证、未做项的原因），再按 `skills/gitx/SKILL.md` 执行 Git 一条龙收口（别名 **`/gitx plan`** 与 **`/gitx`** 同契约）。CreatePlan 产出的 frontmatter **最后一条** todo 必须将该收口写进可执行项（见 **CreatePlan 输出契约**）。

## Continuity 与工件

- **分层与 hook**：控制面、证据与续跑注入边界以 `docs/harness_architecture.md` 为准；不要在 skill 正文发明第二套账本格式。
- **计划落盘**：权威草稿/链接建议 `.cursor/plans/`；仓库协作或审计需要的摘要可复制或同步到 `docs/plans/`，与仓库内其它计划文档同一叙事。
- **官方 Plan → Build 与 `/autopilot` goal 门控（可选）**：Cursor 无独立 Build hook；若工作区已挂本仓库 **`router-rs` Cursor `beforeSubmit`**，可设 **`ROUTER_RS_CURSOR_PLAN_BUILD_AUTOPILOT_GOAL_GATE=1`**，使 **Build 首条**载荷里出现 **`.cursor/plans/*.plan.md`** 时**视同** **`/autopilot`** 拉起同一套 **`goal_required`** 门控（不自动跑 shell）。需要 Build 即 pre-goal 提示时可再开 **`ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED=1`**。开关语义以仓库根 **`AGENTS.md`**（个人使用）为准。

## Todo 可执行性（四元组、对齐与依赖）

### 必备四元组（每条 todo 自检）

同一条 todo 的叙述里（`todos[].content` 和/或正文对应 checkbox）须能直接看到四项，缺一不可：

| 维度 | 要求 |
|------|------|
| **动作** | 动词 + 对象：改什么行为、加什么断言、写什么段落。**`plan_profile: research`** 下应为「读 / 对照 / 归纳 / 只读审查」等，避免「实现 / 改代码」类表述。 |
| **范围** | 主要路径 1–3 个（文件或目录）；忌「全仓优化」式模糊 owner。 |
| **完成定义（Done when）** | 可勾选、可客观判定（如某 `rg` 范围、某测试名通过、某节标题存在且含关键词）。 |
| **验证手段（Verify）** | 完整命令（含 `--manifest-path` 等若需要）或明确人工步骤（如「打开 `path` §X，30 秒内可定位」）。 |

可选第五行 **Non-goals**：一行写明本步**不**改什么，防止范围 creep。

### 单条模板（复制用）

```text
[id] <动词> <对象> @ <path1>[, <path2>]
Done: <条件1>; <条件2>
Verify: <命令 或 人工检查步骤>
Non-goals: <可选>
```

### 弱例与强例

- **弱**：`优化 registry 双轨`（无范围、无 Done、无 Verify）。
- **强**：`从 RUNTIME_REGISTRY 移除 host_targets.entrypoint_files 并同步 fixture @ configs/framework/RUNTIME_REGISTRY.json, tests/common/mod.rs | Done: rg 在 configs/framework 与 tests 下无该键（例外在 § 单列）| Verify: cargo test --manifest-path scripts/router-rs/Cargo.toml`（仓库以实际约定命令为准）。
- **强（execution 收口与 gitx 习惯对齐）**：末条或关联 closeout 文档中写明 **`git diff --stat`**（或一句「本次无代码 diff」）；`Verify` 在 **`/gitx plan`** 之外可附带 `git status --short --branch`，与 [`skills/gitx/SKILL.md`](../gitx/SKILL.md) 中实质性 diff 记录习惯一致。
- **强（可选审 plan 修订可复核）**：仅当用户明确要求审 plan / review plan 时，对**本计划文件**执行例如 `git diff .cursor/plans/<本计划>.plan.md | head -n 40`（路径按实际替换），或将等价 diff 摘要写入 closeout；避免仅用 `rg Finding` 而看不到计划正文是否已合并修订。
- **强（深度 review 防空壳）**：若 todo 指向深度代码审，`Done` 须要求 P0–P2 中**至少一条**含具体**符号锚点**（函数名/常量名等）；`Verify` 用 `rg` 命中该符号之一。

### YAML 与正文对齐

- **禁止**只在正文 § 写验收、而 frontmatter `todos[].content` 仅写阶段名（如「P0 执行」）：IDE 勾选区会失真。
- **id / 顺序 / 验收标准**在 YAML `todos` 与正文 checklist 之间保持一致；修订计划时两边同步改。

### 拆分与依赖

- **一条 todo ≈ 一个可合并 PR 级结果**（或更小）。出现「且 / 然后 / 另外」时拆成多条。
- **分支 / 多选一**（如 A/B/C）：为每条分支写独立 todo，并标明 **仅当** 某决策成立时执行；依赖另一条时写 **`Blocked by: <todo-id>`** 或等价「先完成 §P0.1」。
- **忌**单条「执行整条 P0 链」：改为链上每一步一条可验证 todo，便于勾选与 **`/gitx plan`**（`execution`）或 **调研矩阵 / 证据**（`research`）对照。

## CreatePlan 输出契约（Cursor）

**适用范围**：宿主通过 **CreatePlan** 新建或更新、落盘为 **`.plan.md`** 的计划（常见路径：工作区 [`.cursor/plans/`](../../.cursor/plans/)；以 Cursor 实际写入为准）。**Skill 路由不会改写磁盘上的 plan 文件**；合规依赖主线程在调用 CreatePlan **之后**对照本节自检，必要时编辑该 `.plan.md` 补齐。

**Profile 分岔**：`plan_profile: research` 时须同时满足 **Plan profile（`plan_profile`）** 与下表 **`research` 列**；**缺省**或 **`execution`** 时满足 **`execution` 列**。

**硬条款**：

0. **`overview` 必含 profile 声明**：
   - **`plan_profile: research`**：`overview` 须含与 **Plan profile** → **`research`：overview 必填声明模板** 等价的「调研期零实现面改动」声明；如声明窄例外（仅回写本 `.plan.md`），同一 `overview` 单句标明，且末条 `Verify` 仍按 `research` 列约束 `git status --porcelain`。
   - **`plan_profile: execution`（缺省）**：`overview` 须有一句标明本计划允许按 todos 修改实现面资产，且末条用 **`/gitx plan`** 收口（见 **Plan profile** → **`execution`：overview 一句式模板**）。

1. **每条** frontmatter `todos[].content` 须在**同一条字符串**内可见 **四元组**（动作、范围 1–3 路径、Done when、Verify），与上文 **Todo 可执行性**一致；禁止「content 只有阶段名、细节全在正文」。
2. **`execution` 正文与前置调研（推荐硬自检）**：若有前置 `plan_profile: research` 或 `docs/plans/` 下调研结论文档，正文须在 todos 前含 **`## 执行计划继承面`**，字段见 **执行计划继承面（research→execution）**；纯冷启动 execution 仍保留该标题且首行可写 **`继承指针：无（冷启动）`**。
3. **`todos` 最后一条**（依 profile）：

| | **`execution`（缺省）** | **`research`** |
|---|-------------------------|------------------|
| **语义** | 计划 vs 实际 + **Git 收口** | **调研合成** + 工作区无意外改动 |
| **`Done when`** | 可客观判定：已对照计划正文与 todos 逐项；未执行项有写明原因或 defer | §调研问题与结论 等逐条有结论文或 **open gap**；与各前置 todo 一致 |
| **`Verify`** | 须显式包含宿主执行 **`/gitx plan`**（与 **`/gitx`** 同契约，见 `skills/gitx/SKILL.md`） | **不得**将 **`/gitx plan`** 作为必需项；须含 **`git status --porcelain`** 约束（见 **Plan profile**）与对照正文 + YAML `todos` 的表述 |

两条 profile 下末条均须含完整四元组（`execution` 动作可写「对照计划与实现并 Git 收口」；`research` 动作可写「对照调研问题矩阵与合成结论并完成调研收口」）。

4. 若正文含 Markdown checkbox 清单：**id / 顺序 / 验收**与 YAML `todos` 对齐。
5. **条件分支**（A/B/C）：每条分支独立 todo + **仅当** / **`Blocked by: <todo-id>`**；禁止单条「执行整条链」替代逐步验收。

**不合规 vs 合规（摘要）**：

- **不合规（`execution`）**：`overview` 不含「允许按 todos 修改实现面 + `/gitx plan` 收口」声明；`content: "实现功能"`；末条无 **`/gitx plan`**。
- **不合规（`research`）**：`overview` 未含调研期零实现面改动硬声明；todo 主线为改代码/加测试；末条 **`Verify`** 仍强制 **`/gitx plan`** 作为唯一收口。
- **合规（`execution` 末条）**：`overview` 已含 execution 一句式声明；`content` 内 `Done:` / `Verify:` 齐全，且 `Verify:` 含 **`/gitx plan`**（可附带 `git status` 等）。
- **合规（`research` 末条）**：`overview` 已含 research 零改动声明（如使用窄例外亦已单句声明）；`Verify:` **不含** **`/gitx plan`**，且含 **`git status --porcelain`** 与正文对照表述（见 **Plan profile** 末条示例）。

## Related

- `skills/SKILL_FRAMEWORK_PROTOCOLS.md` — 讨论 → 规划 → 执行 → 验证 与最小 findings / execution / verification 形状。
- `skills/gitx/SKILL.md` — `/gitx` / `/gitx plan` 收口与深度 review checklist。
- `skills/code-review-deep/SKILL.md` — 对抗式/全切片深度代码审（review-only verdict 与符号锚点习惯与本 skill **强例**对齐）。
- `docs/plans/plan_todo_checklist.md` — Todo 四元组与对齐的勾选短清单（与本节互补）。
- `docs/plans/plan_review_findings_round1.md` — 独立 reviewer 对样例 execution plan 的 findings（可复核 Verify、closeout 与 `--stat` 等）。
- `docs/plans/RESEARCH_plan_execution_handoff_first_principles.md` — research→execution 第一性 / 减法 / 外部准入与继承面理由。
- `.cursor/rules/cursor-plan-output.mdc` — Cursor alwaysApply 下对 CreatePlan 产出的硬自检清单。
