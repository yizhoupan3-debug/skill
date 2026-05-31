# Academic Presentation Design System (ACADEMIC_DESIGN.md)

This design system is extracted directly from the "Statistical Computation & Software Group 7 Project" presentation. It codifies the visual guidelines for all upcoming academic presentation slide decks.

```yaml
design-system:
  name: academic-classic
  version: 1.0.0
  theme: light-editorial
  colors:
    primary: "4874CB"       # WPS Academic Deep Blue (accent1)
    primary-dark: "44546A"  # Dark Slate Blue (dk2)
    accent: "EE822F"        # Active Orange (accent2)
    accent-warm: "F2BA02"   # Gold Yellow (accent3)
    canvas: "FFFFFF"        # Pure White (lt1)
    panel-soft: "E7E6E6"    # Warm Light Gray (lt2)
    line: "E7E6E6"          # Structural Separator (lt2)
    text-main: "000000"     # Pure Black (dk1)
    text-muted: "888888"    # Soft Gray
  typography:
    major-font: "Arial"
    ea-font: "Microsoft YaHei" # 微软雅黑
    title-size: 32pt
    subtitle-size: 20pt
    body-size: 18pt
    note-size: 11pt
  spacing:
    margins: "5% to 7%"
```

---

## Overview

本设计系统旨在为学术汇报、论文展示及科研演讲提供极简、严谨且高对比度的专业视觉规范。它反对杂乱无章的 AI-slop 填充图案，提倡利用大面积的白色呼吸空间、硬挺的几何线条、极高可读性的 Arial 字体，以及恰到好处的**学术深蓝色（`#4874CB`）**作为全局强调锚点。

---

## Colors

- **背景画布（Canvas）**：使用纯白 `#FFFFFF`，在投影仪和高亮屏幕下均能呈现极佳的清晰度与清爽感。
- **主强调色（Primary）**：`#4874CB`（学术深蓝）。它负责页面的焦点引导，如标题前缀、核心流程框、关键数据背景等。
- **正文及标题（Text）**：主标题及正文采用纯黑 `#000000`，保证最大对比度；副标题及弱强调字段采用 `#44546A`，形成优雅的层级过渡。
- **次要对比色（Accent）**：`#EE822F`（活力橙）与 `#F2BA02`（金黄），仅用于多组模型的对比（如对比 LM/GLM 失败与 Penalized/XGBoost 成功）、特殊警告及极端指标警示。

---

## Typography

- **英文字体**：统一锁死在 **Arial**（标题及正文均适用），确保在 macOS 和 Windows 上的多端绝对一致渲染与字形对齐。
- **中文字体**：统一使用**微软雅黑**（Microsoft YaHei），规避系统默认宋体产生的边缘发虚现象。
- **字号阶梯**：
  - 幻灯片大标题：`32pt` (粗体，前置学术深蓝的章节序号，例如 `01 Introduction`)
  - 小标题 / Takeaway：`20pt` (中粗)
  - 核心正文：不低于 `18pt` (底线)
  - 页脚 / 引用说明：`11pt` (灰色)

---

## Layout

- **网格对齐**：采用坚挺的左对齐轴线。所有的图片、正文块、表格都应 snaps 到统一的左边距（`5% to 7%`）。
- **页面节奏（Rhythm）**：
  - **章节过渡页**：采用轻度深色卡片或大面积空白，搭配极粗大标题与极小副标题，产生视觉呼吸。
  - **数据展示页**：一页只讲一个结论。将繁复的表格进行语义缩减，突出最重要的行/列，并附带明确的学术图表。
  - **比较页**：使用对称的双栏/多栏布局，并在重点栏采用浅灰 `#E7E6E6` 卡片打底以示区分。

---

## Elevation & Depth

- **拒绝无谓的投影**：完全废除一切卡片阴影、外发光等华而不实的修饰。所有的深度和层次都完全由空间留白（Margins）与浅灰色 `#E7E6E6` 卡片进行纯粹的划分。

---

## Shapes

- 所有的边框、分隔线一律采用纯平、粗度为 `1pt` 到 `2pt` 的极简线条。
- 文本框与卡片不带任何圆角（或者采用微圆角），保持学术汇报的严峻质感。

---

## Do's and Don'ts

### Do's (提倡)
- 一页幻灯片的中英文字数控制在 45 个字以内，结论要极度精简。
- 用学术蓝 `#4874CB` 来高亮图表中的关键拟合曲线或优胜模型。
- 中英文混合排版时，确保英文缩写、特殊单位与数字在换行时保持整体完整性。

### Don'ts (禁止)
- **禁止使用渐变色背景**。
- **禁止使用三套以上的 saturated 饱和色**在一张幻灯片中（即彩虹配色的列卡片）。
- **禁止将字体收缩到 18pt 以下**来迎合超量文字，如果放不下，请无条件拆分成两页。
