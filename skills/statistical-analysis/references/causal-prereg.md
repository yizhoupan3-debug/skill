# 因果 / 识别与预注册（Causal identification & prereg）

与 [`../SKILL.md`](../SKILL.md) 的检验选择与报告规范配合：在声称**因果效应、政策含义、机制**之前，先完成识别叙事，再把分析写进预注册或锁定分析计划。

## DAG 与可检验含义

- 画出 **有向无环图（DAG）** 或等价单页示意图：处理、结果、测得/未测得混淆、中介、碰撞子。
- 对每个箭头问：**交换性（exchangeability）**、**positivity**、**一致性（consistency）** 是否在正文可辩护；缺失数据与选择入组如何进入图。
- **探索性路径**：DAG 可以迭代，但须在文中区分 **confirmatory** 与 **exploratory**（见 [`../../experiment-reproducibility/references/research-record-minimum.md`](../../experiment-reproducibility/references/research-record-minimum.md)）。

## 工具变量（IV）

- **相关性**：工具与处理相关。
- **排他性**：工具仅通过处理影响结果（可陈述可反驳的违反情形）。
- **单调性/同质性**：按设计声明（如 LATE 解释 vs 结构假设）。
- **弱工具**：报告 F 统计量或等价诊断；弱工具下置信区间方法（如 Anderson–Rubin）是否使用。

## 双重差分（DiD）与面板

- **平行趋势**：事前趋势检验或图示；说明预期违反时的偏误方向。
- **处理错分 / 交错采纳**：是否需要事件研究、队列异质性、或 Callaway–Sant’Anna 类估计。
- **聚类与推断**：在何种单位聚类标准误；是否使用置换/自助法。

## 确认性 vs 探索性

| 类型 | 要求 |
|------|------|
| **Confirmatory** | 预注册或锁定分析中的主假设；多重比较按家族预先声明；偏离须记录 |
| **Exploratory** | 子群、机制、后验分层；**不得**用同一 p 值叙事冒充主结论 |

## 与手稿栈对齐

- 主张—证据：`paper-workbench` → [`claim-evidence-ladder.md`](../../paper-workbench/references/claim-evidence-ladder.md)。
- 统计执行面：`PAPER_GATE_PROTOCOL` 统计 gate 若启用，只产出 gate 要求的证据并交回主链。
