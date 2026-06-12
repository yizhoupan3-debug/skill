#![recursion_limit = "256"]

// ── Re-exports from runtime-core (B3 migration: single source of truth) ──
// Only `cli` and `browser_mcp` are consumed externally (by router-rs-cli).
// The rest are pub(crate) — used only by this crate's tests (included via #[path]).
pub use browser_mcp;
pub use runtime_core::cli;

#[cfg(test)]
pub(crate) use runtime_core::autopilot_goal;
#[cfg(test)]
pub(crate) use runtime_core::background_state;
#[cfg(test)]
pub(crate) use runtime_core::closeout_enforcement;
#[cfg(test)]
pub(crate) use runtime_core::eval_route;
#[cfg(test)]
pub(crate) use runtime_core::execution_contract;
#[cfg(test)]
pub(crate) use runtime_core::formal_toolchain;
#[cfg(test)]
pub(crate) use runtime_core::framework_host_targets;
#[cfg(test)]
pub(crate) use runtime_core::framework_maint;
#[cfg(test)]
pub(crate) use runtime_core::framework_profile;
#[cfg(test)]
pub(crate) use runtime_core::framework_runtime;
#[cfg(test)]
pub(crate) use runtime_core::framework_skills;
#[cfg(test)]
pub(crate) use runtime_core::harness_context_signals;
#[cfg(test)]
pub(crate) use runtime_core::harness_contract;
#[cfg(test)]
pub(crate) use runtime_core::harness_operator_nudges;
#[cfg(test)]
pub(crate) use runtime_core::hook_event_routing;
#[cfg(test)]
pub(crate) use runtime_core::hook_observation_rules;
#[cfg(test)]
pub(crate) use runtime_core::hook_outbound_protect;
#[cfg(test)]
pub(crate) use runtime_core::hook_timing;
#[cfg(test)]
pub(crate) use runtime_core::host_entrypoint_sync;
#[cfg(test)]
pub(crate) use runtime_core::host_integration;
#[cfg(test)]
pub(crate) use runtime_core::hosts;
#[cfg(test)]
pub(crate) use runtime_core::kernel_bootstrap;
#[cfg(test)]
pub(crate) use runtime_core::mcp_pre_guard;
#[cfg(test)]
pub(crate) use runtime_core::paper_adversarial_hook;
#[cfg(test)]
pub(crate) use runtime_core::paper_prose_hook;
#[cfg(test)]
pub(crate) use runtime_core::review_gate;
#[cfg(test)]
pub(crate) use runtime_core::rfv_loop;
#[cfg(test)]
pub(crate) use runtime_core::route;
#[cfg(test)]
pub(crate) use runtime_core::router_env_flags;
#[cfg(test)]
pub(crate) use runtime_core::router_rs_observation;
#[cfg(test)]
pub(crate) use runtime_core::router_self;
#[cfg(test)]
pub(crate) use runtime_core::runtime_envelope_ids;
#[cfg(test)]
pub(crate) use runtime_core::runtime_registry;
#[cfg(test)]
pub(crate) use runtime_core::runtime_storage;
#[cfg(test)]
pub(crate) use runtime_core::schema_drift;
#[cfg(test)]
pub(crate) use runtime_core::session_call_tracker;
#[cfg(test)]
pub(crate) use runtime_core::session_supervisor;
#[cfg(test)]
pub(crate) use runtime_core::skill_repo;
#[cfg(test)]
pub(crate) use runtime_core::stdio_payload_types;
#[cfg(test)]
pub(crate) use runtime_core::stdio_transport;
#[cfg(test)]
pub(crate) use runtime_core::task_command;
#[cfg(test)]
pub(crate) use runtime_core::telemetry_emit;
#[cfg(test)]
pub(crate) use runtime_core::trace_runtime;
#[cfg(test)]
pub(crate) use runtime_core::web_fetch_guard;

// ── host submodule re-exports (for `crate::X` path compat) ──
#[cfg(test)]
pub(crate) use runtime_core::hosts::mcp_stdio_harness;
#[cfg(test)]
pub(crate) use runtime_core::hosts::claude_code_hooks;
#[cfg(test)]
pub(crate) use runtime_core::hosts::codex_hooks;
#[cfg(test)]
pub(crate) use runtime_core::hosts::cursor_hooks;

// ── core-state re-exports (for pub(crate) compat) ──
#[cfg(test)]
pub(crate) use runtime_core::task_state;
#[cfg(test)]
pub(crate) use runtime_core::task_state_aggregate;
#[cfg(test)]
pub(crate) use runtime_core::task_ledger;
#[cfg(test)]
pub(crate) use runtime_core::step_ledger;

#[cfg(test)]
pub use runtime_core::hook_posttool_normalize;

// ── Additional re-exports for test compatibility ──
#[cfg(test)]
pub use runtime_core::runtime_envelope_ids::{
    BACKGROUND_CONTROL_AUTHORITY, BACKGROUND_CONTROL_SCHEMA_VERSION,
    RUNTIME_CONTROL_PLANE_AUTHORITY, SANDBOX_CONTROL_SCHEMA_VERSION,
    SANDBOX_EVENT_SCHEMA_VERSION,
};
#[cfg(test)]
pub use runtime_core::stdio_payload_types::{
    BackgroundControlRequestPayload, SandboxControlRequestPayload,
};
#[cfg(test)]
pub use runtime_core::execution_contract::{
    EXECUTION_RESPONSE_SHAPE_DRY_RUN, EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::live_execute::{
    build_live_execute_prompt, validate_live_execute_aggregator_base_url,
    EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::write_framework_session_artifacts;
#[cfg(test)]
pub use runtime_core::framework_runtime::build_background_control_response;
#[cfg(test)]
pub use runtime_core::framework_runtime::build_sandbox_control_response;
#[cfg(test)]
pub use runtime_core::framework_runtime::build_framework_runtime_snapshot_envelope;
#[cfg(test)]
pub use runtime_core::background_state::handle_background_state_operation;
#[cfg(test)]
pub use runtime_core::route::records::load_records;
#[cfg(test)]
pub use runtime_core::stdio_transport::handle_stdio_json_line;
#[cfg(test)]
pub use runtime_core::runtime_storage::runtime_storage_operation;

// ── Trace stream re-exports ──
#[cfg(test)]
pub use runtime_core::runtime_envelope_ids::{
    DEFAULT_MAX_BACKGROUND_JOBS, DEFAULT_MAX_CONCURRENT_SUBAGENTS,
    MAX_CONCURRENT_SUBAGENTS_LIMIT, RUNTIME_CONTROL_PLANE_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION,
    RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION,
    RUNTIME_STORAGE_AUTHORITY, RUNTIME_STORAGE_SCHEMA_VERSION,
    SANDBOX_CONTROL_AUTHORITY, TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION,
    TRACE_METADATA_WRITE_AUTHORITY, TRACE_METADATA_WRITE_SCHEMA_VERSION,
    TRACE_STREAM_IO_AUTHORITY, TRACE_STREAM_INSPECT_SCHEMA_VERSION,
    TRACE_STREAM_REPLAY_SCHEMA_VERSION,
};
#[cfg(test)]
pub use runtime_core::stdio_payload_types::{
    ExecuteRequestPayload, TraceCompactionDeltaWriteRequestPayload,
    TraceMetadataWriteRequestPayload,
    TraceStreamInspectRequestPayload,
    TraceStreamReplayRequestPayload,
};
#[cfg(test)]
pub use runtime_core::stdio_transport::{
    DEFAULT_ROUTER_STDIO_POOL_SIZE, MAX_ROUTER_STDIO_POOL_SIZE,
};
#[cfg(test)]
pub use runtime_core::execution_contract::{
    EXECUTION_AUTHORITY, EXECUTION_MODEL_ID_SOURCE, EXECUTION_SCHEMA_VERSION,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::live_execute::{
    LiveExecuteResult, DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS,
};
#[cfg(test)]
pub use runtime_core::route::{
    ROUTE_AUTHORITY, ROUTE_SNAPSHOT_SCHEMA_VERSION,
};

// ── Remaining re-exports for integration_test_prelude parity ──
#[cfg(test)]
pub use runtime_core::framework_runtime::{
    FrameworkAliasBuildOptions, build_framework_alias_envelope,
    build_framework_statusline, framework_hook_evidence_append,
    run_continuity_audit,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::trace_attach::{
    attach_runtime_event_transport,
    subscribe_attached_runtime_events,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::trace_stream_io::{
    inspect_trace_stream, replay_trace_stream, sha256_hex,
    write_trace_compaction_delta, write_trace_metadata,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::json_payload::{
    optional_non_empty_string, required_non_empty_string,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::live_execute::{
    build_live_execute_response, execute_request,
    extract_chat_completion_content, live_execute_http_client,
    normalize_chat_completions_endpoint, perform_live_execute_with_sender,
};
// orchestration_controller is pub(crate) — use parent module re-exports
#[cfg(test)]
pub use runtime_core::framework_runtime::{
    build_runtime_control_plane_payload, build_runtime_integrator_payload,
    build_runtime_metric_record, build_runtime_observability_exporter_descriptor,
    build_runtime_observability_health_snapshot,
    build_runtime_observability_metric_catalog_payload,
    runtime_observability_dashboard_schema,
};
#[cfg(test)]
pub use runtime_core::task_state::resolve_task_view;
#[cfg(test)]
pub use runtime_core::trace_runtime::{record_trace_event, TraceRecordEventRequestPayload};
#[cfg(test)]
pub use runtime_core::runtime_storage::{
    RuntimeStorageRequestPayload, build_checkpoint_control_plane_compiler_payload,
};
#[cfg(test)]
pub use runtime_core::cli::runtime_ops::write_text_payload;

// ── CLI args re-exports ──
#[cfg(test)]
pub use runtime_core::cli::args::{Cli, CodexSubcommand, HostCommand, RouterCommand};

// ── Route re-exports ──
#[cfg(test)]
pub use runtime_core::route::{
    build_route_diff_report, build_route_policy, build_route_snapshot,
    load_records_cached_for_stdio, route_task, search_skills,
    RouteSnapshotEnvelopePayload,
};
#[cfg(test)]
pub use runtime_core::execution_contract::{
    build_execution_kernel_contracts_by_mode, build_execution_kernel_metadata_contract,
};
#[cfg(test)]
pub use runtime_core::framework_runtime::route_manifest_fallback::resolve_runtime_declared_manifest_fallback;

// ── Test helpers (cfg(test)) ──
#[cfg(test)]
pub use runtime_core::router_self::resolve_router_rs_test_bin;
#[cfg(test)]
pub use runtime_core::hosts::mcp_stdio_harness::{
    get_snapshot_ttl_for_test, get_task_view_ttl_for_test, init_tracker_for_test,
    read_mcp_message_test_helper, tool_closeout_record_write_for_test,
    tool_goal_state_manage_test_helper, tool_rfv_loop_manage_test_helper,
};
#[cfg(test)]
pub use runtime_core::harness_operator_nudges::harness_nudges_env_test_lock;
#[cfg(test)]
pub use runtime_core::hosts::cursor_hooks::set_test_review_gate_disable_override;
#[cfg(test)]
pub use runtime_core::hosts::claude_code_hooks::dispatch_claude_hook_payload_for_test;
#[cfg(test)]
pub use runtime_core::session_call_tracker::test_lock_roundtrip;

// ── re-exports from core-policy ──
#[cfg(test)]
pub use core_policy::hook_common;
#[cfg(test)]
pub use core_policy::hook_policy;
#[cfg(test)]
pub use core_policy::lane_normalize;
#[cfg(test)]
pub use core_policy::review_gate_engine;
#[cfg(test)]
pub use core_policy::review_output_lint;
#[cfg(test)]
pub use core_policy::review_routing_signals;

#[cfg(test)]
pub use routing_engine;

// ── test support re-exports from runtime-core ──
#[cfg(test)]
pub use runtime_core::mcp_stdio_test_support;
#[cfg(test)]
pub use runtime_core::integration_test_prelude;

// ── router-rs-only modules (NOT in runtime-core) ──
#[cfg(feature = "codegraph")]
pub use runtime_core::codegraph_mcp;
#[cfg(feature = "codegraph")]
pub(crate) mod mcp_common;

// ── proxy modules (thin re-exports kept in router-rs, used only by tests) ──
#[cfg(test)]
mod path_guard {
    pub use runtime_core::path_guard::*;
}
#[cfg(test)]
mod atomic_write {
    pub use runtime_core::atomic_write::*;
}
#[cfg(test)]
mod task_write_lock {
    pub use runtime_core::task_write_lock::*;
}

// ── hook_status (inline, test-only) ──
#[cfg(test)]
pub(crate) mod hook_status {
    pub const REVIEW_GATE_CHECKING: &str = "Loading Codex turn context";
    pub const REVIEW_GATE_UPDATING: &str = "Recording Codex tool evidence";
    pub const REVIEW_GATE_ENFORCING: &str = "Enforcing Codex review gate";
}

// ── crate-level re-exports ──
#[cfg(test)]
pub(crate) use cli::route_task_with_manifest_fallback;

// ── cli re-exports (from framework_runtime public API, not cli cfg(test) items) ──
#[cfg(test)]
pub(crate) use framework_runtime::{
    classify_stdio_op, dispatch_stdio_json_request, StdioOpDomain,
};
// is_*_stdio_op helpers: local wrappers since the originals are cfg(test) in runtime-core
#[cfg(test)]
pub(crate) fn is_framework_stdio_op(op: &str) -> bool {
    classify_stdio_op(op) == Some(StdioOpDomain::Framework)
}
#[cfg(test)]
pub(crate) fn is_routing_stdio_op(op: &str) -> bool {
    classify_stdio_op(op) == Some(StdioOpDomain::Routing)
}
#[cfg(test)]
pub(crate) fn is_runtime_stdio_op(op: &str) -> bool {
    classify_stdio_op(op) == Some(StdioOpDomain::Runtime)
}
#[cfg(test)]
pub(crate) fn is_trace_stdio_op(op: &str) -> bool {
    classify_stdio_op(op) == Some(StdioOpDomain::Trace)
}

#[cfg(test)]
use execution_contract::{
    EXECUTION_KERNEL_AUTHORITY, EXECUTION_KERNEL_FALLBACK_POLICY, EXECUTION_KERNEL_KIND,
    EXECUTION_METADATA_CONTRACT_SCHEMA_VERSION, EXECUTION_METADATA_SCHEMA_VERSION,
    EXECUTION_PROMPT_PREVIEW_OWNER,
};
#[cfg(test)]
use framework_runtime::FRAMEWORK_ALIAS_SCHEMA_VERSION;
#[cfg(test)]
use route::ROUTE_REPORT_SCHEMA_VERSION;

#[cfg(test)]
#[ctor::ctor]
fn router_rs_test_kernel_bootstrap() {
    crate::kernel_bootstrap::ensure_kernel_bootstrap();
}

#[cfg(test)]
mod test_env_sync;

#[cfg(test)]
static TEST_KERNEL_BOOTSTRAP: std::sync::LazyLock<()> =
    std::sync::LazyLock::new(crate::kernel_bootstrap::ensure_kernel_bootstrap);

#[cfg(test)]
pub(crate) fn touch_test_kernel_bootstrap() {
    let _ = &*TEST_KERNEL_BOOTSTRAP;
}

#[cfg(test)]
#[path = "../tests/main_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/mcp_stdio_harness_tests.rs"]
mod mcp_stdio_harness_tests;

#[cfg(test)]
#[path = "../tests/smoke_workflow_contract_tests.rs"]
mod smoke_workflow_contract_tests;

#[cfg(test)]
#[path = "../tests/smoke_cross_host_closeout_tests.rs"]
mod smoke_cross_host_closeout_tests;

#[cfg(test)]
#[path = "../tests/smoke_sandbox_shutdown_tests.rs"]
mod smoke_sandbox_shutdown_tests;

#[cfg(test)]
#[path = "../tests/smoke_isolation_contract_tests.rs"]
mod smoke_isolation_contract_tests;

#[cfg(test)]
#[path = "../tests/smoke_p0_hook_policy_tests.rs"]
mod smoke_p0_hook_policy_tests;

#[cfg(test)]
#[path = "../tests/smoke_p0_atomic_write_tests.rs"]
mod smoke_p0_atomic_write_tests;

#[cfg(test)]
#[path = "../tests/smoke_p0_rfv_state_tests.rs"]
mod smoke_p0_rfv_state_tests;

#[cfg(test)]
#[path = "../tests/smoke_p0_task_pointers_tests.rs"]
mod smoke_p0_task_pointers_tests;

#[cfg(test)]
#[path = "../tests/smoke_cli_backward_compat_tests.rs"]
mod smoke_cli_backward_compat_tests;

#[cfg(test)]
#[path = "../tests/smoke_codegraph_semantic_dispatch_tests.rs"]
mod smoke_codegraph_semantic_dispatch_tests;

#[cfg(all(test, feature = "codegraph"))]
#[path = "../tests/smoke_codegraph_e2e_minimal_tests.rs"]
mod smoke_codegraph_e2e_minimal_tests;

#[cfg(all(test, feature = "codegraph"))]
#[path = "../tests/smoke_codegraph_five_host_install_projection_tests.rs"]
mod smoke_codegraph_five_host_install_projection_tests;

#[cfg(all(test, feature = "codegraph"))]
#[path = "../tests/smoke_codegraph_five_host_stdio_e2e_tests.rs"]
mod smoke_codegraph_five_host_stdio_e2e_tests;

#[cfg(test)]
#[path = "../tests/smoke_p0_trace_runtime_compaction_tests.rs"]
mod smoke_p0_trace_runtime_compaction_tests;

#[cfg(test)]
#[path = "../tests/smoke_p0_router_self_tests.rs"]
mod smoke_p0_router_self_tests;

#[cfg(test)]
#[path = "../tests/smoke_workspace_dag_compliance_tests.rs"]
mod smoke_workspace_dag_compliance_tests;

#[cfg(test)]
#[path = "../tests/hook_contract/mod.rs"]
mod hook_contract_matrix;

// ── browser dispatch registration ──
/// Register browser-mcp's dispatch function with runtime-core's hook.
/// Must be called once before any `router-rs browser` CLI command.
pub fn init_browser_mcp_dispatch() {
    browser_mcp::register_browser_dispatch();
}
