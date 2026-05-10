//! Framework runtime path and schema constants.

/// Status values that signal the caller is claiming the task is finished.
/// When the status matches one of these **and** programmatic closeout enforcement
/// is active, `write_framework_session_artifacts` requires a `closeout_record`
/// and refuses the write if evaluation fails. When enforcement is off (see
/// `closeout_enforcement_disabled_by_env`), completion writes proceed without
/// that gate. Non-completion statuses skip parsing `closeout_record` on this
/// path so in-progress checkpoints are not blocked by draft records.
pub(super) const CLOSEOUT_COMPLETION_STATUSES: &[&str] = &[
    "completed",
    "complete",
    "done",
    "finished",
    "succeeded",
    "passed",
];

pub const FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION: &str =
    "router-rs-framework-runtime-snapshot-v1";
pub const FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION: &str =
    "router-rs-framework-contract-summary-v1";
pub const FRAMEWORK_ALIAS_SCHEMA_VERSION: &str = "router-rs-framework-alias-v1";
pub const FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION: &str =
    "router-rs-framework-session-artifact-write-v1";
pub const FRAMEWORK_PROMPT_COMPRESSION_SCHEMA_VERSION: &str =
    "router-rs-framework-prompt-compression-v1";
pub const FRAMEWORK_RUNTIME_AUTHORITY: &str = "rust-framework-runtime-read-model";
pub const FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY: &str =
    "rust-framework-session-artifact-writer";
pub const FRAMEWORK_PROMPT_COMPRESSION_AUTHORITY: &str = "rust-framework-prompt-policy";

pub(super) const CURRENT_ARTIFACT_DIR: &str = "current";
pub(super) const ACTIVE_TASK_POINTER_NAME: &str = "active_task.json";
pub(super) const FOCUS_TASK_POINTER_NAME: &str = "focus_task.json";
pub(super) const TASK_REGISTRY_NAME: &str = "task_registry.json";
pub(super) const SESSION_SUMMARY_FILENAME: &str = "SESSION_SUMMARY.md";
pub(super) const NEXT_ACTIONS_FILENAME: &str = "NEXT_ACTIONS.json";
pub(super) const EVIDENCE_INDEX_FILENAME: &str = "EVIDENCE_INDEX.json";
pub(super) const TRACE_METADATA_FILENAME: &str = "TRACE_METADATA.json";
pub(super) const CONTINUITY_JOURNAL_FILENAME: &str = "CONTINUITY_JOURNAL.json";
pub(super) const SUPERVISOR_STATE_FILENAME: &str = ".supervisor_state.json";
pub(super) const NEXT_ACTIONS_SCHEMA_VERSION: &str = "next-actions-v2";
pub(super) const EVIDENCE_INDEX_SCHEMA_VERSION: &str = "evidence-index-v2";
pub(super) const TRACE_METADATA_SCHEMA_VERSION: &str = "trace-metadata-v2";
pub(super) const CONTINUITY_JOURNAL_SCHEMA_VERSION: &str = "continuity-journal-v1";
pub(super) const SUPERVISOR_STATE_SCHEMA_VERSION: &str = "supervisor-state-v2";
pub(super) const TASK_REGISTRY_SCHEMA_VERSION: &str = "task-registry-v1";
pub(super) const TERMINAL_STORY_STATES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
];
pub(super) const TERMINAL_PHASES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
    "done",
];
pub(super) const TERMINAL_VERIFICATION_STATUSES: &[&str] = &[
    "completed",
    "passed",
    "verified",
    "cancelled",
    "abandoned",
    "failed",
];
pub(super) const STALE_STORY_STATES: &[&str] = &["stale", "expired", "invalid"];
