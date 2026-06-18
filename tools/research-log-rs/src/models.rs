use serde::{Deserialize, Serialize};

/// Core research log entry — the primary unit of research logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub direction: String,
    pub question: String,
    /// JSON blob: env fingerprint, git state, experiment parameters
    pub context: Option<String>,
    /// How this exploration was initiated
    pub entry_point: String,
    pub barrier_id: Option<String>,
    /// Importance 0-5 (0=default, 5=critical)
    pub importance: i32,
    /// Entry lifecycle: active | archived | superseded
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A unified finding/decision/insight — replaces the old separate tables + key_findings blob.
///
/// `kind` discriminates the type:
/// - `finding`: a factual observation
/// - `decision`: a methodological choice with rationale
/// - `insight`: a synthesized understanding
/// - `question`: an open question or research gap
/// - `plan`: a planned next step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: i64,
    pub entry_id: String,
    pub kind: String,
    pub content: String,
    /// 0.0-1.0 confidence, None = unrated
    pub confidence: Option<f64>,
    /// JSON: supporting data, parameter snapshots
    pub metadata: Option<String>,
    pub created_at: String,
}

/// Normalized tag (entry_id, tag) pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryTag {
    pub entry_id: String,
    pub tag: String,
}

/// Reference to external resource (paper, code, dataset, URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ref {
    pub id: i64,
    pub entry_id: String,
    pub ref_type: String,
    /// DOI, arXiv ID, URL, file path
    pub ref_key: Option<String>,
    pub title: Option<String>,
    /// JSON array of author names
    pub authors: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

/// Typed directed edge between two entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConnection {
    pub id: i64,
    pub entry_id_a: String,
    pub entry_id_b: String,
    /// extends | contradicts | supports | supersedes
    pub relation: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

/// Barrier escalation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarrierReport {
    pub barrier_id: String,
    pub entry_id: Option<String>,
    pub loop_id: Option<String>,
    /// JSON full report content
    pub report: Option<String>,
    pub created_at: String,
}

/// Experiment run record (replaces run-ledger.jsonl).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: i64,
    pub entry_id: String,
    pub outcome: String,
    pub summary: String,
    /// JSON: {metric_name: value, ...}
    pub metrics: Option<String>,
    /// JSON: commit, branch, diff
    pub git_state: Option<String>,
    /// JSON: tool versions, OS
    pub env_fingerprint: Option<String>,
    pub created_at: String,
}

/// Auto-recorded activity (from PostToolUse hook).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: i64,
    pub tool_name: String,
    pub summary: String,
    /// auto | manual | hook
    pub source: String,
    /// JSON metadata
    pub metadata: Option<String>,
    pub created_at: String,
}

/// FTS5 search result with relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub direction: String,
    pub question: String,
    pub snippet: String,
    pub score: f64,
    pub created_at: String,
}

/// Entry status constants.
pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_ARCHIVED: &str = "archived";
pub const STATUS_SUPERSEDED: &str = "superseded";

/// Finding kind constants.
pub const FINDING_KIND_FINDING: &str = "finding";
pub const FINDING_KIND_DECISION: &str = "decision";
pub const FINDING_KIND_INSIGHT: &str = "insight";
pub const FINDING_KIND_QUESTION: &str = "question";
pub const FINDING_KIND_PLAN: &str = "plan";

/// Entry point constants.
pub const ENTRY_POINT_MANUAL: &str = "manual";
pub const ENTRY_POINT_BARRIER: &str = "barrier_escalation";
pub const ENTRY_POINT_LOOP: &str = "loop";

/// Run outcome constants.
pub const OUTCOME_CONFIRMATORY: &str = "confirmatory";
pub const OUTCOME_EXPLORATORY: &str = "exploratory";
pub const OUTCOME_FAILED: &str = "failed";
pub const OUTCOME_AMBIGUOUS: &str = "ambiguous";

/// Connection relation constants.
pub const RELATION_EXTENDS: &str = "extends";
pub const RELATION_CONTRADICTS: &str = "contradicts";
pub const RELATION_SUPPORTS: &str = "supports";
pub const RELATION_SUPERSEDES: &str = "supersedes";

/// Ref type constants.
pub const REF_TYPE_PAPER: &str = "paper";
pub const REF_TYPE_CODE: &str = "code";
pub const REF_TYPE_DATASET: &str = "dataset";
pub const REF_TYPE_URL: &str = "url";
