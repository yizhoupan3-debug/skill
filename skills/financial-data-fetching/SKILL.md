---
name: financial-data-fetching
description: |
  Fetch, validate, normalize, and export real financial market data: OHLCV,
  financial statements, shareholder structure, and capital metrics for U.S.
  equities, China A-shares, and crypto.
  Expert in CCXT, yfinance, Stooq, and AKShare. Use proactively when asked
  for market data pipelines, 行情获取, 真实数据, 财报获取, fundamental
  analysis, or backtest-ready exports.
metadata:
  version: "2.0.0"
  platforms: [codex]
  category: finance
  tags:
    - financial-data
    - market-data
    - data-fetching
    - api-validation
    - fundamentals
    - roe
    - financial-statements
    - shareholders
    - capital-metrics
    - crypto
    - us-stocks
    - csi300
    - csi500
    - ccxt
    - yfinance
    - stooq
    - akshare
    - parquet
    - vectorbt
    - backtrader
risk: medium
source: local
runtime_requirements:
  python:
    - akshare
    - ccxt
    - pandas
    - requests
    - yfinance
  rust:
    - cargo
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - financial data
  - market data
  - data fetching
  - api validation
  - fundamentals
  - roe
---

# financial-data-fetching

This skill owns **real financial data acquisition and validation**. It should be selected before broad quant/trading skills when the main job is to fetch, verify, normalize, or export market data — including OHLCV, financial statements, shareholder structure, and capital/valuation metrics.

## When to use

- Pulling **real** market data through public APIs or exchange APIs
- Fetching **financial statements** (income, balance sheet, cashflow) or **key metrics** (ROE, margins, EPS)
- Fetching **shareholder structure** (major holders, institutional holders, top-10 circulating shareholders)
- Fetching **capital/valuation metrics** (market cap, PE, PB, dividend yield, beta, etc.)
- Verifying whether a data source is actually usable in the current environment
- Building or reusing a financial data loader / data-ingestion module
- Exporting backtest-ready data for `generic`, `vectorbt`, or `backtrader`
- Working with:
  - crypto OHLCV / tickers / order books
  - U.S. stock data without tokens
  - 沪深300 / 中证500 index history, constituents, and weights
  - A-share individual stock fundamentals and capital data
- Best for requests like:
  - "拉真实行情数据"
  - "验证这个金融数据 API 能不能用"
  - "不要 token，抓美股数据"
  - "导出 vectorbt/backtrader 回测输入"
  - "取沪深300和中证500权重"
  - "拿 AAPL 的 ROE 和利润表"
  - "获取茅台的十大流通股东"
  - "查看 AAPL 的市值和 PE"
  - "获取 A 股财务数据"

## Do not use

- The primary task is strategy design, alpha research, execution logic, or risk rules -> use `/Users/joe/Documents/skill/skills/algo-trading/SKILL.md`
- The task is personal investment advice
- The task is macro commentary without data engineering work

## Task ownership and boundaries

This skill owns:
- market-data provider selection
- API reachability validation
- schema normalization
- data quality checks
- reusable data-loader code
- backtest input export formats
- financial statement data (income / balance / cashflow / key metrics)
- shareholder structure data (major / institutional / top-10)
- capital / valuation metric snapshots

This skill does **not** own:
- trading signal design
- portfolio construction logic
- **Dual-Dimension Audit (Pre: Provider/Probe, Post: Data-Fidelity/Normalization Results)** → `$execution-audit` [Overlay]
- live execution architecture beyond data ingress

## Safety and data-integrity rules

- **Never fabricate prices, volumes, constituents, weights, financial metrics, or timestamps.**
- If the user asks for latest / real-time / today data, fetch fresh data.
- Always report source, symbol/code, timezone, and adjusted/unadjusted status.
- Call out research-grade vs execution-grade data limitations.
- Do not hide failed probes; report them explicitly.

## Required workflow

1. Identify market, symbol/code format, timeframe, data type, and output schema.
2. Choose the narrowest validated source.
3. Run the probe script before calling a source verified here.
4. Fetch through the reusable module instead of re-implementing loaders.
5. Validate duplicates, gaps, stale bars, constituent counts, and weight sums.
6. Export in the user's requested schema and format.

## Primary assets

- Rust core: `/Users/joe/Documents/skill/rust_tools/financial_data_rs/`
- Package: `/Users/joe/Documents/skill/skills/financial-data-fetching/financial_data/`
- CLI: `/Users/joe/Documents/skill/skills/financial-data-fetching/scripts/financial_data_cli.py`
- Probe script: `/Users/joe/Documents/skill/skills/financial-data-fetching/scripts/validate_financial_data_sources.py`
- README: `/Users/joe/Documents/skill/skills/financial-data-fetching/README.md`
- Example script: `/Users/joe/Documents/skill/skills/financial-data-fetching/examples/python_quickstart.py`

## Runtime split

- Rust owns the hot path: crypto / U.S. OHLCV fetch, retries, timeouts, concurrent probes, and backtest export.
- Python remains the compatibility layer for adjusted U.S. OHLCV, fundamentals, holders, capital metrics, and China AKShare-backed surfaces.

## Verified no-token source map

### Crypto
- `CCXT`
- Verified exchanges in this workspace:
  - Binance `BTC/USDT`
  - Kraken `BTC/USD`
  - Coinbase `BTC/USD`

### U.S. equities / ETFs
- Rust Yahoo chart fetcher for:
  - OHLCV (default hot path)
- `yfinance` for:
  - OHLCV (flexible intervals and periods)
  - Financial statements (`get_income_stmt`, `get_balance_sheet`, `get_cashflow`)
  - Key metrics via `Ticker.info` (ROE, margins, EPS, debt ratios)
  - Shareholder data (`major_holders`, `institutional_holders`)
  - Capital metrics via `Ticker.info` (market cap, PE, PB, dividend yield, beta, etc.)
- `Stooq` as an environment-dependent daily fallback (currently apikey-gated in this workspace)
- Default to these no-token sources unless the user explicitly asks for credential-gated providers

### China A-shares / indices
- `AKShare` for:
  - Index OHLCV: `stock_zh_index_daily(symbol="sh000300")`
  - Index constituents: `index_stock_cons_csindex(symbol="000300" | "000905")`
  - Index weights: `index_stock_cons_weight_csindex(symbol="000300" | "000905")`
  - Financial statements: `stock_financial_report_sina(stock, symbol)`
  - Key financial indicators: `stock_financial_analysis_indicator(symbol)`
  - Top-10 circulating shareholders: `stock_gdfx_free_holding_detail_em(symbol)`
  - Institutional holders: `stock_gdfx_institution_holding_detail_em(symbol)`
  - Capital / valuation snapshot: `stock_zh_a_spot_em()` (filtered by code)

## Reusable usage

```python
from pathlib import Path
import sys

ROOT = Path('/Users/joe/Documents/skill/skills/financial-data-fetching')
sys.path.insert(0, str(ROOT))

from financial_data import MarketDataClient

client = MarketDataClient()

# OHLCV
result = client.fetch_ohlcv(market='us', symbol='AAPL', interval='1h', period='5d', source='yfinance')

# Fundamentals (ROE, margins, EPS)
metrics = client.fetch_fundamentals(market='us', symbol='AAPL', report='key_metrics')

# Income statement
income = client.fetch_fundamentals(market='us', symbol='AAPL', report='income', freq='yearly')

# Shareholder structure
holders = client.fetch_holders(market='us', symbol='AAPL', holder_type='institutional')

# Capital / valuation metrics
capital = client.fetch_capital_metrics(market='us', symbol='AAPL')

# China A-share fundamentals
cn_metrics = client.fetch_fundamentals(market='cn', symbol='600519', report='key_metrics')

# China top-10 shareholders
cn_holders = client.fetch_holders(market='cn', symbol='600519', holder_type='top10')
```

## CLI examples

```bash
# OHLCV
python .../scripts/financial_data_cli.py ohlcv --market us --symbol AAPL --interval 1h --period 5d --source yfinance

# Fundamentals
python .../scripts/financial_data_cli.py fundamentals --market us --symbol AAPL --report key_metrics
python .../scripts/financial_data_cli.py fundamentals --market us --symbol AAPL --report income --freq quarterly
python .../scripts/financial_data_cli.py fundamentals --market cn --symbol 600519 --report key_metrics

# Holders
python .../scripts/financial_data_cli.py holders --market us --symbol AAPL --type institutional
python .../scripts/financial_data_cli.py holders --market cn --symbol 600519 --type top10

# Capital metrics
python .../scripts/financial_data_cli.py capital --market us --symbol AAPL
python .../scripts/financial_data_cli.py capital --market cn --symbol 600519
```

## Validation gate

Run before calling a provider verified in this environment:

```bash
python /Users/joe/Documents/skill/skills/financial-data-fetching/scripts/validate_financial_data_sources.py
```

- Only call a source **verified here** if its probe returns `ok: true`.
- **Superior Quality Audit**: For research-grade financial pipelines, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## Trigger examples

- "做个金融数据获取 skill"
- "拉真实行情并验证 API"
- "做 market data loader"
- "不要 token，抓美股行情"
- "导出 backtrader 输入数据"
- "取沪深300和中证500成分股和权重"
- "拿 AAPL 的 ROE"
- "获取茅台的财务数据"
- "查看美股机构持仓"
- "获取 A 股市值和 PE"
- "强制进行金融数据审计 / 检查行情数据真实性与标准化结果。"
- "Use $execution-audit to audit this data fetcher for normalization-fidelity idealism."
