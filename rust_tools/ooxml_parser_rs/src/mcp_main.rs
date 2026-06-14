//! MCP stdio server binary for OOXML parsing.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    mcp_stdio_common::stdio_server::run_stdio_mcp(
        &repo_root,
        "mcp-ooxml",
        ooxml_parser_rs::mcp::tool_definitions,
        ooxml_parser_rs::mcp::dispatch,
    )
}
