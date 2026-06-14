//! MCP stdio server binary for financial market data.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    mcp_stdio_common::stdio_server::run_stdio_mcp(
        &repo_root,
        "mcp-financial-data",
        financial_data_rs::mcp::tool_definitions,
        financial_data_rs::mcp::dispatch,
    )
}
