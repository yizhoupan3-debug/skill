---
name: statistical-analysis
description: |
  Guide research statistics for test choice, effect sizes, uncertainty reporting, and interpretation.
  Use when the user asks 用什么检验、显著性怎么算、p 值、效应量、贝叶斯、多重比较、统计功效、回归诊断, or needs help choosing, executing, or interpreting hypothesis tests, Bayesian inference, confidence intervals, power analysis, regression diagnostics, or statistical figures for research data.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: preferred
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 用什么检验
  - 显著性怎么算
  - p 值
  - 效应量
  - 贝叶斯
  - 多重比较
  - 统计功效
  - 回归诊断
  - interpreting hypothesis tests
  - 假设检验怎么选
  - 置信区间怎么报
  - A/B 测试怎么做显著性
  - 多组比较怎么校正
  - 需要多少样本量
  - power analysis
metadata:
  version: "1.0.2"
  platforms: [codex]
  tags:
    - statistics
    - hypothesis-testing
    - effect-size
    - bayesian
    - regression
    - research
risk: low
source: local

---

# Statistical Analysis

- **Two-stage rigor check (test selection -> result interpretation)** for high-stakes analyses
- Manuscript stack when co-invoked with paper work: [`../paper-workbench/references/RESEARCH_PAPER_STACK.md`](../paper-workbench/references/RESEARCH_PAPER_STACK.md)

This skill owns **statistical method selection, execution, and interpretation** for research.

## Causal claims, identification, and prereg

- **先识别、后 p 值**：因果语言（「导致」「政策含义」「机制」）需要 **识别策略**（RCT、IV、DiD/RD、前后对照设计等）与 **可反驳假设**；不要仅用回归系数 + 显著性顶替因果叙事。
- **预注册与偏离**：主终点、主对比、分析计划与事后偏离的记录义务见 [`references/causal-prereg.md`](references/causal-prereg.md) 与 [`../experiment-reproducibility/references/research-record-minimum.md`](../experiment-reproducibility/references/research-record-minimum.md)。
- **探索性分析**：子群、机制、多终点扫描须标明 exploratory，并避免与 confirmatory 同一多重比较叙事混写。

## When to use

- The user needs to choose the right statistical test for their data
- The user wants help with hypothesis testing, confidence intervals, or p-values
- The user needs effect size calculation or power analysis
- The user wants Bayesian inference guidance
- The user needs multiple comparison correction
- The user wants regression diagnostics or model selection
- The user needs statistical figures (QQ plots, residual plots, forest plots)

## Do not use

- The user wants one front door for a research-project task rather than statistics only -> use `$research-workbench` and keep this skill only for statistical questions
- The task is ML model training or evaluation -> use `$mac-memory-management` when Apple Silicon memory/runtime constraints dominate; otherwise answer in the current implementation context
- The task is data wrangling or cleaning -> answer in the current data/implementation context
- The task is plotting without statistical analysis → use `$scientific-figure-plotting`
- The task is paper writing → use `$paper-writing` (but may co-invoke for results sections)
- The task is about paper-level scientific logic or claims-vs-evidence alignment → use `$paper-reviewer` logic mode (which may route statistical questions here)

## References

- Causal DAG、IV/DiD 假设模板与预注册边界：[`references/causal-prereg.md`](references/causal-prereg.md)

### Comparing Groups

| Situation | Parametric | Non-parametric |
|---|---|---|
| 2 independent groups | Independent t-test | Mann-Whitney U |
| 2 paired groups | Paired t-test | Wilcoxon signed-rank |
| 3+ independent groups | One-way ANOVA | Kruskal-Wallis |
| 3+ paired groups | RM-ANOVA | Friedman |
| 2×2 factorial | Two-way ANOVA | Permutation |

### Association

| Situation | Method |
|---|---|
| Continuous, linear | Pearson r |
| Non-linear / ordinal | Spearman ρ |
| Two categorical | Chi-squared / Fisher |
| Continuous→categorical | Logistic regression |
| Multiple predictors | Multiple regression / GLM |

Use non-parametric when: n<30 + normality violated, ordinal data, heavy outliers, clearly non-normal (Shapiro-Wilk).

## Effect Size Reporting

Always report effect sizes alongside p-values:

| Test | Effect Size | Small / Medium / Large |
|------|-------------|------------------------|
| t-test | Cohen's d | 0.2 / 0.5 / 0.8 |
| ANOVA | η² (eta squared) | 0.01 / 0.06 / 0.14 |
| Correlation | r | 0.1 / 0.3 / 0.5 |
| Chi-squared | Cramér's V | depends on df |
| Regression | R², adjusted R² | context-dependent |

## Multiple Comparison Correction

When running multiple tests, apply correction:

| Situation | Method |
|-----------|--------|
| Few planned comparisons | Bonferroni |
| Many pairwise comparisons | Tukey HSD (ANOVA post-hoc) |
| Control vs multiple treatments | Dunnett |
| Exploratory (many tests) | Benjamini-Hochberg (FDR) |
| Genome-wide / large-scale | FDR with q-values |

## Bayesian Analysis Quick Guide

Prefer Bayesian when: small samples + informative priors, need evidence FOR null, sequential analysis, hierarchical data.

Key outputs: **Bayes Factor** (BF>10 strong, 3–10 moderate, 1–3 anecdotal, <1 favors H0), **posterior distributions**, **credible intervals**.

## Regression Diagnostics Checklist

- [ ] Linearity: residuals vs fitted plot shows no pattern
- [ ] Homoscedasticity: constant variance of residuals
- [ ] Normality: QQ plot of residuals is roughly linear
- [ ] Independence: no autocorrelation (Durbin-Watson test)
- [ ] Multicollinearity: VIF < 5 for all predictors
- [ ] Influential points: Cook's distance < 1
- [ ] No omitted variables: Ramsey RESET test (if available)

## Output Defaults

Use `统计分析报告`:
- research question → statistical hypothesis
- data description (sample size, distributions, assumptions)
- test selection rationale
- test results (statistic, p-value, effect size, CI)
- interpretation in context
- limitations and assumptions

For research-project orchestration, return the statistical blocker and hand the
workflow back to `$research-workbench` after the test choice,
assumption check, or interpretation is settled.

## Hard Constraints

- Do not report p-values without effect sizes
- Do not claim "no effect" from a non-significant result (absence of evidence ≠ evidence of absence)
- Do not run parametric tests on clearly non-normal data without justification
- Do not apply multiple tests without correction
- Do not confuse statistical significance with practical importance
- Always state assumptions and check them before running tests
- For high-stakes statistical results, run a dedicated rigor verification pass against the claim/evidence bar in [`../paper-workbench/references/claim-evidence-ladder.md`](../paper-workbench/references/claim-evidence-ladder.md).
- Report exact p-values (p = 0.037) not just threshold labels (p < 0.05)

## Cross-references

- `$research-workbench` and current project owners may use this skill as the statistics / uncertainty lane
- `$paper-reviewer` logic mode routes deep statistical method questions (effect size, power analysis, significance testing) to this skill
- `$paper-reviewer` Tier-1 statistical rigor checks may route here
- `$experiment-reproducibility` routes result validation statistics here
- When invoked as a **gate-chain lane owner** (G2 / G3 / G5 statistical rigor
  checks under the manuscript protocol), follow the lane contract in
  [`../PAPER_GATE_PROTOCOL.md`](../PAPER_GATE_PROTOCOL.md); produce only the
  gate-required evidence and hand back to the protocol main chain.

## Trigger examples

- "帮我选一个合适的统计检验"
- "这两组数据的差异显著吗"
- "帮我算效应量"
- "做一个 power analysis 看需要多少样本"
- "回归模型的残差图怎么看"
- "多重比较要怎么校正"
- "强制进行统计分析深度复核 / 检查检验方法与效应量结果。"
- "Run a dedicated rigor verification pass on this statistical analysis."
