---
name: financial-data-fetching
description: |
  Fetch, validate, normalize, and export real financial market data: OHLCV,
  OHLCV, capital metrics, and backtest exports for U.S. equities, China
  A-shares, and crypto.
  Expert in Rust-native Yahoo/crypto/Eastmoney/Stooq fetchers. Use proactively when asked
  for market data pipelines, 行情获取, 真实数据, capital metrics, or
  backtest-ready exports.
metadata:
  version: "2.0.0"
  platforms: [codex]
  category: finance
  tags:
    - financial-data
    - market-data
    - data-fetching
    - api-validation
    - capital-metrics
    - crypto
    - us-stocks
    - csi300
    - csi500
    - stooq
    - parquet
    - vectorbt
    - backtrader
risk: medium
source: local
runtime_requirements:
  rust:
    - cargo
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - financial data
  - market data
  - data fetching
  - api validation
  - fundamentals
  - roe

---

# financial-data-fetching

This skill owns **real financial data acquisition and validation**. It should be selected before broad quant/trading skills when the main job is to fetch, verify, normalize, or export market data — including OHLCV and capital/valuation metrics.

## When to use

- Pulling **real** market data through public APIs or exchange APIs
- Fetching **capital/valuation metrics** (market cap, PE, PB, dividend yield, beta, etc.)
- Verifying whether a data source is actually usable in the current environment
- Building or reusing a financial data loader / data-ingestion module
- Exporting backtest-ready data for `generic`, `vectorbt`, or `backtrader`
- Working with:
  - crypto OHLCV / tickers / order books
  - U.S. stock data without tokens
  - A-share individual stock capital data
- Best for requests like:
  - "拉真实行情数据"
  - "验证这个金融数据 API 能不能用"
  - "不要 token，抓美股数据"
  - "导出 vectorbt/backtrader 回测输入"
  - "查看 AAPL 的市值和 PE"
  - "获取 A 股市值和 PE"

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
- capital / valuation metric snapshots

This skill does **not** own:
- trading signal design
- portfolio construction logic
- **Dual-Dimension Audit (Pre: Provider/Probe, Post: Data-Fidelity/Normalization Results)** → runtime verification gate
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
3. Run the Rust validation command before calling a source verified here.
4. Fetch through the Rust CLI instead of re-implementing loaders.
5. Validate duplicates, gaps, stale bars, constituent counts, and weight sums.
6. Export in the user's requested schema and format.

## Primary assets

- Rust core: `/Users/joe/Documents/skill/rust_tools/financial_data_rs/`
- README: `/Users/joe/Documents/skill/skills/financial-data-fetching/README.md`

## Rust runtime

- Rust owns active local execution: crypto / U.S. OHLCV fetch, U.S. and China capital snapshots, retries, timeouts, concurrent probes, and backtest export.
- Statements, detailed fundamentals, holders, CSI constituents, and CSI weights are outside active local execution until Rust-owned equivalents exist.

## Verified no-token source map

### Crypto
- Rust-owned exchange endpoints:
  - Binance `BTC/USDT`
  - Kraken `BTC/USD`
  - Coinbase `BTC/USD`

### U.S. equities / ETFs
- Rust Yahoo chart fetcher for:
  - OHLCV (default hot path)
  - lightweight capital/price metadata snapshot
- `Stooq` as an environment-dependent daily fallback (currently apikey-gated in this workspace)
- Default to these no-token sources unless the user explicitly asks for credential-gated providers

### China A-shares / indices
- Rust Eastmoney spot path for capital / valuation snapshots

## CLI examples

```bash
# OHLCV
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  ohlcv --market us --symbol AAPL --interval 1h --period 5d --source yahoo

# Capital metrics
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  capital --market us --symbol AAPL
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  capital --market cn --symbol 600519

# Backtest export
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- \
  export --market us --symbol AAPL --interval 1d --period 1y \
  --schema vectorbt --file-format csv --output output/financial-data/aapl.csv
```

## Validation gate

Run before calling a provider verified in this environment:

```bash
cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/financial_data_rs/Cargo.toml -- validate
```

- Only call a source **verified here** if its probe returns `ok: true`.
- **Superior Quality Audit**: For research-grade financial pipelines, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "做个金融数据获取 skill"
- "拉真实行情并验证 API"
- "做 market data loader"
- "不要 token，抓美股行情"
- "导出 backtrader 输入数据"
- "获取 A 股市值和 PE"
- "强制进行金融数据审计 / 检查行情数据真实性与标准化结果。"
- "Use the runtime verification gate to audit this data fetcher for normalization-fidelity idealism."
