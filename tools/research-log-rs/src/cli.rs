use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "research-log", about = "Layered research logging CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Record a new exploration log entry (text + DB)
    Record {
        /// Direction/project name
        direction: String,
        /// Research question or problem
        question: String,
        /// Entry point: manual | barrier_escalation | loop
        #[arg(long, default_value = "manual")]
        entry_point: String,
        /// Barrier ID if escalation-triggered
        #[arg(long)]
        barrier_id: Option<String>,
    },
    /// Full-text search across all logs
    Search {
        /// FTS5 query string
        query: String,
        /// Max results
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Add insight to an existing log entry
    Insight {
        /// Log entry ID (UUID)
        log_id: String,
        /// Insight text
        text: String,
        /// Confidence: high | medium | low
        #[arg(long, default_value = "medium")]
        confidence: String,
    },
    /// Connect two log entries (cross-reference)
    Connect {
        /// First log entry ID
        log_id_a: String,
        /// Second log entry ID
        log_id_b: String,
        /// Relationship description
        #[arg(long)]
        relation: Option<String>,
    },
    /// List barrier reports for a loop
    Barrier {
        /// Loop ID filter
        #[arg(long)]
        loop_id: Option<String>,
    },
    /// Trace full research path from a barrier
    Route {
        /// Barrier ID to trace from
        barrier_id: String,
    },
}
