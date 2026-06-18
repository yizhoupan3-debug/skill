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
    // ── Knowledge Graph ──
    /// Show neighbors (direct connections) of an entry
    Neighbors {
        /// Entry ID
        entry_id: String,
        /// Relation filter: extends,contradicts,supports,supersedes (comma-separated)
        #[arg(long)]
        relation: Option<String>,
        /// Max results
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Find shortest path between two entries via BFS
    Path {
        /// Start entry ID
        #[arg(long)]
        from: String,
        /// Target entry ID
        #[arg(long)]
        to: String,
        /// Max search depth
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
    },
    /// Show subgraph centered on an entry
    Subgraph {
        /// Center entry ID
        entry_id: String,
        /// Max depth in hops
        #[arg(long, default_value_t = 2)]
        max_depth: usize,
        /// Output format: text | dot
        #[arg(long, default_value = "text")]
        format: GraphFormat,
    },
    /// Show graph statistics
    GraphStats,
    /// Visualize the knowledge graph
    Viz {
        /// Center entry ID (shows full graph if omitted)
        #[arg(long)]
        entry_id: Option<String>,
        /// Max depth
        #[arg(long, default_value_t = 2)]
        max_depth: usize,
        /// Min connections an entry must have to appear
        #[arg(long, default_value_t = 0)]
        min_connections: usize,
        /// Output format: text | dot
        #[arg(long, default_value = "text")]
        format: GraphFormat,
    },
    /// Trace full research path from a barrier report
    Route {
        /// Barrier ID (e.g., br-...)
        #[arg(long = "barrier-id")]
        barrier_id: String,
        /// Max depth from barrier's entries
        #[arg(long, default_value_t = 3)]
        max_depth: usize,
    },
    // ── Entity Management ──
    /// Auto-extract entities from an entry (question + findings)
    ExtractEntities {
        /// Entry ID
        entry_id: String,
    },
    /// Manually add a knowledge entity
    AddEntity {
        /// Entity name
        name: String,
        /// Entity kind: method, dataset, theorem, metric, concept, tool, author, model, other
        #[arg(long, default_value = "concept")]
        kind: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// Link two entities with a relation
    LinkEntities {
        /// First entity name or ID
        entity_a: String,
        /// Second entity name or ID
        entity_b: String,
        /// Relation: uses, trains-on, evaluates, improves, depends-on, contradicts, is-a, part-of
        #[arg(long)]
        relation: String,
        /// Optional entry ID for provenance
        #[arg(long)]
        entry_id: Option<String>,
    },
    /// FTS5 search entities
    SearchEntities {
        /// FTS5 query string
        query: String,
        /// Max results
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Show entities associated with an entry
    EntryEntities {
        /// Entry ID
        entry_id: String,
    },
    // ── Cross-Workspace Hub ──
    /// Register current workspace in the research knowledge hub
    HubRegister {
        /// Workspace path (default: current directory)
        #[arg(long)]
        path: Option<String>,
        /// Workspace name (default: directory name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Index one or all workspaces into the hub
    HubIndex {
        /// Specific workspace path to index (indexes all if omitted)
        #[arg(long)]
        path: Option<String>,
    },
    /// Cross-workspace FTS5 search
    HubSearch {
        /// FTS5 query string
        query: String,
        /// Max results
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// List registered workspaces
    HubList,
}

#[derive(Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
    Obsidian,
}

#[derive(Clone, ValueEnum)]
pub enum GraphFormat {
    Text,
    Dot,
}
