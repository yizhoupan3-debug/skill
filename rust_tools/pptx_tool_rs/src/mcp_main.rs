//! MCP stdio server binary for PPTX text extraction.

use anyhow::Result;

fn main() -> Result<()> {
    pptx_tool_rs::mcp::run_stdio_mcp()
}
