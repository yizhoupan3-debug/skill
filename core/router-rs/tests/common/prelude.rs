//! Consolidated test prelude — replaces ~360 lines of `#[cfg(test)] pub(crate) use` in lib.rs.
//!
//! Included via `#[path]` from `router-rs/src/lib.rs` so that `crate::` path resolution
//! works transparently for all `#[path = "../tests/…"] mod tests;` imports.

pub use runtime_core::{
    execution_contract, goal_drive, framework_host_targets,
    harness_context_signals, kernel_bootstrap, router_self, task_state,
    mcp_stdio_test_support,
};

pub use framework_kernel::runtime_registry;

// host submodule re-exports (registry-driven: single RegistryDispatcher for all hosts)
pub use runtime_core::hosts::mcp_stdio_harness;

// specific function / constant re-exports
pub use runtime_core::{
    background_state::handle_background_state_operation,
    execution_contract::{
        EXECUTION_AUTHORITY,
        EXECUTION_MODEL_ID_SOURCE, EXECUTION_SCHEMA_VERSION,
        EXECUTION_RESPONSE_SHAPE_DRY_RUN, EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
        build_execution_kernel_contracts_by_mode, build_execution_kernel_metadata_contract,
    },
};

// framework-runtime 原 re-export items — 现直接用原始 crate
pub use fr_utils::types::FrameworkAliasBuildOptions;
pub use framework_extra::alias::build_framework_alias_envelope;
pub use framework_extra::snapshot::build_framework_runtime_snapshot_envelope;
pub use framework_extra::statusline::build_framework_statusline;
pub use framework_extra::evidence::framework_hook_evidence_append;
pub use framework_extra::framework_doctor::run_continuity_audit;
pub use framework_extra::session_artifacts::write_framework_session_artifacts;
pub use fr_exec::live_execute::{
    DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS, EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV,
    LiveExecuteResult, build_live_execute_prompt, build_live_execute_response,
    execute_request, extract_chat_completion_content, live_execute_http_client,
    normalize_chat_completions_endpoint, perform_live_execute_with_sender,
    validate_live_execute_aggregator_base_url,
};
pub use fr_exec::trace_attach::{
    attach_runtime_event_transport, subscribe_attached_runtime_events,
};
pub use fr_exec::trace_stream_io::{
    inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta,
    write_trace_metadata,
};
pub use runtime_core::trace_runtime::sha256_hex;
pub use fr_exec::trace_transport::write_text_payload;
pub use runtime_core::route::{
    ROUTE_AUTHORITY, ROUTE_SNAPSHOT_SCHEMA_VERSION,
    RouteSnapshotEnvelopePayload, build_route_diff_report, build_route_policy,
    build_route_snapshot, load_records_cached_for_stdio, load_records, route_task,
    search_skills,
};
pub use runtime_core::runtime_envelope_ids::{
    BACKGROUND_CONTROL_AUTHORITY, BACKGROUND_CONTROL_SCHEMA_VERSION,
    DEFAULT_MAX_BACKGROUND_JOBS, DEFAULT_MAX_CONCURRENT_SUBAGENTS, RUNTIME_CONTROL_PLANE_AUTHORITY,
    RUNTIME_CONTROL_PLANE_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION, RUNTIME_STORAGE_AUTHORITY,
    RUNTIME_STORAGE_SCHEMA_VERSION, SANDBOX_CONTROL_AUTHORITY,
    SANDBOX_CONTROL_SCHEMA_VERSION, SANDBOX_EVENT_SCHEMA_VERSION,
    TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION, TRACE_METADATA_WRITE_AUTHORITY,
    TRACE_METADATA_WRITE_SCHEMA_VERSION, TRACE_STREAM_INSPECT_SCHEMA_VERSION,
    TRACE_STREAM_IO_AUTHORITY, TRACE_STREAM_REPLAY_SCHEMA_VERSION,
};
pub use runtime_core::runtime_storage::{
    RuntimeStorageRequestPayload, build_checkpoint_control_plane_compiler_payload,
    runtime_storage_operation,
};
pub use runtime_core::stdio_payload_types::{
    ExecuteRequestPayload,
    TraceCompactionDeltaWriteRequestPayload, TraceMetadataWriteRequestPayload,
};
pub use runtime_core::stdio_transport::{
    DEFAULT_ROUTER_STDIO_POOL_SIZE, MAX_ROUTER_STDIO_POOL_SIZE, handle_stdio_json_line,
};
pub use runtime_core::task_state::resolve_task_view;
pub use runtime_core::trace_runtime::{TraceRecordEventRequestPayload, record_trace_event};

// observability/control-plane re-exports (from framework-extra)
pub use framework_extra::orchestration_controller::{
    build_background_control_response, build_runtime_control_plane_payload, build_runtime_metric_record,
    build_runtime_observability_exporter_descriptor,
    build_runtime_observability_health_snapshot,
    build_runtime_observability_metric_catalog_payload,
    runtime_observability_dashboard_schema,
};

pub use fr_exec::sandbox_control::build_sandbox_control_response;
pub use framework_extra::route_manifest_fallback::resolve_runtime_declared_manifest_fallback;
