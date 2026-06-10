use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::json;
use std::fs;

use financial_data_rs::{
    self as lib, generic_records_to_csv, records_to_csv, FetchResult, GenericResult, HttpClient,
};

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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let http = HttpClient::new()?;

    match cli.command {
        Commands::Ohlcv(args) => {
            args.validate()?;
            let market = match args.market {
                Market::Crypto => "crypto",
                Market::Us => "us",
            };
            let source = match args.source {
                UsSource::Auto => "auto",
                UsSource::Yahoo => "yahoo",
                UsSource::Stooq => "stooq",
            };
            let result = lib::fetch_ohlcv(
                &http,
                market,
                &args.symbol,
                &args.exchange,
                &args.interval,
                args.limit,
                &args.period,
                source,
                args.adjusted,
            )
            .await?;
            emit_result(&result, args.format)?;
        }
        Commands::Export(args) => {
            args.ohlcv.validate()?;
            let market = match args.ohlcv.market {
                Market::Crypto => "crypto",
                Market::Us => "us",
            };
            let source = match args.ohlcv.source {
                UsSource::Auto => "auto",
                UsSource::Yahoo => "yahoo",
                UsSource::Stooq => "stooq",
            };
            let result = lib::fetch_ohlcv(
                &http,
                market,
                &args.ohlcv.symbol,
                &args.ohlcv.exchange,
                &args.ohlcv.interval,
                args.ohlcv.limit,
                &args.ohlcv.period,
                source,
                args.ohlcv.adjusted,
            )
            .await?;
            export_backtest(&result, &args)?;
        }
        Commands::Capital(args) => {
            let market = match args.market {
                CapitalMarket::Us => "us",
                CapitalMarket::Cn => "cn",
            };
            let result = lib::fetch_capital_metrics(&http, market, &args.symbol).await?;
            emit_generic_result(&result, args.format)?;
        }
        Commands::Validate => {
            let payload = lib::run_validate(&http).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&payload)
                    .context("failed to serialize validate payload")?
            );
        }
    }

    Ok(())
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

fn backtest_csv(result: &FetchResult, schema: BacktestSchema) -> Result<String> {
    use csv::Writer;
    let mut writer = Writer::from_writer(Vec::new());
    let include_adj_close = result.has_adj_close();
    match schema {
        BacktestSchema::Generic => {
            let mut header = vec![
                "timestamp", "open", "high", "low", "close", "volume", "symbol", "market",
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
                "datetime", "open", "high", "low", "close", "volume", "openinterest",
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

fn backtest_json(result: &FetchResult, schema: BacktestSchema) -> Result<serde_json::Value> {
    use serde_json::json;
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
    Ok(serde_json::Value::Array(records))
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
