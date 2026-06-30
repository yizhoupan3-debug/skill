//! Core types for QG Route: severity, findings, verdicts, and context.

use serde::{Deserialize, Serialize};

/// Finding severity level, ordered by gate impact.
///
/// Gate rule: any finding at P0, A, or B blocks the gate (verdict.passed = false).
/// Warning and C findings are advisory only (advisories in GateVerdict).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Blocking — unconditional gate failure. Anti-fraud / data integrity violations.
    P0,
    /// Blocking — high-priority issue. Must be fixed before gate passes.
    A,
    /// Blocking — medium-priority issue. Must be fixed before gate passes.
    B,
    /// Non-blocking — advisory finding. Recorded but does not block the gate.
    Warning,
    /// Informational — logged for awareness only. Never blocks the gate.
    C,
}

/// A single finding produced by a checker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Unique identifier within the checker scope.
    pub id: String,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable description of the issue.
    pub description: String,
    /// Optional source location (file:line or equivalent).
    pub location: Option<String>,
    /// Optional remediation suggestion.
    pub suggestion: Option<String>,
}

/// Result from a single checker invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Checker identity (matches GateChecker::id()).
    pub checker_id: String,
    /// Whether this checker passed. Note: aggregation logic uses `findings`
    /// severity, not this field. Kept for backward compatibility.
    pub passed: bool,
    /// All findings produced by this checker.
    pub findings: Vec<Finding>,
}

/// Aggregated verdict from running all checkers for a scene.
///
/// `passed` is false if any finding has severity P0, A, or B.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateVerdict {
    /// True iff all checkers passed (no P0/A/B findings).
    pub passed: bool,
    /// The normalized scene that was evaluated.
    pub scene: String,
    /// Number of checkers that ran.
    pub checkers_ran: usize,
    /// Findings that block the gate (severity P0/A/B).
    pub blockers: Vec<Finding>,
    /// Non-blocking findings (severity Warning/C).
    pub advisories: Vec<Finding>,
    /// Optional human-readable summary reason.
    pub reason: Option<String>,
}

/// Per-invocation context passed to every checker.
#[derive(Debug, Clone)]
pub struct CheckContext {
    /// Scene identifier (one of `scene::*` constants).
    pub scene: String,
    /// Optional sub-scene (Wave 6), e.g. "literature_review".
    pub sub_scene: Option<String>,
    /// Goal identifier string.
    pub goal: String,
    /// Current verification round (1-based).
    pub round: u64,
    /// Repository root path.
    pub repo_root: std::path::PathBuf,
    /// Active task ID.
    pub task_id: String,
    /// Optional path to EVIDENCE_INDEX.json for anti-fraud checks.
    pub evidence_path: Option<std::path::PathBuf>,
    /// Async runtime handle for checkers that need to call async APIs.
    pub runtime_handle: Option<tokio::runtime::Handle>,
    /// Optional structured task output data (from MCP tool payload), enabling
    /// checkers to access task results without scanning repo files.
    pub output_data: Option<serde_json::Value>,
    /// ISO 8601 timestamp of when this evaluation was initiated.
    pub evaluated_at: String,
}
