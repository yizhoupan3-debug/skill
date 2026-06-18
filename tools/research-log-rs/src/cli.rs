use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "research-log", about = "Layered research logging CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Record a new research log entry (direction + question)
    Record {
        /// Research direction / project name
        direction: String,
        /// Research question or problem
        question: String,
        /// Entry point: manual | barrier_escalation | loop
        #[arg(long, default_value = "manual")]
        entry_point: String,
        /// Barrier ID if escalation-triggered
        #[arg(long)]
        barrier_id: Option<String>,
        /// Importance 0-5
        #[arg(long, default_value_t = 0)]
        importance: i32,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
    /// Add a finding/insight/decision to an existing entry
    AddFinding {
        /// Entry ID (from search or record output)
        entry_id: String,
        /// finding | decision | insight | question | plan
        #[arg(long, default_value = "finding")]
        kind: String,
        /// Content text
        content: String,
        /// Confidence 0.0-1.0
        #[arg(long)]
        confidence: Option<f64>,
    },
    /// Full-text search across entries
    Search {
        /// FTS5 query string
        query: String,
        /// Filter by direction
        #[arg(long)]
        direction: Option<String>,
        /// Filter by status: active | archived | superseded
        #[arg(long)]
        status: Option<String>,
        /// Date from (ISO date string)
        #[arg(long)]
        date_from: Option<String>,
        /// Date to (ISO date string)
        #[arg(long)]
        date_to: Option<String>,
        /// Max results
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Search findings/insights/decisions
    SearchFindings {
        /// FTS5 query string
        query: String,
        /// Filter by kind: finding | decision | insight | question | plan
        #[arg(long)]
        kind: Option<String>,
        /// Max results
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Render an entry as Markdown
    Render {
        /// Entry ID
        entry_id: String,
        /// Write entry to file system
        #[arg(long)]
        write: bool,
        /// Output directory for --write (default: artifacts/research-log)
        #[arg(long)]
        output: Option<String>,
    },
    /// Show database statistics
    Status,
    /// Consolidate auto-activity logs into the database
    Consolidate,
    /// Export entries
    Export {
        /// Output format: json | csv | obsidian
        #[arg(long, default_value = "json")]
        format: ExportFormat,
        /// Output directory for multi-file export
        #[arg(long)]
        output: Option<String>,
    },
    /// Connect two log entries
    Connect {
        /// First entry ID
        log_id_a: String,
        /// Second entry ID
        log_id_b: String,
        /// extends | contradicts | supports | supersedes
        #[arg(long)]
        relation: Option<String>,
        /// Notes about the connection
        #[arg(long)]
        notes: Option<String>,
    },
    /// Barrier report operations
    Barrier {
        /// Loop ID filter
        #[arg(long)]
        loop_id: Option<String>,
    },
}

#[derive(Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
    Obsidian,
}
