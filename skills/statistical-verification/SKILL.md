---
description: 'Statistical verification capability: p-value recomputation, GRIM test, effect size reporting, multiple comparison correction, assumption checking.'
metadata:
  platforms:
  - supported
  tags:
  - statistics
  - verification
  - audit
  - research
  version: '1.0.0'
name: statistical-verification
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: n/a
source: local
trigger_hints:
- GRIM test
- p值验证
- statistical verification
- stats audit
- 假设检验审查
- 统计审查
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

Rust 实现：`research_harness::verification::statistical`（通过 MCP tool 或直接调用）

```
# GRIM 检验：
research_harness::verification::statistical::grim_test(observed_mean, n)

# p 值验证：
research_harness::verification::statistical::verify_p_value(observed, expected, tolerance)

# 多重比较校正检查：
research_harness::verification::statistical::check_multiple_comparison_correction(num_tests, correction_applied)
```

| # | 检查名 | PASS 条件 |
|---|--------|-----------|
| 1 | p 值重跑 | 重算 p 与报告 p 偏差 < 1e-4（需 Python + scipy + 原始数据） |
| 2 | GRIM test | 均值的最后一位整数粒度可恢复 |
| 3 | 效应量报告 | 每项检验均报告效应量（d / eta2 / r） |
| 4 | 多重比较校正 | ≥ 3 次比较时使用了 Bonferroni / BH / Tukey |
| 5 | 前提假设检查 | 正态性（Shapiro-Wilk p > .05）且方差齐性（Levene p > .05） |

## References

- statistical-analysis skill：[`../statistical-analysis/SKILL.md`](../statistical-analysis/SKILL.md)（统计方法选择与解读的知识库）
- statistical-analysis 因果与预注册：[`../statistical-analysis/references/causal-prereg.md`](../statistical-analysis/references/causal-prereg.md)

## Integration

前门 skill 在以下时机内联调用本 skill：

- **research-execution**：实验结果分析完成后，审计统计报告
- **paper-workbench**：结果章节写完后，做统计正确性门禁检查

调用方式：按验证清单逐项执行，FAIL 项作为 blocker 回写前门 skill。
