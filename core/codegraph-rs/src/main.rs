use clap::Parser;
use std::path::PathBuf;
use tracing;

#[derive(Parser)]
#[command(name = "mcp-codegraph")]
#[command(
    about = "CodeGraph MCP server (stdio) — code knowledge graph with symbol search, call graph, and impact analysis"
)]
struct Cli {
    #[arg(long, default_value = ".")]
    repo_root: PathBuf,

    /// Force a full re-index of all source files, ignoring cached content hashes
    #[arg(long)]
    force_rebuild: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.force_rebuild {
        // Force-rebuild mode: build full index then exit
        let index = codegraph_rs::CodeGraphIndex::open(&cli.repo_root)?;
        let report = codegraph_rs::graph::build_full_index(&index, &cli.repo_root)?;
        tracing::info!(
            "codegraph: force rebuild complete — {} files, {} nodes, {} edges",
            report.files_updated,
            report.nodes_added,
            report.edges_added
        );
        return Ok(());
    }

    codegraph_rs::mcp::run_stdio_mcp(&cli.repo_root)
}
