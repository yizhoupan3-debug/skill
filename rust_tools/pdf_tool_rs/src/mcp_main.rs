//! MCP stdio server binary for PDF text extraction.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    pdf_tool_rs::mcp::run_stdio_mcp(&repo_root)
}
