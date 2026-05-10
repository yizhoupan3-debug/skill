//! Internal value types for the framework runtime read model.
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(super) struct ArtifactPaths<'a> {
    pub(super) summary: &'a Path,
    pub(super) next_actions: &'a Path,
    pub(super) evidence: &'a Path,
    pub(super) trace_metadata: Option<&'a Path>,
    pub(super) journal: Option<&'a Path>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ArtifactPayloads<'a> {
    pub(super) summary_text: &'a str,
    pub(super) next_actions: &'a Value,
    pub(super) evidence: &'a Value,
    pub(super) trace_metadata: &'a Value,
    pub(super) journal: Option<&'a Value>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SupervisorStateInput<'a> {
    pub(super) task_id: &'a str,
    pub(super) task: &'a str,
    pub(super) phase: &'a str,
    pub(super) status: &'a str,
    pub(super) summary: &'a str,
    pub(super) next_actions_payload: &'a Value,
    pub(super) evidence_payload: &'a Value,
    pub(super) trace_metadata_payload: &'a Value,
    pub(super) artifact_dir: &'a Path,
    pub(super) supervisor_state: Option<&'a Value>,
    pub(super) execution_contract: Option<&'a Value>,
    pub(super) blockers: Option<&'a Value>,
    pub(super) continuity: Option<&'a Value>,
}

#[derive(Debug, Clone)]
pub(super) struct ContinuityJournalInput<'a> {
    pub(super) task_id: &'a str,
    pub(super) task: &'a str,
    pub(super) phase: &'a str,
    pub(super) status: &'a str,
    pub(super) artifact_dir: &'a Path,
    pub(super) summary_text: &'a str,
    pub(super) next_actions_payload: &'a Value,
    pub(super) evidence_payload: &'a Value,
    pub(super) trace_metadata_payload: &'a Value,
    pub(super) supervisor_state_payload: &'a Value,
    pub(super) existing_journal: Value,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TaskRegistryEntry<'a> {
    pub(super) task_id: &'a str,
    pub(super) task: &'a str,
    pub(super) phase: &'a str,
    pub(super) status: &'a str,
    pub(super) resume_allowed: Option<bool>,
    pub(super) updated_at: &'a str,
    pub(super) focus_task_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub(super) struct SessionArtifactWritePlan {
    pub(super) task: String,
    pub(super) phase: String,
    pub(super) status: String,
    pub(super) summary: String,
    pub(super) task_id: String,
    pub(super) focus: bool,
    pub(super) repo_root: Option<PathBuf>,
    pub(super) mirror_output_dir: Option<PathBuf>,
    pub(super) summary_path: PathBuf,
    pub(super) next_actions_path: PathBuf,
    pub(super) evidence_path: PathBuf,
    pub(super) trace_metadata_path: PathBuf,
    pub(super) journal_path: PathBuf,
    pub(super) next_actions_payload: Value,
    pub(super) evidence_payload: Value,
    pub(super) trace_metadata_payload: Value,
    pub(super) supervisor_state_payload: Value,
    pub(super) journal_payload: Value,
    pub(super) expected_active_task_hash: Option<String>,
    pub(super) expected_focus_task_hash: Option<String>,
    pub(super) expected_supervisor_state_hash: Option<String>,
    pub(super) changed_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct FrameworkRuntimeView {
    pub(super) session_summary_text: String,
    pub(super) next_actions: Value,
    pub(super) evidence_index: Value,
    pub(super) trace_metadata: Value,
    pub(super) supervisor_state: Map<String, Value>,
    pub(super) routing_runtime_version: u64,
    pub(super) repo_root: PathBuf,
    pub(super) artifact_base: PathBuf,
    pub(super) current_root: PathBuf,
    pub(super) mirror_root: PathBuf,
    pub(super) task_root: PathBuf,
    pub(super) active_task_pointer_present: bool,
    pub(super) focus_task_pointer_present: bool,
    pub(super) task_registry_present: bool,
    pub(super) active_task_id: Option<String>,
    pub(super) focus_task_id: Option<String>,
    pub(super) control_plane_inconsistency_reasons: Vec<String>,
    pub(super) known_task_ids: Vec<String>,
    pub(super) recoverable_task_ids: Vec<String>,
    pub(super) registered_tasks: Value,
    pub(super) collected_at: String,
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
pub(super) struct StaleContinuityInputs<'a> {
    pub(super) continuity: &'a Map<String, Value>,
    pub(super) story_state: &'a str,
    pub(super) task: &'a str,
    pub(super) supervisor_phase: &'a str,
    pub(super) verification_status: &'a str,
    pub(super) next_actions: &'a [String],
    pub(super) session_summary_missing: bool,
    pub(super) terminal_reasons_empty: bool,
}
