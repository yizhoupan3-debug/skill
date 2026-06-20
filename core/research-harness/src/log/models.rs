// Migrated from tools/research-log-rs/src/models.rs

//! 日志数据模型 — 研究活动日志的核心类型和常量。

use serde::{Deserialize, Serialize};

// ── Research Log Types (migrated from research-log-rs) ──

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
    /// Edge weight 0.0-1.0 for graph traversal priority (default 1.0)
    pub weight: f64,
    /// Confidence in this relationship 0.0-1.0, None = unrated
    pub confidence: Option<f64>,
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

/// 一条研究活动日志（保留为兼容别名）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// ISO 8601 时间戳。
    pub timestamp: String,
    /// 日志正文。
    pub content: String,
    /// 来源标识（工具名、文件路径等）。
    pub source: String,
    /// 自由标签。
    pub tags: Vec<String>,
}

// ── Constants ──

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

// ── Entity / Knowledge Graph ──

/// Entity kind constants.
pub const ENTITY_KIND_METHOD: &str = "method";
pub const ENTITY_KIND_DATASET: &str = "dataset";
pub const ENTITY_KIND_THEOREM: &str = "theorem";
pub const ENTITY_KIND_METRIC: &str = "metric";
pub const ENTITY_KIND_CONCEPT: &str = "concept";
pub const ENTITY_KIND_TOOL: &str = "tool";
pub const ENTITY_KIND_AUTHOR: &str = "author";
pub const ENTITY_KIND_MODEL: &str = "model";
pub const ENTITY_KIND_OTHER: &str = "other";

/// Entity relation constants.
pub const ENTITY_REL_USES: &str = "uses";
pub const ENTITY_REL_TRAINS_ON: &str = "trains-on";
pub const ENTITY_REL_EVALUATES: &str = "evaluates";
pub const ENTITY_REL_IMPROVES: &str = "improves";
pub const ENTITY_REL_DEPENDS_ON: &str = "depends-on";
pub const ENTITY_REL_CONTRADICTS: &str = "contradicts";
pub const ENTITY_REL_IS_A: &str = "is-a";
pub const ENTITY_REL_PART_OF: &str = "part-of";

/// Entry-entity role constants.
pub const ENTRY_ENTITY_ROLE_PRIMARY: &str = "primary";
pub const ENTRY_ENTITY_ROLE_MENTIONED: &str = "mentioned";
pub const ENTRY_ENTITY_ROLE_DERIVED: &str = "derived";
pub const ENTRY_ENTITY_ROLE_COMPARED: &str = "compared";

/// A knowledge entity — a research concept, method, dataset, theorem, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

/// Typed relation between two entities, with provenance to an entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelation {
    pub id: i64,
    pub entity_id_a: i64,
    pub entity_id_b: i64,
    pub relation: String,
    pub entry_id: Option<String>,
    pub confidence: Option<f64>,
    pub metadata: Option<String>,
    pub created_at: String,
}

/// Maps an entry to an entity with a role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryEntity {
    pub entry_id: String,
    pub entity_id: i64,
    pub role: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_construction() {
        let entry = Entry {
            id: "e1".into(),
            direction: "deepen".into(),
            question: "test".into(),
            context: None,
            entry_point: "cli".into(),
            barrier_id: None,
            importance: 3,
            status: "active".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(entry.id, "e1");
        assert_eq!(entry.importance, 3);
    }

    #[test]
    fn finding_construction() {
        let finding = Finding {
            id: 1,
            entry_id: "e1".into(),
            kind: "insight".into(),
            content: "test finding".into(),
            confidence: Some(0.9),
            metadata: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(finding.kind, "insight");
        assert_eq!(finding.confidence, Some(0.9));
    }

    #[test]
    fn entity_construction() {
        let entity = Entity {
            id: 1,
            name: "BERT".into(),
            kind: "model".into(),
            description: Some("language model".into()),
            metadata: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(entity.name, "BERT");
    }

    #[test]
    fn search_result_construction() {
        let result = SearchResult {
            id: "e1".into(),
            direction: "deepen".into(),
            question: "test".into(),
            snippet: "highlighted text".into(),
            score: 1.5,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(result.score, 1.5);
    }

    #[test]
    fn log_connection_construction() {
        let conn = LogConnection {
            id: 1,
            entry_id_a: "e1".into(),
            entry_id_b: "e2".into(),
            relation: Some("extends".into()),
            weight: 0.8,
            
            confidence: None,
            notes: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(conn.relation.as_deref(), Some("extends"));
    }
}
