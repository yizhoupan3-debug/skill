//! Stdio / live-execute JSON 载荷类型（Roadmap v5 P7：自 `cli/args.rs` 下沉，减轻 B7 行数）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecuteRequestPayload {
    pub(crate) schema_version: String,
    pub(crate) task: String,
    pub(crate) session_id: String,
    pub(crate) user_id: String,
    pub(crate) selected_skill: String,
    pub(crate) overlay_skill: Option<String>,
    pub(crate) layer: String,
    pub(crate) route_engine: Option<String>,
    pub(crate) diagnostic_route_mode: Option<String>,
    pub(crate) reasons: Vec<String>,
    pub(crate) prompt_preview: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) trace_event_count: usize,
    pub(crate) trace_output_path: Option<String>,
    pub(crate) default_output_tokens: usize,
    #[serde(default)]
    pub(crate) research_mode: Option<String>,
    #[serde(default)]
    pub(crate) execution_protocol: Option<String>,
    #[serde(default)]
    pub(crate) verification_required: Option<bool>,
    #[serde(default)]
    pub(crate) evidence_required: Option<bool>,
    pub(crate) model_id: String,
    pub(crate) aggregator_base_url: String,
    pub(crate) aggregator_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecuteUsagePayload {
    pub(crate) input_tokens: usize,
    pub(crate) output_tokens: usize,
    pub(crate) total_tokens: usize,
    pub(crate) mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecuteResponsePayload {
    pub(crate) execution_schema_version: String,
    pub(crate) authority: String,
    pub(crate) session_id: String,
    pub(crate) user_id: String,
    pub(crate) skill: String,
    pub(crate) overlay: Option<String>,
    pub(crate) live_run: bool,
    pub(crate) content: String,
    pub(crate) usage: ExecuteUsagePayload,
    pub(crate) prompt_preview: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackgroundControlRequestPayload {
    pub(crate) schema_version: String,
    pub(crate) operation: String,
    pub(crate) multitask_strategy: Option<String>,
    pub(crate) current_status: Option<String>,
    pub(crate) task_active: Option<bool>,
    pub(crate) task_done: Option<bool>,
    pub(crate) active_job_count: Option<usize>,
    pub(crate) capacity_limit: Option<usize>,
    pub(crate) attempt: Option<usize>,
    pub(crate) retry_count: Option<usize>,
    pub(crate) max_attempts: Option<usize>,
    pub(crate) backoff_base_seconds: Option<f64>,
    pub(crate) backoff_multiplier: Option<f64>,
    pub(crate) max_backoff_seconds: Option<f64>,
    pub(crate) requested_parallel_group_id: Option<String>,
    pub(crate) request_parallel_group_ids: Option<Vec<Option<String>>>,
    pub(crate) request_lane_ids: Option<Vec<Option<String>>>,
    pub(crate) lane_id_prefix: Option<String>,
    pub(crate) batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SandboxControlRequestPayload {
    pub(crate) schema_version: String,
    pub(crate) operation: String,
    pub(crate) sandbox_id: Option<String>,
    pub(crate) profile_id: Option<String>,
    pub(crate) current_state: Option<String>,
    pub(crate) next_state: Option<String>,
    pub(crate) cleanup_failed: Option<bool>,
    pub(crate) tool_category: Option<String>,
    pub(crate) capability_categories: Option<Vec<String>>,
    pub(crate) dedicated_profile: Option<bool>,
    pub(crate) budget_cpu: Option<f64>,
    pub(crate) budget_memory: Option<i64>,
    pub(crate) budget_wall_clock: Option<f64>,
    pub(crate) budget_output_size: Option<i64>,
    pub(crate) probe_cpu: Option<f64>,
    pub(crate) probe_memory: Option<i64>,
    pub(crate) probe_wall_clock: Option<f64>,
    pub(crate) probe_output_size: Option<i64>,
    pub(crate) error_kind: Option<String>,
    pub(crate) event_log_path: Option<String>,
    pub(crate) trace_event: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SandboxControlResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) operation: String,
    pub(crate) current_state: Option<String>,
    pub(crate) next_state: Option<String>,
    pub(crate) allowed: bool,
    pub(crate) resolved_state: Option<String>,
    pub(crate) reason: String,
    pub(crate) error: Option<String>,
    pub(crate) failure_reason: Option<String>,
    pub(crate) budget_violation: Option<String>,
    pub(crate) cleanup_required: Option<bool>,
    pub(crate) quarantined: Option<bool>,
    pub(crate) effective_capabilities: Option<Vec<String>>,
    pub(crate) sandbox_id: Option<String>,
    pub(crate) profile_id: Option<String>,
    pub(crate) event_schema_version: Option<String>,
    pub(crate) event_log_path: Option<String>,
    pub(crate) event_written: bool,
    pub(crate) event_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackgroundControlEffectPlanPayload {
    pub(crate) next_step: String,
    pub(crate) terminal_status: Option<String>,
    pub(crate) resolved_status: Option<String>,
    pub(crate) finalize_immediately: Option<bool>,
    pub(crate) cancel_running_task: Option<bool>,
    pub(crate) next_retry_count: Option<usize>,
    pub(crate) backoff_seconds: Option<f64>,
    pub(crate) wait_timeout_seconds: Option<f64>,
    pub(crate) wait_poll_interval_seconds: Option<f64>,
    pub(crate) sleep_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackgroundControlResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) operation: String,
    pub(crate) resolved_parallel_group_id: Option<String>,
    pub(crate) lane_ids: Option<Vec<String>>,
    pub(crate) normalized_multitask_strategy: Option<String>,
    pub(crate) supported_multitask_strategies: Vec<String>,
    pub(crate) strategy_supported: bool,
    pub(crate) accepted: Option<bool>,
    pub(crate) requires_takeover: Option<bool>,
    pub(crate) error: Option<String>,
    pub(crate) should_retry: Option<bool>,
    pub(crate) next_retry_count: Option<usize>,
    pub(crate) backoff_seconds: Option<f64>,
    pub(crate) terminal_status: Option<String>,
    pub(crate) resolved_status: Option<String>,
    pub(crate) finalize_immediately: Option<bool>,
    pub(crate) cancel_running_task: Option<bool>,
    pub(crate) reason: String,
    pub(crate) effect_plan: BackgroundControlEffectPlanPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayRequestPayload {
    pub(crate) path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
    pub(crate) after_event_id: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamInspectRequestPayload {
    pub(crate) path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayCursorPayload {
    pub(crate) event_id: Option<String>,
    pub(crate) event_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) source_kind: String,
    pub(crate) event_count: usize,
    pub(crate) latest_event_id: Option<String>,
    pub(crate) latest_event_kind: Option<String>,
    pub(crate) latest_event_timestamp: Option<String>,
    pub(crate) latest_cursor: Option<Value>,
    pub(crate) after_event_id: Option<String>,
    pub(crate) window_start_index: usize,
    pub(crate) has_more: bool,
    pub(crate) next_cursor: Option<TraceStreamReplayCursorPayload>,
    pub(crate) events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamInspectResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) source_kind: String,
    pub(crate) event_count: usize,
    pub(crate) latest_event_id: Option<String>,
    pub(crate) latest_event_kind: Option<String>,
    pub(crate) latest_event_timestamp: Option<String>,
    pub(crate) latest_cursor: Option<Value>,
    pub(crate) recovery: Option<Value>,
    pub(crate) reroute_count: usize,
    pub(crate) retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceCompactionDeltaWriteRequestPayload {
    pub(crate) path: String,
    pub(crate) delta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceCompactionDeltaWriteResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceMetadataWriteRequestPayload {
    pub(crate) output_path: String,
    #[serde(default)]
    pub(crate) mirror_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub(crate) write_outputs: bool,
    pub(crate) task: String,
    #[serde(default)]
    pub(crate) matched_skills: Vec<String>,
    pub(crate) owner: String,
    pub(crate) gate: String,
    pub(crate) overlay: Option<String>,
    pub(crate) reroute_count: Option<usize>,
    pub(crate) retry_count: Option<usize>,
    #[serde(default)]
    pub(crate) artifact_paths: Vec<String>,
    pub(crate) verification_status: String,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) event_stream_path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
    pub(crate) framework_version: Option<String>,
    pub(crate) metadata_schema_version: Option<String>,
    pub(crate) routing_runtime_version: Option<u64>,
    pub(crate) runtime_path: Option<String>,
    pub(crate) ts: Option<String>,
    pub(crate) trace_event_schema_version: Option<String>,
    pub(crate) trace_event_sink_schema_version: Option<String>,
    pub(crate) parallel_group: Option<Value>,
    pub(crate) supervisor_projection: Option<Value>,
    pub(crate) control_plane: Option<Value>,
    pub(crate) stream: Option<Value>,
    pub(crate) events: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceMetadataWriteResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) output_path: String,
    pub(crate) mirror_paths: Vec<String>,
    pub(crate) bytes_written: usize,
    pub(crate) routing_runtime_version: u64,
    pub(crate) payload_text: String,
}
