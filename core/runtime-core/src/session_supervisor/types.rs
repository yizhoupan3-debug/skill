use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SESSION_SUPERVISOR_SCHEMA_VERSION: &str = "router-rs-session-supervisor-response-v1";
pub const SESSION_SUPERVISOR_STORE_SCHEMA_VERSION: &str = "router-rs-session-supervisor-store-v1";
pub const SESSION_SUPERVISOR_AUTHORITY: &str = "rust-session-supervisor";
pub(super) const DEFAULT_BACKOFF_SECONDS: i64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct SessionSupervisorStore {
    pub(super) schema_version: String,
    pub(super) version: u64,
    pub(super) workers: Vec<WorkerSessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WorkerEvent {
    pub(super) event: String,
    pub(super) status: String,
    pub(super) timestamp: String,
    pub(super) detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WorkerSessionRecord {
    pub(super) worker_id: String,
    pub(super) host: String,
    pub(super) driver_id: String,
    pub(super) cwd: String,
    pub(super) worktree_path: Option<String>,
    pub(super) status: String,
    pub(super) tmux_session: Option<String>,
    pub(super) tmux_pane: Option<String>,
    pub(super) attached_session_id: Option<String>,
    pub(super) resume_target: Option<String>,
    pub(super) resume_mode: Option<String>,
    pub(super) blocked_reason: Option<String>,
    pub(super) next_resume_at: Option<String>,
    pub(super) retry_policy: Value,
    pub(super) prompt: Option<String>,
    pub(super) launch_command: DriverCommandSpec,
    pub(super) resume_command: Option<DriverCommandSpec>,
    pub(super) native_tmux_requested: bool,
    pub(super) last_error: Option<String>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) metadata: Value,
    pub(super) events: Vec<WorkerEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverCommandSpec {
    pub(super) driver_id: String,
    pub(super) binary: String,
    pub(super) args: Vec<String>,
    pub(super) shell_command: String,
    pub(super) supports_resume: bool,
    pub(super) supports_native_tmux: bool,
    pub(super) supports_external_tmux: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockClassification {
    pub(super) host: String,
    pub(super) blocked_reason: String,
    pub(super) status: String,
    pub(super) matched_text: Option<String>,
    pub(super) backoff_seconds: i64,
}
