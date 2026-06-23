//! Consolidated test prelude — replaces ~360 lines of `#[cfg(test)] pub(crate) use` in lib.rs.
//!
//! Included via `#[path]` from `router-rs/src/lib.rs` so that `crate::` path resolution
//! works transparently for all `#[path = "../tests/…"] mod tests;` imports.

pub use runtime_core::{
    goal_drive, closeout_enforcement, execution_contract, framework_host_targets,
    framework_runtime, harness_context_signals,
    harness_operator_nudges, hook_event_routing, hosts, kernel_bootstrap, router_self, runtime_envelope_ids, session_call_tracker, session_supervisor, stdio_payload_types, trace_runtime, task_state,
    mcp_stdio_test_support,
};

pub use framework_kernel::runtime_registry;

// host submodule re-exports (registry-driven: single RegistryDispatcher for all hosts)
pub use runtime_core::hosts::{
    mcp_stdio_harness,
    host_extensions::dispatch::RegistryDispatcher,
};

// specific function / constant re-exports
pub use runtime_core::{
    background_state::handle_background_state_operation,
    execution_contract::{
        EXECUTION_AUTHORITY,
        EXECUTION_MODEL_ID_SOURCE, EXECUTION_SCHEMA_VERSION,
        EXECUTION_RESPONSE_SHAPE_DRY_RUN, EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
        build_execution_kernel_contracts_by_mode, build_execution_kernel_metadata_contract,
    },
    framework_runtime::{
        FrameworkAliasBuildOptions,
        build_background_control_response, build_framework_alias_envelope,
        build_framework_runtime_snapshot_envelope, build_framework_statusline,
        build_sandbox_control_response, framework_hook_evidence_append,
        run_continuity_audit, write_framework_session_artifacts,
    },
    framework_runtime::live_execute::{
        DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS, EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV,
        LiveExecuteResult, build_live_execute_prompt, build_live_execute_response,
        execute_request, extract_chat_completion_content, live_execute_http_client,
        normalize_chat_completions_endpoint, perform_live_execute_with_sender,
        validate_live_execute_aggregator_base_url,
    },
    framework_runtime::resolve_runtime_declared_manifest_fallback,
    framework_runtime::trace_attach::{
        attach_runtime_event_transport, subscribe_attached_runtime_events,
    },
    framework_runtime::trace_stream_io::{
        inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta,
        write_trace_metadata,
    },
    framework_runtime::sha256_hex,
    framework_runtime::trace_transport::write_text_payload,
    route::{
        ROUTE_AUTHORITY, ROUTE_SNAPSHOT_SCHEMA_VERSION,
        RouteSnapshotEnvelopePayload, build_route_diff_report, build_route_policy,
        build_route_snapshot, load_records_cached_for_stdio, load_records, route_task,
        search_skills,
    },
    runtime_envelope_ids::{
        BACKGROUND_CONTROL_AUTHORITY, BACKGROUND_CONTROL_SCHEMA_VERSION,
        DEFAULT_MAX_BACKGROUND_JOBS, DEFAULT_MAX_CONCURRENT_SUBAGENTS,
        MAX_CONCURRENT_SUBAGENTS_LIMIT, RUNTIME_CONTROL_PLANE_AUTHORITY,
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
    },
    runtime_storage::{
        RuntimeStorageRequestPayload, build_checkpoint_control_plane_compiler_payload,
        runtime_storage_operation,
    },
    stdio_payload_types::{
        ExecuteRequestPayload,
        TraceCompactionDeltaWriteRequestPayload, TraceMetadataWriteRequestPayload,
    },
    stdio_transport::{
        DEFAULT_ROUTER_STDIO_POOL_SIZE, MAX_ROUTER_STDIO_POOL_SIZE, handle_stdio_json_line,
    },
    task_state::resolve_task_view,
    trace_runtime::{TraceRecordEventRequestPayload, record_trace_event},
    framework_runtime::{
        build_runtime_control_plane_payload, build_runtime_metric_record,
        build_runtime_observability_exporter_descriptor,
        build_runtime_observability_health_snapshot,
        build_runtime_observability_metric_catalog_payload,
        runtime_observability_dashboard_schema,
    },
};

pub use core_policy::hook_common;

