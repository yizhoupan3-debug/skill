use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;

pub const BACKGROUND_STATE_STORE_SCHEMA_VERSION: &str = "router-rs-background-state-store-v1";
pub const BACKGROUND_STATE_STORE_AUTHORITY: &str = "rust-background-state-store";
pub(super) const BACKGROUND_STATE_REQUEST_SCHEMA_VERSION: &str = "router-rs-background-state-request-v1";
pub(super) const BACKGROUND_STATE_SCHEMA_VERSION: &str = "runtime-background-state-v5";
pub(super) const BACKGROUND_STATE_CONTROL_PLANE_SCHEMA_VERSION: &str =
    "runtime-background-state-control-plane-v1";
pub(super) const BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION: &str =
    "runtime-background-session-takeover-arbitration-v1";
pub(super) const DEFAULT_BACKGROUND_JOB_MULTITASK_STRATEGY: &str = "reject";
pub(super) const DEFAULT_BACKGROUND_JOB_ATTEMPT: i64 = 1;
pub(super) const DEFAULT_BACKGROUND_JOB_RETRY_COUNT: i64 = 0;
pub(super) const DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS: i64 = 1;
pub(super) const DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS: f64 = 0.0;
pub(super) const DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER: f64 = 2.0;
pub(super) const DEFAULT_MAX_BACKGROUND_JOBS: usize = 16;
pub(super) const MAX_BACKGROUND_JOBS_LIMIT: usize = 64;

/// Reap window for jobs whose status is still active (queued/running/...) but
/// whose `updated_at` heartbeat has gone silent. Such jobs are typically
/// produced when a host process is killed without a clean transition; if we
/// don't reap them they hold session reservations forever and block legitimate
/// new owners. Default 1h gives plenty of slack for slow-but-alive workers.
pub(super) const STALE_ACTIVE_HEARTBEAT_TTL_SECS: i64 = 3600;

/// Garbage-collection window for jobs already in a terminal state
/// (completed/failed/interrupted/retry_exhausted). After this, the job is
/// dropped from the in-memory map so the persisted file does not grow without
/// bound across long-running deployments.
pub(super) const STALE_TERMINAL_JOB_TTL_SECS: i64 = 24 * 3600;

pub(super) fn default_multitask_strategy() -> String {
    DEFAULT_BACKGROUND_JOB_MULTITASK_STRATEGY.to_string()
}

pub(super) fn default_attempt() -> i64 {
    DEFAULT_BACKGROUND_JOB_ATTEMPT
}

pub(super) fn default_retry_count() -> i64 {
    DEFAULT_BACKGROUND_JOB_RETRY_COUNT
}

pub(super) fn default_max_attempts() -> i64 {
    DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS
}

pub(super) fn default_backoff_base_seconds() -> f64 {
    DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS
}

pub(super) fn default_backoff_multiplier() -> f64 {
    DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BackgroundStateRequestPayload {
    pub(super) schema_version: String,
    pub(super) operation: String,
    pub(super) state_path: Option<String>,
    pub(super) backend_family: Option<String>,
    pub(super) sqlite_db_path: Option<String>,
    pub(super) state_payload_text: Option<String>,
    pub(super) control_plane_descriptor: Option<Value>,
    pub(super) job_id: Option<String>,
    pub(super) arbitration_operation: Option<String>,
    pub(super) mutation: Option<BackgroundJobStatusMutation>,
    pub(super) session_id: Option<String>,
    pub(super) incoming_job_id: Option<String>,
    pub(super) parallel_group_id: Option<String>,
    pub(super) capacity_limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct BackgroundRunStatus {
    pub(super) job_id: String,
    pub(super) session_id: Option<String>,
    pub(super) status: String,
    pub(super) parallel_group_id: Option<String>,
    pub(super) lane_id: Option<String>,
    pub(super) parent_job_id: Option<String>,
    #[serde(default = "default_multitask_strategy")]
    pub(super) multitask_strategy: String,
    #[serde(default)]
    pub(super) result: Option<Value>,
    #[serde(default)]
    pub(super) error: Option<String>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    #[serde(default = "default_attempt")]
    pub(super) attempt: i64,
    #[serde(default = "default_retry_count")]
    pub(super) retry_count: i64,
    #[serde(default = "default_max_attempts")]
    pub(super) max_attempts: i64,
    #[serde(default)]
    pub(super) timeout_seconds: Option<f64>,
    #[serde(default)]
    pub(super) claimed_by: Option<String>,
    #[serde(default)]
    pub(super) claimed_at: Option<String>,
    #[serde(default = "default_backoff_base_seconds")]
    pub(super) backoff_base_seconds: f64,
    #[serde(default = "default_backoff_multiplier")]
    pub(super) backoff_multiplier: f64,
    #[serde(default)]
    pub(super) max_backoff_seconds: Option<f64>,
    #[serde(default)]
    pub(super) backoff_seconds: Option<f64>,
    #[serde(default)]
    pub(super) next_retry_at: Option<String>,
    #[serde(default)]
    pub(super) retry_scheduled_at: Option<String>,
    #[serde(default)]
    pub(super) retry_claimed_at: Option<String>,
    #[serde(default)]
    pub(super) interrupt_requested_at: Option<String>,
    #[serde(default)]
    pub(super) interrupted_at: Option<String>,
    #[serde(default)]
    pub(super) last_attempt_started_at: Option<String>,
    #[serde(default)]
    pub(super) last_attempt_finished_at: Option<String>,
    #[serde(default)]
    pub(super) last_failure_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct BackgroundJobStatusMutation {
    pub(super) status: String,
    pub(super) session_id: Option<String>,
    pub(super) parallel_group_id: Option<String>,
    pub(super) lane_id: Option<String>,
    pub(super) parent_job_id: Option<String>,
    pub(super) multitask_strategy: Option<String>,
    pub(super) result: Option<Value>,
    pub(super) error: Option<String>,
    pub(super) timeout_seconds: Option<f64>,
    pub(super) claimed_by: Option<String>,
    pub(super) attempt: Option<i64>,
    pub(super) retry_count: Option<i64>,
    pub(super) max_attempts: Option<i64>,
    pub(super) claimed_at: Option<String>,
    pub(super) backoff_base_seconds: Option<f64>,
    pub(super) backoff_multiplier: Option<f64>,
    pub(super) max_backoff_seconds: Option<f64>,
    pub(super) backoff_seconds: Option<f64>,
    pub(super) next_retry_at: Option<String>,
    pub(super) retry_scheduled_at: Option<String>,
    pub(super) retry_claimed_at: Option<String>,
    pub(super) interrupt_requested_at: Option<String>,
    pub(super) interrupted_at: Option<String>,
    pub(super) last_attempt_started_at: Option<String>,
    pub(super) last_attempt_finished_at: Option<String>,
    pub(super) last_failure_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PersistedActiveSession {
    pub(super) session_id: String,
    pub(super) job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PersistedPendingTakeover {
    pub(super) session_id: String,
    pub(super) incoming_job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PersistedBackgroundState {
    pub(super) version: i64,
    pub(super) schema_version: String,
    pub(super) control_plane: Option<Value>,
    pub(super) jobs: Vec<BackgroundRunStatus>,
    pub(super) active_sessions: Vec<PersistedActiveSession>,
    pub(super) pending_session_takeovers: Vec<PersistedPendingTakeover>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BackgroundSessionTakeoverArbitration {
    pub(super) schema_version: String,
    pub(super) operation: String,
    pub(super) session_id: String,
    pub(super) incoming_job_id: String,
    pub(super) previous_active_job_id: Option<String>,
    pub(super) previous_pending_job_id: Option<String>,
    pub(super) active_job_id: Option<String>,
    pub(super) pending_job_id: Option<String>,
    pub(super) outcome: String,
    pub(super) changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BackgroundParallelGroupSummary {
    pub(super) parallel_group_id: String,
    pub(super) job_ids: Vec<String>,
    pub(super) session_ids: Vec<String>,
    pub(super) lane_ids: Vec<String>,
    pub(super) parent_job_ids: Vec<String>,
    pub(super) status_counts: Map<String, Value>,
    pub(super) active_job_count: usize,
    pub(super) terminal_job_count: usize,
    pub(super) total_job_count: usize,
    pub(super) latest_updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct BackgroundStateStore {
    pub(super) state_path: PathBuf,
    pub(super) backend_family: String,
    pub(super) sqlite_db_path: Option<PathBuf>,
    pub(super) control_plane: Value,
    pub(super) jobs: HashMap<String, BackgroundRunStatus>,
    pub(super) active_sessions: HashMap<String, String>,
    pub(super) pending_session_takeovers: HashMap<String, String>,
    /// Set by `load` when reaper modified jobs in memory but did not yet
    /// persist them. Mutating handlers consume this flag via
    /// `flush_reap_if_dirty` to fold the reap into their persist step;
    /// read-only handlers leave it alone so reads stay disk-side-effect-free.
    pub(super) reaped_dirty: bool,
}
