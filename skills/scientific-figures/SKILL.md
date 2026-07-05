---
description: 科研画图统一入口 — 数据出版级图表（matplotlib/seaborn）和 TikZ/LaTeX 示意图双 lane。入口根据输入自动分 lanes。
metadata:
  platforms:
  - supported
  tags:
  - figure
  - data-viz
  - tikz
  - latex
  - publication
  version: '2.0.0'
name: scientific-figures
scene: research
risk: low
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: n/a
short_description: 科研画图统一入口 — 数据出版级图表 + TikZ/LaTeX 示意图双 lane
trigger_hints:
- 科研图表
- 数据可视化
- 论文配图
- 出版级图表
- tikz
- latex figure
- 示意图
- 架构图
- 流程图
- 方法图
- pipeline 图
- 论文插图
- 期刊配图
- 画图
- 画个图
- 科研画图
- scientific-figures
---

# scientific-figures — 科研画图统一入口

本 skill 是科研画图的统一入口，包含两个子 lane：

| Lane | 适用场景 | 推荐 |
|------|---------|------|
| **data-plot** | 数值型数据 → 出版级数据图表 | 折线/柱状/散点/箱线/热力图/分布图/多面板 |
| **tikz-schematic** | AI 截图/草图/示意图 → TikZ/LaTeX 矢量图 | 方法图/架构图/流程图/pipeline |

## 子 lane：data-plot（数据出版级图表）

来自原 `scipilot-figure` skill。

- 输入：数值型数据（CSV/DataFrame/数组）
- 输出：matplotlib/seaborn/plotnine 出版级图表源码
- 能力：EDA → 图型推荐 → 期刊适配 → 色盲安全 → 视觉自检
- **不做**：流程图、架构图、AI 截图转 TikZ

详细指南见 [`../scipilot-figure/SKILL.md`](../scipilot-figure/SKILL.md)。

## 子 lane：tikz-schematic（TikZ/LaTeX 示意图）

来自原 `tikz-paper-figure` skill。

- 输入：AI 生成图、截图、草图、粗略视觉稿
- 输出：TikZ/LaTeX standalone 可编译源文件
- 能力：方法图/架构图/pipeline/协议流程/序列式交互/几何说明/算法图
- **不做**：Mermaid/Graphviz/DOT 图、matplotlib/seaborn 数据图表

详细指南见 [`../tikz-paper-figure/SKILL.md`](../tikz-paper-figure/SKILL.md)。

## Do not use

- 用户需要流程图/时序图（Mermaid/Graphviz）→ 用 `diagramming` 或内置工具
- 用户需要论文的可见证据审查（已存在截图）→ 用 `$visual-review`
- 用户需要 UI/网站设计原型 → 用 `$hallmark` 或 `$huashu-design`
- 用户需要设计 token/design contract → 用 `$design-md`
- 非科研类视觉设计（品牌/营销/PPT）→ 用 `$hallmark` / `$huashu-design`

## 路由分发

检测输入中的意图信号，分发到对应 lane：

| 信号关键词 | Lane |
|-----------|------|
| 数据、csv、matplotlib、seaborn、折线、柱状、散点、箱线、热力图、分布图、多面板 | **data-plot** |
| tikz、standalone、方法图、架构图、流程图、pipeline、截图转、screenshot to、AI image to | **tikz-schematic** |
| 不确定 | 输出简短路由菜单 |
