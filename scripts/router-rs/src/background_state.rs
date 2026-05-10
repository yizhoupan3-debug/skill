use crate::runtime_storage::{acquire_runtime_path_lock, runtime_backend_capabilities};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const BACKGROUND_STATE_STORE_SCHEMA_VERSION: &str = "router-rs-background-state-store-v1";
pub const BACKGROUND_STATE_STORE_AUTHORITY: &str = "rust-background-state-store";
const BACKGROUND_STATE_REQUEST_SCHEMA_VERSION: &str = "router-rs-background-state-request-v1";
const BACKGROUND_STATE_SCHEMA_VERSION: &str = "runtime-background-state-v5";
const BACKGROUND_STATE_CONTROL_PLANE_SCHEMA_VERSION: &str =
    "runtime-background-state-control-plane-v1";
const BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION: &str =
    "runtime-background-session-takeover-arbitration-v1";
const DEFAULT_STATE_SERVICE_AUTHORITY: &str = "rust-runtime-control-plane";
const DEFAULT_STATE_SERVICE_ROLE: &str = "durable-background-state";
const DEFAULT_STATE_SERVICE_PROJECTION: &str = "rust-native-projection";
const DEFAULT_BACKGROUND_JOB_MULTITASK_STRATEGY: &str = "reject";
const DEFAULT_BACKGROUND_JOB_ATTEMPT: i64 = 1;
const DEFAULT_BACKGROUND_JOB_RETRY_COUNT: i64 = 0;
const DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS: i64 = 1;
const DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS: f64 = 0.0;
const DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER: f64 = 2.0;
const DEFAULT_MAX_BACKGROUND_JOBS: usize = 16;
const MAX_BACKGROUND_JOBS_LIMIT: usize = 64;
const SQLITE_TABLE_NAME: &str = "runtime_storage_payloads";

/// Reap window for jobs whose status is still active (queued/running/...) but
/// whose `updated_at` heartbeat has gone silent. Such jobs are typically
/// produced when a host process is killed without a clean transition; if we
/// don't reap them they hold session reservations forever and block legitimate
/// new owners. Default 1h gives plenty of slack for slow-but-alive workers.
const STALE_ACTIVE_HEARTBEAT_TTL_SECS: i64 = 3600;

/// Garbage-collection window for jobs already in a terminal state
/// (completed/failed/interrupted/retry_exhausted). After this, the job is
/// dropped from the in-memory map so the persisted file does not grow without
/// bound across long-running deployments.
const STALE_TERMINAL_JOB_TTL_SECS: i64 = 24 * 3600;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackgroundStateRequestPayload {
    schema_version: String,
    operation: String,
    state_path: Option<String>,
    backend_family: Option<String>,
    sqlite_db_path: Option<String>,
    state_payload_text: Option<String>,
    control_plane_descriptor: Option<Value>,
    job_id: Option<String>,
    arbitration_operation: Option<String>,
    mutation: Option<BackgroundJobStatusMutation>,
    session_id: Option<String>,
    incoming_job_id: Option<String>,
    parallel_group_id: Option<String>,
    capacity_limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BackgroundRunStatus {
    job_id: String,
    session_id: Option<String>,
    status: String,
    parallel_group_id: Option<String>,
    lane_id: Option<String>,
    parent_job_id: Option<String>,
    #[serde(default = "default_multitask_strategy")]
    multitask_strategy: String,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default = "default_attempt")]
    attempt: i64,
    #[serde(default = "default_retry_count")]
    retry_count: i64,
    #[serde(default = "default_max_attempts")]
    max_attempts: i64,
    #[serde(default)]
    timeout_seconds: Option<f64>,
    #[serde(default)]
    claimed_by: Option<String>,
    #[serde(default)]
    claimed_at: Option<String>,
    #[serde(default = "default_backoff_base_seconds")]
    backoff_base_seconds: f64,
    #[serde(default = "default_backoff_multiplier")]
    backoff_multiplier: f64,
    #[serde(default)]
    max_backoff_seconds: Option<f64>,
    #[serde(default)]
    backoff_seconds: Option<f64>,
    #[serde(default)]
    next_retry_at: Option<String>,
    #[serde(default)]
    retry_scheduled_at: Option<String>,
    #[serde(default)]
    retry_claimed_at: Option<String>,
    #[serde(default)]
    interrupt_requested_at: Option<String>,
    #[serde(default)]
    interrupted_at: Option<String>,
    #[serde(default)]
    last_attempt_started_at: Option<String>,
    #[serde(default)]
    last_attempt_finished_at: Option<String>,
    #[serde(default)]
    last_failure_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BackgroundJobStatusMutation {
    status: String,
    session_id: Option<String>,
    parallel_group_id: Option<String>,
    lane_id: Option<String>,
    parent_job_id: Option<String>,
    multitask_strategy: Option<String>,
    result: Option<Value>,
    error: Option<String>,
    timeout_seconds: Option<f64>,
    claimed_by: Option<String>,
    attempt: Option<i64>,
    retry_count: Option<i64>,
    max_attempts: Option<i64>,
    claimed_at: Option<String>,
    backoff_base_seconds: Option<f64>,
    backoff_multiplier: Option<f64>,
    max_backoff_seconds: Option<f64>,
    backoff_seconds: Option<f64>,
    next_retry_at: Option<String>,
    retry_scheduled_at: Option<String>,
    retry_claimed_at: Option<String>,
    interrupt_requested_at: Option<String>,
    interrupted_at: Option<String>,
    last_attempt_started_at: Option<String>,
    last_attempt_finished_at: Option<String>,
    last_failure_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedActiveSession {
    session_id: String,
    job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedPendingTakeover {
    session_id: String,
    incoming_job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedBackgroundState {
    version: i64,
    schema_version: String,
    control_plane: Option<Value>,
    jobs: Vec<BackgroundRunStatus>,
    active_sessions: Vec<PersistedActiveSession>,
    pending_session_takeovers: Vec<PersistedPendingTakeover>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackgroundSessionTakeoverArbitration {
    schema_version: String,
    operation: String,
    session_id: String,
    incoming_job_id: String,
    previous_active_job_id: Option<String>,
    previous_pending_job_id: Option<String>,
    active_job_id: Option<String>,
    pending_job_id: Option<String>,
    outcome: String,
    changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackgroundParallelGroupSummary {
    parallel_group_id: String,
    job_ids: Vec<String>,
    session_ids: Vec<String>,
    lane_ids: Vec<String>,
    parent_job_ids: Vec<String>,
    status_counts: Map<String, Value>,
    active_job_count: usize,
    terminal_job_count: usize,
    total_job_count: usize,
    latest_updated_at: Option<String>,
}

#[derive(Debug, Clone)]
struct BackgroundStateStore {
    state_path: PathBuf,
    backend_family: String,
    sqlite_db_path: Option<PathBuf>,
    control_plane: Value,
    jobs: HashMap<String, BackgroundRunStatus>,
    active_sessions: HashMap<String, String>,
    pending_session_takeovers: HashMap<String, String>,
    /// Set by `load` when reaper modified jobs in memory but did not yet
    /// persist them. Mutating handlers consume this flag via
    /// `flush_reap_if_dirty` to fold the reap into their persist step;
    /// read-only handlers leave it alone so reads stay disk-side-effect-free.
    reaped_dirty: bool,
}

fn default_multitask_strategy() -> String {
    DEFAULT_BACKGROUND_JOB_MULTITASK_STRATEGY.to_string()
}

fn default_attempt() -> i64 {
    DEFAULT_BACKGROUND_JOB_ATTEMPT
}

fn default_retry_count() -> i64 {
    DEFAULT_BACKGROUND_JOB_RETRY_COUNT
}

fn default_max_attempts() -> i64 {
    DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS
}

fn default_backoff_base_seconds() -> f64 {
    DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS
}

fn default_backoff_multiplier() -> f64 {
    DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Best-effort RFC3339 parse used by the reaper. Non-RFC3339 timestamps
/// (legacy or hand-edited state) are treated as "unknown age" and skipped.
fn parse_rfc3339_to_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn is_active_status(status: &str) -> bool {
    matches!(
        status,
        "queued" | "running" | "interrupt_requested" | "retry_scheduled" | "retry_claimed"
    )
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "failed" | "interrupted" | "retry_exhausted"
    )
}

fn validate_transition(previous_status: Option<&str>, next_status: &str) -> Result<(), String> {
    let allowed = match previous_status {
        None => matches!(
            next_status,
            "queued"
                | "running"
                | "interrupt_requested"
                | "retry_scheduled"
                | "retry_claimed"
                | "completed"
                | "failed"
                | "interrupted"
                | "retry_exhausted"
        ),
        Some("queued") => matches!(
            next_status,
            "queued" | "running" | "interrupt_requested" | "interrupted" | "failed"
        ),
        Some("running") => matches!(
            next_status,
            "running"
                | "interrupt_requested"
                | "completed"
                | "failed"
                | "interrupted"
                | "retry_scheduled"
                | "retry_exhausted"
        ),
        Some("interrupt_requested") => matches!(next_status, "interrupt_requested" | "interrupted"),
        Some("retry_scheduled") => matches!(
            next_status,
            "retry_scheduled"
                | "retry_claimed"
                | "interrupt_requested"
                | "interrupted"
                | "retry_exhausted"
        ),
        Some("retry_claimed") => matches!(
            next_status,
            "retry_claimed"
                | "queued"
                | "running"
                | "interrupt_requested"
                | "interrupted"
                | "failed"
                | "retry_scheduled"
                | "retry_exhausted"
        ),
        Some("completed") => next_status == "completed",
        Some("failed") => next_status == "failed",
        Some("interrupted") => next_status == "interrupted",
        Some("retry_exhausted") => next_status == "retry_exhausted",
        // Unknown prior status (legacy / hand-edited / corrupted state).
        // Previously we returned `true` here as a permissive escape hatch,
        // which let zombie or invalid statuses transition to anything and
        // hid storage corruption. Be strict instead: refuse any transition
        // out of an unrecognized state. Operators can still reset such jobs
        // explicitly via reseed (status=None branch above), forcing the
        // problem to the surface.
        Some(_) => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "Invalid background job transition: {:?} -> {:?}",
            previous_status, next_status
        ))
    }
}

fn backend_capabilities(backend_family: &str) -> Result<(bool, bool, bool, bool), String> {
    let capabilities = runtime_backend_capabilities(backend_family)
        .map_err(|err| format!("Unsupported durable background-state backend family: {err}"))?;
    Ok((
        capabilities.supports_atomic_replace,
        capabilities.supports_compaction,
        capabilities.supports_snapshot_delta,
        capabilities.supports_remote_event_transport,
    ))
}

fn normalized_backend_family(value: &str) -> String {
    value.trim().to_lowercase().replace('-', "_")
}

fn background_delegate_kind(backend_family: &str) -> String {
    format!(
        "{}-state-store",
        backend_family.trim().to_lowercase().replace('_', "-")
    )
}

fn build_state_control_plane(
    control_plane_descriptor: Option<&Value>,
    backend_family: &str,
    state_path: &Path,
) -> Result<Value, String> {
    let normalized_backend = normalized_backend_family(backend_family);
    let (
        supports_atomic_replace,
        supports_compaction,
        supports_snapshot_delta,
        supports_remote_event_transport,
    ) = backend_capabilities(&normalized_backend)?;
    let mut payload = json!({
        "schema_version": BACKGROUND_STATE_CONTROL_PLANE_SCHEMA_VERSION,
        "runtime_control_plane_schema_version": control_plane_descriptor
            .and_then(|value| value.get("schema_version"))
            .cloned()
            .unwrap_or(Value::Null),
        "runtime_control_plane_authority": control_plane_descriptor
            .and_then(|value| value.get("authority"))
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_STATE_SERVICE_AUTHORITY),
        "service": "state",
        "authority": DEFAULT_STATE_SERVICE_AUTHORITY,
        "role": DEFAULT_STATE_SERVICE_ROLE,
        "projection": DEFAULT_STATE_SERVICE_PROJECTION,
        "delegate_kind": background_delegate_kind(&normalized_backend),
        "transport_family": "checkpoint-artifact",
        "health_family": "runtime-health",
        "backend_family": normalized_backend,
        "supports_atomic_replace": supports_atomic_replace,
        "supports_compaction": supports_compaction,
        "supports_snapshot_delta": supports_snapshot_delta,
        "supports_remote_event_transport": supports_remote_event_transport,
        "supports_consistent_append": runtime_backend_capabilities(&normalized_backend)?.supports_consistent_append,
        "supports_sqlite_wal": runtime_backend_capabilities(&normalized_backend)?.supports_sqlite_wal,
        "state_path": state_path.to_string_lossy(),
    });
    if let Some(Value::Object(descriptor)) = control_plane_descriptor {
        if let Some(Value::Object(services)) = descriptor.get("services") {
            if let Some(Value::Object(service)) = services.get("state") {
                for field in ["authority", "role", "projection", "delegate_kind"] {
                    if let Some(value) = service.get(field) {
                        payload[field] = value.clone();
                    }
                }
            }
        }
    }
    if payload.get("delegate_kind").and_then(Value::as_str) == Some("filesystem-state-store")
        && normalized_backend != "filesystem"
    {
        payload["delegate_kind"] = Value::String(background_delegate_kind(&normalized_backend));
    }
    Ok(payload)
}

impl BackgroundRunStatus {
    fn touched(&self) -> Self {
        let mut updated = self.clone();
        updated.updated_at = now_iso();
        updated
    }

    fn claimed_placeholder(job_id: &str, session_id: &str) -> Self {
        let now = now_iso();
        BackgroundRunStatus {
            job_id: job_id.to_string(),
            session_id: Some(session_id.to_string()),
            status: "retry_claimed".to_string(),
            parallel_group_id: None,
            lane_id: None,
            parent_job_id: None,
            multitask_strategy: default_multitask_strategy(),
            result: None,
            error: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            attempt: DEFAULT_BACKGROUND_JOB_ATTEMPT,
            retry_count: DEFAULT_BACKGROUND_JOB_RETRY_COUNT,
            max_attempts: DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS,
            timeout_seconds: None,
            claimed_by: Some(job_id.to_string()),
            claimed_at: Some(now.clone()),
            backoff_base_seconds: DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS,
            backoff_multiplier: DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER,
            max_backoff_seconds: None,
            backoff_seconds: None,
            next_retry_at: None,
            retry_scheduled_at: None,
            retry_claimed_at: Some(now),
            interrupt_requested_at: None,
            interrupted_at: None,
            last_attempt_started_at: None,
            last_attempt_finished_at: None,
            last_failure_at: None,
        }
    }
}

impl BackgroundJobStatusMutation {
    fn apply(&self, job_id: &str, existing: Option<&BackgroundRunStatus>) -> BackgroundRunStatus {
        match existing {
            None => BackgroundRunStatus {
                job_id: job_id.to_string(),
                session_id: self.session_id.clone(),
                status: self.status.clone(),
                parallel_group_id: self.parallel_group_id.clone(),
                lane_id: self.lane_id.clone(),
                parent_job_id: self.parent_job_id.clone(),
                multitask_strategy: self
                    .multitask_strategy
                    .clone()
                    .unwrap_or_else(default_multitask_strategy),
                result: self.result.clone(),
                error: self.error.clone(),
                created_at: now_iso(),
                updated_at: now_iso(),
                attempt: self.attempt.unwrap_or(DEFAULT_BACKGROUND_JOB_ATTEMPT),
                retry_count: self
                    .retry_count
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_RETRY_COUNT),
                max_attempts: self
                    .max_attempts
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS),
                timeout_seconds: self.timeout_seconds,
                claimed_by: self.claimed_by.clone(),
                claimed_at: self.claimed_at.clone(),
                backoff_base_seconds: self
                    .backoff_base_seconds
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS),
                backoff_multiplier: self
                    .backoff_multiplier
                    .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER),
                max_backoff_seconds: self.max_backoff_seconds,
                backoff_seconds: self.backoff_seconds,
                next_retry_at: self.next_retry_at.clone(),
                retry_scheduled_at: self.retry_scheduled_at.clone(),
                retry_claimed_at: self.retry_claimed_at.clone(),
                interrupt_requested_at: self.interrupt_requested_at.clone(),
                interrupted_at: self.interrupted_at.clone(),
                last_attempt_started_at: self.last_attempt_started_at.clone(),
                last_attempt_finished_at: self.last_attempt_finished_at.clone(),
                last_failure_at: self.last_failure_at.clone(),
            },
            Some(existing) => {
                let mut updated = existing.touched();
                updated.status = self.status.clone();
                updated.session_id = self.session_id.clone();
                if self.parallel_group_id.is_some() {
                    updated.parallel_group_id = self.parallel_group_id.clone();
                }
                if self.lane_id.is_some() {
                    updated.lane_id = self.lane_id.clone();
                }
                if self.parent_job_id.is_some() {
                    updated.parent_job_id = self.parent_job_id.clone();
                }
                if self.multitask_strategy.is_some() {
                    updated.multitask_strategy =
                        self.multitask_strategy.clone().unwrap_or_default();
                }
                updated.result = self.result.clone();
                updated.error = self.error.clone();
                if self.attempt.is_some() {
                    updated.attempt = self.attempt.unwrap_or(DEFAULT_BACKGROUND_JOB_ATTEMPT);
                }
                if self.retry_count.is_some() {
                    updated.retry_count = self
                        .retry_count
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_RETRY_COUNT);
                }
                if self.max_attempts.is_some() {
                    updated.max_attempts = self
                        .max_attempts
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_MAX_ATTEMPTS);
                }
                if self.timeout_seconds.is_some() {
                    updated.timeout_seconds = self.timeout_seconds;
                }
                if self.claimed_by.is_some() {
                    updated.claimed_by = self.claimed_by.clone();
                }
                if self.claimed_at.is_some() {
                    updated.claimed_at = self.claimed_at.clone();
                }
                if self.backoff_base_seconds.is_some() {
                    updated.backoff_base_seconds = self
                        .backoff_base_seconds
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_BASE_SECONDS);
                }
                if self.backoff_multiplier.is_some() {
                    updated.backoff_multiplier = self
                        .backoff_multiplier
                        .unwrap_or(DEFAULT_BACKGROUND_JOB_BACKOFF_MULTIPLIER);
                }
                if self.max_backoff_seconds.is_some() {
                    updated.max_backoff_seconds = self.max_backoff_seconds;
                }
                updated.backoff_seconds = self.backoff_seconds;
                updated.next_retry_at = self.next_retry_at.clone();
                updated.retry_scheduled_at = self.retry_scheduled_at.clone();
                updated.retry_claimed_at = self.retry_claimed_at.clone();
                if self.interrupt_requested_at.is_some() {
                    updated.interrupt_requested_at = self.interrupt_requested_at.clone();
                }
                if self.interrupted_at.is_some() {
                    updated.interrupted_at = self.interrupted_at.clone();
                }
                if self.last_attempt_started_at.is_some() {
                    updated.last_attempt_started_at = self.last_attempt_started_at.clone();
                }
                if self.last_attempt_finished_at.is_some() {
                    updated.last_attempt_finished_at = self.last_attempt_finished_at.clone();
                }
                if self.last_failure_at.is_some() {
                    updated.last_failure_at = self.last_failure_at.clone();
                }
                updated
            }
        }
    }
}

impl BackgroundStateStore {
    fn load(request: &BackgroundStateRequestPayload) -> Result<Self, String> {
        let state_path = request
            .state_path
            .as_ref()
            .ok_or_else(|| "Background state request is missing state_path.".to_string())
            .map(PathBuf::from)?;
        let backend_family = request
            .backend_family
            .clone()
            .unwrap_or_else(|| "filesystem".to_string());
        let sqlite_db_path = request.sqlite_db_path.as_ref().map(PathBuf::from);
        let control_plane = build_state_control_plane(
            request.control_plane_descriptor.as_ref(),
            &backend_family,
            &state_path,
        )?;
        let persisted = read_persisted_state(
            &state_path,
            &backend_family,
            sqlite_db_path.as_deref(),
            request.state_payload_text.as_deref(),
        )?;
        let mut store = Self {
            state_path,
            backend_family: normalized_backend_family(&backend_family),
            sqlite_db_path,
            control_plane,
            jobs: HashMap::new(),
            active_sessions: HashMap::new(),
            pending_session_takeovers: HashMap::new(),
            reaped_dirty: false,
        };
        if let Some(persisted) = persisted {
            store.merge_persisted(persisted)?;
        }
        // Reap zombie / over-aged jobs into the in-memory view so every
        // operation sees a clean snapshot. We deliberately do **not** persist
        // here: that would turn pure-read operations (snapshot/get/health)
        // into silent disk writers, breaking the "read = read-only" contract
        // and forcing every reader through the path-lock + filesystem rename
        // machinery. Instead, `reaped_dirty` is set so mutating handlers
        // (`apply_mutation`, arbitration, reservation) flush the cleanup as
        // part of their normal persist step. Pure readers keep the cleanup
        // in their local view and re-derive it on the next load — cheap,
        // since the reap is an in-memory HashMap scan.
        let now = Utc::now();
        let reaped_active = store.reap_stale_active_jobs(now);
        let reaped_terminal = store.reap_stale_terminal_jobs(now);
        let reaped_ghost = store.reap_ghost_status_jobs(now);
        if reaped_active + reaped_terminal + reaped_ghost > 0 {
            store.reaped_dirty = true;
        }
        store.compact_terminal_over_capacity(request.capacity_limit);
        Ok(store)
    }

    /// Best-effort persist of the in-memory reap cleanup. Called by mutating
    /// handlers right after `load` so reaped state lands on disk together
    /// with the user-driven mutation. Failures are logged loudly to stderr
    /// instead of being silently dropped: an indefinitely-failing reap
    /// persist (full disk, permissions, etc.) is an operational concern that
    /// must surface in logs, not hide behind `let _ =`.
    fn flush_reap_if_dirty(&mut self) {
        if !self.reaped_dirty {
            return;
        }
        match self.persist() {
            Ok(_) => {
                self.reaped_dirty = false;
            }
            Err(err) => {
                eprintln!(
                    "[router-rs] background_state reaper persist failed for {} (non-fatal, will retry on next mutation): {err}",
                    self.state_path.display()
                );
            }
        }
    }

    /// Transition active jobs whose `updated_at` heartbeat is older than
    /// `STALE_ACTIVE_HEARTBEAT_TTL_SECS` to `interrupted` so they release
    /// session reservations and become eligible for garbage collection.
    /// Returns the number of jobs reaped.
    fn reap_stale_active_jobs(&mut self, now: DateTime<Utc>) -> usize {
        let cutoff = now - chrono::Duration::seconds(STALE_ACTIVE_HEARTBEAT_TTL_SECS);
        let stale_ids: Vec<String> = self
            .jobs
            .iter()
            .filter(|(_, job)| is_active_status(&job.status))
            .filter(|(_, job)| {
                parse_rfc3339_to_utc(&job.updated_at)
                    .map(|ts| ts < cutoff)
                    .unwrap_or(false)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let now_iso = now.to_rfc3339();
        for job_id in &stale_ids {
            if let Some(job) = self.jobs.get_mut(job_id) {
                let prev_status = job.status.clone();
                job.status = "interrupted".to_string();
                job.interrupted_at = Some(now_iso.clone());
                job.updated_at = now_iso.clone();
                let reaper_msg = format!(
                    "reaped: {prev_status} heartbeat stale > {STALE_ACTIVE_HEARTBEAT_TTL_SECS}s"
                );
                job.error = Some(match job.error.as_deref() {
                    Some(prev) if !prev.is_empty() => format!("{prev}; {reaper_msg}"),
                    _ => reaper_msg,
                });
            }
        }
        if !stale_ids.is_empty() {
            self.active_sessions
                .retain(|_, owner| !stale_ids.iter().any(|id| id == owner));
            self.pending_session_takeovers
                .retain(|_, incoming| !stale_ids.iter().any(|id| id == incoming));
        }
        stale_ids.len()
    }

    /// Drop terminal jobs older than `STALE_TERMINAL_JOB_TTL_SECS` so the
    /// persisted file stays bounded across days/weeks of long-running use.
    fn reap_stale_terminal_jobs(&mut self, now: DateTime<Utc>) -> usize {
        let cutoff = now - chrono::Duration::seconds(STALE_TERMINAL_JOB_TTL_SECS);
        let drop_ids: Vec<String> = self
            .jobs
            .iter()
            .filter(|(_, job)| is_terminal_status(&job.status))
            .filter(|(_, job)| {
                parse_rfc3339_to_utc(&job.updated_at)
                    .map(|ts| ts < cutoff)
                    .unwrap_or(false)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let count = drop_ids.len();
        for id in drop_ids {
            self.jobs.remove(&id);
        }
        count
    }

    /// Sweep "ghost" jobs whose `status` is neither active nor terminal.
    ///
    /// After we tightened `validate_transition` to refuse transitions out of
    /// unrecognized prior statuses, any pre-existing ghost (e.g. a status
    /// string introduced by a future schema, hand-edited, or persisted from
    /// a corrupted run) becomes permanently uncoverable: the FSM rejects
    /// every mutation and the active/terminal reapers never see it. Force
    /// such jobs into `interrupted` with a diagnostic `error` so:
    ///   - the next terminal-TTL pass can drop them on schedule,
    ///   - operators see exactly which ghost status was observed,
    ///   - session/takeover maps releasing the slot follow the same flow as
    ///     stale-active reaping.
    ///
    /// Returns the number of jobs converted.
    fn reap_ghost_status_jobs(&mut self, now: DateTime<Utc>) -> usize {
        let ghost_pairs: Vec<(String, String)> = self
            .jobs
            .iter()
            .filter(|(_, job)| !is_active_status(&job.status) && !is_terminal_status(&job.status))
            .map(|(id, job)| (id.clone(), job.status.clone()))
            .collect();
        if ghost_pairs.is_empty() {
            return 0;
        }
        let now_iso = now.to_rfc3339();
        for (job_id, prev_status) in &ghost_pairs {
            if let Some(job) = self.jobs.get_mut(job_id) {
                job.status = "interrupted".to_string();
                job.interrupted_at = Some(now_iso.clone());
                job.updated_at = now_iso.clone();
                let reaper_msg =
                    format!("reaped: ghost_status={prev_status:?} not in active/terminal FSM");
                job.error = Some(match job.error.as_deref() {
                    Some(prev) if !prev.is_empty() => format!("{prev}; {reaper_msg}"),
                    _ => reaper_msg,
                });
            }
        }
        let ghost_ids: Vec<&str> = ghost_pairs.iter().map(|(id, _)| id.as_str()).collect();
        self.active_sessions
            .retain(|_, owner| !ghost_ids.iter().any(|id| *id == owner));
        self.pending_session_takeovers
            .retain(|_, incoming| !ghost_ids.iter().any(|id| *id == incoming));
        ghost_pairs.len()
    }

    fn merge_persisted(&mut self, persisted: PersistedBackgroundState) -> Result<(), String> {
        if let Some(Value::Object(persisted_control_plane)) = persisted.control_plane {
            if let Value::Object(ref mut current) = self.control_plane {
                for (key, value) in persisted_control_plane {
                    if !value.is_null() {
                        current.insert(key, value);
                    }
                }
            }
        }
        self.jobs = persisted
            .jobs
            .into_iter()
            .map(|job| (job.job_id.clone(), job))
            .collect();
        self.active_sessions = if persisted.active_sessions.is_empty() {
            self.rebuild_active_sessions()
        } else {
            persisted
                .active_sessions
                .into_iter()
                .map(|row| (row.session_id, row.job_id))
                .collect()
        };
        self.active_sessions.retain(|_, job_id| {
            self.jobs
                .get(job_id)
                .map(|job| is_active_status(&job.status))
                .unwrap_or(false)
        });
        self.pending_session_takeovers = persisted
            .pending_session_takeovers
            .into_iter()
            .filter(|row| {
                // Keep pending takeover when either:
                // 1) incoming job exists and is still active, or
                // 2) the target session is still known in persisted jobs
                //    (including recently completed owners) so a follow-up claim
                //    can finish the handoff.
                self.jobs
                    .get(&row.incoming_job_id)
                    .map(|job| is_active_status(&job.status))
                    .unwrap_or(false)
                    || self
                        .jobs
                        .values()
                        .any(|job| job.session_id.as_deref() == Some(row.session_id.as_str()))
            })
            .map(|row| (row.session_id, row.incoming_job_id))
            .collect();
        Ok(())
    }

    fn rebuild_active_sessions(&self) -> HashMap<String, String> {
        let mut rows = self
            .jobs
            .values()
            .filter(|job| job.session_id.is_some() && is_active_status(&job.status))
            .map(|job| {
                (
                    job.updated_at.clone(),
                    job.job_id.clone(),
                    job.session_id.clone().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        let mut rebuilt = HashMap::new();
        for (_, job_id, session_id) in rows {
            rebuilt.insert(session_id, job_id);
        }
        rebuilt
    }

    fn serialized_payload(&self) -> Result<String, String> {
        let persisted = PersistedBackgroundState {
            version: 2,
            schema_version: BACKGROUND_STATE_SCHEMA_VERSION.to_string(),
            control_plane: Some(self.control_plane.clone()),
            jobs: sorted_jobs(&self.jobs),
            active_sessions: sorted_string_pairs(&self.active_sessions)
                .into_iter()
                .map(|(session_id, job_id)| PersistedActiveSession { session_id, job_id })
                .collect(),
            pending_session_takeovers: sorted_string_pairs(&self.pending_session_takeovers)
                .into_iter()
                .map(|(session_id, incoming_job_id)| PersistedPendingTakeover {
                    session_id,
                    incoming_job_id,
                })
                .collect(),
        };
        serde_json::to_string_pretty(&persisted)
            .map(|payload| payload + "\n")
            .map_err(|err| err.to_string())
    }

    fn persist(&self) -> Result<Option<String>, String> {
        let payload = self.serialized_payload()?;
        if self.backend_family == "memory" {
            return Ok(Some(payload));
        }
        write_persisted_state(
            &self.state_path,
            &self.backend_family,
            self.sqlite_db_path.as_deref(),
            &payload,
        )?;
        Ok(None)
    }

    fn apply_mutation(
        &mut self,
        job_id: &str,
        mutation: &BackgroundJobStatusMutation,
    ) -> Result<(BackgroundRunStatus, Option<String>), String> {
        let existing = self.jobs.get(job_id).cloned();
        let previous_status = existing.as_ref().map(|job| job.status.as_str());
        validate_transition(previous_status, &mutation.status)?;
        let previous_session_id = existing.as_ref().and_then(|job| job.session_id.clone());
        let resolved_session_id = mutation
            .session_id
            .clone()
            .or_else(|| previous_session_id.clone());
        self.reserve_session(job_id, resolved_session_id.as_deref(), &mutation.status)?;
        let resolved_mutation = BackgroundJobStatusMutation {
            status: mutation.status.clone(),
            session_id: resolved_session_id.clone(),
            parallel_group_id: mutation.parallel_group_id.clone(),
            lane_id: mutation.lane_id.clone(),
            parent_job_id: mutation.parent_job_id.clone(),
            multitask_strategy: mutation.multitask_strategy.clone(),
            result: mutation.result.clone(),
            error: mutation.error.clone(),
            timeout_seconds: mutation.timeout_seconds,
            claimed_by: mutation.claimed_by.clone(),
            attempt: mutation.attempt,
            retry_count: mutation.retry_count,
            max_attempts: mutation.max_attempts,
            claimed_at: mutation.claimed_at.clone(),
            backoff_base_seconds: mutation.backoff_base_seconds,
            backoff_multiplier: mutation.backoff_multiplier,
            max_backoff_seconds: mutation.max_backoff_seconds,
            backoff_seconds: mutation.backoff_seconds,
            next_retry_at: mutation.next_retry_at.clone(),
            retry_scheduled_at: mutation.retry_scheduled_at.clone(),
            retry_claimed_at: mutation.retry_claimed_at.clone(),
            interrupt_requested_at: mutation.interrupt_requested_at.clone(),
            interrupted_at: mutation.interrupted_at.clone(),
            last_attempt_started_at: mutation.last_attempt_started_at.clone(),
            last_attempt_finished_at: mutation.last_attempt_finished_at.clone(),
            last_failure_at: mutation.last_failure_at.clone(),
        };
        let updated = resolved_mutation.apply(job_id, existing.as_ref());
        self.jobs.insert(job_id.to_string(), updated.clone());
        self.release_previous_session(
            job_id,
            previous_session_id.as_deref(),
            resolved_session_id.as_deref(),
        );
        self.finalize_session(job_id, resolved_session_id.as_deref(), &mutation.status);
        let persisted_payload_text = self.persist()?;
        Ok((updated, persisted_payload_text))
    }

    fn reserve_session(
        &mut self,
        job_id: &str,
        session_id: Option<&str>,
        status: &str,
    ) -> Result<(), String> {
        let Some(session_id) = session_id else {
            return Ok(());
        };
        if !is_active_status(status) {
            return Ok(());
        }
        if let Some(owner) = self.active_sessions.get(session_id) {
            if owner != job_id {
                return Err(format!(
                    "Session {session_id:?} is already active in job {owner:?}."
                ));
            }
        }
        self.active_sessions
            .insert(session_id.to_string(), job_id.to_string());
        Ok(())
    }

    fn release_previous_session(
        &mut self,
        job_id: &str,
        previous_session_id: Option<&str>,
        next_session_id: Option<&str>,
    ) {
        let Some(previous_session_id) = previous_session_id else {
            return;
        };
        if Some(previous_session_id) == next_session_id {
            return;
        }
        if self
            .active_sessions
            .get(previous_session_id)
            .map(String::as_str)
            == Some(job_id)
        {
            self.active_sessions.remove(previous_session_id);
        }
    }

    fn finalize_session(&mut self, job_id: &str, session_id: Option<&str>, status: &str) {
        let Some(session_id) = session_id else {
            return;
        };
        if !is_terminal_status(status) {
            return;
        }
        if self.active_sessions.get(session_id).map(String::as_str) == Some(job_id) {
            self.active_sessions.remove(session_id);
        }
    }

    fn get(&self, job_id: &str) -> Option<BackgroundRunStatus> {
        self.jobs.get(job_id).cloned()
    }

    fn active_job(&self, session_id: &str) -> Option<String> {
        self.active_sessions.get(session_id).cloned()
    }

    fn active_job_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| is_active_status(&job.status))
            .count()
    }

    fn terminal_job_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| is_terminal_status(&job.status))
            .count()
    }

    fn resolved_capacity_limit(&self, override_limit: Option<usize>) -> usize {
        override_limit
            .or_else(|| {
                self.control_plane
                    .get("max_background_jobs")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
            })
            .unwrap_or(DEFAULT_MAX_BACKGROUND_JOBS)
            .clamp(1, MAX_BACKGROUND_JOBS_LIMIT)
    }

    fn compact_terminal_over_capacity(&mut self, capacity_limit: Option<usize>) {
        let limit = self.resolved_capacity_limit(capacity_limit);
        if self.jobs.len() <= limit {
            return;
        }
        let mut terminal_jobs = self
            .jobs
            .values()
            .filter(|job| is_terminal_status(&job.status))
            .map(|job| (job.updated_at.clone(), job.job_id.clone()))
            .collect::<Vec<_>>();
        terminal_jobs.sort();
        let remove_count = self
            .jobs
            .len()
            .saturating_sub(limit)
            .min(terminal_jobs.len());
        for (_, job_id) in terminal_jobs.into_iter().take(remove_count) {
            self.jobs.remove(&job_id);
        }
    }

    fn pending_session_takeovers(&self) -> usize {
        self.pending_session_takeovers.len()
    }

    fn parallel_group_summary(
        &self,
        parallel_group_id: &str,
    ) -> Option<BackgroundParallelGroupSummary> {
        let jobs = self
            .jobs
            .values()
            .filter(|job| job.parallel_group_id.as_deref() == Some(parallel_group_id))
            .cloned()
            .collect::<Vec<_>>();
        if jobs.is_empty() {
            return None;
        }
        Some(build_parallel_group_summary(parallel_group_id, &jobs))
    }

    fn parallel_group_summaries(&self) -> Vec<BackgroundParallelGroupSummary> {
        let mut grouped: HashMap<String, Vec<BackgroundRunStatus>> = HashMap::new();
        for job in self.jobs.values() {
            if let Some(group_id) = job.parallel_group_id.clone() {
                grouped.entry(group_id).or_default().push(job.clone());
            }
        }
        let mut group_ids = grouped.keys().cloned().collect::<Vec<_>>();
        group_ids.sort();
        group_ids
            .into_iter()
            .filter_map(|group_id| {
                grouped
                    .get(&group_id)
                    .map(|jobs| build_parallel_group_summary(&group_id, jobs))
            })
            .collect()
    }

    fn arbitrate_session_takeover(
        &mut self,
        operation: &str,
        session_id: &str,
        incoming_job_id: &str,
    ) -> Result<(BackgroundSessionTakeoverArbitration, Option<String>), String> {
        let previous_active_job_id = self.active_sessions.get(session_id).cloned();
        let previous_pending_job_id = self.pending_session_takeovers.get(session_id).cloned();
        let mut changed = false;
        let outcome = match operation {
            "reserve" => {
                if let Some(previous_pending) = previous_pending_job_id.as_deref() {
                    if previous_pending != incoming_job_id {
                        return Err(format!(
                            "Session {session_id:?} already has a pending takeover for job {previous_pending:?}."
                        ));
                    }
                }
                match previous_active_job_id.as_deref() {
                    None => {
                        if previous_pending_job_id.as_deref() == Some(incoming_job_id) {
                            "pending".to_string()
                        } else {
                            "available".to_string()
                        }
                    }
                    Some(active_job_id) if active_job_id == incoming_job_id => "owned".to_string(),
                    Some(_) => {
                        if previous_pending_job_id.as_deref() != Some(incoming_job_id) {
                            self.pending_session_takeovers
                                .insert(session_id.to_string(), incoming_job_id.to_string());
                            changed = true;
                        }
                        "pending".to_string()
                    }
                }
            }
            "claim" => {
                if previous_pending_job_id.as_deref() != Some(incoming_job_id) {
                    return Err(format!(
                        "Session {session_id:?} is not reserved for incoming job {incoming_job_id:?}."
                    ));
                }
                if let Some(active_job_id) = previous_active_job_id.as_deref() {
                    if active_job_id != incoming_job_id {
                        return Err(format!(
                            "Session {session_id:?} is still active in job {active_job_id:?}."
                        ));
                    }
                }
                if previous_active_job_id.as_deref() != Some(incoming_job_id) {
                    self.active_sessions
                        .insert(session_id.to_string(), incoming_job_id.to_string());
                    changed = true;
                }
                if !self.jobs.contains_key(incoming_job_id) {
                    self.jobs.insert(
                        incoming_job_id.to_string(),
                        BackgroundRunStatus::claimed_placeholder(incoming_job_id, session_id),
                    );
                    changed = true;
                }
                if previous_pending_job_id.is_some() {
                    self.pending_session_takeovers.remove(session_id);
                    changed = true;
                }
                "claimed".to_string()
            }
            "release" => {
                if previous_pending_job_id.as_deref() == Some(incoming_job_id) {
                    self.pending_session_takeovers.remove(session_id);
                    changed = true;
                }
                if self.active_sessions.get(session_id).map(String::as_str) == Some(incoming_job_id)
                    && !self.jobs.contains_key(incoming_job_id)
                {
                    self.active_sessions.remove(session_id);
                    changed = true;
                }
                if changed {
                    "released".to_string()
                } else {
                    "noop".to_string()
                }
            }
            other => {
                return Err(format!(
                    "Unsupported takeover arbitration operation: {:?}",
                    other
                ))
            }
        };
        let persisted_payload_text = if changed { self.persist()? } else { None };
        Ok((
            BackgroundSessionTakeoverArbitration {
                schema_version: BACKGROUND_SESSION_TAKEOVER_ARBITRATION_SCHEMA_VERSION.to_string(),
                operation: operation.to_string(),
                session_id: session_id.to_string(),
                incoming_job_id: incoming_job_id.to_string(),
                previous_active_job_id,
                previous_pending_job_id,
                active_job_id: self.active_sessions.get(session_id).cloned(),
                pending_job_id: self.pending_session_takeovers.get(session_id).cloned(),
                outcome,
                changed,
            },
            persisted_payload_text,
        ))
    }

    fn snapshot_payload(&self) -> Value {
        json!({
            "control_plane": self.control_plane,
            "jobs": sorted_jobs(&self.jobs),
            "active_sessions": sorted_string_pairs(&self.active_sessions)
                .into_iter()
                .map(|(session_id, job_id)| json!({"session_id": session_id, "job_id": job_id}))
                .collect::<Vec<_>>(),
            "pending_session_takeovers": sorted_string_pairs(&self.pending_session_takeovers)
                .into_iter()
                .map(|(session_id, incoming_job_id)| json!({"session_id": session_id, "incoming_job_id": incoming_job_id}))
                .collect::<Vec<_>>(),
        })
    }

    fn health_payload(&self) -> Value {
        json!({
            "control_plane_authority": self.control_plane.get("authority").cloned().unwrap_or(Value::Null),
            "control_plane_role": self.control_plane.get("role").cloned().unwrap_or(Value::Null),
            "control_plane_projection": self.control_plane.get("projection").cloned().unwrap_or(Value::Null),
            "control_plane_delegate_kind": self.control_plane.get("delegate_kind").cloned().unwrap_or(Value::Null),
            "runtime_control_plane_authority": self.control_plane.get("runtime_control_plane_authority").cloned().unwrap_or(Value::Null),
            "runtime_control_plane_schema_version": self.control_plane.get("runtime_control_plane_schema_version").cloned().unwrap_or(Value::Null),
            "backend_family": self.control_plane.get("backend_family").cloned().unwrap_or(Value::Null),
            "supports_atomic_replace": self.control_plane.get("supports_atomic_replace").cloned().unwrap_or(Value::Bool(false)),
            "supports_compaction": self.control_plane.get("supports_compaction").cloned().unwrap_or(Value::Bool(false)),
            "supports_snapshot_delta": self.control_plane.get("supports_snapshot_delta").cloned().unwrap_or(Value::Bool(false)),
            "supports_remote_event_transport": self.control_plane.get("supports_remote_event_transport").cloned().unwrap_or(Value::Bool(false)),
            "supports_consistent_append": self.control_plane.get("supports_consistent_append").cloned().unwrap_or(Value::Bool(false)),
            "supports_sqlite_wal": self.control_plane.get("supports_sqlite_wal").cloned().unwrap_or(Value::Bool(false)),
            "state_path": self.control_plane.get("state_path").cloned().unwrap_or(Value::Null),
            "job_count": self.jobs.len(),
            "active_job_count": self.active_job_count(),
            "terminal_job_count": self.terminal_job_count(),
            "max_background_jobs": self.resolved_capacity_limit(None),
            "max_background_jobs_limit": MAX_BACKGROUND_JOBS_LIMIT,
            "parallel_group_count": self.parallel_group_summaries().len(),
            "pending_session_takeovers": self.pending_session_takeovers(),
        })
    }
}

fn sorted_jobs(jobs: &HashMap<String, BackgroundRunStatus>) -> Vec<BackgroundRunStatus> {
    let mut rows = jobs.values().cloned().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.job_id.cmp(&right.job_id));
    rows
}

fn sorted_string_pairs(rows: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut entries = rows
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn build_parallel_group_summary(
    parallel_group_id: &str,
    jobs: &[BackgroundRunStatus],
) -> BackgroundParallelGroupSummary {
    let mut status_counts = Map::new();
    let mut session_ids = Vec::new();
    let mut lane_ids = Vec::new();
    let mut parent_job_ids = Vec::new();
    let mut active_job_count = 0usize;
    let mut terminal_job_count = 0usize;
    let mut latest_updated_at: Option<String> = None;
    let mut job_ids = jobs
        .iter()
        .map(|job| job.job_id.clone())
        .collect::<Vec<_>>();
    job_ids.sort();
    for job in jobs {
        let current = status_counts
            .get(&job.status)
            .and_then(Value::as_u64)
            .unwrap_or(0);
        status_counts.insert(job.status.clone(), Value::from(current + 1));
        if let Some(session_id) = job.session_id.clone() {
            session_ids.push(session_id);
        }
        if let Some(lane_id) = job.lane_id.clone() {
            lane_ids.push(lane_id);
        }
        if let Some(parent_job_id) = job.parent_job_id.clone() {
            parent_job_ids.push(parent_job_id);
        }
        if is_active_status(&job.status) {
            active_job_count += 1;
        }
        if is_terminal_status(&job.status) {
            terminal_job_count += 1;
        }
        if latest_updated_at
            .as_ref()
            .map(|current| job.updated_at > *current)
            .unwrap_or(true)
        {
            latest_updated_at = Some(job.updated_at.clone());
        }
    }
    session_ids.sort();
    session_ids.dedup();
    lane_ids.sort();
    lane_ids.dedup();
    parent_job_ids.sort();
    parent_job_ids.dedup();
    BackgroundParallelGroupSummary {
        parallel_group_id: parallel_group_id.to_string(),
        job_ids,
        session_ids,
        lane_ids,
        parent_job_ids,
        status_counts,
        active_job_count,
        terminal_job_count,
        total_job_count: jobs.len(),
        latest_updated_at,
    }
}

fn read_persisted_state(
    state_path: &Path,
    backend_family: &str,
    sqlite_db_path: Option<&Path>,
    state_payload_text: Option<&str>,
) -> Result<Option<PersistedBackgroundState>, String> {
    match normalized_backend_family(backend_family).as_str() {
        "filesystem" | "file" => {
            if !state_path.is_file() {
                return Ok(None);
            }
            let text = fs::read_to_string(state_path).map_err(|err| err.to_string())?;
            let persisted = serde_json::from_str::<PersistedBackgroundState>(&text)
                .map_err(|err| err.to_string())?;
            Ok(Some(persisted))
        }
        "memory" => {
            let Some(text) = state_payload_text else {
                return Ok(None);
            };
            let persisted = serde_json::from_str::<PersistedBackgroundState>(text)
                .map_err(|err| err.to_string())?;
            Ok(Some(persisted))
        }
        "sqlite" | "sqlite3" => {
            let Some(db_path) = sqlite_db_path else {
                return Err(
                    "SQLite background state request is missing sqlite_db_path.".to_string()
                );
            };
            if !db_path.exists() {
                return Ok(None);
            }
            let storage_root = state_path.parent().ok_or_else(|| {
                "Background state path is missing a parent directory.".to_string()
            })?;
            let stable_key = sqlite_storage_key(storage_root, state_path)?;
            let legacy_key = state_path
                .canonicalize()
                .unwrap_or_else(|_| state_path.to_path_buf());
            let conn = open_sqlite_connection(db_path)?;
            let row: Option<String> = conn
                .query_row(
                    &format!("SELECT payload_text FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1"),
                    params![stable_key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|err| err.to_string())?
                .or_else(|| {
                    conn.query_row(
                        &format!(
                            "SELECT payload_text FROM {SQLITE_TABLE_NAME} WHERE payload_key = ?1"
                        ),
                        params![legacy_key.to_string_lossy().to_string()],
                        |row| row.get(0),
                    )
                    .optional()
                    .ok()
                    .flatten()
                });
            let Some(text) = row else {
                return Ok(None);
            };
            let persisted = serde_json::from_str::<PersistedBackgroundState>(&text)
                .map_err(|err| err.to_string())?;
            Ok(Some(persisted))
        }
        other => Err(format!(
            "Unsupported durable background-state backend family: {:?}",
            other
        )),
    }
}

fn write_persisted_state(
    state_path: &Path,
    backend_family: &str,
    sqlite_db_path: Option<&Path>,
    payload: &str,
) -> Result<(), String> {
    match normalized_backend_family(backend_family).as_str() {
        "filesystem" | "file" => {
            if let Some(parent) = state_path.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            let tmp_path = state_path.with_extension(
                state_path
                    .extension()
                    .map(|value| format!("{}.tmp", value.to_string_lossy()))
                    .unwrap_or_else(|| "tmp".to_string()),
            );
            fs::write(&tmp_path, payload).map_err(|err| err.to_string())?;
            fs::rename(&tmp_path, state_path).map_err(|err| err.to_string())?;
            Ok(())
        }
        "sqlite" | "sqlite3" => {
            let Some(db_path) = sqlite_db_path else {
                return Err(
                    "SQLite background state request is missing sqlite_db_path.".to_string()
                );
            };
            let storage_root = state_path.parent().ok_or_else(|| {
                "Background state path is missing a parent directory.".to_string()
            })?;
            let payload_key = sqlite_storage_key(storage_root, state_path)?;
            let conn = open_sqlite_connection(db_path)?;
            conn.execute(
                &format!(
                    "INSERT INTO {SQLITE_TABLE_NAME} (payload_key, payload_text) VALUES (?1, ?2)
                     ON CONFLICT(payload_key) DO UPDATE SET payload_text = excluded.payload_text"
                ),
                params![payload_key, payload],
            )
            .map_err(|err| err.to_string())?;
            Ok(())
        }
        other => Err(format!(
            "Unsupported durable background-state backend family: {:?}",
            other
        )),
    }
}

fn sqlite_storage_key(storage_root: &Path, state_path: &Path) -> Result<String, String> {
    let resolved_root = storage_root
        .canonicalize()
        .unwrap_or_else(|_| storage_root.to_path_buf());
    let resolved_state = if state_path.exists() {
        state_path
            .canonicalize()
            .unwrap_or_else(|_| state_path.to_path_buf())
    } else {
        state_path.to_path_buf()
    };
    let relative = resolved_state.strip_prefix(&resolved_root).map_err(|_| {
        format!(
            "SQLite background state path {} must stay under storage root {}",
            resolved_state.display(),
            resolved_root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn open_sqlite_connection(db_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let conn = Connection::open(db_path).map_err(|err| err.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|err| err.to_string())?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|err| err.to_string())?;
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {SQLITE_TABLE_NAME} (
                payload_key TEXT PRIMARY KEY,
                payload_text TEXT NOT NULL
            )"
        ),
        [],
    )
    .map_err(|err| err.to_string())?;
    Ok(conn)
}

/// Returns true for operations that mutate the persisted store. Used by
/// `handle_background_state_operation` to decide whether to flush the
/// in-memory reaper cleanup as part of this operation's persist step
/// (mutating) versus deferring it to the next mutation (read-only).
fn is_mutating_background_operation(op: &str) -> bool {
    matches!(
        op,
        "apply_mutation" | "arbitrate_session_takeover" | "reserve" | "claim" | "release"
    )
}

pub fn handle_background_state_operation(payload: Value) -> Result<Value, String> {
    let request = serde_json::from_value::<BackgroundStateRequestPayload>(payload)
        .map_err(|err| format!("parse background state request failed: {err}"))?;
    if request.schema_version != BACKGROUND_STATE_REQUEST_SCHEMA_VERSION {
        return Err(format!(
            "unknown background state request schema_version: {}",
            request.schema_version
        ));
    }
    // Cross-process critical section: durable-backed state must serialize
    // load -> mutate -> persist so concurrent writers (codex+cursor+tests)
    // cannot clobber each other. We acquire an advisory lock on a sentinel
    // file keyed by `state_path` for both filesystem and sqlite backends:
    //   - filesystem: serializes the load+rename cycle on the JSON file.
    //   - sqlite: serializes the load+UPDATE cycle on the row keyed by
    //     `state_path`. SQLite's own row-level locking only protects a
    //     single SQL statement, not our higher-level read-modify-write
    //     compound operation; without this sentinel two concurrent
    //     handlers that both load, both reap, both mutate could interleave
    //     and lose updates. The sentinel file is independent of the sqlite
    //     db file, so it does not interfere with SQLite's own locks.
    // Memory backend has no cross-process surface and skips the advisory
    // lock entirely.
    let backend_family = request.backend_family.as_deref().unwrap_or("filesystem");
    let normalized_backend = normalized_backend_family(backend_family);
    let needs_path_lock = matches!(
        normalized_backend.as_str(),
        "filesystem" | "file" | "sqlite" | "sqlite3"
    );
    let _path_lock = if needs_path_lock {
        match request.state_path.as_deref() {
            Some(p) => Some(acquire_runtime_path_lock(Path::new(p))?),
            None => None,
        }
    } else {
        None
    };
    let mut store = BackgroundStateStore::load(&request)?;
    // Mutating operations fold the in-memory reaper cleanup into their
    // persist step. Read-only operations leave the store as-is so reads
    // remain disk-side-effect-free.
    if is_mutating_background_operation(&request.operation) {
        store.flush_reap_if_dirty();
    }
    let operation = request.operation.clone();
    let mut response = json!({
        "schema_version": BACKGROUND_STATE_STORE_SCHEMA_VERSION,
        "authority": BACKGROUND_STATE_STORE_AUTHORITY,
        "operation": operation,
        "state": store.snapshot_payload(),
        "health": store.health_payload(),
    });
    match request.operation.as_str() {
        "snapshot" => {}
        "apply_mutation" => {
            let job_id = request
                .job_id
                .as_deref()
                .ok_or_else(|| "Background state apply_mutation is missing job_id.".to_string())?;
            let mutation = request.mutation.as_ref().ok_or_else(|| {
                "Background state apply_mutation is missing mutation.".to_string()
            })?;
            let (job, persisted_payload_text) = store.apply_mutation(job_id, mutation)?;
            response["job"] = serde_json::to_value(job).map_err(|err| err.to_string())?;
            if let Some(payload_text) = persisted_payload_text {
                response["persisted_payload_text"] = Value::String(payload_text);
            }
            response["state"] = store.snapshot_payload();
            response["health"] = store.health_payload();
        }
        "get" => {
            let job_id = request
                .job_id
                .as_deref()
                .ok_or_else(|| "Background state get is missing job_id.".to_string())?;
            response["job"] = store
                .get(job_id)
                .map(|job| serde_json::to_value(job).map_err(|err| err.to_string()))
                .transpose()?
                .unwrap_or(Value::Null);
        }
        "get_active_job" => {
            let session_id = request.session_id.as_deref().ok_or_else(|| {
                "Background state get_active_job is missing session_id.".to_string()
            })?;
            response["active_job_id"] = store
                .active_job(session_id)
                .map(Value::String)
                .unwrap_or(Value::Null);
        }
        "arbitrate_session_takeover" => {
            let arbitration_operation =
                request.arbitration_operation.as_deref().ok_or_else(|| {
                    "Background state arbitration is missing arbitration_operation.".to_string()
                })?;
            let session_id = request
                .session_id
                .as_deref()
                .ok_or_else(|| "Background state arbitration is missing session_id.".to_string())?;
            let incoming_job_id = request.incoming_job_id.as_deref().ok_or_else(|| {
                "Background state arbitration is missing incoming_job_id.".to_string()
            })?;
            let (takeover, persisted_payload_text) = store.arbitrate_session_takeover(
                arbitration_operation,
                session_id,
                incoming_job_id,
            )?;
            response["takeover"] = serde_json::to_value(takeover).map_err(|err| err.to_string())?;
            if let Some(payload_text) = persisted_payload_text {
                response["persisted_payload_text"] = Value::String(payload_text);
            }
            response["state"] = store.snapshot_payload();
            response["health"] = store.health_payload();
        }
        "reserve" | "claim" | "release" => {
            let session_id = request
                .session_id
                .as_deref()
                .ok_or_else(|| "Background state arbitration is missing session_id.".to_string())?;
            let incoming_job_id = request.incoming_job_id.as_deref().ok_or_else(|| {
                "Background state arbitration is missing incoming_job_id.".to_string()
            })?;
            let (takeover, persisted_payload_text) = store.arbitrate_session_takeover(
                &request.operation,
                session_id,
                incoming_job_id,
            )?;
            response["takeover"] = serde_json::to_value(takeover).map_err(|err| err.to_string())?;
            if let Some(payload_text) = persisted_payload_text {
                response["persisted_payload_text"] = Value::String(payload_text);
            }
            response["state"] = store.snapshot_payload();
            response["health"] = store.health_payload();
        }
        "parallel_group_summary" => {
            let parallel_group_id = request.parallel_group_id.as_deref().ok_or_else(|| {
                "Background state parallel_group_summary is missing parallel_group_id.".to_string()
            })?;
            response["parallel_group_summary"] = store
                .parallel_group_summary(parallel_group_id)
                .map(|summary| serde_json::to_value(summary).map_err(|err| err.to_string()))
                .transpose()?
                .unwrap_or(Value::Null);
        }
        "parallel_group_summaries" => {
            response["parallel_group_summaries"] =
                serde_json::to_value(store.parallel_group_summaries())
                    .map_err(|err| err.to_string())?;
        }
        "health" => {}
        other => {
            return Err(format!(
                "unsupported background state operation: {:?}",
                other
            ));
        }
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_transition_rejects_unknown_prior_status() {
        // Strict-FSM: any unrecognized prior status (legacy data, hand-edits,
        // disk corruption) must NOT silently transition to anything. Previously
        // the wildcard arm returned `true` and let zombie statuses propagate.
        assert!(validate_transition(Some("ghost_status"), "running").is_err());
        assert!(validate_transition(Some("legacy_v0"), "completed").is_err());
        // Known transitions still work.
        assert!(validate_transition(None, "queued").is_ok());
        assert!(validate_transition(Some("running"), "completed").is_ok());
        assert!(validate_transition(Some("interrupted"), "interrupted").is_ok());
        // Known invalid transitions still rejected.
        assert!(validate_transition(Some("completed"), "running").is_err());
    }

    fn make_test_store() -> BackgroundStateStore {
        BackgroundStateStore {
            state_path: PathBuf::from("/tmp/router-rs-reaper-test-state.json"),
            backend_family: "filesystem".to_string(),
            sqlite_db_path: None,
            control_plane: Value::Object(serde_json::Map::new()),
            jobs: HashMap::new(),
            active_sessions: HashMap::new(),
            pending_session_takeovers: HashMap::new(),
            reaped_dirty: false,
        }
    }

    fn make_job(id: &str, status: &str, updated_at: &str) -> BackgroundRunStatus {
        BackgroundRunStatus {
            job_id: id.to_string(),
            session_id: Some(format!("session-{id}")),
            status: status.to_string(),
            updated_at: updated_at.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn reap_stale_active_jobs_marks_old_running_as_interrupted() {
        // Heartbeat ttl = 3600s; a `running` job last seen 2h ago is stale.
        let mut store = make_test_store();
        let now = Utc::now();
        let stale_ts =
            (now - chrono::Duration::seconds(STALE_ACTIVE_HEARTBEAT_TTL_SECS + 1800)).to_rfc3339();
        let fresh_ts = (now - chrono::Duration::seconds(60)).to_rfc3339();
        store
            .jobs
            .insert("stale".to_string(), make_job("stale", "running", &stale_ts));
        store
            .jobs
            .insert("fresh".to_string(), make_job("fresh", "running", &fresh_ts));
        store
            .active_sessions
            .insert("session-stale".to_string(), "stale".to_string());

        let reaped = store.reap_stale_active_jobs(now);
        assert_eq!(reaped, 1);
        let stale_job = store.jobs.get("stale").expect("stale job kept");
        assert_eq!(stale_job.status, "interrupted");
        assert!(stale_job.interrupted_at.is_some());
        assert!(stale_job
            .error
            .as_deref()
            .map(|e| e.contains("heartbeat stale"))
            .unwrap_or(false));
        // session reservation released
        assert!(!store.active_sessions.contains_key("session-stale"));
        // fresh job untouched
        assert_eq!(store.jobs.get("fresh").unwrap().status, "running");
    }

    #[test]
    fn reap_stale_terminal_jobs_drops_old_terminal() {
        // Terminal-TTL = 24h; older terminal jobs should be dropped wholesale.
        let mut store = make_test_store();
        let now = Utc::now();
        let stale_ts =
            (now - chrono::Duration::seconds(STALE_TERMINAL_JOB_TTL_SECS + 60)).to_rfc3339();
        let fresh_ts = (now - chrono::Duration::seconds(3600)).to_rfc3339();
        store.jobs.insert(
            "old-completed".to_string(),
            make_job("old-completed", "completed", &stale_ts),
        );
        store.jobs.insert(
            "recent-completed".to_string(),
            make_job("recent-completed", "completed", &fresh_ts),
        );
        store.jobs.insert(
            "running".to_string(),
            make_job("running", "running", &stale_ts),
        );

        let reaped = store.reap_stale_terminal_jobs(now);
        assert_eq!(reaped, 1);
        assert!(!store.jobs.contains_key("old-completed"));
        assert!(store.jobs.contains_key("recent-completed"));
        // active job (even if old) untouched by terminal reaper
        assert!(store.jobs.contains_key("running"));
    }

    #[test]
    fn reap_ghost_status_jobs_marks_unknown_as_interrupted() {
        // Ghost status (not in active or terminal FSM) must be force-converted
        // to `interrupted` with diagnostic so terminal-TTL eventually drops
        // them and operators can see the corruption.
        let mut store = make_test_store();
        let now = Utc::now();
        let ts = (now - chrono::Duration::seconds(60)).to_rfc3339();
        store.jobs.insert(
            "ghost".to_string(),
            make_job("ghost", "weird_legacy_status", &ts),
        );
        store
            .jobs
            .insert("ok".to_string(), make_job("ok", "running", &ts));
        store
            .active_sessions
            .insert("session-ghost".to_string(), "ghost".to_string());
        store
            .pending_session_takeovers
            .insert("session-x".to_string(), "ghost".to_string());

        let reaped = store.reap_ghost_status_jobs(now);
        assert_eq!(reaped, 1);
        let ghost = store.jobs.get("ghost").expect("ghost job kept after reap");
        assert_eq!(ghost.status, "interrupted");
        assert!(ghost
            .error
            .as_deref()
            .map(|e| e.contains("ghost_status") && e.contains("weird_legacy_status"))
            .unwrap_or(false));
        // active maps released
        assert!(!store.active_sessions.contains_key("session-ghost"));
        assert!(!store.pending_session_takeovers.contains_key("session-x"));
        // healthy job untouched
        assert_eq!(store.jobs.get("ok").unwrap().status, "running");
    }

    #[test]
    fn reap_preserves_recent_jobs() {
        // Sanity: nothing should be reaped when all jobs are within TTL.
        let mut store = make_test_store();
        let now = Utc::now();
        let recent = (now - chrono::Duration::seconds(60)).to_rfc3339();
        store
            .jobs
            .insert("a".to_string(), make_job("a", "running", &recent));
        store
            .jobs
            .insert("b".to_string(), make_job("b", "completed", &recent));
        let active_reaped = store.reap_stale_active_jobs(now);
        let terminal_reaped = store.reap_stale_terminal_jobs(now);
        let ghost_reaped = store.reap_ghost_status_jobs(now);
        assert_eq!(active_reaped, 0);
        assert_eq!(terminal_reaped, 0);
        assert_eq!(ghost_reaped, 0);
        assert_eq!(store.jobs.len(), 2);
    }

    #[test]
    fn is_mutating_background_operation_classifies_correctly() {
        // Read-only ops must be classified as non-mutating so they don't
        // trigger reap-flush disk writes.
        for op in [
            "snapshot",
            "get",
            "get_active_job",
            "parallel_group_summary",
            "parallel_group_summaries",
            "health",
        ] {
            assert!(
                !is_mutating_background_operation(op),
                "{op} should be read-only"
            );
        }
        for op in [
            "apply_mutation",
            "arbitrate_session_takeover",
            "reserve",
            "claim",
            "release",
        ] {
            assert!(
                is_mutating_background_operation(op),
                "{op} should be mutating"
            );
        }
    }

    #[test]
    fn snapshot_drops_dangling_session_mappings() {
        let persisted = json!({
            "version": 2,
            "schema_version": BACKGROUND_STATE_SCHEMA_VERSION,
            "control_plane": null,
            "jobs": [],
            "active_sessions": [{"session_id": "s-1", "job_id": "missing-job"}],
            "pending_session_takeovers": [{"session_id": "s-2", "incoming_job_id": "missing-job"}]
        });
        let response = handle_background_state_operation(json!({
            "schema_version": BACKGROUND_STATE_REQUEST_SCHEMA_VERSION,
            "operation": "snapshot",
            "state_path": "/tmp/router-rs-background-state-test.json",
            "backend_family": "memory",
            "state_payload_text": format!("{}\n", persisted)
        }))
        .expect("snapshot should parse persisted state");
        let state = response
            .get("state")
            .expect("snapshot response state payload");
        assert_eq!(
            state
                .get("active_sessions")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            state
                .get("pending_session_takeovers")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
    }
}
