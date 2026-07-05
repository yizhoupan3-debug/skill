---
allowed_tools:
- shell
approval_required_tools: []
description: 'Financial data query: stocks, crypto, A-shares OHLCV and capital metrics.'
metadata:
  platforms:
  - supported
  tags:
  - financial
  - stock
  - crypto
  - market
  version: '1.0.0'
name: financial-data
scene: general
network_access: conditional
routing_gate: none
routing_layer: L3
routing_owner: owner
routing_priority: P2
session_start: n/a
source: local
risk: low
trigger_hints:
- 股票
- 股价
- 行情
- 金融数据
- 股票行情
- A股
- 加密
- 币价
- OHLCV
- 收盘价
- 开盘价
- 成交量
- 市值
- stock
- crypto
- financial
- market data
- stock price
- cryptocurrency
short_description: 金融数据查询：股票/加密市场/A股 OHLCV 和资金指标
tags:
- financial
- stock
- crypto
- market-data
when_to_use: 用户查询股票行情、加密币价、A股数据、金融指标时
do_not_use: 用户需要的是非金融类数据爬取（走 browser-automation）；用户需要的是论文的统计分析（走 statistical-analysis）
---
# Financial Data

## Persona

Act as a **financial data analyst** capable of querying market data for US stocks, cryptocurrencies, and Chinese A-shares.

## When to use

- User wants to look up stock prices, OHLCV data, or capital metrics
- User asks about a specific ticker (e.g. AAPL, BTC/USDT, 600519.SH)
- User needs historical market data for analysis

## How to use

Use the `financial_data` MCP tool with the appropriate symbol format:

- **US stocks**: `AAPL`, `MSFT`, `GOOGL` (auto-detects Yahoo/Stooq)
- **Crypto**: `BTC/USDT`, `ETH/USDT`, `SOL/USDT` (auto-detects Binance/Coinbase/Kraken)
- **CN A-shares**: `600519`, `000001` (auto-detects Eastmoney)

Specify period (e.g. `1d`, `1mo`, `3mo`, `1y`) and metric (`ohlcv` or `capital`).

## Do not use

- User needs non-financial data scraping → use `browser-automation` or `deep-search`
- User needs statistical analysis of financial data → first get data here, then use `statistical-analysis`
