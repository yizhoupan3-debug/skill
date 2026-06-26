//! Internal value types for the framework runtime read model.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct TaskRegistryEntry<'a> {
    pub task_id: &'a str,
    pub task: &'a str,
    pub phase: &'a str,
    pub status: &'a str,
    pub resume_allowed: Option<bool>,
    pub updated_at: &'a str,
    pub focus_task_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct FrameworkRuntimeView {
    pub session_summary_text: String,
    pub next_actions: Value,
    pub evidence_index: Value,
    pub trace_metadata: Value,
    pub supervisor_state: Map<String, Value>,
    pub routing_runtime_version: u64,
    pub repo_root: PathBuf,
    pub artifact_base: PathBuf,
    pub current_root: PathBuf,
    pub mirror_root: PathBuf,
    pub task_root: PathBuf,
    pub task_pointers_present: bool,
    pub active_task_id: Option<String>,
    pub focus_task_id: Option<String>,
    pub control_plane_inconsistency_reasons: Vec<String>,
    pub known_task_ids: Vec<String>,
    pub recoverable_task_ids: Vec<String>,
    pub registered_tasks: Value,
    pub collected_at: String,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameworkAliasBuildOptions<'a> {
    pub max_lines: usize,
    pub compact: bool,
    pub host_id: Option<&'a str>,
}

impl<'a> Default for FrameworkAliasBuildOptions<'a> {
    fn default() -> Self {
        Self {
            max_lines: 4,
            compact: false,
            host_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StaleContinuityInputs<'a> {
    pub continuity: &'a Map<String, Value>,
    pub story_state: &'a str,
    pub task: &'a str,
    pub supervisor_phase: &'a str,
    pub verification_status: &'a str,
    pub next_actions: &'a [String],
    pub session_summary_missing: bool,
    pub terminal_reasons_empty: bool,
}

/// Stdio JSON request payload (canonical definition).
#[derive(Debug, Clone, Deserialize)]
pub struct StdioJsonRequestPayload {
    pub id: Value,
    pub op: String,
    #[serde(default)]
    pub payload: Value,
    /// Optional concurrency hint for the stdio transport worker pool.
    #[serde(default)]
    pub concurrency: Option<usize>,
}

/// Stdio JSON response payload (mirrored from `runtime-core::stdio_transport`).
#[derive(Debug, Clone, Serialize)]
pub struct StdioJsonResponsePayload {
    pub id: Value,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
