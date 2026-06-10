//! MCP stdio server binary for financial market data.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    financial_data_rs::mcp::run_stdio_mcp(&repo_root)
}
