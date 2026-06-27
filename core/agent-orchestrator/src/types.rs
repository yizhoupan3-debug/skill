use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ORCHESTRATOR_SCHEMA_VERSION: &str = "router-rs-orchestrator-response-v1";
pub const ORCHESTRATOR_STORE_SCHEMA_VERSION: &str = "router-rs-orchestrator-store-v1";
pub const ORCHESTRATOR_AUTHORITY: &str = "rust-agent-orchestrator";
pub(crate) const DEFAULT_BACKOFF_SECONDS: i64 = 300;
/// Workers without a live process that stay in active statuses longer than this are reaped on `list`.
pub(crate) const DEFAULT_WORKER_STALE_AFTER_SECS: i64 = 60;

/// Agent health tracking entry: records subagent lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealthEntry {
    pub agent_id: String,
    pub host_id: String,
    pub status: String, // running | completed | failed | interrupted
    pub spawned_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub spawned_by_tool: String, // "agent" | "task" | "workflow"
}

impl AgentHealthEntry {
    pub fn is_alive(&self) -> bool {
        matches!(self.status.as_str(), "running")
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed" | "interrupted")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentHealthStore {
    pub schema_version: String,
    pub agents: Vec<AgentHealthEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrchestratorStore {
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
