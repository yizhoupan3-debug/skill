mod api;
mod types;

use clap::Parser;
use std::process;

/// Search for all occurrences of a TRON address on the blockchain
#[derive(Parser, Debug)]
#[command(
    name = "tron-search",
    about = "Search TRON blockchain for address occurrences"
)]
struct Cli {
    /// TRON address to search (base58, starts with T)
    address: String,

    /// Maximum number of results per category
    #[arg(short, long, default_value = "20")]
    limit: i64,

    /// Output as raw JSON instead of formatted text
    #[arg(long)]
    json: bool,

    /// Use Shasta testnet instead of mainnet
    #[arg(long)]
    testnet: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if !validate_address(&cli.address) {
        eprintln!(
            "Error: Invalid TRON address '{}'. Must start with 'T' and be 34 characters.",
            cli.address
        );
        process::exit(1);
    }

    let network = if cli.testnet {
        "Shasta Testnet"
    } else {
        "Mainnet"
    };
    eprintln!("Searching for address: {} ({})", cli.address, network);
    eprintln!("========================================\n");

    match api::search_address(&cli.address, cli.limit, cli.testnet, cli.json).await {
        Ok(report) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                print_report(&report);
            }
        }
        Err(e) => {
            eprintln!("Error during search: {e}");
            process::exit(1);
        }
    }
}

fn validate_address(addr: &str) -> bool {
    addr.starts_with('T') && addr.len() == 34 && addr.chars().all(|c| c.is_ascii_alphanumeric())
}

fn print_report(report: &types::SearchReport) {
    // Account Info
    println!("=== Account Information ===");
    match &report.account {
        Some(info) => {
            println!("  Address:        {}", info.address);
            println!("  Balance:        {} TRX", format_trx(info.balance));
            if let Some(created) = info.create_time {
                println!("  Created:        {}", format_timestamp(created));
            }
            if let Some(latest) = info.latest_operation_time {
                println!("  Last Activity:  {}", format_timestamp(latest));
            }
            println!("  Bandwidth:      {}", info.bandwidth.unwrap_or(0));
            println!("  Energy:         {}", info.energy.unwrap_or(0));
            if !info.trc20.is_empty() {
                let show_count = 10;
                println!("  TRC20 Tokens ({} total):", info.trc20.len());
                for tok in info.trc20.iter().take(show_count) {
                    for (key, val) in tok {
                        let display = if let Some(s) = val.as_str() {
                            let amount: f64 = s.parse().unwrap_or(0.0);
                            if amount > 1e18 {
                                format!("{:.4}", amount / 1e18)
                            } else {
                                s.to_string()
                            }
                        } else {
                            val.to_string()
                        };
                        println!("    {}: {}", key, display);
                    }
                }
                if info.trc20.len() > show_count {
                    println!("    ... and {} more tokens", info.trc20.len() - show_count);
                }
            }
        }
        None => println!("  (No account found or account not activated)"),
    }

    // Transactions
    println!(
        "\n=== Transactions ({} found) ===",
        report.transactions.len()
    );
    for (i, tx) in report.transactions.iter().enumerate() {
        println!("  {}. Hash: {}", i + 1, tx.hash);
        println!("     Time:     {}", format_timestamp(tx.timestamp));
        println!("     From:     {}", tx.owner_address);
        println!("     To:       {}", tx.to_address);
        println!("     Amount:   {} TRX", format_trx(tx.amount.unwrap_or(0)));
        if let Some(ref status) = tx.contract_ret {
            println!("     Status:   {status}");
        }
        println!();
    }

    // TRC20 Transfers
    println!(
        "=== TRC20 Token Transfers ({} found) ===",
        report.trc20_transfers.len()
    );
    for (i, tr) in report.trc20_transfers.iter().enumerate() {
        println!("  {}. Hash: {}", i + 1, tr.transaction_id);
        println!("     Time:     {}", format_timestamp(tr.block_ts));
        println!("     From:     {}", tr.from);
        println!("     To:       {}", tr.to);
        println!("     Amount:   {}", tr.value);
        if let Some(ref name) = tr.token_info.name {
            let symbol = tr.token_info.symbol.as_deref().unwrap_or("");
            println!("     Token:    {name} ({symbol})");
        }
        println!();
    }

    // Internal Transactions
    println!(
        "=== Internal Transactions ({} found) ===",
        report.internal_transactions.len()
    );
    for (i, it) in report.internal_transactions.iter().enumerate() {
        println!("  {}. Hash: {}", i + 1, it.hash);
        println!("     From:     {}", it.from);
        println!("     To:       {}", it.to);
        println!("     Amount:   {} TRX", format_trx(it.amount.unwrap_or(0)));
        if let Some(ref note) = it.note {
            println!("     Note:     {note}");
        }
        println!();
    }

    // Blocks Produced
    println!(
        "=== Blocks Produced ({} found) ===",
        report.blocks_produced.len()
    );
    for (i, b) in report.blocks_produced.iter().enumerate() {
        println!("  {}. Block #{}", i + 1, b.number);
        println!("     Time:     {}", format_timestamp(b.timestamp));
        println!("     Tx Count: {}", b.tx_count.unwrap_or(0));
        println!();
    }

    // Summary
    let total = report.transactions.len()
        + report.trc20_transfers.len()
        + report.internal_transactions.len()
        + report.blocks_produced.len();
    println!("========================================");
    println!("Total occurrences found: {total}");
}

fn format_trx(sun: i64) -> String {
    let trx = sun as f64 / 1_000_000.0;
    if trx >= 1_000_000.0 {
        format!("{trx:.2}")
    } else if trx >= 1.0 {
        format!("{trx:.6}")
    } else {
        format!("{trx}")
    }
}

fn format_timestamp(ms: i64) -> String {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| format!("({ms}ms)"))
}
