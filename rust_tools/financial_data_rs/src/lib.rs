//! financial_data_rs library — core types, HTTP client, and fetch functions.

pub mod mcp;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

// ---------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    retries: usize,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .user_agent("financial-data-fetching-rs/1.0")
            .build()
            .context("failed to build reqwest client")?;
        Ok(Self { client, retries: 5 })
    }

    pub async fn get_json(
        &self,
        url: &str,
        query: &[(&str, String)],
        headers: &[(&str, &str)],
    ) -> Result<Value> {
        let text = self.get_text(url, query, headers).await?;
        serde_json::from_str(&text).with_context(|| format!("failed to decode JSON from {url}"))
    }

    pub async fn get_text(
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

// ---------------------------------------------------------------------------
// Core data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct OhlcvRecord {
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adj_close: Option<f64>,
    pub symbol: String,
    pub market: String,
    pub source: String,
}

impl OhlcvRecord {
    pub fn timestamp_utc(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.timestamp)
            .ok()
            .map(|value| value.with_timezone(&Utc))
    }
}

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub dataset: String,
    pub source: String,
    pub market: String,
    pub symbol: String,
    pub interval: Option<String>,
    pub timezone: Option<String>,
    pub adjusted: Option<bool>,
    pub fetched_at_utc: String,
    pub records: Vec<OhlcvRecord>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GenericResult {
    pub dataset: String,
    pub source: String,
    pub market: String,
    pub symbol: String,
    pub interval: Option<String>,
    pub timezone: Option<String>,
    pub adjusted: Option<bool>,
    pub fetched_at_utc: String,
    pub records: Vec<Value>,
    pub notes: Vec<String>,
}

impl GenericResult {
    pub fn metadata(&self) -> Value {
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

    pub fn columns(&self) -> Vec<String> {
        self.records
            .first()
            .and_then(Value::as_object)
            .map(|record| record.keys().cloned().collect())
            .unwrap_or_default()
    }
}

impl FetchResult {
    pub fn metadata(&self) -> Value {
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

    pub fn columns(&self) -> Vec<&'static str> {
        let mut cols = vec!["timestamp", "open", "high", "low", "close"];
        if self.has_adj_close() {
            cols.push("adj_close");
        }
        cols.extend(["volume", "symbol", "market", "source"]);
        cols
    }

    pub fn has_adj_close(&self) -> bool {
        self.records.iter().any(|record| record.adj_close.is_some())
    }
}

#[derive(Serialize)]
pub struct ProbeResult {
    pub name: String,
    pub ok: bool,
    pub details: Value,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Fetch functions — OHLCV
// ---------------------------------------------------------------------------

pub async fn fetch_ohlcv(
    http: &HttpClient,
    market: &str,
    symbol: &str,
    exchange: &str,
    interval: &str,
    limit: usize,
    period: &str,
    source: &str,
    adjusted: bool,
) -> Result<FetchResult> {
    match market {
        "crypto" => fetch_crypto_ohlcv(http, symbol, exchange, interval, limit).await,
        "us" => fetch_us_ohlcv(http, symbol, interval, period, source, adjusted).await,
        _ => bail!("unsupported market: {market}"),
    }
}

async fn fetch_crypto_ohlcv(
    http: &HttpClient,
    symbol: &str,
    exchange: &str,
    interval: &str,
    limit: usize,
) -> Result<FetchResult> {
    let exchange = exchange.to_lowercase();
    match exchange.as_str() {
        "binance" => fetch_binance_ohlcv(http, symbol, interval, limit).await,
        "coinbase" => fetch_coinbase_ohlcv(http, symbol, interval, limit).await,
        "kraken" => fetch_kraken_ohlcv(http, symbol, interval, limit).await,
        _ => bail!("unsupported crypto exchange for Rust path: {exchange}"),
    }
}

async fn fetch_us_ohlcv(
    http: &HttpClient,
    symbol: &str,
    interval: &str,
    period: &str,
    source: &str,
    adjusted: bool,
) -> Result<FetchResult> {
    let attempts: Vec<&str> = match source {
        "auto" if adjusted => vec!["yahoo"],
        "auto" => vec!["yahoo", "stooq"],
        "stooq" if adjusted => {
            bail!("Stooq does not support adjusted OHLCV in the Rust path")
        }
        other => vec![other],
    };
    let mut last_error: Option<anyhow::Error> = None;

    for src in attempts {
        let attempt = match src {
            "yahoo" => {
                fetch_yahoo_ohlcv(http, symbol, interval, period, adjusted).await
            }
            "stooq" => fetch_stooq_ohlcv(http, symbol).await,
            _ => unreachable!(),
        };

        match attempt {
            Ok(result) => return Ok(result),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("all U.S. OHLCV sources failed for {symbol}")))
}

pub async fn fetch_binance_ohlcv(
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

pub async fn fetch_coinbase_ohlcv(
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

pub async fn fetch_kraken_ohlcv(
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

pub async fn fetch_yahoo_ohlcv(
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

pub async fn fetch_stooq_ohlcv(http: &HttpClient, symbol: &str) -> Result<FetchResult> {
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

// ---------------------------------------------------------------------------
// Fetch functions — Capital metrics
// ---------------------------------------------------------------------------

pub async fn fetch_capital_metrics(
    http: &HttpClient,
    market: &str,
    symbol: &str,
) -> Result<GenericResult> {
    match market {
        "us" => fetch_us_capital_metrics(http, symbol).await,
        "cn" => fetch_cn_capital_metrics(http, symbol).await,
        _ => bail!("unsupported capital market: {market}"),
    }
}

pub async fn fetch_us_capital_metrics(http: &HttpClient, symbol: &str) -> Result<GenericResult> {
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

pub async fn fetch_cn_capital_metrics(http: &HttpClient, symbol: &str) -> Result<GenericResult> {
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

// ---------------------------------------------------------------------------
// Validate (data probes)
// ---------------------------------------------------------------------------

pub async fn run_validate(http: &HttpClient) -> Result<Value> {
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

// ---------------------------------------------------------------------------
// Utility / finalize
// ---------------------------------------------------------------------------

pub fn finalize_result(mut result: FetchResult) -> Result<FetchResult> {
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

pub fn records_to_csv(records: &[OhlcvRecord]) -> Result<String> {
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

pub fn generic_records_to_csv(records: &[Value]) -> Result<String> {
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

pub fn csv_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Interval helpers
// ---------------------------------------------------------------------------

pub fn yahoo_interval(interval: &str) -> Result<&str> {
    match interval {
        "1m" | "2m" | "5m" | "15m" | "30m" | "60m" | "90m" | "1d" | "5d" | "1wk" | "1mo"
        | "3mo" => Ok(interval),
        "1h" => Ok("60m"),
        _ => bail!("unsupported Yahoo interval for Rust path: {interval}"),
    }
}

pub fn coinbase_granularity(interval: &str) -> Result<u32> {
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

pub fn kraken_interval_minutes(interval: &str) -> Result<u32> {
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

pub fn kraken_pair(symbol: &str) -> Result<String> {
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

pub fn normalize_cn_stock_code(symbol: &str) -> String {
    let value = symbol.trim();
    if value.len() > 2 {
        let prefix = &value[..2].to_ascii_lowercase();
        if prefix == "sh" || prefix == "sz" {
            return value[2..].to_string();
        }
    }
    value.to_string()
}

// ---------------------------------------------------------------------------
// Epoch / date / time helpers
// ---------------------------------------------------------------------------

pub fn epoch_seconds_to_iso(seconds: i64) -> Result<String> {
    let dt = Utc
        .timestamp_opt(seconds, 0)
        .single()
        .context("invalid epoch seconds")?;
    Ok(dt.to_rfc3339())
}

pub fn epoch_millis_to_iso(millis: i64) -> Result<String> {
    let dt = Utc
        .timestamp_millis_opt(millis)
        .single()
        .context("invalid epoch milliseconds")?;
    Ok(dt.to_rfc3339())
}

pub fn date_to_utc_iso(date: &str) -> Result<String> {
    let value = format!("{date}T00:00:00+00:00");
    Ok(value)
}

pub fn now_utc() -> String {
    Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// JSON value helpers
// ---------------------------------------------------------------------------

pub fn value_to_i64(value: &Value) -> Result<i64> {
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

pub fn value_to_f64(value: &Value) -> Result<f64> {
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

pub fn opt_value_to_f64(value: Option<&Value>) -> Result<Option<f64>> {
    match value {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(other) => value_to_f64(other).map(Some),
    }
}

pub fn value_array(value: Option<&Value>) -> Result<&Vec<Value>> {
    value
        .and_then(Value::as_array)
        .context("expected array in JSON payload")
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

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // OhlcvRecord
    // ------------------------------------------------------------------

    #[test]
    fn test_ohlcv_record_construction() {
        let record = OhlcvRecord {
            timestamp: "2024-01-15T00:00:00+00:00".to_string(),
            open: 100.0,
            high: 105.0,
            low: 99.0,
            close: 103.5,
            volume: 1_000_000.0,
            adj_close: Some(103.5),
            symbol: "AAPL".to_string(),
            market: "us".to_string(),
            source: "yahoo".to_string(),
        };
        assert_eq!(record.symbol, "AAPL");
        assert_eq!(record.open, 100.0);
        assert!(record.adj_close.is_some());
        assert_eq!(record.adj_close.unwrap(), 103.5);
    }

    #[test]
    fn test_ohlcv_timestamp_utc_valid() {
        let record = OhlcvRecord {
            timestamp: "2024-06-15T10:30:00Z".to_string(),
            open: 1.0, high: 2.0, low: 1.0, close: 1.5, volume: 100.0,
            adj_close: None, symbol: "T".to_string(), market: "us".to_string(), source: "test".to_string(),
        };
        let dt = record.timestamp_utc();
        assert!(dt.is_some());
        assert_eq!(dt.unwrap().to_rfc3339(), "2024-06-15T10:30:00+00:00");
    }

    #[test]
    fn test_ohlcv_timestamp_utc_invalid() {
        let record = OhlcvRecord {
            timestamp: "not-a-date".to_string(),
            open: 1.0, high: 2.0, low: 1.0, close: 1.5, volume: 100.0,
            adj_close: None, symbol: "T".to_string(), market: "us".to_string(), source: "test".to_string(),
        };
        assert!(record.timestamp_utc().is_none());
    }

    // ------------------------------------------------------------------
    // FetchResult
    // ------------------------------------------------------------------

    fn sample_records() -> Vec<OhlcvRecord> {
        vec![
            OhlcvRecord {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                open: 100.0, high: 105.0, low: 99.0, close: 103.0, volume: 1_000.0,
                adj_close: None, symbol: "AAPL".to_string(), market: "us".to_string(), source: "yahoo".to_string(),
            },
            OhlcvRecord {
                timestamp: "2024-01-02T00:00:00Z".to_string(),
                open: 104.0, high: 106.0, low: 102.0, close: 105.0, volume: 1_200.0,
                adj_close: Some(105.0), symbol: "AAPL".to_string(), market: "us".to_string(), source: "yahoo".to_string(),
            },
        ]
    }

    #[test]
    fn test_fetch_result_has_adj_close() {
        let records = sample_records();
        let result = FetchResult {
            dataset: "ohlcv".to_string(),
            source: "yahoo".to_string(),
            market: "us".to_string(),
            symbol: "AAPL".to_string(),
            interval: Some("1d".to_string()),
            timezone: Some("America/New_York".to_string()),
            adjusted: Some(false),
            fetched_at_utc: "2024-01-15T00:00:00Z".to_string(),
            records,
            notes: vec![],
        };
        assert!(result.has_adj_close());
    }

    #[test]
    fn test_fetch_result_columns() {
        let records = sample_records();
        let result = FetchResult {
            dataset: "ohlcv".to_string(),
            source: "yahoo".to_string(), market: "us".to_string(), symbol: "AAPL".to_string(),
            interval: None, timezone: None, adjusted: None,
            fetched_at_utc: "2024-01-15T00:00:00Z".to_string(),
            records, notes: vec![],
        };
        let cols = result.columns();
        assert!(cols.contains(&"timestamp"));
        assert!(cols.contains(&"adj_close"));
        assert!(cols.contains(&"volume"));
    }

    #[test]
    fn test_fetch_result_metadata() {
        let records = sample_records();
        let result = FetchResult {
            dataset: "ohlcv".to_string(),
            source: "yahoo".to_string(), market: "us".to_string(), symbol: "AAPL".to_string(),
            interval: Some("1d".to_string()), timezone: None, adjusted: Some(false),
            fetched_at_utc: "2024-01-15T00:00:00Z".to_string(),
            records, notes: vec!["public endpoint".to_string()],
        };
        let meta = result.metadata();
        assert_eq!(meta["row_count"], 2);
        assert_eq!(meta["symbol"], "AAPL");
        assert_eq!(meta["interval"], "1d");
    }

    // ------------------------------------------------------------------
    // GenericResult
    // ------------------------------------------------------------------

    #[test]
    fn test_generic_result_columns_empty() {
        let result = GenericResult {
            dataset: "test".to_string(), source: "test".to_string(),
            market: "us".to_string(), symbol: "X".to_string(),
            interval: None, timezone: None, adjusted: None,
            fetched_at_utc: "now".to_string(),
            records: vec![], notes: vec![],
        };
        assert!(result.columns().is_empty());
    }

    #[test]
    fn test_generic_result_columns_from_records() {
        let result = GenericResult {
            dataset: "test".to_string(), source: "test".to_string(),
            market: "us".to_string(), symbol: "X".to_string(),
            interval: None, timezone: None, adjusted: None,
            fetched_at_utc: "now".to_string(),
            records: vec![serde_json::json!({"a": 1, "b": 2, "c": 3})],
            notes: vec![],
        };
        assert_eq!(result.columns(), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_generic_result_metadata() {
        let result = GenericResult {
            dataset: "capital".to_string(), source: "eastmoney".to_string(),
            market: "cn".to_string(), symbol: "600519".to_string(),
            interval: None, timezone: Some("Asia/Shanghai".to_string()), adjusted: None,
            fetched_at_utc: "2024-01-01T00:00:00Z".to_string(),
            records: vec![serde_json::json!({"price": 1500.0})],
            notes: vec![],
        };
        let meta = result.metadata();
        assert_eq!(meta["row_count"], 1);
        assert_eq!(meta["columns"].as_array().unwrap(), &[serde_json::json!("price")]);
    }

    // ------------------------------------------------------------------
    // value helpers
    // ------------------------------------------------------------------

    #[test]
    fn test_value_to_i64_from_number() {
        assert_eq!(value_to_i64(&serde_json::json!(42)).unwrap(), 42);
        assert_eq!(value_to_i64(&serde_json::json!(-5)).unwrap(), -5);
    }

    #[test]
    fn test_value_to_i64_from_string() {
        assert_eq!(value_to_i64(&serde_json::json!("12345")).unwrap(), 12345);
        assert!(value_to_i64(&serde_json::json!("abc")).is_err());
    }

    #[test]
    fn test_value_to_f64_from_number() {
        assert_eq!(value_to_f64(&serde_json::json!(3.14)).unwrap(), 3.14);
        assert_eq!(value_to_f64(&serde_json::json!(42)).unwrap(), 42.0);
    }

    #[test]
    fn test_value_to_f64_from_string() {
        assert_eq!(value_to_f64(&serde_json::json!("3.14")).unwrap(), 3.14);
        assert!(value_to_f64(&serde_json::json!("not-a-number")).is_err());
    }

    #[test]
    fn test_value_to_f64_invalid_type() {
        assert!(value_to_f64(&serde_json::json!([1, 2, 3])).is_err());
    }

    #[test]
    fn test_opt_value_to_f64() {
        assert_eq!(opt_value_to_f64(None).unwrap(), None);
        assert_eq!(opt_value_to_f64(Some(&serde_json::Value::Null)).unwrap(), None);
        assert_eq!(opt_value_to_f64(Some(&serde_json::json!(42.5))).unwrap(), Some(42.5));
    }

    #[test]
    fn test_value_array_valid() {
        let arr = vec![1, 2, 3];
        let val = serde_json::json!(arr);
        let result = value_array(Some(&val)).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_value_array_invalid() {
        assert!(value_array(Some(&serde_json::json!("not-array"))).is_err());
    }

    // ------------------------------------------------------------------
    // interval helpers
    // ------------------------------------------------------------------

    #[test]
    fn test_yahoo_interval() {
        assert_eq!(yahoo_interval("1h").unwrap(), "60m");
        assert_eq!(yahoo_interval("1d").unwrap(), "1d");
        assert_eq!(yahoo_interval("1wk").unwrap(), "1wk");
        assert!(yahoo_interval("invalid").is_err());
    }

    #[test]
    fn test_coinbase_granularity() {
        assert_eq!(coinbase_granularity("1m").unwrap(), 60);
        assert_eq!(coinbase_granularity("1h").unwrap(), 3600);
        assert_eq!(coinbase_granularity("1d").unwrap(), 86400);
        assert!(coinbase_granularity("invalid").is_err());
    }

    #[test]
    fn test_kraken_interval_minutes() {
        assert_eq!(kraken_interval_minutes("1m").unwrap(), 1);
        assert_eq!(kraken_interval_minutes("1h").unwrap(), 60);
        assert_eq!(kraken_interval_minutes("1d").unwrap(), 1440);
        assert!(kraken_interval_minutes("invalid").is_err());
    }

    // ------------------------------------------------------------------
    // symbol helpers
    // ------------------------------------------------------------------

    #[test]
    fn test_kraken_pair_valid() {
        assert_eq!(kraken_pair("BTC/USD").unwrap(), "XBTUSD");
        assert_eq!(kraken_pair("ETH/USD").unwrap(), "ETHUSD");
    }

    #[test]
    fn test_kraken_pair_with_dash() {
        assert_eq!(kraken_pair("BTC-USD").unwrap(), "XBTUSD");
    }

    #[test]
    fn test_kraken_pair_too_many_parts() {
        assert!(kraken_pair("BTC/USD/EUR").is_err());
    }

    #[test]
    fn test_normalize_cn_stock_code() {
        assert_eq!(normalize_cn_stock_code("600519"), "600519");
        assert_eq!(normalize_cn_stock_code("SH600519"), "600519");
        assert_eq!(normalize_cn_stock_code("sz000001"), "000001");
    }

    // ------------------------------------------------------------------
    // epoch / date helpers
    // ------------------------------------------------------------------

    #[test]
    fn test_epoch_seconds_to_iso() {
        let result = epoch_seconds_to_iso(1704067200).unwrap();
        assert_eq!(result, "2024-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_epoch_millis_to_iso() {
        let result = epoch_millis_to_iso(1704067200000).unwrap();
        assert_eq!(result, "2024-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_date_to_utc_iso() {
        assert_eq!(date_to_utc_iso("2024-01-15").unwrap(), "2024-01-15T00:00:00+00:00");
    }

    #[test]
    fn test_now_utc_format() {
        let s = now_utc();
        // rfc3339 format: should contain 'T' and end with 'Z' or offset
        assert!(s.contains('T'));
    }

    // ------------------------------------------------------------------
    // CSV helpers
    // ------------------------------------------------------------------

    #[test]
    fn test_csv_value() {
        assert_eq!(csv_value(&serde_json::Value::Null), "");
        assert_eq!(csv_value(&serde_json::json!("hello")), "hello");
        assert_eq!(csv_value(&serde_json::json!(42)), "42");
        assert_eq!(csv_value(&serde_json::json!(3.14)), "3.14");
    }

    #[test]
    fn test_records_to_csv_basic() {
        let records = vec![
            OhlcvRecord {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                open: 100.0, high: 105.0, low: 99.0, close: 103.5, volume: 1_000.0,
                adj_close: None, symbol: "AAPL".to_string(), market: "us".to_string(), source: "yahoo".to_string(),
            },
        ];
        let csv = records_to_csv(&records).unwrap();
        assert!(csv.contains("timestamp"));
        assert!(csv.contains("AAPL"));
        assert!(csv.contains("100"));
        // no adj_close column because all records have adj_close=None
        assert!(!csv.contains("adj_close"));
    }

    #[test]
    fn test_records_to_csv_with_adj_close() {
        let records = vec![
            OhlcvRecord {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                open: 100.0, high: 105.0, low: 99.0, close: 103.5, volume: 1_000.0,
                adj_close: Some(103.5), symbol: "AAPL".to_string(), market: "us".to_string(), source: "yahoo".to_string(),
            },
        ];
        let csv = records_to_csv(&records).unwrap();
        assert!(csv.contains("adj_close"));
        assert!(csv.contains("103.5"));
    }

    #[test]
    fn test_generic_records_to_csv() {
        let records = vec![
            serde_json::json!({"symbol": "AAPL", "price": 150.0}),
        ];
        let csv = generic_records_to_csv(&records).unwrap();
        assert!(csv.contains("symbol"));
        assert!(csv.contains("AAPL"));
    }

    #[test]
    fn test_generic_records_to_csv_empty() {
        assert_eq!(generic_records_to_csv(&[]).unwrap(), "");
    }

    // ------------------------------------------------------------------
    // finalize_result validation
    // ------------------------------------------------------------------

    #[test]
    fn test_finalize_result_empty_records_fails() {
        let result = FetchResult {
            dataset: "ohlcv".to_string(),
            source: "test".to_string(), market: "us".to_string(), symbol: "X".to_string(),
            interval: None, timezone: None, adjusted: None,
            fetched_at_utc: "now".to_string(),
            records: vec![], notes: vec![],
        };
        assert!(finalize_result(result).is_err());
    }

    #[test]
    fn test_finalize_result_valid() {
        let records = vec![
            OhlcvRecord {
                timestamp: "2024-01-02T00:00:00Z".to_string(),
                open: 101.0, high: 106.0, low: 100.0, close: 105.0, volume: 1_000.0,
                adj_close: None, symbol: "AAPL".to_string(), market: "us".to_string(), source: "yahoo".to_string(),
            },
            OhlcvRecord {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                open: 100.0, high: 105.0, low: 99.0, close: 103.0, volume: 1_000.0,
                adj_close: None, symbol: "AAPL".to_string(), market: "us".to_string(), source: "yahoo".to_string(),
            },
        ];
        // Should sort and dedup
        let result = finalize_result(FetchResult {
            dataset: "ohlcv".to_string(),
            source: "test".to_string(), market: "us".to_string(), symbol: "AAPL".to_string(),
            interval: None, timezone: None, adjusted: None,
            fetched_at_utc: "now".to_string(),
            records, notes: vec![],
        }).unwrap();
        assert_eq!(result.records.len(), 2);
        // After sorting, first record should be 2024-01-01
        assert_eq!(result.records[0].timestamp, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_finalize_result_invalid_ohlcv_fails() {
        let records = vec![
            OhlcvRecord {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                open: 100.0, high: 99.0, low: 101.0, close: 100.0, volume: 1_000.0,
                adj_close: None, symbol: "X".to_string(), market: "us".to_string(), source: "test".to_string(),
            },
        ];
        // low > high should fail
        assert!(finalize_result(FetchResult {
            dataset: "ohlcv".to_string(),
            source: "test".to_string(), market: "us".to_string(), symbol: "X".to_string(),
            interval: None, timezone: None, adjusted: None,
            fetched_at_utc: "now".to_string(),
            records, notes: vec![],
        }).is_err());
    }

    // ------------------------------------------------------------------
    // ProbeResult
    // ------------------------------------------------------------------

    #[test]
    fn test_probe_result_serialization() {
        let probe = ProbeResult {
            name: "test_probe".to_string(),
            ok: true,
            details: serde_json::json!({"key": "value"}),
            error: None,
        };
        let json = serde_json::to_value(&probe).unwrap();
        assert_eq!(json["name"], "test_probe");
        assert_eq!(json["ok"], true);
        assert!(json.get("error").unwrap().is_null());
    }
}
