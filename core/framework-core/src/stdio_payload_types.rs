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

impl ExecuteResponsePayload {
    pub fn new(
        session_id: impl Into<String>,
        user_id: impl Into<String>,
        skill: impl Into<String>,
        live_run: bool,
        content: impl Into<String>,
        usage: ExecuteUsagePayload,
    ) -> Self {
        Self {
            execution_schema_version: "execute-response-v1".into(),
            authority: "framework-runtime".into(),
            session_id: session_id.into(),
            user_id: user_id.into(),
            skill: skill.into(),
            overlay: None,
            live_run,
            content: content.into(),
            usage,
            prompt_preview: None,
            model_id: None,
            metadata: serde_json::Value::Null,
        }
    }
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

impl BackgroundControlEffectPlanPayload {
    pub fn new(next_step: impl Into<String>) -> Self {
        Self {
            next_step: next_step.into(),
            terminal_status: None,
            resolved_status: None,
            finalize_immediately: None,
            cancel_running_task: None,
            next_retry_count: None,
            backoff_seconds: None,
            wait_timeout_seconds: None,
            wait_poll_interval_seconds: None,
            sleep_seconds: None,
        }
    }
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

impl BackgroundControlResponsePayload {
    pub fn new(
        operation: impl Into<String>,
        reason: impl Into<String>,
        effect_plan: BackgroundControlEffectPlanPayload,
    ) -> Self {
        Self {
            schema_version: "background-control-response-v1".into(),
            authority: "framework-runtime".into(),
            operation: operation.into(),
            resolved_parallel_group_id: None,
            lane_ids: None,
            normalized_multitask_strategy: None,
            supported_multitask_strategies: Vec::new(),
            strategy_supported: false,
            accepted: None,
            requires_takeover: None,
            error: None,
            should_retry: None,
            next_retry_count: None,
            backoff_seconds: None,
            terminal_status: None,
            resolved_status: None,
            finalize_immediately: None,
            cancel_running_task: None,
            reason: reason.into(),
            effect_plan,
        }
    }
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

impl TraceStreamReplayResponsePayload {
    pub fn new(
        path: impl Into<String>,
        source_kind: impl Into<String>,
        event_count: usize,
        events: Vec<serde_json::Value>,
    ) -> Self {
        Self {
            schema_version: "trace-stream-replay-response-v1".into(),
            authority: "framework-runtime".into(),
            path: path.into(),
            source_kind: source_kind.into(),
            event_count,
            latest_event_id: None,
            latest_event_kind: None,
            latest_event_timestamp: None,
            latest_cursor: None,
            after_event_id: None,
            window_start_index: 0,
            has_more: false,
            next_cursor: None,
            events,
        }
    }
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

impl TraceStreamInspectResponsePayload {
    pub fn new(
        path: impl Into<String>,
        source_kind: impl Into<String>,
        event_count: usize,
    ) -> Self {
        Self {
            schema_version: "trace-stream-inspect-response-v1".into(),
            authority: "framework-runtime".into(),
            path: path.into(),
            source_kind: source_kind.into(),
            event_count,
            latest_event_id: None,
            latest_event_kind: None,
            latest_event_timestamp: None,
            latest_cursor: None,
            recovery: None,
            reroute_count: 0,
            retry_count: 0,
        }
    }
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

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    // ── Concurrency constant sanity checks ──

    #[test]
    fn pool_size_constants_are_sane() {
        assert!(DEFAULT_ROUTER_STDIO_POOL_SIZE > 0);
        assert!(MAX_ROUTER_STDIO_POOL_SIZE >= DEFAULT_ROUTER_STDIO_POOL_SIZE);
    }

    #[test]
    fn compute_constants_are_sane() {
        assert!(DEFAULT_COMPUTE_THREADS_LOCAL > 0);
        assert!(MAX_COMPUTE_THREADS_LOCAL >= DEFAULT_COMPUTE_THREADS_LOCAL);
    }

    #[test]
    fn background_job_constants_are_sane() {
        assert!(DEFAULT_MAX_BACKGROUND_JOBS_LOCAL > 0);
        assert!(MAX_BACKGROUND_JOBS_LIMIT_LOCAL >= DEFAULT_MAX_BACKGROUND_JOBS_LOCAL);
        assert!(DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS_LOCAL > 0);
    }

    #[test]
    fn subagent_constants_are_sane() {
        assert!(DEFAULT_MAX_CONCURRENT_SUBAGENTS_LOCAL > 0);
        assert!(MAX_CONCURRENT_SUBAGENTS_LIMIT_LOCAL >= DEFAULT_MAX_CONCURRENT_SUBAGENTS_LOCAL);
        assert!(DEFAULT_SUBAGENT_TIMEOUT_SECONDS_LOCAL > 0);
    }

    // ── runtime_concurrency_defaults_payload ──

    #[test]
    fn runtime_concurrency_defaults_payload_populates_fields() {
        let payload = runtime_concurrency_defaults_payload();
        assert_eq!(
            payload.router_stdio.default_pool_size,
            DEFAULT_ROUTER_STDIO_POOL_SIZE
        );
        assert_eq!(
            payload.router_stdio.max_pool_size,
            MAX_ROUTER_STDIO_POOL_SIZE
        );
        assert!(!payload.router_stdio.env_keys.is_empty());
        assert!(!payload.router_stdio.stdio_max_concurrency_arg.is_empty());
        assert!(!payload.router_stdio.request_concurrency_field.is_empty());
        assert!(!payload.router_stdio.scheduling.is_empty());
        assert!(!payload.router_stdio.backpressure.is_empty());
    }

    #[test]
    fn runtime_concurrency_defaults_compute_section() {
        let payload = runtime_concurrency_defaults_payload();
        assert_eq!(
            payload.compute.default_threads,
            DEFAULT_COMPUTE_THREADS_LOCAL
        );
        assert_eq!(payload.compute.max_threads, MAX_COMPUTE_THREADS_LOCAL);
        assert!(!payload.compute.env_keys.is_empty());
        assert!(!payload.compute.cli_arg.is_empty());
        assert!(!payload.compute.scheduling.is_empty());
    }

    #[test]
    fn runtime_concurrency_defaults_background_and_subagent() {
        let payload = runtime_concurrency_defaults_payload();
        assert_eq!(
            payload.max_background_jobs,
            DEFAULT_MAX_BACKGROUND_JOBS_LOCAL
        );
        assert_eq!(
            payload.max_background_jobs_limit,
            MAX_BACKGROUND_JOBS_LIMIT_LOCAL
        );
        assert_eq!(
            payload.background_job_timeout_seconds,
            DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS_LOCAL
        );
        assert_eq!(
            payload.max_concurrent_subagents,
            DEFAULT_MAX_CONCURRENT_SUBAGENTS_LOCAL
        );
        assert_eq!(
            payload.max_concurrent_subagents_limit,
            MAX_CONCURRENT_SUBAGENTS_LIMIT_LOCAL
        );
        assert_eq!(
            payload.subagent_timeout_seconds,
            DEFAULT_SUBAGENT_TIMEOUT_SECONDS_LOCAL
        );
    }

    #[test]
    fn runtime_concurrency_defaults_serializes() {
        let payload = runtime_concurrency_defaults_payload();
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.is_object());
        assert!(json.get("router_stdio").is_some());
        assert!(json.get("compute").is_some());
        assert!(json.get("max_background_jobs").is_some());
        assert!(json.get("subagent_timeout_seconds").is_some());
    }

    #[test]
    fn router_stdio_pool_has_three_env_keys() {
        let payload = runtime_concurrency_defaults_payload();
        assert_eq!(payload.router_stdio.env_keys.len(), 3);
    }

    #[test]
    fn compute_has_two_env_keys() {
        let payload = runtime_concurrency_defaults_payload();
        assert_eq!(payload.compute.env_keys.len(), 2);
    }

    // ── ExecuteRequestPayload round-trip ──

    #[test]
    fn execute_request_payload_round_trip() {
        let payload = ExecuteRequestPayload {
            schema_version: "v1".into(),
            task: "do stuff".into(),
            session_id: "s1".into(),
            user_id: "u1".into(),
            selected_skill: "test".into(),
            overlay_skill: None,
            layer: "main".into(),
            route_engine: None,
            diagnostic_route_mode: None,
            reasons: vec![],
            prompt_preview: None,
            dry_run: false,
            trace_event_count: 0,
            trace_output_path: None,
            default_output_tokens: 4096,
            model_id: "claude-sonnet".into(),
            aggregator_base_url: "http://localhost".into(),
            aggregator_api_key: "key".into(),
        };
        let json_str = serde_json::to_string(&payload).unwrap();
        let deserialized: ExecuteRequestPayload = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.schema_version, "v1");
        assert_eq!(deserialized.task, "do stuff");
        assert_eq!(deserialized.default_output_tokens, 4096);
        assert!(!deserialized.dry_run);
        assert!(deserialized.overlay_skill.is_none());
    }

    // ── ExecuteUsagePayload round-trip ──

    #[test]
    fn execute_usage_payload_round_trip() {
        let usage = ExecuteUsagePayload {
            input_tokens: 100,
            output_tokens: 200,
            total_tokens: 300,
            mode: "normal".into(),
        };
        let json = serde_json::to_value(&usage).unwrap();
        assert_eq!(json["input_tokens"], 100);
        assert_eq!(json["total_tokens"], 300);
        let back: ExecuteUsagePayload = serde_json::from_value(json).unwrap();
        assert_eq!(back.mode, "normal");
    }

    // ── BackgroundControlRequestPayload round-trip ──

    #[test]
    fn background_control_request_payload_round_trip() {
        let payload = BackgroundControlRequestPayload {
            schema_version: "v1".into(),
            operation: "checkpoint".into(),
            multitask_strategy: Some("parallel".into()),
            current_status: None,
            task_active: None,
            task_done: None,
            active_job_count: None,
            capacity_limit: None,
            attempt: None,
            retry_count: None,
            max_attempts: None,
            backoff_base_seconds: None,
            backoff_multiplier: None,
            max_backoff_seconds: None,
            requested_parallel_group_id: None,
            request_parallel_group_ids: None,
            request_lane_ids: None,
            lane_id_prefix: None,
            batch_size: None,
        };
        let json_str = serde_json::to_string(&payload).unwrap();
        let back: BackgroundControlRequestPayload = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.operation, "checkpoint");
        assert_eq!(back.multitask_strategy.as_deref(), Some("parallel"));
    }

    // ── TraceMetadataWriteRequestPayload with defaults ──

    #[test]
    fn trace_metadata_write_request_defaults() {
        let payload: TraceMetadataWriteRequestPayload =
            serde_json::from_str(r#"{"output_path":"/tmp/out","task":"t","owner":"o","gate":"g","verification_status":"PASS"}"#).unwrap();
        assert_eq!(payload.output_path, "/tmp/out");
        assert_eq!(payload.task, "t");
        // Default values from serde
        assert!(payload.write_outputs); // default_true()
        assert!(payload.mirror_paths.is_empty());
        assert!(payload.matched_skills.is_empty());
        assert!(payload.artifact_paths.is_empty());
    }
}
