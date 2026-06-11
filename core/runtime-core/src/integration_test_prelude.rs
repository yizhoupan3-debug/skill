//! Re-exports for `main_tests` (crate root stays intentionally thin after CLI split).
//!
//! Each symbol below is actually referenced by at least one test in `main_tests.rs`
//! (see `rg -c '\\b<symbol>\\b' src/main_tests.rs`). Drop the global `allow(unused_imports)`
//! so a future drift (re-export of a truly dead symbol) surfaces as a compile warning
//! rather than rotting silently behind the wildcard pull-in.

pub(crate) use std::collections::HashSet;
pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::{Mutex, OnceLock};
pub(crate) use std::time::Duration;

pub(crate) use crate::background_state::handle_background_state_operation;
pub(crate) use crate::cli::common::resolve_runtime_declared_manifest_fallback;
pub(crate) use crate::framework_runtime::orchestration_controller::{
    build_background_control_response, build_runtime_control_plane_payload,
    build_runtime_metric_record, build_runtime_observability_exporter_descriptor,
    build_runtime_observability_health_snapshot, build_runtime_observability_metric_catalog_payload,
    runtime_observability_dashboard_schema,
};
pub(crate) use crate::framework_runtime::live_execute::{
    build_live_execute_prompt, build_live_execute_response, execute_request,
    extract_chat_completion_content, live_execute_http_client, normalize_chat_completions_endpoint,
    perform_live_execute_with_sender, validate_live_execute_aggregator_base_url,
    DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS, EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV,
};
pub(crate) use crate::framework_runtime::sandbox_control::build_sandbox_control_response;
pub(crate) use crate::cli::runtime_ops::write_text_payload;
pub(crate) use crate::framework_runtime::{
    attach_runtime_event_transport, inspect_trace_stream, replay_trace_stream, sha256_hex,
    subscribe_attached_runtime_events, write_trace_compaction_delta, write_trace_metadata,
};
pub(crate) use crate::execution_contract::{
    build_execution_kernel_contracts_by_mode, build_execution_kernel_metadata_contract,
    EXECUTION_AUTHORITY, EXECUTION_MODEL_ID_SOURCE, EXECUTION_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY, EXECUTION_SCHEMA_VERSION,
};
pub(crate) use crate::framework_runtime::{
    build_framework_alias_envelope, build_framework_runtime_snapshot_envelope,
    build_framework_runtime_snapshot_envelope_with_level,
    build_framework_statusline, framework_hook_evidence_append, run_continuity_audit,
    write_framework_session_artifacts, FrameworkAliasBuildOptions,
};
pub(crate) use crate::route::{
    build_route_diff_report, build_route_policy, build_route_snapshot, load_records,
    load_records_cached_for_stdio, load_records_from_manifest, route_task, search_skills,
    RouteSnapshotEnvelopePayload, ROUTE_AUTHORITY, ROUTE_SNAPSHOT_SCHEMA_VERSION,
};
pub(crate) use crate::runtime_envelope_ids::{
    BACKGROUND_CONTROL_AUTHORITY, BACKGROUND_CONTROL_SCHEMA_VERSION, DEFAULT_MAX_BACKGROUND_JOBS,
    DEFAULT_MAX_CONCURRENT_SUBAGENTS, MAX_CONCURRENT_SUBAGENTS_LIMIT,
    RUNTIME_CONTROL_PLANE_AUTHORITY, RUNTIME_CONTROL_PLANE_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION, RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION, RUNTIME_STORAGE_AUTHORITY,
    RUNTIME_STORAGE_SCHEMA_VERSION, SANDBOX_CONTROL_AUTHORITY, SANDBOX_CONTROL_SCHEMA_VERSION,
    SANDBOX_EVENT_SCHEMA_VERSION, TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION,
    TRACE_METADATA_WRITE_AUTHORITY, TRACE_METADATA_WRITE_SCHEMA_VERSION,
    TRACE_STREAM_INSPECT_SCHEMA_VERSION, TRACE_STREAM_IO_AUTHORITY,
    TRACE_STREAM_REPLAY_SCHEMA_VERSION,
};
pub(crate) use crate::runtime_storage::{
    build_checkpoint_control_plane_compiler_payload, runtime_storage_operation,
    RuntimeStorageRequestPayload,
};
pub(crate) use crate::stdio_transport::{
    handle_stdio_json_line, DEFAULT_ROUTER_STDIO_POOL_SIZE, MAX_ROUTER_STDIO_POOL_SIZE,
};
pub(crate) use crate::task_state::resolve_task_view;
pub(crate) use crate::trace_runtime::{record_trace_event, TraceRecordEventRequestPayload};
