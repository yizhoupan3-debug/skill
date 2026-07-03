//! Framework runtime path and schema constants.

/// Status values that signal the caller is claiming the task is finished.
/// When the status matches one of these **and** programmatic closeout enforcement
/// is active, `write_framework_session_artifacts` requires a `closeout_record`
/// and refuses the write if evaluation fails. When enforcement is off (see
/// `closeout_enforcement_disabled_by_env`), completion writes proceed without
/// that gate. Non-completion statuses skip parsing `closeout_record` on this
/// path so in-progress checkpoints are not blocked by draft records.
pub const CLOSEOUT_COMPLETION_STATUSES: &[&str] = &[
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
pub const FRAMEWORK_RUNTIME_AUTHORITY: &str = "rust-framework-runtime-read-model";
pub const FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY: &str =
    "rust-framework-session-artifact-writer";

pub const CURRENT_ARTIFACT_DIR: &str = "current";
pub const TASK_POINTERS_FILENAME: &str = "TASK_POINTERS.json";
pub const TASK_POINTERS_SCHEMA_VERSION: &str = "task-pointers-v1";
pub const EVIDENCE_INDEX_FILENAME: &str = "EVIDENCE_INDEX.json";
pub const SUPERVISOR_STATE_FILENAME: &str = ".supervisor_state.json";
pub const EVIDENCE_INDEX_SCHEMA_VERSION: &str = "evidence-index-v2";
pub const SUPERVISOR_STATE_SCHEMA_VERSION: &str = "supervisor-state-v2";
pub const TASK_REGISTRY_SCHEMA_VERSION: &str = "task-registry-v1";

// ---------------------------------------------------------------------------
// Evidence / hook schema versions (previously inline strings)
// ---------------------------------------------------------------------------

/// Evidence append hook — used by `framework-extra/src/evidence.rs`.
pub const HOOK_EVIDENCE_APPEND_SCHEMA_VERSION: &str = "router-rs-hook-evidence-append-v1";

// ---------------------------------------------------------------------------
// Framework-extra schema versions (previously inline strings)
// ---------------------------------------------------------------------------

/// Framework alias state machine schema — used by `framework-extra/src/alias.rs`.
pub const FRAMEWORK_ALIAS_STATE_MACHINE_SCHEMA_VERSION: &str = "framework-alias-state-machine-v1";
/// Runtime background orchestration schema — used by `framework-extra/src/orchestration_controller.rs`.
pub const RUNTIME_BACKGROUND_ORCHESTRATION_SCHEMA_VERSION: &str =
    "runtime-background-orchestration-v1";
/// Runtime event sink schema — used by `framework-extra/src/orchestration_controller.rs`.
pub const RUNTIME_EVENT_SINK_SCHEMA_VERSION: &str = "runtime-event-sink-v1";
/// Runtime event stream schema — used by `framework-extra/src/orchestration_controller.rs`.
pub const RUNTIME_EVENT_STREAM_SCHEMA_VERSION: &str = "runtime-event-stream-v1";
/// Runtime event handoff schema — used by `framework-extra/src/orchestration_controller.rs`.
pub const RUNTIME_EVENT_HANDOFF_SCHEMA_VERSION: &str = "runtime-event-handoff-v1";

pub const TERMINAL_STORY_STATES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
];
pub const TERMINAL_PHASES: &[&str] = &[
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
    "done",
];
pub const TERMINAL_VERIFICATION_STATUSES: &[&str] = &[
    "completed",
    "passed",
    "verified",
    "cancelled",
    "abandoned",
    "failed",
];
pub const STALE_STORY_STATES: &[&str] = &["stale", "expired", "invalid"];

/// Field name in RUNTIME_REGISTRY.json that specifies the artifact root.
pub const ARTIFACT_ROOT_REGISTRY_FIELD: &str = "runtime_artifact_root";
