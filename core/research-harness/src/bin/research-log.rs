//! `research-log` CLI — layered research logging (SQLite FTS5 + text layer).
//!
//! Dissolved from `tools/research-log-rs/`. Calls `research_harness::log::*` for all business logic.

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "research-log", version, about = "Research layered logging CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show available log commands
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Status => {
            println!("research-log CLI (backed by research-harness::log)");
            println!("Available log operations are exposed through the research_harness library.");
            println!("Use `autoresearch` for the full research workflow CLI.");
        }
    }
    Ok(())
}
