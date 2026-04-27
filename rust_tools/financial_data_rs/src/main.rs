use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser)]
#[command(author, version, about = "Rust-first financial market data CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch OHLCV data through Rust-native HTTP clients.
    Ohlcv(OhlcvArgs),
    /// Fetch OHLCV data and export it in a backtest-friendly schema.
    Export(ExportArgs),
    /// Fetch lightweight U.S. valuation/capital metrics from Yahoo chart metadata.
    Capital(CapitalArgs),
    /// Validate Rust-owned data probes concurrently.
    Validate,
}

#[derive(Args, Clone)]
struct OhlcvArgs {
    #[arg(long, value_enum)]
    market: Market,
    #[arg(long)]
    symbol: String,
    #[arg(long, default_value = "binance")]
    exchange: String,
    #[arg(long, default_value = "1d")]
    interval: String,
    #[arg(long, default_value_t = 200)]
    limit: usize,
    #[arg(long, default_value = "1mo")]
    period: String,
    #[arg(long, value_enum, default_value = "auto")]
    source: UsSource,
    #[arg(long, default_value_t = false)]
    adjusted: bool,
    #[arg(long, value_enum, default_value = "json")]
    format: OutputFormat,
}

impl OhlcvArgs {
    fn validate(&self) -> Result<()> {
        if self.limit == 0 {
            bail!("--limit must be greater than zero");
        }
        if self.market == Market::Crypto && self.limit > 1000 {
            bail!("--limit must be at most 1000 for crypto OHLCV");
        }
        if self.market == Market::Us && self.adjusted && self.source == UsSource::Stooq {
            bail!("Stooq does not support adjusted OHLCV in the Rust path");
        }
        Ok(())
    }
}

#[derive(Args, Clone)]
struct ExportArgs {
    #[command(flatten)]
    ohlcv: OhlcvArgs,
    #[arg(long, value_enum, default_value = "generic")]
    schema: BacktestSchema,
    #[arg(long = "file-format", value_enum, default_value = "csv")]
    file_format: FileFormat,
    #[arg(long)]
    output: String,
    #[arg(long)]
    metadata_output: Option<String>,
}

#[derive(Args, Clone)]
struct CapitalArgs {
    #[arg(long, value_enum)]
    market: CapitalMarket,
    #[arg(long)]
    symbol: String,
    #[arg(long, value_enum, default_value = "json")]
    format: OutputFormat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Market {
    Crypto,
    Us,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CapitalMarket {
    Us,
    Cn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum UsSource {
    Auto,
    Yahoo,
    Stooq,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
    Csv,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BacktestSchema {
    Generic,
    Vectorbt,
    Backtrader,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum FileFormat {
    Csv,
    Json,
}

#[derive(Clone)]
struct HttpClient {
    client: Client,
    retries: usize,
}

impl HttpClient {
    fn new() -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .user_agent("financial-data-fetching-rs/1.0")
            .build()
            .context("failed to build reqwest client")?;
        Ok(Self { client, retries: 5 })
    }

    async fn get_json(
        &self,
        url: &str,
        query: &[(&str, String)],
        headers: &[(&str, &str)],
    ) -> Result<Value> {
        let text = self.get_text(url, query, headers).await?;
        serde_json::from_str(&text).with_context(|| format!("failed to decode JSON from {url}"))
    }

    async fn get_text(
        &self,
        url: &str,
        query: &[(&str, String)],
        headers: &[(&str, &str)],
    ) -> Result<String> {
        let mut last_error: Option<anyhow::Error> = None;
        for attempt in 0..=self.retries {
            let mut request = self.client.get(url).query(query);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    let body = response
                        .text()
                        .await
                        .with_context(|| format!("failed to read response body from {url}"))?;
                    if status.is_success() {
                        return Ok(body);
                    }
                    last_error = Some(anyhow!(
                        "HTTP {} from {}: {}",
                        status.as_u16(),
                        url,
                        truncate(&body, 240)
                    ));
                }
                Err(error) => {
                    last_error = Some(anyhow!(error).context(format!("request failed for {url}")));
                }
            }

            if attempt < self.retries {
                sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("request failed for {url}")))
    }
}

#[derive(Debug, Clone, Serialize)]
struct OhlcvRecord {
    timestamp: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    adj_close: Option<f64>,
    symbol: String,
    market: String,
    source: String,
}

impl OhlcvRecord {
    fn timestamp_utc(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.timestamp)
            .ok()
            .map(|value| value.with_timezone(&Utc))
    }
}

#[derive(Debug, Clone)]
struct FetchResult {
    dataset: String,
    source: String,
    market: String,
    symbol: String,
    interval: Option<String>,
    timezone: Option<String>,
    adjusted: Option<bool>,
    fetched_at_utc: String,
    records: Vec<OhlcvRecord>,
    notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct GenericResult {
    dataset: String,
    source: String,
    market: String,
    symbol: String,
    interval: Option<String>,
    timezone: Option<String>,
    adjusted: Option<bool>,
    fetched_at_utc: String,
    records: Vec<Value>,
    notes: Vec<String>,
}

impl GenericResult {
    fn metadata(&self) -> Value {
        json!({
            "dataset": self.dataset,
            "source": self.source,
            "market": self.market,
            "symbol": self.symbol,
            "interval": self.interval,
            "timezone": self.timezone,
            "adjusted": self.adjusted,
            "fetched_at_utc": self.fetched_at_utc,
            "row_count": self.records.len(),
            "columns": self.columns(),
            "notes": self.notes,
        })
    }

    fn columns(&self) -> Vec<String> {
        self.records
            .first()
            .and_then(Value::as_object)
            .map(|record| record.keys().cloned().collect())
            .unwrap_or_default()
    }
}

impl FetchResult {
    fn metadata(&self) -> Value {
        json!({
            "dataset": self.dataset,
            "source": self.source,
            "market": self.market,
            "symbol": self.symbol,
            "interval": self.interval,
            "timezone": self.timezone,
            "adjusted": self.adjusted,
            "fetched_at_utc": self.fetched_at_utc,
            "row_count": self.records.len(),
            "columns": self.columns(),
            "notes": self.notes,
        })
    }

    fn columns(&self) -> Vec<&'static str> {
        let mut cols = vec!["timestamp", "open", "high", "low", "close"];
        if self.has_adj_close() {
            cols.push("adj_close");
        }
        cols.extend(["volume", "symbol", "market", "source"]);
        cols
    }

    fn has_adj_close(&self) -> bool {
        self.records.iter().any(|record| record.adj_close.is_some())
    }
}

#[derive(Serialize)]
struct ProbeResult {
    name: String,
    ok: bool,
    details: Value,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let http = HttpClient::new()?;

    match cli.command {
        Commands::Ohlcv(args) => {
            args.validate()?;
            let result = fetch_ohlcv(&http, &args).await?;
            emit_result(&result, args.format)?;
        }
        Commands::Export(args) => {
            args.ohlcv.validate()?;
            let result = fetch_ohlcv(&http, &args.ohlcv).await?;
            export_backtest(&result, &args)?;
        }
        Commands::Capital(args) => {
            let result = fetch_capital_metrics(&http, &args).await?;
            emit_generic_result(&result, args.format)?;
        }
        Commands::Validate => {
            let payload = run_validate(&http).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&payload)
                    .context("failed to serialize validate payload")?
            );
        }
    }

    Ok(())
}

async fn fetch_ohlcv(http: &HttpClient, args: &OhlcvArgs) -> Result<FetchResult> {
    match args.market {
        Market::Crypto => fetch_crypto_ohlcv(http, args).await,
        Market::Us => fetch_us_ohlcv(http, args).await,
    }
}

async fn fetch_crypto_ohlcv(http: &HttpClient, args: &OhlcvArgs) -> Result<FetchResult> {
    let exchange = args.exchange.to_lowercase();
    match exchange.as_str() {
        "binance" => fetch_binance_ohlcv(http, &args.symbol, &args.interval, args.limit).await,
        "coinbase" => fetch_coinbase_ohlcv(http, &args.symbol, &args.interval, args.limit).await,
        "kraken" => fetch_kraken_ohlcv(http, &args.symbol, &args.interval, args.limit).await,
        _ => bail!(
            "unsupported crypto exchange for Rust path: {}",
            args.exchange
        ),
    }
}

async fn fetch_us_ohlcv(http: &HttpClient, args: &OhlcvArgs) -> Result<FetchResult> {
    let attempts = match args.source {
        UsSource::Auto if args.adjusted => vec![UsSource::Yahoo],
        UsSource::Auto => vec![UsSource::Yahoo, UsSource::Stooq],
        UsSource::Stooq if args.adjusted => {
            bail!("Stooq does not support adjusted OHLCV in the Rust path")
        }
        source => vec![source],
    };
    let mut last_error: Option<anyhow::Error> = None;

    for source in attempts {
        let attempt = match source {
            UsSource::Yahoo => {
                fetch_yahoo_ohlcv(
                    http,
                    &args.symbol,
                    &args.interval,
                    &args.period,
                    args.adjusted,
                )
                .await
            }
            UsSource::Stooq => fetch_stooq_ohlcv(http, &args.symbol).await,
            UsSource::Auto => unreachable!(),
        };

        match attempt {
            Ok(result) => return Ok(result),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("all U.S. OHLCV sources failed for {}", args.symbol)))
}

async fn fetch_binance_ohlcv(
    http: &HttpClient,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<FetchResult> {
    let normalized = symbol.replace('/', "").to_uppercase();
    let payload = http
        .get_json(
            "https://api.binance.com/api/v3/klines",
            &[
                ("symbol", normalized.clone()),
                ("interval", interval.to_string()),
                ("limit", limit.to_string()),
            ],
            &[],
        )
        .await?;

    let rows = payload
        .as_array()
        .context("unexpected Binance payload shape")?;
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row.as_array().context("unexpected Binance kline row")?;
        if row.len() < 6 {
            continue;
        }
        records.push(OhlcvRecord {
            timestamp: epoch_millis_to_iso(value_to_i64(&row[0])?)?,
            open: value_to_f64(&row[1])?,
            high: value_to_f64(&row[2])?,
            low: value_to_f64(&row[3])?,
            close: value_to_f64(&row[4])?,
            volume: value_to_f64(&row[5])?,
            adj_close: None,
            symbol: symbol.to_string(),
            market: "crypto".to_string(),
            source: "binance".to_string(),
        });
    }

    finalize_result(FetchResult {
        dataset: "ohlcv".to_string(),
        source: "binance".to_string(),
        market: "crypto".to_string(),
        symbol: symbol.to_string(),
        interval: Some(interval.to_string()),
        timezone: Some("UTC".to_string()),
        adjusted: Some(false),
        fetched_at_utc: now_utc(),
        records,
        notes: vec!["exchange-native HTTP API".to_string()],
    })
}

async fn fetch_coinbase_ohlcv(
    http: &HttpClient,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<FetchResult> {
    let product_id = symbol.replace('/', "-").to_uppercase();
    let granularity = coinbase_granularity(interval)?;
    let payload = http
        .get_json(
            &format!("https://api.exchange.coinbase.com/products/{product_id}/candles"),
            &[("granularity", granularity.to_string())],
            &[("Accept", "application/json")],
        )
        .await?;

    let rows = payload
        .as_array()
        .context("unexpected Coinbase payload shape")?;
    let mut records = Vec::with_capacity(rows.len());
    for row in rows.iter().take(limit) {
        let row = row.as_array().context("unexpected Coinbase candle row")?;
        if row.len() < 6 {
            continue;
        }
        records.push(OhlcvRecord {
            timestamp: epoch_seconds_to_iso(value_to_i64(&row[0])?)?,
            low: value_to_f64(&row[1])?,
            high: value_to_f64(&row[2])?,
            open: value_to_f64(&row[3])?,
            close: value_to_f64(&row[4])?,
            volume: value_to_f64(&row[5])?,
            adj_close: None,
            symbol: symbol.to_string(),
            market: "crypto".to_string(),
            source: "coinbase".to_string(),
        });
    }

    finalize_result(FetchResult {
        dataset: "ohlcv".to_string(),
        source: "coinbase".to_string(),
        market: "crypto".to_string(),
        symbol: symbol.to_string(),
        interval: Some(interval.to_string()),
        timezone: Some("UTC".to_string()),
        adjusted: Some(false),
        fetched_at_utc: now_utc(),
        records,
        notes: vec!["exchange-native HTTP API".to_string()],
    })
}

async fn fetch_kraken_ohlcv(
    http: &HttpClient,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<FetchResult> {
    let pair = kraken_pair(symbol)?;
    let interval_minutes = kraken_interval_minutes(interval)?;
    let payload = http
        .get_json(
            "https://api.kraken.com/0/public/OHLC",
            &[
                ("pair", pair.clone()),
                ("interval", interval_minutes.to_string()),
            ],
            &[],
        )
        .await?;

    let errors = payload
        .get("error")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !errors.is_empty() {
        bail!("Kraken returned errors: {}", Value::Array(errors));
    }

    let result = payload
        .get("result")
        .and_then(Value::as_object)
        .context("unexpected Kraken payload shape")?;
    let candles = result
        .iter()
        .find(|(key, _)| key.as_str() != "last")
        .map(|(_, value)| value)
        .and_then(Value::as_array)
        .context("Kraken payload missing OHLC series")?;

    let start = candles.len().saturating_sub(limit);
    let mut records = Vec::with_capacity(candles.len() - start);
    for row in candles.iter().skip(start) {
        let row = row.as_array().context("unexpected Kraken OHLC row")?;
        if row.len() < 7 {
            continue;
        }
        records.push(OhlcvRecord {
            timestamp: epoch_seconds_to_iso(value_to_i64(&row[0])?)?,
            open: value_to_f64(&row[1])?,
            high: value_to_f64(&row[2])?,
            low: value_to_f64(&row[3])?,
            close: value_to_f64(&row[4])?,
            volume: value_to_f64(&row[6])?,
            adj_close: None,
            symbol: symbol.to_string(),
            market: "crypto".to_string(),
            source: "kraken".to_string(),
        });
    }

    finalize_result(FetchResult {
        dataset: "ohlcv".to_string(),
        source: "kraken".to_string(),
        market: "crypto".to_string(),
        symbol: symbol.to_string(),
        interval: Some(interval.to_string()),
        timezone: Some("UTC".to_string()),
        adjusted: Some(false),
        fetched_at_utc: now_utc(),
        records,
        notes: vec!["exchange-native HTTP API".to_string()],
    })
}

async fn fetch_yahoo_ohlcv(
    http: &HttpClient,
    symbol: &str,
    interval: &str,
    period: &str,
    adjusted: bool,
) -> Result<FetchResult> {
    let payload = http
        .get_json(
            &format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbol}"),
            &[
                ("interval", yahoo_interval(interval)?.to_string()),
                ("range", period.to_string()),
                ("includePrePost", "false".to_string()),
                ("events", "div,splits".to_string()),
                ("includeAdjustedClose", "true".to_string()),
            ],
            &[],
        )
        .await?;

    let result = payload
        .get("chart")
        .and_then(|chart| chart.get("result"))
        .and_then(Value::as_array)
        .and_then(|results| results.first())
        .context("unexpected Yahoo Finance payload shape")?;
    let meta = result.get("meta").cloned().unwrap_or(Value::Null);
    let timezone = meta
        .get("exchangeTimezoneName")
        .and_then(Value::as_str)
        .unwrap_or("UTC")
        .to_string();
    let timestamps = result
        .get("timestamp")
        .and_then(Value::as_array)
        .context("Yahoo payload missing timestamps")?;
    let quote = result
        .get("indicators")
        .and_then(|indicators| indicators.get("quote"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .context("Yahoo payload missing quote block")?;
    let adjclose = result
        .get("indicators")
        .and_then(|indicators| indicators.get("adjclose"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("adjclose"))
        .and_then(Value::as_array);

    let opens = value_array(quote.get("open"))?;
    let highs = value_array(quote.get("high"))?;
    let lows = value_array(quote.get("low"))?;
    let closes = value_array(quote.get("close"))?;
    let volumes = value_array(quote.get("volume"))?;

    let mut records = Vec::new();
    for (index, timestamp_value) in timestamps.iter().enumerate() {
        let timestamp = value_to_i64(timestamp_value)?;
        let open = opt_value_to_f64(opens.get(index))?;
        let high = opt_value_to_f64(highs.get(index))?;
        let low = opt_value_to_f64(lows.get(index))?;
        let close = opt_value_to_f64(closes.get(index))?;
        let volume = opt_value_to_f64(volumes.get(index))?;
        if let (Some(open), Some(high), Some(low), Some(close), Some(volume)) =
            (open, high, low, close, volume)
        {
            let adj_value = adjclose
                .and_then(|items| items.get(index))
                .map(value_to_f64)
                .transpose()?;
            let (open, high, low, close) = if adjusted {
                let adj_close =
                    adj_value.context("Yahoo payload missing adjusted close for adjusted fetch")?;
                if close == 0.0 {
                    bail!("cannot adjust Yahoo OHLCV with zero close for {symbol}");
                }
                let factor = adj_close / close;
                (open * factor, high * factor, low * factor, adj_close)
            } else {
                (open, high, low, close)
            };
            records.push(OhlcvRecord {
                timestamp: epoch_seconds_to_iso(timestamp)?,
                open,
                high,
                low,
                close,
                volume,
                adj_close: if adjusted { None } else { adj_value },
                symbol: symbol.to_string(),
                market: "us".to_string(),
                source: "yahoo".to_string(),
            });
        }
    }

    finalize_result(FetchResult {
        dataset: "ohlcv".to_string(),
        source: "yahoo".to_string(),
        market: "us".to_string(),
        symbol: symbol.to_string(),
        interval: Some(interval.to_string()),
        timezone: Some(timezone),
        adjusted: Some(adjusted),
        fetched_at_utc: now_utc(),
        records,
        notes: vec![
            format!("period={period}"),
            "public chart endpoint".to_string(),
            if adjusted {
                "OHLC adjusted with Yahoo adjclose ratio".to_string()
            } else {
                "unadjusted OHLC with adj_close column when available".to_string()
            },
        ],
    })
}

async fn fetch_stooq_ohlcv(http: &HttpClient, symbol: &str) -> Result<FetchResult> {
    let normalized = format!("{}.us", symbol.to_lowercase());
    let csv_text = http
        .get_text(
            "https://stooq.com/q/d/l/",
            &[("s", normalized), ("i", "d".to_string())],
            &[],
        )
        .await?;
    let text = csv_text.trim();
    if text.is_empty() || text.to_lowercase().starts_with("no data") {
        bail!("stooq returned no data for {symbol}");
    }
    if text.contains("get_apikey") {
        bail!("stooq now requires an apikey flow in this environment");
    }

    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let mut records = Vec::new();
    for row in reader.deserialize::<StooqRow>() {
        let row = row.context("failed to parse Stooq CSV row")?;
        records.push(OhlcvRecord {
            timestamp: date_to_utc_iso(&row.date)?,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume,
            adj_close: None,
            symbol: symbol.to_uppercase(),
            market: "us".to_string(),
            source: "stooq".to_string(),
        });
    }

    finalize_result(FetchResult {
        dataset: "ohlcv".to_string(),
        source: "stooq".to_string(),
        market: "us".to_string(),
        symbol: symbol.to_uppercase(),
        interval: Some("1d".to_string()),
        timezone: Some("UTC".to_string()),
        adjusted: Some(false),
        fetched_at_utc: now_utc(),
        records,
        notes: vec![
            "no-token daily csv source".to_string(),
            "daily-only".to_string(),
        ],
    })
}

async fn fetch_capital_metrics(http: &HttpClient, args: &CapitalArgs) -> Result<GenericResult> {
    match args.market {
        CapitalMarket::Us => fetch_us_capital_metrics(http, &args.symbol).await,
        CapitalMarket::Cn => fetch_cn_capital_metrics(http, &args.symbol).await,
    }
}

async fn fetch_us_capital_metrics(http: &HttpClient, symbol: &str) -> Result<GenericResult> {
    let payload = http
        .get_json(
            &format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbol}"),
            &[
                ("interval", "1d".to_string()),
                ("range", "5d".to_string()),
                ("includePrePost", "false".to_string()),
                ("events", "div,splits".to_string()),
            ],
            &[],
        )
        .await?;

    let result = payload
        .get("chart")
        .and_then(|chart| chart.get("result"))
        .and_then(Value::as_array)
        .and_then(|results| results.first())
        .context("unexpected Yahoo Finance payload shape")?;
    let meta = result.get("meta").context("Yahoo payload missing meta")?;

    let record = json!({
        "symbol": symbol,
        "currency": meta.get("currency").cloned().unwrap_or(Value::Null),
        "exchange": meta.get("exchangeName").cloned().unwrap_or(Value::Null),
        "instrument_type": meta.get("instrumentType").cloned().unwrap_or(Value::Null),
        "regular_market_price": meta.get("regularMarketPrice").cloned().unwrap_or(Value::Null),
        "chart_previous_close": meta.get("chartPreviousClose").cloned().unwrap_or(Value::Null),
        "previous_close": meta.get("previousClose").cloned().unwrap_or(Value::Null),
        "fifty_two_week_high": meta.get("fiftyTwoWeekHigh").cloned().unwrap_or(Value::Null),
        "fifty_two_week_low": meta.get("fiftyTwoWeekLow").cloned().unwrap_or(Value::Null),
        "regular_market_volume": meta.get("regularMarketVolume").cloned().unwrap_or(Value::Null),
        "first_trade_date": meta
            .get("firstTradeDate")
            .and_then(Value::as_i64)
            .map(epoch_seconds_to_iso)
            .transpose()?,
    });

    Ok(GenericResult {
        dataset: "capital_metrics".to_string(),
        source: "yahoo:chart-meta".to_string(),
        market: "us".to_string(),
        symbol: symbol.to_string(),
        interval: None,
        timezone: meta
            .get("exchangeTimezoneName")
            .and_then(Value::as_str)
            .map(str::to_string),
        adjusted: None,
        fetched_at_utc: now_utc(),
        records: vec![record],
        notes: vec![
            "Rust-native Yahoo chart metadata".to_string(),
            "valuation snapshot is lighter than yfinance Ticker.info".to_string(),
        ],
    })
}

async fn fetch_cn_capital_metrics(http: &HttpClient, symbol: &str) -> Result<GenericResult> {
    let code = normalize_cn_stock_code(symbol);
    let mut matched: Option<Value> = None;
    for page in 1..=60 {
        let payload = http
            .get_json(
                "https://push2.eastmoney.com/api/qt/clist/get",
                &[
                    ("pn", page.to_string()),
                    ("pz", "100".to_string()),
                    ("po", "1".to_string()),
                    ("np", "1".to_string()),
                    ("ut", "bd1d9ddb04089700cf9c27f6f7426281".to_string()),
                    ("fltt", "2".to_string()),
                    ("invt", "2".to_string()),
                    ("fid", "f3".to_string()),
                    ("fs", "m:0+t:6,m:0+t:80,m:1+t:2,m:1+t:23".to_string()),
                    (
                        "fields",
                        "f12,f14,f2,f3,f5,f6,f7,f15,f16,f17,f18,f8,f9,f23,f20,f21,f10".to_string(),
                    ),
                ],
                &[],
            )
            .await?;

        let rows = payload
            .get("data")
            .and_then(|data| data.get("diff"))
            .and_then(Value::as_array)
            .context("Eastmoney payload missing stock rows")?;
        if let Some(row) = rows
            .iter()
            .find(|item| item.get("f12").and_then(Value::as_str) == Some(code.as_str()))
        {
            matched = Some(row.clone());
            break;
        }
        if rows.len() < 100 {
            break;
        }
    }
    let row = matched
        .as_ref()
        .with_context(|| format!("symbol {code} not found in Eastmoney A-share spot data"))?;

    let record = json!({
        "symbol": code,
        "name": row.get("f14").cloned().unwrap_or(Value::Null),
        "price": row.get("f2").cloned().unwrap_or(Value::Null),
        "change_pct": row.get("f3").cloned().unwrap_or(Value::Null),
        "volume": row.get("f5").cloned().unwrap_or(Value::Null),
        "turnover": row.get("f6").cloned().unwrap_or(Value::Null),
        "amplitude": row.get("f7").cloned().unwrap_or(Value::Null),
        "high": row.get("f15").cloned().unwrap_or(Value::Null),
        "low": row.get("f16").cloned().unwrap_or(Value::Null),
        "open": row.get("f17").cloned().unwrap_or(Value::Null),
        "prev_close": row.get("f18").cloned().unwrap_or(Value::Null),
        "turnover_rate": row.get("f8").cloned().unwrap_or(Value::Null),
        "pe_ratio": row.get("f9").cloned().unwrap_or(Value::Null),
        "pb_ratio": row.get("f23").cloned().unwrap_or(Value::Null),
        "total_market_cap": row.get("f20").cloned().unwrap_or(Value::Null),
        "circulating_market_cap": row.get("f21").cloned().unwrap_or(Value::Null),
        "volume_ratio": row.get("f10").cloned().unwrap_or(Value::Null),
    });

    Ok(GenericResult {
        dataset: "capital_metrics".to_string(),
        source: "eastmoney:qt-clist".to_string(),
        market: "cn".to_string(),
        symbol: code,
        interval: None,
        timezone: Some("Asia/Shanghai".to_string()),
        adjusted: None,
        fetched_at_utc: now_utc(),
        records: vec![record],
        notes: vec!["Rust-native Eastmoney A-share spot snapshot".to_string()],
    })
}

fn emit_result(result: &FetchResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let payload = json!({
                "metadata": result.metadata(),
                "records": result.records,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&payload)
                    .context("failed to serialize result payload")?
            );
        }
        OutputFormat::Csv => {
            let csv = records_to_csv(&result.records)?;
            print!("{csv}");
        }
    }
    Ok(())
}

fn emit_generic_result(result: &GenericResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let payload = json!({
                "metadata": result.metadata(),
                "records": result.records,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&payload)
                    .context("failed to serialize generic result payload")?
            );
        }
        OutputFormat::Csv => {
            let csv = generic_records_to_csv(&result.records)?;
            print!("{csv}");
        }
    }
    Ok(())
}

fn export_backtest(result: &FetchResult, args: &ExportArgs) -> Result<()> {
    match args.file_format {
        FileFormat::Csv => {
            let csv = backtest_csv(result, args.schema)?;
            fs::write(&args.output, csv)
                .with_context(|| format!("failed to write {}", args.output))?;
        }
        FileFormat::Json => {
            let payload = backtest_json(result, args.schema)?;
            fs::write(
                &args.output,
                serde_json::to_string_pretty(&payload)
                    .context("failed to serialize export payload")?,
            )
            .with_context(|| format!("failed to write {}", args.output))?;
        }
    }

    if let Some(metadata_output) = &args.metadata_output {
        fs::write(
            metadata_output,
            serde_json::to_string_pretty(&result.metadata())
                .context("failed to serialize metadata output")?,
        )
        .with_context(|| format!("failed to write {metadata_output}"))?;
    }

    let payload = json!({
        "output": args.output,
        "schema": schema_name(args.schema),
        "file_format": file_format_name(args.file_format),
        "metadata": result.metadata(),
        "metadata_output": args.metadata_output,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("failed to serialize export response")?
    );
    Ok(())
}

async fn run_validate(http: &HttpClient) -> Result<Value> {
    let mut tasks = FuturesUnordered::new();

    {
        let http = http.clone();
        tasks.push(tokio::spawn(async move {
            run_probe("crypto.binance.BTCUSDT.1h".to_string(), async move {
                fetch_binance_ohlcv(&http, "BTC/USDT", "1h", 5).await
            })
            .await
        }));
    }
    {
        let http = http.clone();
        tasks.push(tokio::spawn(async move {
            run_probe("crypto.kraken.BTCUSD.1h".to_string(), async move {
                fetch_kraken_ohlcv(&http, "BTC/USD", "1h", 5).await
            })
            .await
        }));
    }
    {
        let http = http.clone();
        tasks.push(tokio::spawn(async move {
            run_probe("crypto.coinbase.BTCUSD.1h".to_string(), async move {
                fetch_coinbase_ohlcv(&http, "BTC/USD", "1h", 5).await
            })
            .await
        }));
    }
    {
        let http = http.clone();
        tasks.push(tokio::spawn(async move {
            run_probe("us.yahoo.AAPL.1h".to_string(), async move {
                fetch_yahoo_ohlcv(&http, "AAPL", "1h", "5d", false).await
            })
            .await
        }));
    }
    {
        let http = http.clone();
        tasks.push(tokio::spawn(async move {
            run_probe("us.stooq.AAPL.1d".to_string(), async move {
                fetch_stooq_ohlcv(&http, "AAPL").await
            })
            .await
        }));
    }
    {
        let http = http.clone();
        tasks.push(tokio::spawn(async move {
            run_generic_probe("us.capital.AAPL".to_string(), async move {
                fetch_us_capital_metrics(&http, "AAPL").await
            })
            .await
        }));
    }
    {
        let http = http.clone();
        tasks.push(tokio::spawn(async move {
            run_generic_probe("cn.capital.600519".to_string(), async move {
                fetch_cn_capital_metrics(&http, "600519").await
            })
            .await
        }));
    }

    let mut results = Vec::new();
    while let Some(result) = tasks.next().await {
        results.push(result.context("validate probe task join failed")?);
    }
    results.sort_by(|left, right| left.name.cmp(&right.name));

    let ok_count = results.iter().filter(|item| item.ok).count();
    let fail_count = results.len().saturating_sub(ok_count);

    Ok(json!({
        "generated_at_utc": now_utc(),
        "summary": {
            "probe_count": results.len(),
            "ok_count": ok_count,
            "fail_count": fail_count,
        },
        "results": results,
    }))
}

async fn run_probe<F>(name: String, future: F) -> ProbeResult
where
    F: std::future::Future<Output = Result<FetchResult>>,
{
    match future.await {
        Ok(result) => ProbeResult {
            name,
            ok: true,
            details: summarize_probe(&result),
            error: None,
        },
        Err(error) => ProbeResult {
            name,
            ok: false,
            details: json!({}),
            error: Some(format!("{error:#}")),
        },
    }
}

async fn run_generic_probe<F>(name: String, future: F) -> ProbeResult
where
    F: std::future::Future<Output = Result<GenericResult>>,
{
    match future.await {
        Ok(result) => ProbeResult {
            name,
            ok: true,
            details: summarize_generic_probe(&result),
            error: None,
        },
        Err(error) => ProbeResult {
            name,
            ok: false,
            details: json!({}),
            error: Some(format!("{error:#}")),
        },
    }
}

fn summarize_generic_probe(result: &GenericResult) -> Value {
    json!({
        "dataset": result.dataset,
        "source": result.source,
        "market": result.market,
        "symbol": result.symbol,
        "interval": result.interval,
        "timezone": result.timezone,
        "adjusted": result.adjusted,
        "fetched_at_utc": result.fetched_at_utc,
        "row_count": result.records.len(),
        "columns": result.columns(),
        "notes": result.notes,
        "preview": result.records.first(),
    })
}

fn summarize_probe(result: &FetchResult) -> Value {
    let first = result.records.first();
    let last = result.records.last();
    let stale_hours = last
        .and_then(OhlcvRecord::timestamp_utc)
        .map(|timestamp| (Utc::now() - timestamp).num_hours());
    json!({
        "dataset": result.dataset,
        "source": result.source,
        "market": result.market,
        "symbol": result.symbol,
        "interval": result.interval,
        "timezone": result.timezone,
        "adjusted": result.adjusted,
        "fetched_at_utc": result.fetched_at_utc,
        "row_count": result.records.len(),
        "columns": result.columns(),
        "notes": result.notes,
        "first_timestamp": first.map(|record| record.timestamp.clone()),
        "last_timestamp": last.map(|record| record.timestamp.clone()),
        "monotonic_increasing": is_monotonic(&result.records),
        "duplicate_timestamps": duplicate_timestamp_count(&result.records),
        "null_adj_close_count": null_adj_close_count(&result.records),
        "stale_hours": stale_hours,
        "last_close": last.map(|record| record.close),
        "last_volume": last.map(|record| record.volume),
    })
}

fn finalize_result(mut result: FetchResult) -> Result<FetchResult> {
    result
        .records
        .sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
    result
        .records
        .dedup_by(|left, right| left.timestamp == right.timestamp);
    if result.records.is_empty() {
        bail!("no OHLCV rows returned for {}", result.symbol);
    }
    if result.records.iter().any(|record| {
        !record.open.is_finite()
            || !record.high.is_finite()
            || !record.low.is_finite()
            || !record.close.is_finite()
            || !record.volume.is_finite()
            || record.volume < 0.0
            || record.low > record.high
    }) {
        bail!("invalid OHLCV values returned for {}", result.symbol);
    }
    Ok(result)
}

fn records_to_csv(records: &[OhlcvRecord]) -> Result<String> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let include_adj_close = records.iter().any(|record| record.adj_close.is_some());
    if include_adj_close {
        writer.write_record([
            "timestamp",
            "open",
            "high",
            "low",
            "close",
            "adj_close",
            "volume",
            "symbol",
            "market",
            "source",
        ])?;
        for record in records {
            writer.write_record([
                record.timestamp.clone(),
                record.open.to_string(),
                record.high.to_string(),
                record.low.to_string(),
                record.close.to_string(),
                record
                    .adj_close
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                record.volume.to_string(),
                record.symbol.clone(),
                record.market.clone(),
                record.source.clone(),
            ])?;
        }
    } else {
        writer.write_record([
            "timestamp",
            "open",
            "high",
            "low",
            "close",
            "volume",
            "symbol",
            "market",
            "source",
        ])?;
        for record in records {
            writer.write_record([
                record.timestamp.clone(),
                record.open.to_string(),
                record.high.to_string(),
                record.low.to_string(),
                record.close.to_string(),
                record.volume.to_string(),
                record.symbol.clone(),
                record.market.clone(),
                record.source.clone(),
            ])?;
        }
    }

    String::from_utf8(writer.into_inner()?).context("failed to encode CSV output as UTF-8")
}

fn generic_records_to_csv(records: &[Value]) -> Result<String> {
    let Some(first) = records.first().and_then(Value::as_object) else {
        return Ok(String::new());
    };
    let columns: Vec<String> = first.keys().cloned().collect();
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(columns.iter())?;
    for record in records {
        let object = record
            .as_object()
            .context("generic CSV export expects object records")?;
        let row = columns
            .iter()
            .map(|column| object.get(column).map(csv_value).unwrap_or_default());
        writer.write_record(row)?;
    }
    String::from_utf8(writer.into_inner()?).context("failed to encode generic CSV as UTF-8")
}

fn csv_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn backtest_csv(result: &FetchResult, schema: BacktestSchema) -> Result<String> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let include_adj_close = result.has_adj_close();
    match schema {
        BacktestSchema::Generic => {
            let mut header = vec![
                "timestamp",
                "open",
                "high",
                "low",
                "close",
                "volume",
                "symbol",
                "market",
                "source",
            ];
            if include_adj_close {
                header.push("adj_close");
            }
            writer.write_record(header)?;
            for record in &result.records {
                let mut row = vec![
                    record.timestamp.clone(),
                    record.open.to_string(),
                    record.high.to_string(),
                    record.low.to_string(),
                    record.close.to_string(),
                    record.volume.to_string(),
                    record.symbol.clone(),
                    record.market.clone(),
                    record.source.clone(),
                ];
                if include_adj_close {
                    row.push(
                        record
                            .adj_close
                            .map(|value| value.to_string())
                            .unwrap_or_default(),
                    );
                }
                writer.write_record(row)?;
            }
        }
        BacktestSchema::Vectorbt => {
            let mut header = vec!["timestamp", "Open", "High", "Low", "Close", "Volume"];
            if include_adj_close {
                header.push("Adj Close");
            }
            writer.write_record(header)?;
            for record in &result.records {
                let mut row = vec![
                    record.timestamp.clone(),
                    record.open.to_string(),
                    record.high.to_string(),
                    record.low.to_string(),
                    record.close.to_string(),
                    record.volume.to_string(),
                ];
                if include_adj_close {
                    row.push(
                        record
                            .adj_close
                            .map(|value| value.to_string())
                            .unwrap_or_default(),
                    );
                }
                writer.write_record(row)?;
            }
        }
        BacktestSchema::Backtrader => {
            writer.write_record([
                "datetime",
                "open",
                "high",
                "low",
                "close",
                "volume",
                "openinterest",
            ])?;
            for record in &result.records {
                writer.write_record([
                    record.timestamp.clone(),
                    record.open.to_string(),
                    record.high.to_string(),
                    record.low.to_string(),
                    record.close.to_string(),
                    record.volume.to_string(),
                    "0.0".to_string(),
                ])?;
            }
        }
    }
    String::from_utf8(writer.into_inner()?).context("failed to encode backtest CSV as UTF-8")
}

fn backtest_json(result: &FetchResult, schema: BacktestSchema) -> Result<Value> {
    let include_adj_close = result.has_adj_close();
    let records = match schema {
        BacktestSchema::Generic => result
            .records
            .iter()
            .map(|record| {
                let mut item = json!({
                    "timestamp": record.timestamp,
                    "open": record.open,
                    "high": record.high,
                    "low": record.low,
                    "close": record.close,
                    "volume": record.volume,
                    "symbol": record.symbol,
                    "market": record.market,
                    "source": record.source,
                });
                if include_adj_close {
                    item["adj_close"] = json!(record.adj_close);
                }
                item
            })
            .collect(),
        BacktestSchema::Vectorbt => result
            .records
            .iter()
            .map(|record| {
                let mut item = json!({
                    "timestamp": record.timestamp,
                    "Open": record.open,
                    "High": record.high,
                    "Low": record.low,
                    "Close": record.close,
                    "Volume": record.volume,
                });
                if include_adj_close {
                    item["Adj Close"] = json!(record.adj_close);
                }
                item
            })
            .collect(),
        BacktestSchema::Backtrader => result
            .records
            .iter()
            .map(|record| {
                json!({
                    "datetime": record.timestamp,
                    "open": record.open,
                    "high": record.high,
                    "low": record.low,
                    "close": record.close,
                    "volume": record.volume,
                    "openinterest": 0.0,
                })
            })
            .collect(),
    };
    Ok(Value::Array(records))
}

fn is_monotonic(records: &[OhlcvRecord]) -> bool {
    records
        .windows(2)
        .all(|window| window[0].timestamp <= window[1].timestamp)
}

fn duplicate_timestamp_count(records: &[OhlcvRecord]) -> usize {
    records
        .windows(2)
        .filter(|window| window[0].timestamp == window[1].timestamp)
        .count()
}

fn null_adj_close_count(records: &[OhlcvRecord]) -> usize {
    if !records.iter().any(|record| record.adj_close.is_some()) {
        return 0;
    }
    records
        .iter()
        .filter(|record| record.adj_close.is_none())
        .count()
}

fn yahoo_interval(interval: &str) -> Result<&str> {
    match interval {
        "1m" | "2m" | "5m" | "15m" | "30m" | "60m" | "90m" | "1d" | "5d" | "1wk" | "1mo"
        | "3mo" => Ok(interval),
        "1h" => Ok("60m"),
        _ => bail!("unsupported Yahoo interval for Rust path: {interval}"),
    }
}

fn coinbase_granularity(interval: &str) -> Result<u32> {
    match interval {
        "1m" => Ok(60),
        "5m" => Ok(300),
        "15m" => Ok(900),
        "1h" => Ok(3600),
        "6h" => Ok(21600),
        "1d" => Ok(86400),
        _ => bail!("unsupported Coinbase interval for Rust path: {interval}"),
    }
}

fn kraken_interval_minutes(interval: &str) -> Result<u32> {
    match interval {
        "1m" => Ok(1),
        "5m" => Ok(5),
        "15m" => Ok(15),
        "30m" => Ok(30),
        "1h" => Ok(60),
        "4h" => Ok(240),
        "1d" => Ok(1440),
        _ => bail!("unsupported Kraken interval for Rust path: {interval}"),
    }
}

fn kraken_pair(symbol: &str) -> Result<String> {
    let normalized = symbol.replace('-', "/").to_uppercase();
    let mut parts = normalized.split('/');
    let base = parts.next().context("missing Kraken base asset")?;
    let quote = parts.next().context("missing Kraken quote asset")?;
    if parts.next().is_some() {
        bail!("unexpected Kraken symbol format: {symbol}");
    }

    let base = match base {
        "BTC" => "XBT",
        other => other,
    };
    Ok(format!("{base}{quote}"))
}

fn normalize_cn_stock_code(symbol: &str) -> String {
    let value = symbol.trim();
    if value.len() > 2 {
        let prefix = &value[..2].to_ascii_lowercase();
        if prefix == "sh" || prefix == "sz" {
            return value[2..].to_string();
        }
    }
    value.to_string()
}

fn epoch_seconds_to_iso(seconds: i64) -> Result<String> {
    let dt = Utc
        .timestamp_opt(seconds, 0)
        .single()
        .context("invalid epoch seconds")?;
    Ok(dt.to_rfc3339())
}

fn epoch_millis_to_iso(millis: i64) -> Result<String> {
    let dt = Utc
        .timestamp_millis_opt(millis)
        .single()
        .context("invalid epoch milliseconds")?;
    Ok(dt.to_rfc3339())
}

fn date_to_utc_iso(date: &str) -> Result<String> {
    let value = format!("{date}T00:00:00+00:00");
    Ok(value)
}

fn now_utc() -> String {
    Utc::now().to_rfc3339()
}

fn value_to_i64(value: &Value) -> Result<i64> {
    if let Some(number) = value.as_i64() {
        return Ok(number);
    }
    if let Some(number) = value.as_u64() {
        return i64::try_from(number).context("numeric value does not fit in i64");
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<i64>()
            .with_context(|| format!("failed to parse integer value from {text}"));
    }
    bail!("expected integer-compatible value, got {value}")
}

fn value_to_f64(value: &Value) -> Result<f64> {
    if let Some(number) = value.as_f64() {
        return Ok(number);
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<f64>()
            .with_context(|| format!("failed to parse float value from {text}"));
    }
    bail!("expected numeric value, got {value}")
}

fn opt_value_to_f64(value: Option<&Value>) -> Result<Option<f64>> {
    match value {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(other) => value_to_f64(other).map(Some),
    }
}

fn value_array(value: Option<&Value>) -> Result<&Vec<Value>> {
    value
        .and_then(Value::as_array)
        .context("expected array in JSON payload")
}

fn schema_name(schema: BacktestSchema) -> &'static str {
    match schema {
        BacktestSchema::Generic => "generic",
        BacktestSchema::Vectorbt => "vectorbt",
        BacktestSchema::Backtrader => "backtrader",
    }
}

fn file_format_name(file_format: FileFormat) -> &'static str {
    match file_format {
        FileFormat::Csv => "csv",
        FileFormat::Json => "json",
    }
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    let truncated: String = value.chars().take(max_len).collect();
    format!("{truncated}...")
}

#[derive(serde::Deserialize)]
struct StooqRow {
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "Open")]
    open: f64,
    #[serde(rename = "High")]
    high: f64,
    #[serde(rename = "Low")]
    low: f64,
    #[serde(rename = "Close")]
    close: f64,
    #[serde(rename = "Volume")]
    volume: f64,
}
