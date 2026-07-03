use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

pub const SESSION_SUPERVISOR_SCHEMA_VERSION: &str = "router-rs-session-supervisor-response-v1";
pub const SESSION_SUPERVISOR_STORE_SCHEMA_VERSION: &str = "router-rs-session-supervisor-store-v1";
pub const SESSION_SUPERVISOR_AUTHORITY: &str = "rust-session-supervisor";
pub(crate) const DEFAULT_BACKOFF_SECONDS: i64 = 300;
/// Workers without a live process that stay in active statuses longer than this are reaped on `list`.
pub(crate) const DEFAULT_WORKER_STALE_AFTER_SECS: i64 = 60;

/// Typed spawn origin for agent health tracking.
///
/// Serializes as a plain string for backward compatibility with existing
/// health-store JSON files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSpawnOrigin {
    Agent,
    Task,
    Workflow,
    TeamMember,
    Other(String),
}

impl AgentSpawnOrigin {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Agent => "agent",
            Self::Task => "task",
            Self::Workflow => "workflow",
            Self::TeamMember => "team_member",
            Self::Other(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for AgentSpawnOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AgentSpawnOrigin {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "agent" => Self::Agent,
            "task" => Self::Task,
            "workflow" => Self::Workflow,
            "team_member" => Self::TeamMember,
            other => Self::Other(other.to_string()),
        })
    }
}

impl From<String> for AgentSpawnOrigin {
    fn from(s: String) -> Self {
        s.parse().expect("Infallible")
    }
}

impl From<AgentSpawnOrigin> for String {
    fn from(origin: AgentSpawnOrigin) -> Self {
        origin.to_string()
    }
}

impl From<&str> for AgentSpawnOrigin {
    fn from(s: &str) -> Self {
        // FromStr is infallible (error type is Infallible), branch directly
        // to avoid unwrap/expect which conflict with deny-level clippy lints.
        match s {
            "agent" => Self::Agent,
            "task" => Self::Task,
            "workflow" => Self::Workflow,
            "team_member" => Self::TeamMember,
            other => Self::Other(other.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for AgentSpawnOrigin {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(AgentSpawnOrigin::from(s))
    }
}

impl Serialize for AgentSpawnOrigin {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// Agent health tracking entry: records subagent lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealthEntry {
    pub agent_id: String,
    pub host_id: String,
    pub status: String, // running | completed | failed | interrupted
    pub spawned_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub spawned_by_tool: AgentSpawnOrigin,
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
