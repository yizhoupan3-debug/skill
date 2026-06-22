//! Stdio / live-execute JSON 载荷类型。

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequestPayload {
    pub schema_version: String,
    pub task: String,
    pub session_id: String,
    pub user_id: String,
    pub selected_skill: String,
    pub overlay_skill: Option<String>,
    pub layer: String,
    pub route_engine: Option<String>,
    pub diagnostic_route_mode: Option<String>,
    pub reasons: Vec<String>,
    pub prompt_preview: Option<String>,
    pub dry_run: bool,
    pub trace_event_count: usize,
    pub trace_output_path: Option<String>,
    pub default_output_tokens: usize,
    #[serde(default)]
    pub research_mode: Option<String>,
    #[serde(default)]
    pub execution_protocol: Option<String>,
    #[serde(default)]
    pub verification_required: Option<bool>,
    #[serde(default)]
    pub evidence_required: Option<bool>,
    pub model_id: String,
    pub aggregator_base_url: String,
    pub aggregator_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteUsagePayload {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponsePayload {
    pub execution_schema_version: String,
    pub authority: String,
    pub session_id: String,
    pub user_id: String,
    pub skill: String,
    pub overlay: Option<String>,
    pub live_run: bool,
    pub content: String,
    pub usage: ExecuteUsagePayload,
    pub prompt_preview: Option<String>,
    pub model_id: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundControlRequestPayload {
    pub schema_version: String,
    pub operation: String,
    pub multitask_strategy: Option<String>,
    pub current_status: Option<String>,
    pub task_active: Option<bool>,
    pub task_done: Option<bool>,
    pub active_job_count: Option<usize>,
    pub capacity_limit: Option<usize>,
    pub attempt: Option<usize>,
    pub retry_count: Option<usize>,
    pub max_attempts: Option<usize>,
    pub backoff_base_seconds: Option<f64>,
    pub backoff_multiplier: Option<f64>,
    pub max_backoff_seconds: Option<f64>,
    pub requested_parallel_group_id: Option<String>,
    pub request_parallel_group_ids: Option<Vec<Option<String>>>,
    pub request_lane_ids: Option<Vec<Option<String>>>,
    pub lane_id_prefix: Option<String>,
    pub batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxControlRequestPayload {
    pub schema_version: String,
    pub operation: String,
    pub sandbox_id: Option<String>,
    pub profile_id: Option<String>,
    pub current_state: Option<String>,
    pub next_state: Option<String>,
    pub cleanup_failed: Option<bool>,
    pub tool_category: Option<String>,
    pub capability_categories: Option<Vec<String>>,
    pub dedicated_profile: Option<bool>,
    pub budget_cpu: Option<f64>,
    pub budget_memory: Option<i64>,
    pub budget_wall_clock: Option<f64>,
    pub budget_output_size: Option<i64>,
    pub probe_cpu: Option<f64>,
    pub probe_memory: Option<i64>,
    pub probe_wall_clock: Option<f64>,
    pub probe_output_size: Option<i64>,
    pub error_kind: Option<String>,
    pub event_log_path: Option<String>,
    pub trace_event: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxControlResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub operation: String,
    pub current_state: Option<String>,
    pub next_state: Option<String>,
    pub allowed: bool,
    pub resolved_state: Option<String>,
    pub reason: String,
    pub error: Option<String>,
    pub failure_reason: Option<String>,
    pub budget_violation: Option<String>,
    pub cleanup_required: Option<bool>,
    pub quarantined: Option<bool>,
    pub effective_capabilities: Option<Vec<String>>,
    pub sandbox_id: Option<String>,
    pub profile_id: Option<String>,
    pub event_schema_version: Option<String>,
    pub event_log_path: Option<String>,
    pub event_written: bool,
    pub event_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundControlEffectPlanPayload {
    pub next_step: String,
    pub terminal_status: Option<String>,
    pub resolved_status: Option<String>,
    pub finalize_immediately: Option<bool>,
    pub cancel_running_task: Option<bool>,
    pub next_retry_count: Option<usize>,
    pub backoff_seconds: Option<f64>,
    pub wait_timeout_seconds: Option<f64>,
    pub wait_poll_interval_seconds: Option<f64>,
    pub sleep_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundControlResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub operation: String,
    pub resolved_parallel_group_id: Option<String>,
    pub lane_ids: Option<Vec<String>>,
    pub normalized_multitask_strategy: Option<String>,
    pub supported_multitask_strategies: Vec<String>,
    pub strategy_supported: bool,
    pub accepted: Option<bool>,
    pub requires_takeover: Option<bool>,
    pub error: Option<String>,
    pub should_retry: Option<bool>,
    pub next_retry_count: Option<usize>,
    pub backoff_seconds: Option<f64>,
    pub terminal_status: Option<String>,
    pub resolved_status: Option<String>,
    pub finalize_immediately: Option<bool>,
    pub cancel_running_task: Option<bool>,
    pub reason: String,
    pub effect_plan: BackgroundControlEffectPlanPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStreamReplayRequestPayload {
    pub path: Option<String>,
    pub event_stream_text: Option<String>,
    pub compaction_manifest_path: Option<String>,
    pub compaction_manifest_text: Option<String>,
    pub compaction_state_text: Option<String>,
    pub compaction_artifact_index_text: Option<String>,
    pub compaction_delta_text: Option<String>,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub stream_scope_fields: Option<Vec<String>>,
    pub after_event_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStreamInspectRequestPayload {
    pub path: Option<String>,
    pub event_stream_text: Option<String>,
    pub compaction_manifest_path: Option<String>,
    pub compaction_manifest_text: Option<String>,
    pub compaction_state_text: Option<String>,
    pub compaction_artifact_index_text: Option<String>,
    pub compaction_delta_text: Option<String>,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub stream_scope_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStreamReplayCursorPayload {
    pub event_id: Option<String>,
    pub event_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStreamReplayResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub path: String,
    pub source_kind: String,
    pub event_count: usize,
    pub latest_event_id: Option<String>,
    pub latest_event_kind: Option<String>,
    pub latest_event_timestamp: Option<String>,
    pub latest_cursor: Option<Value>,
    pub after_event_id: Option<String>,
    pub window_start_index: usize,
    pub has_more: bool,
    pub next_cursor: Option<TraceStreamReplayCursorPayload>,
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStreamInspectResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub path: String,
    pub source_kind: String,
    pub event_count: usize,
    pub latest_event_id: Option<String>,
    pub latest_event_kind: Option<String>,
    pub latest_event_timestamp: Option<String>,
    pub latest_cursor: Option<Value>,
    pub recovery: Option<Value>,
    pub reroute_count: usize,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCompactionDeltaWriteRequestPayload {
    pub path: String,
    pub delta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCompactionDeltaWriteResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub path: String,
    pub bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceMetadataWriteRequestPayload {
    pub output_path: String,
    #[serde(default)]
    pub mirror_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub write_outputs: bool,
    pub task: String,
    #[serde(default)]
    pub matched_skills: Vec<String>,
    pub owner: String,
    pub gate: String,
    pub overlay: Option<String>,
    pub reroute_count: Option<usize>,
    pub retry_count: Option<usize>,
    #[serde(default)]
    pub artifact_paths: Vec<String>,
    pub verification_status: String,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub event_stream_path: Option<String>,
    pub event_stream_text: Option<String>,
    pub compaction_manifest_path: Option<String>,
    pub compaction_manifest_text: Option<String>,
    pub compaction_state_text: Option<String>,
    pub compaction_artifact_index_text: Option<String>,
    pub compaction_delta_text: Option<String>,
    pub stream_scope_fields: Option<Vec<String>>,
    pub framework_version: Option<String>,
    pub metadata_schema_version: Option<String>,
    pub routing_runtime_version: Option<u64>,
    pub runtime_path: Option<String>,
    pub ts: Option<String>,
    pub trace_event_schema_version: Option<String>,
    pub trace_event_sink_schema_version: Option<String>,
    pub parallel_group: Option<Value>,
    pub supervisor_projection: Option<Value>,
    pub control_plane: Option<Value>,
    pub stream: Option<Value>,
    pub events: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceMetadataWriteResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub output_path: String,
    pub mirror_paths: Vec<String>,
    pub bytes_written: usize,
    pub routing_runtime_version: u64,
    pub payload_text: String,
}

// ── Concurrency defaults (moved from runtime-core/infrastructure/stdio_transport.rs) ──

pub const DEFAULT_ROUTER_STDIO_POOL_SIZE: usize = 8;
pub const MAX_ROUTER_STDIO_POOL_SIZE: usize = 32;

/// Defaults mirrored from `rt_storage::runtime_envelope_ids`.
pub const DEFAULT_COMPUTE_THREADS_LOCAL: usize = 8;
pub const MAX_COMPUTE_THREADS_LOCAL: usize = 32;
pub const DEFAULT_MAX_BACKGROUND_JOBS_LOCAL: usize = 4;
pub const MAX_BACKGROUND_JOBS_LIMIT_LOCAL: usize = 16;
pub const DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS_LOCAL: u64 = 1800;
pub const DEFAULT_MAX_CONCURRENT_SUBAGENTS_LOCAL: usize = 4;
pub const MAX_CONCURRENT_SUBAGENTS_LIMIT_LOCAL: usize = 8;
pub const DEFAULT_SUBAGENT_TIMEOUT_SECONDS_LOCAL: u64 = 600;

#[derive(Debug, Clone, Serialize)]
pub struct StdioRouterConcurrencyDescriptor {
    pub default_pool_size: usize,
    pub max_pool_size: usize,
    pub env_keys: Vec<&'static str>,
    pub stdio_max_concurrency_arg: &'static str,
    pub request_concurrency_field: &'static str,
    pub scheduling: &'static str,
    pub backpressure: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputeConcurrencyDescriptor {
    pub default_threads: usize,
    pub max_threads: usize,
    pub env_keys: Vec<&'static str>,
    pub cli_arg: &'static str,
    pub scheduling: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeConcurrencyDefaultsPayload {
    pub router_stdio: StdioRouterConcurrencyDescriptor,
    pub compute: ComputeConcurrencyDescriptor,
    pub max_background_jobs: usize,
    pub max_background_jobs_limit: usize,
    pub background_job_timeout_seconds: u64,
    pub max_concurrent_subagents: usize,
    pub max_concurrent_subagents_limit: usize,
    pub subagent_timeout_seconds: u64,
}

/// Build a payload describing the framework's concurrency defaults and env-var keys.
pub fn runtime_concurrency_defaults_payload() -> RuntimeConcurrencyDefaultsPayload {
    RuntimeConcurrencyDefaultsPayload {
        router_stdio: StdioRouterConcurrencyDescriptor {
            default_pool_size: DEFAULT_ROUTER_STDIO_POOL_SIZE,
            max_pool_size: MAX_ROUTER_STDIO_POOL_SIZE,
            env_keys: vec![
                "ROUTER_RS_STDIO_POOL_SIZE",
                "BROWSER_MCP_ROUTER_STDIO_POOL_SIZE",
                "CODEX_ROUTER_STDIO_POOL_SIZE",
            ],
            stdio_max_concurrency_arg: "--stdio-max-concurrency",
            request_concurrency_field: "concurrency",
            scheduling: "bounded FIFO with completion-order response emission",
            backpressure: "reader stops admitting new work while in-flight requests reach the limit",
        },
        compute: ComputeConcurrencyDescriptor {
            default_threads: DEFAULT_COMPUTE_THREADS_LOCAL,
            max_threads: MAX_COMPUTE_THREADS_LOCAL,
            env_keys: vec!["ROUTER_RS_COMPUTE_THREADS", "RAYON_NUM_THREADS"],
            cli_arg: "--compute-threads",
            scheduling: "bounded Rayon work-stealing for CPU record scans and batch eval",
        },
        max_background_jobs: DEFAULT_MAX_BACKGROUND_JOBS_LOCAL,
        max_background_jobs_limit: MAX_BACKGROUND_JOBS_LIMIT_LOCAL,
        background_job_timeout_seconds: DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS_LOCAL,
        max_concurrent_subagents: DEFAULT_MAX_CONCURRENT_SUBAGENTS_LOCAL,
        max_concurrent_subagents_limit: MAX_CONCURRENT_SUBAGENTS_LIMIT_LOCAL,
        subagent_timeout_seconds: DEFAULT_SUBAGENT_TIMEOUT_SECONDS_LOCAL,
    }
}
