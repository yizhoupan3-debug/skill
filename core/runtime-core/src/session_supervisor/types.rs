use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SESSION_SUPERVISOR_SCHEMA_VERSION: &str = "router-rs-session-supervisor-response-v1";
pub const SESSION_SUPERVISOR_STORE_SCHEMA_VERSION: &str = "router-rs-session-supervisor-store-v1";
pub const SESSION_SUPERVISOR_AUTHORITY: &str = "rust-session-supervisor";
pub const DEFAULT_BACKOFF_SECONDS: i64 = 300;
/// Workers without a live process that stay in active statuses longer than this are reaped on `list`.
pub const DEFAULT_WORKER_STALE_AFTER_SECS: i64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSupervisorStore {
    pub schema_version: String,
    pub version: u64,
    pub workers: Vec<WorkerSessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerEvent {
    pub event: String,
    pub status: String,
    pub timestamp: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkerSessionRecord {
    pub worker_id: String,
    pub host: String,
    pub driver_id: String,
    pub cwd: String,
    pub worktree_path: Option<String>,
    pub status: String,
    pub pid: Option<u32>,
    pub log_path: Option<String>,
    pub attached_session_id: Option<String>,
    pub resume_target: Option<String>,
    pub resume_mode: Option<String>,
    pub blocked_reason: Option<String>,
    pub next_resume_at: Option<String>,
    pub retry_policy: Value,
    pub prompt: Option<String>,
    pub launch_command: DriverCommandSpec,
    pub resume_command: Option<DriverCommandSpec>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Value,
    pub events: Vec<WorkerEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverCommandSpec {
    pub driver_id: String,
    pub binary: String,
    pub args: Vec<String>,
    pub shell_command: String,
    pub supports_resume: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockClassification {
    pub host: String,
    pub blocked_reason: String,
    pub status: String,
    pub matched_text: Option<String>,
    pub backoff_seconds: i64,
}
