# financial-data-fetching

A dedicated Codex skill for **real financial market data fetching, validation, normalization, and backtest-data export**.

Use this skill when the main task is to:
- fetch real crypto or U.S. stock OHLCV data
- fetch U.S. or China capital / valuation snapshots
- verify whether the Rust-owned market-data probes work here
- export backtest-ready data for `generic`, `vectorbt`, or `backtrader`

This skill owns **data acquisition, validation, normalization, and export only**. It must **not** own any task that decides strategy, alpha, execution, or risk. If the task includes any of those decisions, start with [`algo-trading`](../algo-trading/SKILL.md) and use this skill only as a supporting data tool.

---

## What this skill supports

### Markets
- Crypto OHLCV through Rust HTTP clients for Binance, Kraken, and Coinbase
- U.S. equities / ETFs OHLCV through the Rust Yahoo chart path
- U.S. and China capital / valuation snapshots

### Data types
- OHLCV time series
- Capital / valuation metrics
- Backtest export files

### Current source status
- Crypto: Rust native HTTP clients for `binance`, `kraken`, `coinbase`
- U.S. stocks: Rust native Yahoo chart fetcher for OHLCV and lightweight capital snapshots
- U.S. `Stooq`: optional Rust daily OHLCV fallback when public CSV access works
- China: Rust Eastmoney path for capital snapshots

Statements, detailed fundamentals, holders, CSI constituents, and CSI weights are outside active local execution until Rust-owned equivalents exist.

---

## File layout

- Skill doc: `/Users/joe/Documents/skill/skills/financial-data-fetching/SKILL.md`
- README: `/Users/joe/Documents/skill/skills/financial-data-fetching/README.md`
- Rust CLI: `/Users/joe/Documents/skill/rust_tools/financial_data_rs/`

---

## Quick start

### 1. Validate the Rust-owned sources first

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- validate
```

Only call a source **verified here** if its probe returns `ok: true`.

### 2. Use the Rust CLI

#### OHLCV

```bash
# U.S. stocks
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  ohlcv --market us --symbol AAPL --interval 1h --period 5d --source yahoo

# Crypto
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  ohlcv --market crypto --exchange binance --symbol BTC/USDT --interval 1h --limit 100
```

#### Capital / Valuation Metrics

```bash
# U.S. stock capital metrics
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  capital --market us --symbol AAPL

# China A-share capital metrics
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  capital --market cn --symbol 600519
```

#### Backtest Export

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  export --market us --symbol AAPL --interval 1d --period 1y \
  --schema vectorbt --file-format csv --output output/financial-data/aapl.csv
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

## Data quality checklist

Before using any fetched dataset in research or backtests, verify:
- timestamps are parseable and monotonic
- no duplicate timestamps
- missing bars / suspicious gaps
- adjusted vs unadjusted status
- stale or incomplete latest bars
- source, symbol/code, timezone, and schema are recorded

---

## Troubleshooting

### A source worked yesterday but fails today
Re-run `validate`. Do not assume a previously working public endpoint is still healthy.

### Need strategy help rather than data help
Use: `/Users/joe/Documents/skill/skills/algo-trading/SKILL.md`
