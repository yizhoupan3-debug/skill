//! MCP stdio server binary for gh-source-gate.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    gh_source_gate_rs::mcp::run_stdio_mcp(&repo_root)
}
