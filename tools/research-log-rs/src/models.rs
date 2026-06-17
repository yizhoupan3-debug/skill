use serde::{Deserialize, Serialize};

/// Core exploration log entry — maps to `exploration_logs` DB table and text file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationLog {
    pub id: String,
    pub direction: String,
    pub question: String,
    pub entry_point: EntryPoint,
    pub barrier_id: Option<String>,
    pub key_findings: String,
    pub open_questions: String,
    pub created_at: String,
    pub updated_at: String,
}

/// How this exploration was initiated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntryPoint {
    Manual,
    BarrierEscalation,
    Loop,
}

impl EntryPoint {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::BarrierEscalation => "barrier_escalation",
            Self::Loop => "loop",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "barrier_escalation" => Self::BarrierEscalation,
            "loop" => Self::Loop,
            _ => Self::Manual,
        }
    }
}

/// A decision or branch point during exploration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationDecision {
    pub id: String,
    pub log_id: String,
    pub decision: String,
    pub rationale: String,
    pub outcome: String,
    pub created_at: String,
}

/// A key insight or discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationInsight {
    pub id: String,
    pub log_id: String,
    pub text: String,
    pub confidence: Confidence,
    pub cross_refs: Vec<String>,
    pub created_at: String,
}

/// Confidence level of an insight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "high" => Self::High,
            "low" => Self::Low,
            _ => Self::Medium,
        }
    }
}

/// Barrier escalation report linked to a loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarrierReport {
    pub id: String,
    pub barrier_id: String,
    pub log_id: String,
    pub loop_id: Option<String>,
    pub report_path: String,
    pub created_at: String,
}

/// FTS5 search result with snippet and relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub direction: String,
    pub question: String,
    pub snippet: String,
    pub score: f64,
    pub created_at: String,
}
