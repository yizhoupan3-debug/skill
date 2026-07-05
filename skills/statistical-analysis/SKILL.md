---

description: Guide research statistics for test choice, effect sizes, uncertainty reporting, and interpretation. Covers beyond-NHST (equivalence/TOST), modern Bayesian workflow, hierarchical multiple comparison procedures, and rigorous preregistration. Use when 用什么检验、显著性怎么算、p 值、效应量、贝叶斯、多重比较、统计功效、回归诊断.
metadata:
  platforms:
  - supported
  tags:
  - statistics
  - hypothesis-testing
  - effect-size
  - bayesian
  - regression
  - research
  version: '2.0.0'
name: statistical-analysis
scene: research
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: preferred
trigger_hints:
- A/B 测试怎么做显著性
- interpreting hypothesis tests
- p 值
- power analysis
- 假设检验怎么选
- 回归诊断
- 多组比较怎么校正
- 多重比较
- 效应量
- 显著性怎么算
- 用什么检验
- 统计功效
- 置信区间怎么报
- 贝叶斯
- 需要多少样本量
- statistical-analysis
---
# Statistical Analysis

- **Two-stage rigor check (test selection -> result interpretation)** for high-stakes analyses
- Manuscript stack when co-invoked with paper work: [`../research/paper-workbench/references/RESEARCH_PAPER_STACK.md`](../research/paper-workbench/references/RESEARCH_PAPER_STACK.md)

This skill owns **statistical method selection, execution, and interpretation** for research.

## Causal claims, identification, and prereg

- **先识别、后 p 值**：因果语言（「导致」「政策含义」「机制」）需要 **识别策略**（RCT、IV、DiD/RD、前后对照设计等）与 **可反驳假设**；不要仅用回归系数 + 显著性顶替因果叙事。
- **预注册与偏离**：主终点、主对比、分析计划与事后偏离的记录义务见 [`references/causal-prereg.md`](references/causal-prereg.md) 与 [`../experiment-reproducibility/references/research-record-minimum.md`](../experiment-reproducibility/references/research-record-minimum.md)。
- **探索性分析**：子群、机制、多终点扫描须标明 exploratory，并避免与 confirmatory 同一多重比较叙事混写。
- **Registered Reports**: 在数据收集前通过同行评议的方案会获得 in-principle acceptance (IPA)，无论结果如何均出版。如需投稿 RR，优先选择有 RR 徽标的期刊（如 Nature、Science、Elsevier RR 徽标期刊）；方法部分须同时提交主分析与敏感性分析。

## When to use

- The user needs to choose the right statistical test for their data
- The user wants help with hypothesis testing, confidence intervals, or p-values
- The user needs effect size calculation or power analysis
- The user wants Bayesian inference guidance
- The user needs multiple comparison correction
- The user wants regression diagnostics or model selection
- The user needs statistical figures (QQ plots, residual plots, forest plots)

## Do not use

- The user wants one front door for a research-project task rather than statistics only -> use `$research` (execution lane) and keep this skill only for statistical questions
- The task is ML model training or evaluation -> answer in the current implementation context when Apple Silicon memory/runtime constraints dominate; otherwise answer in the current implementation context
- The task is data wrangling or cleaning -> answer in the current data/implementation context
- The task is paper writing -> use `@lane:writer` (but may co-invoke for results sections)
- The task is about paper-level scientific logic or claims-vs-evidence alignment -> use `@lane:reviewer` logic mode (which may route statistical questions here)

## References

- Causal DAG、IV/DiD 假设模板与预注册边界：[`references/causal-prereg.md`](references/causal-prereg.md)

## Beyond NHST — Estimation-Based Reporting

现代统计报告应超越简单显著性检验，采用更丰富的推断框架。

### Four-Outcome Inferential Taxonomy

结合传统 NHST 与 **equivalence testing (TOST)** 得到四种结果：

| NHST 显著 | 等价检验通过 | 结论 |
|-----------|-------------|------|
| ✅ | ❌ | 存在有意义效应 (Reject H₀) |
| ❌ | ✅ | 可接受无有意义效应 (Equivalence) |
| ✅ | ✅ | 效应存在但不超过 SESOI (Both) |
| ❌ | ❌ | 无法判定 (Undetermined — 增加样本量) |

### Equivalence Testing (TOST) 工作流

1. **指定 SESOI (Smallest Effect Size of Interest)** 作为等价边界 ∆，在收集数据前确定（基于 practical significance、prior literature 或资源约束）
2. 设定两个非等价原假设：H₀₁: 效应 ≤ −∆，H₀₂: 效应 ≥ +∆
3. 若 (1−2α) 置信区间完全落在 [−∆, +∆] 内，则拒绝两个原假设 → 推断等价
4. **报告工具**：Lakens TOSTER (R/Python)、PROC POWER (SAS)、Jamovi TOST 模块

> 当理论或实践上需要支持"效应不存在"时，始终同时报告 NHST p 值与 TOST 结果。
> **Source**: Lakens, Scheel & Isager (2018), *Advances in Methods and Practices in Psychological Science*; Lakens (2024), *The American Statistician*
> `doi: 10.1177/2515245918770963` | `doi: 10.1080/00031305.2019.1701530`

### Effect Sizes with Uncertainty Intervals

除了传统的点估计 + p 值，始终报告 **点估计 + 置信区间**。对于预注册关键指标，使用 **bootstrap-t CI** 而非 Normal approximation，尤其当分布偏离时。

## Comparing Groups

| Situation | Parametric | Non-parametric |
|---|---|---|
| 2 independent groups | Independent t-test | Mann-Whitney U |
| 2 paired groups | Paired t-test | Wilcoxon signed-rank |
| 3+ independent groups | One-way ANOVA | Kruskal-Wallis |
| 3+ paired groups | RM-ANOVA | Friedman |
| 2×2 factorial | Two-way ANOVA | Permutation |

Use non-parametric when: n<30 + normality violated, ordinal data, heavy outliers, clearly non-normal (Shapiro-Wilk).

## Association

| Situation | Method |
|---|---|
| Continuous, linear | Pearson r |
| Non-linear / ordinal | Spearman ρ |
| Two categorical | Chi-squared / Fisher |
| Continuous→categorical | Logistic regression |
| Multiple predictors | Multiple regression / GLM |

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

### 经典方法（低维对比）

| Situation | Method |
|-----------|--------|
| Few planned comparisons | Bonferroni |
| Many pairwise comparisons | Tukey HSD (ANOVA post-hoc) |
| Control vs multiple treatments | Dunnett |
| Exploratory (many tests) | Benjamini-Hochberg (FDR) |
| Genome-wide / large-scale | FDR with q-values |

> **注意**：Bonferroni 虽保守但在控制 FWER 时数学上稳定且不依赖假设结构——它在控制 per-family error rate (PFER) 场景下实际并不那么保守（Gordon et al. 2007）。Holm 递降法（1979）在任何依赖结构下都控制 FWER 且比 Bonferroni 更有效（无需依赖假设）；Hochberg 递升法则需 PRDS/独立性假设。

### 现代方法（结构化路径）

当假设之间有已知层次或逻辑关系时，现代分层方法比 Bonferroni 更有效且可解释：

| 场景 | 推荐方法 | 说明 |
|------|---------|------|
| 有自然检验顺序 | **Fixed-sequence** | 按预定义顺序依次检验，直到首个不显著为止 |
| 需要回退保护 | **Fallback** | 主假设若未显著，权重转给备选假设 |
| 多终点/多维度家族 | **Gatekeeping** (Dmitrienko et al.) | 分层门控，上层全部显著后才进入下层 |
| 结构化的双向边 | **Graphical approach** (Bretz et al. 2009) | 有向图表示假设关系，图形化分配/传递 α 权重 |

**Graphical approach**（Bretz et al. 2009）已被扩展以控制 **k-FWER**（允许至少 k 次错误拒绝的概率）和 **FDP tail probability**（错误发现比例的尾概率），适用于高维、结构化假设集。实现工具：R 包 `gMCP`、SAS `PROC MULTTEST`。

> **Source**: Robertson, Wason & Bretz (2020), *Statistics in Medicine*
> `doi: 10.1002/sim.8595` | `arXiv: 2004.01759`

## Bayesian Analysis Guide

### 何时倾向贝叶斯方法
- 小样本 + 有信息先验
- 需要证据支持 H₀（BF 框架而非 NS 不显著）
- 顺序分析/自适应设计
- 层次/嵌套结构数据
- 复杂模型（MCMC 可计算但经典方法封闭形式不可得）

### 关键输出
- **Bayes Factor**: BF > 10 强烈支持，3–10 中等，1–3 微弱，<1 支持 H₀
- **后验分布**：报告均值 + 可信区间（HDI 或等尾区间）
- **ROPE** (Region of Practical Equivalence)：若后验 HDI 完全落在 ROPE 内，支持=无有意义效应（TOST 的贝叶斯对应）

### Modern MCMC Convergence Diagnostics

传统 R-hat（Gelman & Rubin, 1992）在链重尾或链间方差不等时无法检测非收敛。始终使用以下现代方法：

| 诊断 | 旧（弃用） | 新（推荐） |
|------|-----------|-----------|
| 收敛统计量 | 传统 R-hat | **Rank-normalized R-hat**（Vehtari et al. 2021） |
| 可视化 | Trace plots | **Rank plots**（多个链的 rank 直方图应重叠均匀） |
| 精度度量 | 原始 MCSE | **分位数 MCSE** + 局部效率度量 |

实践要求：以下方法现已被 Stan、PyMC、brms、rstanarm 设为默认：
- R-hat < 1.01 为收敛阈值（非传统 1.1）
- 用 `bayesplot::mcmc_rank_overlay()` 替代 `traceplot()`
- 报告 bulk-ESS 与 tail-ESS 替代单一有效样本量

> **Source**: Vehtari, Gelman, Simpson, Carpenter, Bürkner (2021), *Bayesian Analysis*
> `doi: 10.1214/20-BA1221` | `arXiv: 1903.08008`

### Bayesian Visual Predictive Checks (VPCs)

VPC 本身是一个拟合数据的模型——它也需要元诊断。常见 VPC 图（如 KDE 密度图用于离散数据）在假设不匹配时会产生误导。按数据类型使用不同 VPC：

| 数据类型 | 推荐 VPC 方式 |
|---------|-------------|
| 连续 | **Quantile dot plots** |
| 计数/离散 | **边界修正 KDE** + **Modified rootograms** |
| 二分类/类别 | **PAV 调整校准图** |

一般原则：
1. 诊断模拟数据是否被错误视为连续；检测离散边界
2. 使用 split predictive checks（训练-模拟分离）避免双重使用数据保守性
3. 数据驱动的 VPC 选择比默认可视化更可靠

> **Source**: Säilynoja, Johnson, Martin & Vehtari (2025)
> `arXiv: 2503.01509`

### 贝叶斯分析检查清单

- [ ] 先验预测检查：在观察数据前，模拟先验分布的隐含含义
- [ ] 收敛诊断：rank-normalized R-hat < 1.01，rank plots 重合良好
- [ ] 有效样本量：bulk-ESS 与 tail-ESS > 400（4 链）
- [ ] 后验预测检查：选择合适的 VPC 方式并按数据类型进行
- [ ] 敏感性分析：对先验的合理变化是否稳健
- [ ] 先验与后验的对比：参数是否被数据推动（非先验主导）

## Regression Diagnostics Checklist

- [ ] Linearity: residuals vs fitted plot shows no pattern
- [ ] Homoscedasticity: constant variance of residuals
- [ ] Normality: QQ plot of residuals is roughly linear
- [ ] Independence: no autocorrelation (Durbin-Watson test)
- [ ] Multicollinearity: VIF < 5 for all predictors
- [ ] Influential points: Cook's distance < 1
- [ ] No omitted variables: Ramsey RESET test (if available)
- [ ] **Cluster-robust standard errors 的 k-条件**：最大聚类的得分应可忽略——若聚类过大（如经济学 AER/Econometrica 实证论文中 77% 违反该条件），聚类稳健标准误的推断可能不可靠。使用 `lmtest::coeftest(., vcov = vcovCL)` 前应检查集群大小分布。
  > **Source**: Chiang, Sasaki & Wang (2023), `arXiv: 2308.10138`（preprint, 八次修订）

## Output Defaults

Use `统计分析报告`:
- research question → statistical hypothesis
- data description (sample size, distributions, assumptions)
- test selection rationale
- test results (statistic, p-value, effect size, CI)
- interpretation in context
- limitations and assumptions

For research-project orchestration, return the statistical blocker and hand the
workflow back to `$research` (execution lane) after the test choice,
assumption check, or interpretation is settled.

## Hard Constraints

- Do not report p-values without effect sizes
- Do not claim "no effect" from a non-significant result (absence of evidence ≠ evidence of absence). **当需要支持"效应不存在"时，使用 TOST 等价检验。**
- Do not run parametric tests on clearly non-normal data without justification
- Do not apply multiple tests without correction; **当假设有结构化关系时，优先使用 hierarchical/graphical 方法**
- Do not confuse statistical significance with practical importance
- Always state assumptions and check them before running tests
- For high-stakes statistical results, run a dedicated rigor verification pass against the claim/evidence bar in [`../research/paper-workbench/references/claim-evidence-ladder.md`](../research/paper-workbench/references/claim-evidence-ladder.md).
- Report exact p-values (p = 0.037) not just threshold labels (p < 0.05)
- **Always report confidence/credible intervals alongside point estimates**
- **For preprint- or template-based papers (registered reports, results-blind review), explicitly state the preregistration details (OSF/AsPredicted ID, deviations from plan)**

## Cross-references

- `$research` (execution lane) and current project owners may use this skill as the statistics / uncertainty lane
- `@lane:reviewer` logic mode routes deep statistical method questions (effect size, power analysis, significance testing) to this skill
- `@lane:reviewer` Tier-1 statistical rigor checks may route here
- `$experiment-reproducibility` routes result validation statistics here
- When invoked as a **gate-chain lane owner** (G2 / G3 / G5 statistical rigor
  checks under the manuscript protocol), follow the lane contract in
  [`../research/paper-workbench/references/paper-gate-protocol.md`](../research/paper-workbench/references/paper-gate-protocol.md); produce only the
  gate-required evidence and hand back to the protocol main chain.

## Trigger examples

- "帮我选一个合适的统计检验"
- "这两组数据的差异显著吗"
- "帮我算效应量"
- "做一个 power analysis 看需要多少样本"
- "回归模型的残差图怎么看"
- "多重比较要怎么校正"
- "什么是 TOST 等价检验？"
- "贝叶斯 MCMC 收敛怎么诊断？"
- "用 rank-normalized R-hat 检查这组链是否收敛"
- "强制进行统计分析深度复核 / 检查检验方法与效应量结果。"
- "Run a dedicated rigor verification pass on this statistical analysis."
