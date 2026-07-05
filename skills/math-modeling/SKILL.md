---
description: Physical/engineering/biological system mathematical modeling — problem formalization, prior art search, governing equations, nondimensionalization, model reduction, verification, regime chart.
metadata:
  platforms:
  - supported
  tags:
  - mathematics
  - modeling
  - physics
  - nondimensionalization
  - governing-equations
  - dimensional-analysis
  - regime-chart
  - model-specification
  - literature-search
  version: '1.0.0'
name: math-modeling
scene: research
sub_scene: execution
risk: medium
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: 物理/工程/生物系统的数学建模全流程——问题形式化、先验调研、控制方程构建、无量纲化、模型简化求解、验证与输出
trigger_hints:
- 建数学模型
- 控制方程
- 量纲分析
- 无量纲化
- 物理建模
- 本构方程
- 方程闭合
- regime chart
- 尺度分析
- 变量归一化
- 特征尺度
- 物理系统建模
- 工程系统建模
- 生物系统建模
- 数学模型设计
- 建模全流程
- 对控制方程做无量纲化
- 推导控制方程
- 构建偏微分方程模型
- 常微分方程组建模
- 量纲齐次性
- 模型简化
- 主导平衡
- 渐近匹配
- PDE建模
- ODE建模
- 输运方程建模
- 生物数学模型
- 化学反应动力学建模
- 系统建模
- 动力学建模
- 参数化建模
- 数学建模
- 建模型
- 推导方程
- math-modeling
---

# Math Modeling

物理/工程/生物系统的数学建模全流程。从问题的物理解读到数学模型的定量输出，贯穿
先验调研、控制方程构建、无量纲化/尺度分析、模型简化求解、验证等全链条。
**允许外部文献探索和 prior art 查证。**

**定位**：L4 执行 skill，执行建模工作流各阶段，可调用外部探索工具做先验调研。
**层级关系**：`$research` (L2) → execution lane → `$math-modeling` (L4)

**风险说明**：本 skill 的输出可用于工程设计、论文发表、科研决策等场景。
错误模型可能导致工程失败或方向错误。所有输出必须标注适用范围，不得声称超出验证范围的结论。

## 与相邻技能的边界

| Skill | 与本 skill 分工 |
|-------|----------------|
| `math-verify` | 对抗式审查已有推导的正确性；本 skill (E) 阶段完成**自检**后，若需要**独立审计**（正式用途），送 `$math-verify` |
| `math-explore` | 发现新数学性质；建模中遇到**未知数学性质**时委托探索 |
| `statistical-analysis` | 数据分析/统计建模；本 skill 做**确定性/连续型物理建模** |
| `formal-verification` | 无状态形式验证退出门；本 skill 的 (E) 验证阶段调用 |

**边界说明**：
- **(E) 是自检，`$math-verify` 是独立审计**：本 skill 自身完成 7 项验证检查。`$math-verify` 的对抗式审查仅当模型将用于正式用途（论文、工程设计、可复现研究）时送审，且可引用 (E) 的结果作为初始检查，避免重复工具调用。
- **建模中遇到未知数学性质 → `$math-explore`**：如果你在建模中需要**理解某个符号表达式的数学结构**（而非构建物理方程），调用 `$math-explore`。
- **验证已有推导的正确性 → `$math-verify`**：如果你已经有一个完整的推导，只需要审查其正确性，不涉及建模流程的其他阶段。

## When to use

- 用户需要从物理/工程问题出发构建数学模型
- 用户需要为一组耦合物理过程推导控制方程
- 用户有控制方程需要做无量纲化和主导平衡分析
- 用户需要评估不同 regime 下哪些项占优、可忽略
- 用户需要将 PDE/ODE 模型简化为可解析/可数值求解的形式
- 用户需要检查数学模型的量纲一致性和闭合性
- 用户需要查阅 prior art 来验证建模假设的合理性
- 适合如下的请求：
  - "对一个对流-扩散系统建 PDE 模型"
  - "推导这个流固耦合问题的控制方程"
  - "对这个边界层方程做无量纲化和尺度分析"
  - "检查生成的模型是否量纲闭合"
  - "为我构建一个 SIR 模型并做 regime 分析"
  - "建一个 Lotka-Volterra 捕食模型"
  - "对这个系统推导主导方程并做尺度分析"
  - "用 Buckingham Pi 定理做量纲分析"

## Do not use

- 用户只需要审查已有推导的正确性（→ `$math-verify`）
- 用户只需要纯数学性质探索（→ `$math-explore`）
- 用户需要统计分析/数据驱动建模（→ `$statistical-analysis`）
- 用户不需要定量模型，仅需概念定性讨论（→ 当前上下文内联处理）
- 离散/组合/逻辑类系统建模（→ 当前上下文内联处理）
- 纯 ML/AI 模型设计（→ 当前上下文内联处理）

## Core workflow

```
输入物理/工程/生物系统描述
    │
    ├── (S) 问题形式化
    │   Gate: modeling brief 完成 + 变量清单完整 → 可选入 (A)
    │   (若信息不足 → loop-back: 追问用户)
    │
    ├── (A) 先验调研 — 外部探索（MUST，不可跳过）
    │   Gate: prior art assessment 完成（含 [FOUND] 或 [NO_PRIOR_ART_FOUND] 标记）
    │   (若搜索无结果 → 三段式 fallback: 精确→宽泛→教科书)
    │   (若发现问题不完整 → loop-back to (S))
    │
    ├── (B) 控制方程构建
    │   Gate: 方程闭合 + 量纲一致
    │   (若未闭合 → 必须标注缺口方程，再判定是否可进入 (C))
    │
    ├── (C) 无量纲化/尺度分析
    │   Gate: regime chart 完成，至少一个 regime 的主导平衡确定
    │   (多 regime 分支: 每个 regime 独立走 (D)→(E)→对比→输出)
    │
    ├── (D) 模型简化求解
    │   Gate: 简化模型及解完成
    │   (若无法解析求解 → [数值求解分支] 或 back to (C) 换 regime)
    │
    ├── (E) 模型验证
    │   Gate: 验证报告完成，P0/P1 blocker 全部解决
    │   (若 FAIL → loop-back: (C) 重新选尺度 / (D) 换策略 / (B) 修改方程)
    │   (最大迭代: 3 轮 → 标注 [MAX_ITERATION_REACHED])
    │
    └── (F) 输出
        产出：model specification + regime chart + model card
```

### 循环与分支规则

1. **loop-back 上限**：任何循环路径最多执行 3 轮。3 轮后标注 `[MAX_ITERATION_REACHED — unresolved: <列表>]`，输出当前最佳结果并注明未解决项。
2. **多 regime 分支**：(C) 产出多个 regime 时，对每个 regime 独立走 (D) 简化 → (E) 验证 → 对比各 regime 的解 → (F) 合并输出时标注各 regime 的适用条件。
3. **数值求解分支**：(D) 无法解析求解时，生成 `[SYMBOLIC_SOLUTION_NOT_FOUND]` 标注，简述数值策略（如有限差分/有限元/谱方法），**不在本 skill 内写数值代码**，返回可委托给 coding context。

## 阶段详解

### (S) 问题形式化

**产出**：modeling brief — 不超过 1 页的结构化描述

- 系统物理描述（1-2 句）
- 域：空间维度、时间依赖（稳态/瞬态）
- 变量清单：因变量、自变量、参数（标注已知/待求、各变量物理含义）
- 物理机制清单：哪些过程参与（对流/扩散/反应/辐射/弹性/塑性/…）
- 已知/假设：对称性假设、材料属性假设、边界条件来源
- 用户目标：解析解 / 数值解 / regime map / 量级估计 / 定性分析

> 如果用户描述不充分，(S) 阶段应追问关键物理细节。如果完全开放，在 (A) 先验调研过程中与用户迭代补充。

**→ (A) Gate**: modeling brief 完成且变量清单完整。如果信息不足以构建 brief，返回追问用户。

### (A) 先验调研 — 外部文献探索（MUST，不可跳过）

**为什么不可跳过**：数学建模的一大常见陷阱是"自创模型忽略已有成果"。
本 step 强制在动手建模前先查 prior art。

#### 两种工作模式

根据建模任务的上下文选择模式：

1. **Discovery 链模式**（推荐）：任务经 `$research` discovery lane 调研后到达本 skill。此时 (A) 简化为引用已有 prior art assessment，仅做遗漏方向的 gap 补充搜索。
2. **直接入口模式**：用户直接要求"建模"、"建模型"等，未经 discovery lane。此时 (A) 承担完整的 prior art 搜索任务，产出完整的 prior art assessment（与 discovery lane 等价）。

#### 调研内容（按优先级排序）

1. 搜索本系统/类似系统的已有模型
2. 搜索本领域的经典控制方程形式和建模约定
3. 搜索 parameter regime 区间和流行的无量纲化方案
4. 搜索已知的 regime map / phase diagram
5. 搜索已有解析解或渐近解（用于后续验证）

#### 领域搜索 query 模板

按用户描述的物理过程类型选择对应的搜索策略：

| 物理过程 | 搜索关键词模式 | 预期方程类型 | 推荐源 |
|----------|---------------|-------------|--------|
| 对流 + 扩散 | `"advection-diffusion equation" governing equation` | 对流-扩散 PDE | arXiv / Wikipedia |
| 流体流动（不可压） | `"Navier-Stokes equations" incompressible model` | N-S 方程 | Wikipedia / arXiv |
| 流体流动（可压） | `"Euler equations" compressible flow` | Euler 方程 | Wikipedia / arXiv |
| 边界层流动 | `"boundary layer" Prandtl equation` | 边界层方程 | Wikipedia / arXiv |
| 传热 | `"convective heat transfer" governing equation energy` | 能量方程 | Wikipedia / arXiv |
| 传热 + 辐射 | `"radiative heat transfer" equation Stefan-Boltzmann` | 辐射传热方程 | Wikipedia |
| 多孔介质流动 | `"Darcy flow" porous media governing equation` | Darcy 定律 | Wikipedia / arXiv |
| 化学反应动力学 | `"Arrhenius kinetics" rate equation combustion` | Arrhenius / 速率方程 | Wikipedia / Semantic Scholar |
| 反应 + 扩散 | `"reaction-diffusion system" Turing pattern` | Turing / Fisher-KPP | arXiv / Semantic Scholar |
| 种群动力学（双物种） | `"Lotka-Volterra" competition model predator-prey` | Lotka-Volterra 系统 | Wikipedia / Semantic Scholar |
| 传染病传播 | `"SIR model" "compartmental model" epidemiology` | SIR / SEIR 系统 | Wikipedia / arXiv |
| 弹性/固体力学 | `"elasticity" governing equation "Cauchy momentum"` | 弹性方程 / Cauchy | Wikipedia / arXiv |
| 电动/电磁场 | `"Maxwell equations" electrodynamics` | Maxwell 方程组 | Wikipedia |
| 量子系统 | `"Schrödinger equation" quantum` | Schrödinger 方程 | Wikipedia |
| 无量纲数查找 | `"Reynolds number" friction factor` | Re / Pe / Nu / Da 等 | Wikipedia |
| 已知 regime map | `"flow regime map" "phase diagram" transport` | 流态图 / 相图 | Wikipedia / arXiv |

以上模板为参考。实际搜索时应根据具体系统的参数范围、边界条件、材料属性等定制关键词。

#### 三段式 fallback 搜索链

当搜索结果不足时，按以下优先级逐级降级：

```
第一级（精确搜索）: arXiv API / Semantic Scholar 论文搜索
  ├── research_literature_search(query="exact system + equation type")
  ├── paperplain__search_research(query="exact system + equation type")
  └── 有结果 → 记录引用 ✅
      无结果 → 进入第二级

第二级（宽泛搜索）: Wikipedia API + 同义术语
  ├── WebFetch("https://en.wikipedia.org/w/api.php?action=query&prop=extracts&exintro&explaintext&titles=<KnownEquationName>&format=json&formatversion=2")
  ├── WebSearch(query="同义术语 / 更宽泛的系统类别 equations")
  └── 有结果 → 记录引用，标注 [GENERAL_SOURCE] ✅
      无结果 → 进入第三级

第三级（教科书级）: 通用 Web 搜索
  ├── WebSearch(query="textbook " + system + " governing equation fundamentals")
  ├── WebFetch("https://www.searchonmath.com/search?q=<LaTeX片段>")
  └── 有结果 → 记录，标注 [TEXTBOOK_LEVEL] ✅
      无结果 → [NO_PRIOR_ART_FOUND — search_scope: 列举搜索范围、源、查询式]
```

**当先验调研发现已有模型时**：
- 评估已有模型的适用性：参数范围是否匹配、假设是否满足。
  - ✅ 条件符合 → 采用已有模型，在本 skill 内参数化修改和扩展
  - ⚠️ 部分符合 → 标注差异点，在 (B) 阶段基于已有方程修改
  - ❌ 不符合 → 进入 (B) 从头构建

**当先验调研未发现结果时**：
- 标注 `[NO_PRIOR_ART_FOUND — search_scope: ...]`，说明所有三级搜索的范围和局限性。
- 进入 (B) 阶段从头构建。

#### 可用工具

| 工具 | 目的 |
|------|------|
| `research_literature_search` | arXiv/Semantic Scholar 跨源文献搜索 |
| `mcp__paperplain__search_research` | PubMed/ArXiv/S2 多源论文搜索 |
| `mcp__paperplain__find_paper_by_title` | 找到已知名称的论文 |
| `mcp__paperplain__fetch_paper` | 获取特定论文的摘要和元数据 |
| `WebSearch` | 通用 Web 搜索（tutorials/textbooks/reports） |
| `WebFetch` | URL 内容抓取（Wikipedia API / arXiv API / Wolfram Alpha public） |

#### 输出规范

```text
## Prior Art Assessment
### 检索引擎与查询
- 第一级: arXiv / Semantic Scholar — query="..." — [HITS / NO_HITS]
- 第二级: Wikipedia API — 页面="..." — [HITS / NO_HITS]
- 第三级: Web / SearchOnMath — [HITS / NO_HITS]

### 已存在模型
- ✅ [模型/方程名称] — DOI/arXiv: ... — 适用条件: ... — 来源级别: [L1/L2/L3]
- ❌ 未找到匹配本系统参数范围的现有模型

### 建模约定
- 典型无量纲化方案：...
- 本领域常用本构关系：...

### 已知 Regime
- 无量纲数 1: [Re/Pe/Da/...] 典型量级范围
- Regime 边界：...
```

**→ (B) Gate**: prior art assessment 完成，含 `[FOUND]` 或 `[NO_PRIOR_ART_FOUND]` 标记。如果搜索范围不明或遗漏关键领域，无标记不可进入 (B)。

### (B) 控制方程构建

列出所有控制方程。从物理守恒律出发：

1. 确定每个因变量对应的守恒/控制方程
2. 写出微分形式
3. 写出本构关系（应力-应变/热流-Fourier/扩散-Fick/反应-Arrhenius/etc.）
4. 写出初边值条件（IC/BC）
5. 方程数 vs 未知数计数 → 闭合性检查
6. **量纲一致性检查（MUST）**

**方程闭合性检查**：

```text
方程数: N_eq
未知数: N_unk
闭合状态: CLOSED / UNDERDETERMINED(N_unk - N_eq extra) / OVERDETERMINED(deficit)
缺口分析: (若未闭合) 需要补充的关系式
```

**量纲一致性检查**：

| 检查类型 | 工具 | 通过条件 |
|----------|------|---------|
| 每项量纲 | `math_sympy_dimension_propagate` | 各项量纲矩阵一致 |
| 整体方程 | `research_verification_formal(check="dimensional")` | LHS 量纲 = RHS 量纲 |

**工具支持**：

| 操作 | 工具 | 阶段 |
|------|------|------|
| 符号化简 | `math_sympy_simplify` | 方程整理 |
| 展开/因式分解 | `math_sympy_expand` / `math_sympy_factor` | 结构分析 |
| 量纲传播 | `math_sympy_dimension_propagate` | 量纲一致性 |
| 形式量纲验证 | `research_verification_formal(check="dimensional")` | 量纲验证 |
| 方程求解 | `math_sympy_solve` | 求解已建方程 |
| 证明追踪 | `math_proof_trace_record` | 推导步骤记录 |

**→ (C) Gate**: 方程闭合且量纲一致。如未闭合但有明确的缺口方程（`UNDERDETERMINED` 且有补救计划），可标注后进入 (C) 但必须在 (E) 验证回补。如完全不可闭合（`OVERDETERMINED` 且矛盾），必须回到 (B) 修正。

### (C) 无量纲化/尺度分析

这是数学建模中"最毛糙也最关键"的一步。核心是选择正确的特征尺度。

**特征尺度选择过程**：

1. 列出所有独立物理量
2. 选择独立基量纲（时间、长度、质量、温度等）
3. 对每个变量构造特征尺度（例如 `x* = x/L`, `t* = tU/L`）
4. 代入方程 → 各项前出现无量纲数

**每个无量纲数的物理含义**：

```text
无量纲数: Re = UL/ν
含义: 惯性力 / 粘性力
典型量级: [本问题中 Re ~ 10^-3]
物理判断: 粘性主导，惯性可忽略
```

**工具支持**：

| 操作 | 工具 |
|------|------|
| 变量替换 | `math_sympy_subs`（将 x → L·x*, t → T·t*） |
| 化简无量纲形式 | `math_sympy_simplify` |
| 量纲检查新方程 | `math_sympy_dimension_propagate` |
| 各无量纲数前因子对比 | `math_asymptotic_estimate` |
| 各 regime 下主导平衡 | `math_sympy_series` / `math_tighten_bounds` |
| 符号→数值 | `math_sympy_lambdify` |
| 渐近关系链 | `math_asymptotic_chain` |

**→ (D) Gate**: regime chart 完成，至少一个 regime 的主导平衡确定。如果多个 regime 成立，对每个 regime 独立执行 (D)→(E)→对比→合并输出。

### (D) 模型简化求解

在 (C) 确定了主导平衡后（如 Re ≪ 1 下忽略惯性项），对简化后的方程求解。

**简化策略**：

| 策略 | 条件 | 工具 |
|------|------|------|
| 渐近展开 | 小参数 ≪ 1 | `math_sympy_series` / `math_asymptotic_estimate` |
| 主导平衡忽略 | 项级对比明确 | `math_sympy_simplify` |
| 对称降维 | 自相似/行波对称性 | 手动推导 + `math_sympy_solve` |
| 匹配渐近 | 边界层/内层结构 | 手动推导 + `math_asymptotic_chain` |
| 同态检测 | 寻找保持结构的变换 | `math_check_homomorphism` |
| 极限退化 | 参数极限行为 | `math_sympy_limit` |

求解后对简化后的方程调用 `math_sympy_solve` 求通解 + 应用边界条件。如果无法解析求解：

```
[SYMBOLIC_SOLUTION_NOT_FOUND]
├── 简述已尝试的求解策略
├── 建议数值策略（有限差分/有限元/谱方法/…）
├── 模型产出中标注 "仅符号推导，未完整求解"
└── 可选：back to (C) 换一个 regime 尝试解析求解
```

**→ (E) Gate**: 简化模型及解完成。如果无法解析求解，必须有 `[SYMBOLIC_SOLUTION_NOT_FOUND]` 标注才能进入 (E)。

### (E) 模型验证

**验证清单**（不可跳过）：

| # | 验证项 | 验证对象来源 | 工具 | PASS 条件 |
|---|--------|-------------|------|----------|
| 1 | 量纲闭合重检 | (B) 控制方程 + (C) 无量纲化方程 | `math_sympy_dimension_propagate` + `research_verification_formal(check="dimensional")` | 所有项量纲一致 |
| 2 | 退化极限验证 | (D) 简化模型 | `math_sympy_subs`（极端参数代入）+ `math_sympy_simplify` | 简化模型退化结果 ≈ 已知退化结果 |
| 3 | 渐近一致性 | (D) 解与 (C) 渐近估计 | `math_asymptotic_chain` | 两模型在匹配区域渐近一致 |
| 4 | 数值 witness | (D) 解的关键参数点 | `math_sympy_lambdify` → `math_witness_consistency` | 代表性参数点通过验证 |
| 5 | 不等式约束 | (D) 解 | `math_prove_inequality` | 解满足所有物理约束 |
| 6 | 恒等式链 | (B)→(C)→(D) 推导 | `math_identity_chain` | 简化推导链无断裂 |
| 7 | 自动证明辅助 | 关键命题 | `math_auto_prove` | SMT 自动证明关键命题 |

**验证报告模板**：

```text
## 验证报告
### 量纲一致性
- 原始方程: PASS（各项量纲 [L^2 T^-1]）
- 无量纲方程: PASS（全无量纲）

### 退化极限
- Re → 0: 方程退化为 Stokes 流 → 已知 Stokes 方程匹配 ✅
- Da → 0: 方程退化为瞬态扩散 → 已知扩散方程匹配 ✅

### 渐近一致性
- 匹配层渐近链: PASS（内层解与外层解在 ε^0 阶匹配）

### 数值 Witness
- 测试点: Re=0.01, Da=0.1, x*=0.5, t*=1.0
- LHS = RHS（残差 < 10^-10）✅

### 物理约束
- 温度始终 > 0: PROVED（math_prove_inequality）✅
- 密度非负: PROVED ✅
```

**→ (F) Gate**: 验证报告完成，P0/P1 blocker 全部解决。

如果验证 FAIL：
- 量纲不一致 → back to (B) 修正方程 ⚠️
- 退化极限不匹配 → back to (C) 重新选尺度 ⚠️
- 渐近不一致 → back to (D) 换简化策略 ⚠️
- 数值 witness 失败 → back to (D) 检查解或 back to (C) 换 regime ⚠️
- **循环上限**: 3 轮后标注 `[MAX_ITERATION_REACHED — unresolved: <列表>]`

### (F) 输出

#### 完整输出模板

各 section 标注 `[REQUIRED]` / `[OPTIONAL]` / `[IF_APPLICABLE]`。
任何不适用 section 用 `[N/A: <原因>]` 标记。

```text
## Modeling Brief [REQUIRED]
**系统**: ...
**域**: [空间维度] × [时间依赖]
**变量**: ...
**物理机制**: ...

## Prior Art Assessment [REQUIRED]
**检索源**: arXiv / Semantic Scholar / Wikipedia / Web
**检索链**: L1(L2(L3(...))) — 或 — L1 only (direct hit)
**结果**: [已有模型] / [NO_PRIOR_ART_FOUND]

## Governing Equations [REQUIRED]
### 控制方程系统
$$ ... $$
### 本构关系
$$ ... $$
### 初边值条件
$$ ... $$
### 闭合性
- 方程数: N, 未知数: M → CLOSED / UNDERDETERMINED

## 无量纲化 [REQUIRED]
### 特征尺度选择
| 变量 | 特征尺度 | 含义 |
|------|---------|------|
| x | L | 系统长度 |
| t | L/U | 对流时间尺度 |

### 无量纲数
| 符号 | 表达式 | 量级 | 含义 |
|------|--------|------|------|
| Re | ρUL/μ | 10^-3 | 惯性/粘性 |

### Regime Chart [REQUIRED]
Regime 1（Pe ≪ 1，扩散主导）：∂_t T* = ∇*² T*
Regime 2（Pe ≫ 1，对流主导）：∂_t T* + u*·∇* T* = 0

## 简化模型 [IF_APPLICABLE]
**选择 Regime**: ...
**简化方程**: ...
**解**: ...
[SYMBOLIC_SOLUTION_NOT_FOUND]（如适用）

## 验证报告 [REQUIRED]
[见上文验证报告模板]

## Model Card [REQUIRED]
**适用条件**: ...
**关键无量纲数**: ...
**局限性与假设**: ...
**迭代轮次**: 1 / 2 / 3 [MAX_ITERATION_REACHED: ...]
**源参考**: [prior art DOI / 推导说明]
```

## Quality requirements

> [!CAUTION]
> 以下要求分为 `[GATE]`（可由工具自动验证）和 `[GUIDELINE]`（依赖 Agent 行为规范）。
> 每个建模输出应声明其遵守状态。

1. **[GATE] (A) 先验调研不可跳过**：每个建模任务必须先做 prior art assessment，标注搜索范围和结论。禁止从零构建模型而不检查已有成果。

2. **[GATE] 量纲一致性检查和闭合性检查不可跳过**：每个控制方程系统必须用 `math_sympy_dimension_propagate` 和闭合性计数检查。未闭合的方程系统必须明确指出缺口方程。

3. **[GUIDELINE] 数值证据 ≠ 模型有效性**：只做了数值验证的说"在数值测试条件下通过"而非"已验证"。

4. **[GUIDELINE] Regime 边界必须量化**：每个 regime 的适用条件须明确对应无量纲数的量级范围（如 "Pe < 0.1 时" 而非 "Pe 小时"）。

5. **[GUIDELINE] 每个简化必须标注假设**：从 (C) 到 (D) 的每一步简化必须记录"为什么可以忽略此项"（基于哪个无量纲数的大小判断），不得静默忽略。

6. **[GATE] 退化极限验证**：每个简化模型至少在一个已知极限下应退化到已知结果（用 `math_sympy_subs` + `math_sympy_simplify` 验证）。如果找不到已知退化，标注 `[NO_DEGENERATION_CHECK]`。

7. **[GUIDELINE] 模型输出必须标注适用范围**：完整列出所有隐含假设、参数范围限制、和已知的失败条件。

8. **[GUIDELINE] 文献检索透明**：标注检索源、检索式和检索范围。`[NO_PRIOR_ART_FOUND]` 不保证不存在——仅表示在检索范围内未命中。

9. **[GUIDELINE] 后端不可用标注**：当 SymPy/Z3 不可用导致建模步骤降级时，标注 `[BACKEND_LIMITED]`。

10. **[GUIDELINE] 模型与代码分离**：建模产出是数学文档（LaTeX + 描述性推理），不包含数值代码。数值实现应在建模完成后另做。

## Lifecycle integration

```text
$research (L2) → execution lane
    │
    ├── "数学建模", "控制方程" 等 → $math-modeling (L4)
    │
    ├── 两种入口模式:
    │   ├── Discovery 链模式: 先经 discovery lane 调研
    │   │   → (A) 简化为引用已有 prior art, 仅 gap 补充搜索
    │   └── 直接入口: 用户直接建模
    │       → (A) 承担完整 prior art 搜索
    │
    ├── 建模产出 (model_spec)
    │   │
    │   ├── 需要独立审计（正式用途） → $math-verify（对抗式审查）
    │   ├── 需要探索数学性质 → $math-explore
    │   ├── 需要形式验证退出门 → $formal-verification
    │   └── 需要 prior art 更新 → loop-back to (A) 先验调研
    │
    └── 完成 → handoff back to execution lane
        handoff 上下文:
          model_spec: <建模规约>
          verification_report: <验证结论>
          unresolved_items: <未解决项列表>
          iteration_count: <迭代轮次>
```

## References

- [`../math-verify/SKILL.md`](../math-verify/SKILL.md) — 数学验证 skill
- [`../math-explore/SKILL.md`](../math-explore/SKILL.md) — 数学探索 skill
- [`../../quality-gates/formal-verification/SKILL.md`](../../quality-gates/formal-verification/SKILL.md) — Quality Gate 形式验证
- [`../research/SKILL.md`](../research/SKILL.md) — 科研统一前门
- [`../research/lanes/execution.md`](../research/lanes/execution.md) — Execution lane（入口）
- [`../../docs/math-reasoning-harness.md`](../../docs/math-reasoning-harness.md) — 数学验证工具链文档
