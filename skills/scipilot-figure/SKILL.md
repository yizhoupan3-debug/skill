---
description: 科研数据可视化顾问 — 数据剖析 → 图型推荐 → 出版级绘制 → 视觉自检。覆盖折线/柱状/散点/箱线/热力图/分布图/多面板，支持期刊规范适配与色盲安全配色。
metadata:
  platforms:
  - supported
  tags:
  - figure
  - data-viz
  - publication
  - matplotlib
  - seaborn
  - journal-specs
  - visual-qa
  version: '2.1.0'
name: scipilot-figure
scene: research
risk: low
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: n/a
short_description: >-
  从数据到出版级图表：EDA → 图型推荐 → 期刊适配 → 色盲安全绘制 → 视觉自检闭环。
  不做示意图/流程图/架构图。
trigger_hints:
- 科研图表
- 数据可视化
- 论文配图
- 出版级图表
- 期刊配图
- matplotlib
- seaborn
- 不知道怎么画图
- figure
- 色盲安全配色
- 多面板
- 投稿图
- 中文论文图表
- scientific figure
- data visualization
- visaul QA
when_to_use: >-
  用户有数值型数据需要生成出版级图表，包括论文配图、期刊投稿图、学术报告图。
  即使只给数据问"这个怎么画"也应使用——本技能首要能力是判断该用什么图。
do_not_use: >-
  不用于示意图、流程图、架构图、AI 截图转 TiKZ（用 tikz-paper-figure）。
  不用于非科研类视觉设计（用 hallmark/huashu-design）。
---

# scipilot-figure — 科研数据可视化顾问

> 从数据剖析到出版级成图 | 本仓库 [Haojae/scipilot-figure-skill](https://github.com/Haojae/scipilot-figure-skill) 的内化集成

## 核心理念

**先思考再画**，而非"给我数据我画出来"。本技能首要能力是判断和决策：

1. **先做 EDA** — 列类型识别、样本量、分布、异常值、相关性
2. **先想论点** — "这张图要说服读者相信什么？" 同样数据不同论点 = 不同图型
3. **主动拦截** — 18 条科研画图禁忌（P1-P18），犯禁时建议替代方案
4. **出版级输出** — 按目标期刊栏宽定 `figsize`，Okabe-Ito 色盲安全配色，绝不二次缩放

## 工作流程

```
用户输入数据（CSV/DataFrame/数值）
    │
    ▼
1. 数据剖析 ── profile_data.py（列类型/分布/异常/相关）
    │
    ▼
2. 图型推荐 ── 基于变量数量+类别+论证意图+样本量的三维决策
    │
    ▼
3. 期刊适配 ── 按目标期刊设置栏宽/字号/DPI/字体
    │
    ▼
4. 出版级绘制 ── setup_style.py + plot_recipes.py（7 类图）
    │
    ▼
5. 自检闭环 ── visual_qa.py（缺字/裁切/重叠）+ AI 读图复核
    │
    ▼
6. 导出 ── 多格式（PDF/PNG/SVG）+ 灰度预览 + 合规审计
```

## 覆盖图型

- 折线图（趋势/时序）
- 柱状图（分组/堆叠/误差棒）
- 散点图（回归线/分组着色）
- 箱线图 / 小提琴图
- 热力图（相关性矩阵）
- 分布图（直方图/KDE）
- 多面板组合（subplots）
- Plotly 交互图（可选）

## 科研画图拦截（P1-P18 核心示例）

| 编号 | 禁忌 | 推荐替代 |
|------|------|---------|
| P1 | 小样本均值柱状（掩盖分布） | 散点+均值线或箱线图 |
| P2 | 双 Y 轴（伪造相关） | 分面图或双面板 |
| P3 | 饼图（人眼无法准确比较角度） | 柱状图或堆积柱状 |
| P4 | Y 轴不从 0 开始（膨胀视觉差异） | 始终从 0 起并标注截断 |
| P5 | rainbow/jet 色图（色盲不友好） | Okabe-Ito / viridis |
| P6 | 把分类变量连接成折线（误导趋势） | 柱状图或散点 |

## 依赖

### Python 核心

```bash
pip install matplotlib>=3.7 seaborn>=0.13 plotly>=5.18 pandas>=2.0 numpy>=1.24 scipy>=1.10 Pillow>=10.0
```

### 可选增强

```bash
pip install "scienceplots>=2.1" pypdf>=4.0 kaleido>=0.2.1 pymupdf>=1.23
```
