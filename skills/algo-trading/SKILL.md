---

description: 'Design and implement algorithmic trading strategies, backtests, execution logic, and risk management. Sub-skill: financial data fetching.'
metadata:
  category: finance
  platforms:
  - supported
  tags:
  - algo-trading
  - quant
  - backtesting
  - strategy-design
  - execution
  - risk-management
  - factor-research
  version: '3.0.0'
name: algo-trading
scene: general
risk: high
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P3
session_start: n/a
source: local
trigger_hints:
- A股数据
- OHLCV
- Sharpe
- algo-trading
- alpha
- backtest
- crypto data
- drawdown
- factor research
- financial-data
- fundamentals
- market data
- pairs trading
- risk rules
- roe
- trading
- trading bot
- 交易策略
- 回测
- 因子研究
- 统计套利
- 量化
- 量化策略
- 金融数据
---
# Algo Trading

This skill owns **strategy logic, backtesting judgment, execution design, and risk management**.

## When to use

- Designing or reviewing trading strategies
- Building backtests and evaluating performance
- Analyzing Sharpe, drawdown, turnover, slippage, and robustness
- Designing live or paper trading logic
- Factor research, signal research, and portfolio rules
- Best for requests like:
  - "帮我写一个双均线策略并回测"
  - "分析这个策略的 Sharpe 和最大回撤"
  - "做一个 pairs trading 统计套利策略"
  - "设计实盘前的 paper trading 方案"

## Do not use

- The main task is fetching/validating/exporting market data -> use `subskills/financial-data-fetching/SKILL.md`
- The task is accounting or generic investment commentary
- The user only wants a data pipeline without strategy logic

## Task ownership and boundaries

This skill owns:
- entry/exit logic
- signal and factor design
- portfolio construction
- backtest assumptions and evaluation
- execution logic and risk rules

This skill does **not** own:
- primary market-data ingestion tooling
- API verification workflows
- backtest dataset export tooling

## Safety

- Never recommend untested live deployment.
- Always include fees, slippage, and realistic execution assumptions.
- Warn about look-ahead bias, survivorship bias, and overfitting.
- Use out-of-sample or walk-forward validation.

## Hard constraints

- 实盘部署建议必须经过 paper trading 阶段——不得跳过纸上交易直接推荐实盘
- 回测报告必须包含 fees、slippage、realistic execution assumptions——不得使用理想化假设
- 过拟合警告必须在回测结果 > 3 个显著策略指标时自动触发
- 风控规则（最大回撤限制、仓位上限）必须在策略定义中显式声明，不得隐含
- 数据质量声明：回测数据来源、时间范围、存活偏差处理必须在报告中标注

## Required workflow

1. Clarify asset class, horizon, capital/risk constraints, and objective.
2. If data is not already clean and verified, route data work to `financial-data-fetching` first.
3. Define strategy rules, position sizing, and risk limits.
4. Backtest with realistic assumptions.
5. Evaluate robustness with out-of-sample or walk-forward checks.
6. If discussing deployment, stage via paper trading before live rollout.

## Output defaults

Default answers under this skill should include:
- strategy hypothesis
- signals and execution rules
- risk controls
- backtest assumptions
- evaluation metrics
- major failure modes / caveats

## Trigger examples

- "帮我做一个量化交易策略"
- "回测这个因子策略"
- "分析交易机器人的风控规则"
- "做一个能 paper trade 的执行方案"

---

## Sub-skills

### financial-data-fetching
当任务涉及金融数据获取、API 验证、数据标准化时，
读取 `subskills/financial-data-fetching/SKILL.md` 获取完整数据管线参考。

**触发条件**：用户要拉行情、查基本面、验证 API、导出金融数据
→ 无需进入策略设计流程，直接使用 data-fetching sub-skill 的工具链。

**Rust 工具**：`cargo run --manifest-path ${SKILL_FRAMEWORK_ROOT}/rust_tools/financial_data_rs/Cargo.toml`
**数据源**：Yahoo Finance, Binance, Kraken, Stooq, 东方财富
**风险等级**：medium（独立于 algo-trading 的 high）
