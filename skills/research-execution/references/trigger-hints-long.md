# research-execution 中长尾 trigger hints (按 lane 分桶)

> Front door (`SKILL.md` frontmatter `trigger_hints`) 只保留 22 个高信号词；
> 下列中长尾 hints 按 `lane=` 分桶供路由扩展层使用。
>
> 本文件聚焦 **execution lanes**：`experiment_design`、`math_verification`、`math_modeling`。
> Discovery lanes (`research_question`, `external_research`, `math_background_inquiry`)
> 的 hints 位于 `research-discovery/references/trigger-hints-long.md`。

## lane=experiment_design (8 hints)

> 实验方案 / 方法核查 / ablation & benchmark 设计等纯实验设计类。
> 路由到 `research-execution` 的 experiment-design 子流程。

- 设计实验
- 实验方案设计
- 方法正确性核查
- 代码和数学联合核查
- 推导方法正确性
- 研究路线设计
- ablation 方案
- benchmark 方案

## lane=math_verification (4 hints)

> 推导正确性 / 假设检查 / 定理依赖 / checker 选项。
> 路由到 `research-execution` 的 math-verification 子流程。

- 推导方法正确性
- 假设检查
- 定理依赖分析
- checker 选项

## lane=math_modeling (10 hints)

> 数学建模 / 量纲 / 控制方程 / 本构方程 / 无量纲化 / 方程搭建。
> 路由到 `research-execution` 的 math-modeling 子流程。

- 控制方程
- 本构方程
- 量纲分析
- 无量纲化
- 数学结构
- 数学建模
- 模型搭建
- governing equation
- mathematical modeling
- modeling

## 维护说明

- frontmatter `trigger_hints` 22 词是**主入口**（高信号、易误触发的反向避免过严）。
- `trigger_hints_long: references/trigger-hints-long.md` 是**长尾扩展**（按 lane 二次分发）。
- `math_verification` 和 `math_modeling` 的 hints 与 `skills/math-derivation/SKILL.md`
  存在重叠；路由侧应优先 `math-derivation` 做纯推导任务，本文件保留为建模/验证场景的入口。
- `lane=research_question`、`lane=math_background_inquiry` 等 discovery 类 hints
  位于 `research-discovery/references/trigger-hints-long.md`，不在本文件中重复。
- 路由层实现面建议参考 `skills/SKILL_ROUTING_RUNTIME.json` + `configs/framework/RUNTIME_REGISTRY.json`
  的 `lane` 字段；本文件本身是文档，不改 routing registry。
