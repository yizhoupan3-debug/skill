# financial-data-fetching

A dedicated Codex skill for **real financial market data fetching, validation, normalization, and backtest-data export**.

Use this skill when the main task is to:
- fetch real crypto / U.S. stock / China index data
- fetch financial statements (income, balance sheet, cashflow) or key metrics (ROE, EPS, margins)
- fetch shareholder structure (major holders, institutional holders, top-10 circulating)
- fetch capital / valuation metrics (market cap, PE, PB, dividend yield, beta)
- verify whether a market-data API actually works here
- build or reuse a market-data loader
- export backtest-ready data for `generic`, `vectorbt`, or `backtrader`

Do **not** use this skill as the primary owner when the main task is strategy design, alpha logic, execution logic, or risk management. In that case, start with [`algo-trading`](../algo-trading/SKILL.md).

---

## What this skill supports

### Markets
- Crypto (OHLCV only)
- U.S. equities / ETFs
- China A-shares / CSI indices (沪深300, 中证500)

### Data types
- OHLCV time series
- Index constituents
- Index weights
- **Financial statements** (income / balance sheet / cashflow)
- **Key financial metrics** (ROE, ROA, margins, EPS, debt ratios)
- **Shareholder structure** (major holders, institutional, top-10 circulating)
- **Capital / valuation metrics** (market cap, PE, PB, dividend yield, beta, etc.)
- Backtest export files

### Verified no-token sources
- Crypto: `CCXT`
- U.S. stocks: `yfinance` (OHLCV + fundamentals + holders + capital), `Stooq` (OHLCV only)
- China: `AKShare` (OHLCV + index data + fundamentals + holders + capital)

---

## File layout

- Skill doc: `/Users/joe/Documents/skill/skills/financial-data-fetching/SKILL.md`
- README: `/Users/joe/Documents/skill/skills/financial-data-fetching/README.md`
- Python package: `/Users/joe/Documents/skill/skills/financial-data-fetching/financial_data/`
- CLI: `/Users/joe/Documents/skill/skills/financial-data-fetching/scripts/financial_data_cli.py`
- Probe script: `/Users/joe/Documents/skill/skills/financial-data-fetching/scripts/validate_financial_data_sources.py`
- Example script: `/Users/joe/Documents/skill/skills/financial-data-fetching/examples/python_quickstart.py`

---

## Quick start

### 1. Validate the data sources first

```bash
python /Users/joe/Documents/skill/skills/financial-data-fetching/scripts/validate_financial_data_sources.py
```

Only call a source **verified here** if its probe returns `ok: true`.

### 2. Use the CLI

#### OHLCV

```bash
# U.S. stocks
python .../scripts/financial_data_cli.py ohlcv --market us --symbol AAPL --interval 1h --period 5d --source yfinance

# Crypto
python .../scripts/financial_data_cli.py ohlcv --market crypto --exchange binance --symbol BTC/USDT --interval 1h --limit 100

# China index
python .../scripts/financial_data_cli.py ohlcv --market cn-index --symbol 000300
```

#### Fundamentals

```bash
# Key metrics (ROE, margins, EPS, etc.)
python .../scripts/financial_data_cli.py fundamentals --market us --symbol AAPL --report key_metrics

# Income statement
python .../scripts/financial_data_cli.py fundamentals --market us --symbol AAPL --report income --freq quarterly

# China A-share key indicators
python .../scripts/financial_data_cli.py fundamentals --market cn --symbol 600519 --report key_metrics

# China balance sheet
python .../scripts/financial_data_cli.py fundamentals --market cn --symbol 600519 --report balance
```

#### Holders

```bash
# U.S. major holders
python .../scripts/financial_data_cli.py holders --market us --symbol AAPL --type major

# U.S. institutional holders
python .../scripts/financial_data_cli.py holders --market us --symbol AAPL --type institutional

# China top-10 circulating shareholders
python .../scripts/financial_data_cli.py holders --market cn --symbol 600519 --type top10
```

#### Capital / Valuation Metrics

```bash
# U.S. stock capital metrics
python .../scripts/financial_data_cli.py capital --market us --symbol AAPL

# China A-share capital metrics
python .../scripts/financial_data_cli.py capital --market cn --symbol 600519
```

#### Constituents & Weights

```bash
python .../scripts/financial_data_cli.py constituents --index 000905
python .../scripts/financial_data_cli.py weights --index 000300
```

---

## Python usage

```python
from pathlib import Path
import sys

ROOT = Path('/Users/joe/Documents/skill/skills/financial-data-fetching')
sys.path.insert(0, str(ROOT))

from financial_data import MarketDataClient

client = MarketDataClient()

# ── OHLCV ───────────────────────────────────────────────
result = client.fetch_ohlcv(market='us', symbol='AAPL', interval='1h', period='5d', source='yfinance')
print(result.metadata())

# ── Fundamentals ────────────────────────────────────────
# Key metrics (ROE, margins, EPS)
metrics = client.fetch_fundamentals(market='us', symbol='AAPL', report='key_metrics')
print(metrics.data)

# Income statement
income = client.fetch_fundamentals(market='us', symbol='AAPL', report='income', freq='yearly')
print(income.data.head())

# China A-share financial indicators
cn_metrics = client.fetch_fundamentals(market='cn', symbol='600519', report='key_metrics')
print(cn_metrics.data)

# ── Holders ─────────────────────────────────────────────
# U.S. institutional holders
holders = client.fetch_holders(market='us', symbol='AAPL', holder_type='institutional')
print(holders.data)

# China top-10 circulating shareholders
cn_holders = client.fetch_holders(market='cn', symbol='600519', holder_type='top10')
print(cn_holders.data)

# ── Capital Metrics ─────────────────────────────────────
capital = client.fetch_capital_metrics(market='us', symbol='AAPL')
print(capital.data)
```

---

## Backtest export schemas

### `generic`
Columns: `timestamp`, `open`, `high`, `low`, `close`, `volume`, optional `symbol`, `market`, `source`, `adj_close`

### `vectorbt`
datetime index, columns: `Open`, `High`, `Low`, `Close`, `Volume`, optional `Adj Close`

### `backtrader`
datetime index named `datetime`, columns: `open`, `high`, `low`, `close`, `volume`, `openinterest`

---

## Source selection guidance

### Crypto
Prefer `CCXT` for unified exchange access.

### U.S. stocks
- Prefer `yfinance` for OHLCV + fundamentals + holders + capital metrics.
- Prefer `Stooq` when daily public OHLCV fallback is enough.
- Treat both as **research-grade**, not exchange-direct execution-grade feeds.

### China A-shares / indices
Use `AKShare` for all data types: OHLCV, index data, fundamentals, holders, and capital metrics.

---

## Data quality checklist

Before using any fetched dataset in research or backtests, verify:
- timestamps are parseable and monotonic
- no duplicate timestamps
- missing bars / suspicious gaps
- adjusted vs unadjusted status
- stale or incomplete latest bars
- CSI constituent counts match expectations
- CSI weights sum to approximately 100
- financial statement periods are consecutive and non-duplicate
- holder data contains expected fields

---

## Troubleshooting

### Parquet export fails
Install one of: `pyarrow`, `fastparquet`

### A source works yesterday but fails today
Re-run the probe script. Do not assume a previously working public endpoint is still healthy.

### Need strategy help rather than data help
Use: `/Users/joe/Documents/skill/skills/algo-trading/SKILL.md`
