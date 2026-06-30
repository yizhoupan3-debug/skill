# research-discovery 中长尾 trigger hints (按 lane 分桶)

> Front door (`SKILL.md` frontmatter `trigger_hints`) 只保留高信号词；
> 下列 hints 按 `lane=` 分桶供路由扩展层使用。
>
> 主参考：当上层 router 看到 `lane=math_*` 时，**建议直接转发 `skills/math-derivation/`**，
> 避免本 skill 与 `math-derivation` 抢路由；`lane=research_question` 留作
> `research-discovery` 自身的扩展词集。

## lane=research_question (17 hints)

> 科研项目推进 / 主题深挖 / 寻找理论工具 / 类比等纯 research 问题类。
> 这些是 frontmatter 之外的研究方向入口，路由到 `research-discovery` 主流程。

- 科研项目推进
- 深度调研这个科研方向
- 学术深调研
- research workbench
- 数学背景
- 理论背景
- 用什么数学
- 该用什么理论
- 相关定理
- 找定理
- 有没有定理
- 类比
- 未知性质
- 性质不清楚
- 非手稿科研

## lane=math_background_inquiry (12 hints, 与 math-derivation 抢路由)

> 数学背景 / 理论地图 / 类比 / 英文信号等与 `math-derivation` 重叠的子集。
> **建议：路由层见 `lane=math_*` 前缀直接转发 `skills/math-derivation/`，本 skill 仅保留 fallback**。
> 前缀形式：`lane=math_background_inquiry` / `lane=math_theory_map` 等由路由层分发。

- 控制方程
- 本构方程
- 量纲分析
- 无量纲化
- 跨领域类比
- 数学结构
- 理论地图
- math background
- theory landscape
- related theorems
- mathematical modeling
- governing equation

## 维护说明

- frontmatter `trigger_hints` 是**主入口**（高信号、易误触发的反向避免过严）。
- `trigger_hints_long: references/trigger-hints-long.md` 是**长尾扩展**（按 lane 二次分发）。
- 路由层实现面建议参考 `skills/SKILL_ROUTING_RUNTIME.json` + `configs/framework/RUNTIME_REGISTRY.json`
  的 `lane` 字段；本文件本身是文档，不改 routing registry。
- 数学相关 12 个 `lane=math_background_inquiry` 与 `skills/math-derivation/SKILL.md` 的 frontmatter 高度重叠，
  路由侧应优先 `math-derivation`，本文件保留为兜底参考。
