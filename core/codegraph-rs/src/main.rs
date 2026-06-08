use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mcp-codegraph")]
#[command(about = "CodeGraph MCP server (stdio)")]
struct Cli {
    #[arg(long, default_value = ".")]
    repo_root: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    codegraph_rs::mcp::run_stdio_mcp(&cli.repo_root)
}
