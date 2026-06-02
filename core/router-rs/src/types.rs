//! Typed data structures for core framework data flows.
//!
//! This module provides strongly-typed replacements for `serde_json::Value`-based
//! construction of key envelope/record types. Use these structs when building or
//! consuming framework envelopes in new code; existing `json!()` call-sites will
//! be migrated incrementally.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// FrameworkRuntimeSnapshot
// ---------------------------------------------------------------------------

/// Top-level envelope returned by `build_framework_runtime_snapshot_envelope`
/// and the `framework_snapshot` MCP tool.
///
/// Field names mirror the JSON keys produced by the current `json!()` builder
/// in `framework_runtime::mod::build_framework_runtime_snapshot_envelope`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkSnapshotEnvelope {
    pub schema_version: String,
    pub authority: String,
    pub runtime_snapshot: FrameworkRuntimeSnapshot,
}

/// The `runtime_snapshot` payload nested inside [`FrameworkSnapshotEnvelope`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkRuntimeSnapshot {
    pub ok: bool,
    pub workspace: String,
    pub artifact_base: String,
    pub current_root: String,
    pub mirror_root: String,
    pub task_root: String,
    pub control_plane_present: bool,
    pub control_plane_missing: Vec<String>,
    pub control_plane_inconsistency_reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_task_id: Option<String>,
    pub known_task_ids: Vec<String>,
    pub recoverable_task_ids: Vec<String>,
    pub parallel_task_count: usize,
    /// Registered tasks from the task registry. Retained as `Value` because
    /// its schema is defined upstream in the state-manager crate.
    pub registered_tasks: serde_json::Value,
    pub collected_at: String,
    pub session_summary_present: bool,
    pub next_action_count: usize,
    pub evidence_count: usize,
    pub trace_skill_count: usize,
    /// Continuity route / next-actions rollup. Retained as `Value` because the
    /// shape is produced by `classify_runtime_continuity` and may evolve.
    pub continuity: serde_json::Value,
    pub supervisor_state: RuntimeSupervisorSnapshot,
    pub paths: RuntimePaths,
}

/// Supervisor state fields extracted into the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSupervisorSnapshot {
    pub task_id: String,
    pub task_summary: String,
    pub active_phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,
}

/// Well-known artifact paths surfaced in the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePaths {
    pub session_summary: String,
    pub next_actions: String,
    pub evidence_index: String,
    pub trace_metadata: String,
    pub current_pointer_root: String,
    pub supervisor_state: String,
}

// ---------------------------------------------------------------------------
// HostProjectionManifest
// ---------------------------------------------------------------------------

/// Manifest written alongside each host projection (Claude Code, Claude Desktop,
/// Codex CLI, Cursor, OpenCode, Antigravity, etc.).
///
/// Common shape extracted from `write_*_projection_manifest` functions in
/// `host_integration::projection`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostProjectionManifest {
    pub schema_version: String,
    pub managed_by: String,
    pub host_projection: String,
    pub scope: String,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<ProjectionSettings>,
}

/// Per-manifest settings bag; currently only carries `managed_key_paths`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionSettings {
    #[serde(default)]
    pub managed_key_paths: Vec<String>,
}

// ---------------------------------------------------------------------------
// VerificationStatus (shared enum)
// ---------------------------------------------------------------------------

/// Canonical verification status values used across closeout enforcement and
/// supervisor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
    Partial,
    NotRun,
}

impl VerificationStatus {
    /// Parse from the free-form string used in closeout records and supervisor
    /// state. Returns `None` for unrecognised values rather than panicking.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "partial" => Some(Self::Partial),
            "not_run" => Some(Self::NotRun),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Partial => "partial",
            Self::NotRun => "not_run",
        }
    }
}

impl std::fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// CloseoutRecord (re-export)
// ---------------------------------------------------------------------------

// CloseoutRecord is already defined as a typed struct in `closeout_enforcement`
// with `#[serde(deny_unknown_fields)]`.  We re-export it here so callers can
// import from a single `types` module when needed.
pub use crate::closeout_enforcement::{
    CloseoutArtifactRecord, CloseoutCommandRecord, CloseoutEnforcementResponse,
    CloseoutRecord, CloseoutViolation,
};
