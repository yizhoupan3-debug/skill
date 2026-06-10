//! MCP stdio server binary for OOXML parsing.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    ooxml_parser_rs::mcp::run_stdio_mcp(&repo_root)
}
