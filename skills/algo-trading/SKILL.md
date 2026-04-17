---
name: algo-trading
description: |
  Design, analyze, and implement algorithmic trading strategies, backtests, execution logic, and risk management.
  Use proactively when the user asks for quant strategy design, 回测, 交易策略, trading bot logic, Sharpe/drawdown analysis, factor research, or execution/risk rules. For pure market-data acquisition, API verification, or backtest-data export, route to `financial-data-fetching` first.
metadata:
  version: "3.0.0"
  platforms: [codex]
  category: finance
  tags:
    - algo-trading
    - quant
    - backtesting
    - strategy-design
    - execution
    - risk-management
    - factor-research
risk: high
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
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

- The main task is fetching/validating/exporting market data -> use `/Users/joe/Documents/skill/skills/financial-data-fetching/SKILL.md`
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
