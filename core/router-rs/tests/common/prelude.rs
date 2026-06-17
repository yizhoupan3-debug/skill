//! Consolidated test prelude — replaces ~360 lines of `#[cfg(test)] pub(crate) use` in lib.rs.
//!
//! Included via `#[path]` from `router-rs/src/lib.rs` so that `crate::` path resolution
//! works transparently for all `#[path = "../tests/…"] mod tests;` imports.

pub use runtime_core::{
    goal_drive, background_state, closeout_enforcement, eval_route, execution_contract,
    formal_toolchain, framework_host_targets, framework_maint, framework_profile,
    framework_runtime, framework_skills, harness_context_signals, harness_contract,
    harness_operator_nudges, hook_event_routing, hook_observation_rules, hook_outbound_protect,
    hook_posttool_normalize, hook_timing, host_entrypoint_sync, host_integration, hosts,
    integration_test_prelude, kernel_bootstrap, mcp_pre_guard, mcp_stdio_test_support,
    paper_adversarial_hook, paper_prose_hook, review_gate, rfv_loop, router_env_flags,
    router_rs_observation, router_self, runtime_envelope_ids, runtime_registry, runtime_storage,
    schema_drift, session_call_tracker, session_supervisor, skill_repo, stdio_payload_types,
    stdio_transport, task_command, telemetry_emit, trace_runtime, web_fetch_guard, step_ledger,
    task_ledger, task_state, task_state_aggregate,
};

// host submodule re-exports
pub use runtime_core::hosts::{
    claude_code_hooks, codex_hooks, cursor_hooks, mcp_stdio_harness, opencode_hooks,
};

// specific function / constant re-exports
pub use runtime_core::{
    background_state::handle_background_state_operation,
    cli::runtime_ops::write_text_payload,
    execution_contract::{
        EXECUTION_AUTHORITY, EXECUTION_KERNEL_FALLBACK_POLICY, EXECUTION_KERNEL_KIND,
        EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION, EXECUTION_METADATA_SCHEMA_VERSION,
        EXECUTION_MODEL_ID_SOURCE, EXECUTION_PROMPT_PREVIEW_OWNER, EXECUTION_SCHEMA_VERSION,
        EXECUTION_RESPONSE_SHAPE_DRY_RUN, EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
        build_execution_kernel_contracts_by_mode, build_execution_kernel_metadata_contract,
    },
    framework_runtime::{
        FRAMEWORK_ALIAS_SCHEMA_VERSION, FrameworkAliasBuildOptions,
        build_background_control_response, build_framework_alias_envelope,
        build_framework_runtime_snapshot_envelope, build_framework_statusline,
        build_sandbox_control_response, framework_hook_evidence_append,
        run_continuity_audit, write_framework_session_artifacts,
    },
    framework_runtime::json_value::{
        optional_non_empty_string, required_non_empty_string,
    },
    framework_runtime::live_execute::{
        DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS, EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV,
        LiveExecuteResult, build_live_execute_prompt, build_live_execute_response,
        execute_request, extract_chat_completion_content, live_execute_http_client,
        normalize_chat_completions_endpoint, perform_live_execute_with_sender,
        validate_live_execute_aggregator_base_url,
    },
    framework_runtime::route_manifest_fallback::resolve_runtime_declared_manifest_fallback,
    framework_runtime::trace_attach::{
        attach_runtime_event_transport, subscribe_attached_runtime_events,
    },
    framework_runtime::trace_stream_io::{
        inspect_trace_stream, replay_trace_stream, sha256_hex, write_trace_compaction_delta,
        write_trace_metadata,
    },
    harness_operator_nudges::harness_nudges_env_test_lock,
    hosts::claude_code_hooks::dispatch_claude_hook_payload_for_test,
    hosts::cursor_hooks::set_test_review_gate_disable_override,
    hosts::mcp_stdio_harness::{
        get_snapshot_ttl_for_test, get_task_view_ttl_for_test, init_tracker_for_test,
        read_mcp_message_test_helper, tool_closeout_record_write_for_test,
        tool_goal_state_manage_test_helper, tool_rfv_loop_manage_test_helper,
    },
    hosts::opencode_hooks::dispatch_opencode_hook_event,
    route::{
        ROUTE_AUTHORITY, ROUTE_REPORT_SCHEMA_VERSION, ROUTE_SNAPSHOT_SCHEMA_VERSION,
        RouteSnapshotEnvelopePayload, build_route_diff_report, build_route_policy,
        build_route_snapshot, load_records_cached_for_stdio, load_records, route_task,
        search_skills,
    },
    router_self::resolve_router_rs_test_bin,
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
    session_call_tracker::test_lock_roundtrip,
    stdio_payload_types::{
        BackgroundControlRequestPayload, ExecuteRequestPayload, SandboxControlRequestPayload,
        TraceCompactionDeltaWriteRequestPayload, TraceMetadataWriteRequestPayload,
        TraceStreamInspectRequestPayload, TraceStreamReplayRequestPayload,
    },
    stdio_transport::{
        DEFAULT_ROUTER_STDIO_POOL_SIZE, MAX_ROUTER_STDIO_POOL_SIZE, handle_stdio_json_line,
    },
    task_state::resolve_task_view,
    trace_runtime::{TraceRecordEventRequestPayload, record_trace_event},
    cli::args::{Cli, CodexSubcommand, HostCommand, RouterCommand},
    framework_runtime::{
        StdioOpDomain, build_runtime_control_plane_payload,
        build_runtime_integrator_payload, build_runtime_metric_record,
        build_runtime_observability_exporter_descriptor,
        build_runtime_observability_health_snapshot,
        build_runtime_observability_metric_catalog_payload,
        classify_stdio_op, dispatch_stdio_json_request,
        runtime_observability_dashboard_schema,
    },
};

pub use core_policy::{
    hook_common, hook_policy, lane_normalize, review_gate_engine, review_output_lint,
    review_routing_signals,
};

pub use routing_engine;
