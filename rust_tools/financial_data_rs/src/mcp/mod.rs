//! MCP stdio server for financial_data_rs — exposes `financial_data` tool.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

const SERVER_NAME: &str = "mcp-financial-data";
const SERVER_VERSION: &str = "0.1.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the MCP stdio server. Reads JSON-RPC lines from stdin, writes to stdout.
pub fn run_stdio_mcp(_repo_root: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(err) => {
                eprintln!("financial-data MCP stdin read error: {err}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(err) => {
                let resp = json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {err}")},
                });
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };
        if let Some(response) = handle_request(&request) {
            writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle_request(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "notifications/initialized" | "notifications/cancelled" => None,
        "initialize" => Some(json!({
            "jsonrpc": "2.0", "id": id,
            "result": {
                "protocolVersion": PROTOCOL_VERSION,
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                "capabilities": {"tools": {"listChanged": false}},
            }
        })),
        "ping" => Some(json!({"jsonrpc": "2.0", "id": id, "result": {}})),
        "tools/list" => Some(json!({
            "jsonrpc": "2.0", "id": id,
            "result": {"tools": tool_definitions()}
        })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match dispatch_tool_call(&params) {
                Ok(result) => Some(json!({
                    "jsonrpc": "2.0", "id": id, "result": result,
                })),
                Err(err) => Some(json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": {"code": -32000, "message": err.to_string()},
                })),
            }
        }
        _ => Some(json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": -32601, "message": format!("Method not found: {method}")},
        })),
    }
}

/// MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "financial_data",
            "description": "Fetch financial market data (OHLCV or capital metrics) for a given symbol. Supports US stocks (Yahoo/Stooq), crypto (Binance/Coinbase/Kraken), and CN A-shares (Eastmoney). Auto-detects market from symbol format.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Ticker symbol (e.g. AAPL, BTC/USDT, 600519)"
                    },
                    "metric": {
                        "type": "string",
                        "description": "Data type: 'ohlcv' (default) or 'capital'",
                        "enum": ["ohlcv", "capital"],
                        "default": "ohlcv"
                    },
                    "period": {
                        "type": "string",
                        "description": "OHLCV period for Yahoo (e.g. 1d, 5d, 1mo, 3mo, 6mo, 1y). Ignored for crypto/CN capital.",
                        "default": "1mo"
                    }
                },
                "required": ["symbol"]
            }
        }),
    ]
}

fn dispatch_tool_call(params: &Value) -> Result<Value, anyhow::Error> {
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match tool_name {
        "financial_data" => tool_financial_data(&args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn tool_financial_data(args: &Value) -> Result<Value, anyhow::Error> {
    let symbol = args
        .get("symbol")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: symbol"))?;
    let metric = args
        .get("metric")
        .and_then(Value::as_str)
        .unwrap_or("ohlcv");
    let period = args
        .get("period")
        .and_then(Value::as_str)
        .unwrap_or("1mo");

    // Run async fetch in a blocking context
    let rt = tokio::runtime::Runtime::new()?;
    let text = rt.block_on(async { run_fetch(symbol, metric, period).await })?;

    Ok(json!({
        "content": [{"type": "text", "text": text}],
        "metadata": {
            "symbol": symbol,
            "metric": metric,
            "period": period,
        }
    }))
}

async fn run_fetch(symbol: &str, metric: &str, period: &str) -> Result<String, anyhow::Error> {
    let http = crate::HttpClient::new()?;

    if metric == "capital" {
        // Try US capital, then CN capital
        match crate::fetch_capital_metrics(&http, "us", symbol).await {
            Ok(result) => {
                let payload = json!({
                    "metadata": result.metadata(),
                    "records": result.records,
                });
                return Ok(serde_json::to_string_pretty(&payload)?);
            }
            Err(us_err) => {
                match crate::fetch_capital_metrics(&http, "cn", symbol).await {
                    Ok(result) => {
                        let payload = json!({
                            "metadata": result.metadata(),
                            "records": result.records,
                        });
                        return Ok(serde_json::to_string_pretty(&payload)?);
                    }
                    Err(cn_err) => {
                        return Err(anyhow::anyhow!(
                            "capital metrics fetch failed:\n  US: {us_err:#}\n  CN: {cn_err:#}"
                        ));
                    }
                }
            }
        }
    }

    // metric == "ohlcv" (default)
    // Heuristic: if symbol contains '/' it is likely crypto
    let is_likely_crypto = symbol.contains('/');

    if is_likely_crypto {
        // Try crypto exchanges
        for exchange in &["binance", "coinbase", "kraken"] {
            match crate::fetch_ohlcv(
                &http, "crypto", symbol, exchange, "1d", 200, period, "auto", false,
            )
            .await
            {
                Ok(result) => {
                    let payload = json!({
                        "metadata": result.metadata(),
                        "records": result.records,
                    });
                    return Ok(serde_json::to_string_pretty(&payload)?);
                }
                Err(_) => continue,
            }
        }
        return Err(anyhow::anyhow!(
            "all crypto exchanges failed for OHLCV: {symbol}"
        ));
    }

    // Try US OHLCV (Yahoo -> Stooq fallback)
    match crate::fetch_ohlcv(
        &http, "us", symbol, "binance", "1d", 200, period, "auto", false,
    )
    .await
    {
        Ok(result) => {
            let payload = json!({
                "metadata": result.metadata(),
                "records": result.records,
            });
            return Ok(serde_json::to_string_pretty(&payload)?);
        }
        Err(us_err) => {
            // Also try crypto as fallback (e.g. BTCUSDT without slash)
            match crate::fetch_ohlcv(
                &http, "crypto", symbol, "binance", "1d", 200, period, "auto", false,
            )
            .await
            {
                Ok(result) => {
                    let payload = json!({
                        "metadata": result.metadata(),
                        "records": result.records,
                    });
                    return Ok(serde_json::to_string_pretty(&payload)?);
                }
                Err(crypto_err) => {
                    return Err(anyhow::anyhow!(
                        "OHLCV fetch failed:\n  US: {us_err:#}\n  crypto: {crypto_err:#}"
                    ));
                }
            }
        }
    }
}
