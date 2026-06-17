---
name: statistical-verification
description: |
  无状态内部 skill：审计统计结果的正确性——重跑 p 值、GRIM 检验、效应量报告、
  多重比较校正和假设检验前提。由 research-execution、paper-workbench 内联调用。
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
user-invocable: false
disable-model-invocation: true
risk: low
source: local
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [statistics, verification, audit, research]
trigger_hints:
  - 统计结果审计
  - p 值重跑
  - GRIM test
  - 效应量检查
  - 前提假设检验
---

# Statistical Verification

无状态能力 skill：对论文或实验报告中的统计结果做独立审计。不独立编排会话。

## When to Use

- 前门 skill 需要验证报告的 p 值、均值、标准差是否可重算
- 需要检查 GRIM（granularity of integer mean）一致性
- 需要验证效应量是否被正确报告
- 需要检查多重比较是否做了适当校正
- 需要确认正态性、方差齐性等前提假设

## Do not use

- 选择统计检验方法或设计实验 → 使用 `$statistical-analysis`
- 数学推导验证 → 使用 `$formal-verification`
- 纯代码实现 without 统计语境 → 在当前 coding context 直接回答

## Hard constraints

- p 值重算偏差 > 1e-2 必须标记为 FAIL（疑似计算错误），偏差 1e-4 ~ 1e-2 标记为 WARN
- GRIM test 失败一律为 P0 blocker——均值与 N 不兼容意味着数据报告有误
- 效应量缺失不自动降级为 FAIL，但必须在报告中显式标注 "效应量未报告"
- 多重比较 ≥ 3 次未校正为 FAIL，不得以 "探索性分析" 为由豁免
- 无原始数据时，只能做格式/完整性检查，不能做重算——必须声明覆盖范围受限

## Input / Output

| 输入 | 输出 |
|------|------|
| 统计结果表（均值、SD、N、p 值、检验类型） | 每项检查的 PASS / FAIL / WARN 状态 |
| 原始数据（可选） | 重算值 vs 报告值的偏差矩阵 |
| 多重比较列表 | 校正后 p 值及是否仍显著 |

## Verification Checklist

> **Note**: verify commands below are pattern templates. Actual commands depend on project setup and available tools.

| # | 检查名 | PASS 条件 | verify command |
|---|--------|-----------|----------------|
| 1 | p 值重跑 | 重算 p 与报告 p 偏差 < 1e-4 | `python -c "from scipy.stats import ttest_ind; ..."` 或对应检验函数 |
| 2 | GRIM test | 均值的最后一位整数粒度可恢复 | `python -c "from math import floor; ..."` 检查 round(mean*N) == sum |
| 3 | 效应量报告 | 每项检验均报告效应量（d / eta2 / r） | `grep -c 'effect.size' results.md` → 等于检验数量 |
| 4 | 多重比较校正 | ≥ 3 次比较时使用了 Bonferroni / BH / Tukey | `grep -E 'correct|adjust|FDR|Bonferroni' methods.md` → 非空 |
| 5 | 前提假设检查 | 正态性（Shapiro-Wilk p > .05）且方差齐性（Levene p > .05） | `python -c "from scipy.stats import shapiro, levene; ..."` |

## References

- statistical-analysis skill：[`../statistical-analysis/SKILL.md`](../statistical-analysis/SKILL.md)（统计方法选择与解读的知识库）
- statistical-analysis 因果与预注册：[`../statistical-analysis/references/causal-prereg.md`](../statistical-analysis/references/causal-prereg.md)

## Integration

前门 skill 在以下时机内联调用本 skill：

- **research-execution**：实验结果分析完成后，审计统计报告
- **paper-workbench**：结果章节写完后，做统计正确性门禁检查

调用方式：按验证清单逐项执行，FAIL 项作为 blocker 回写前门 skill。
