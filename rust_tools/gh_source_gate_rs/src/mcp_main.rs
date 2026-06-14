//! MCP stdio server binary for gh-source-gate.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    mcp_stdio_common::stdio_server::run_stdio_mcp(
        &repo_root,
        "mcp-gh-source-gate",
        gh_source_gate_rs::mcp::tool_definitions,
        gh_source_gate_rs::mcp::dispatch,
    )
}
