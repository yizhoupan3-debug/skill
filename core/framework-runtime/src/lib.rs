//! framework-runtime: extracted from `runtime-core` to its own crate.
//!
//! Framework runtime core loop, trace/transport, execution contracts,
//! closeout enforcement, RFV loop, and lifecycle management.
//!
//! Depends on leaf crates (core-state, core-policy, framework-kernel, routing-engine,
//! runtime-storage, trace-runtime) and registers RuntimeCoreHooks for functionality
//! that still lives in `runtime-core`.

#![recursion_limit = "256"]

pub mod hooks;

// ── extracted from runtime-core/src/framework_runtime/ ──
pub mod codex_hooks_duplicate;
pub mod constants;
pub mod evolution_observer;
pub mod io_utils;
pub mod json_io;
pub mod json_value;
pub mod live_execute;
pub mod repo_roots;
pub mod runtime_view;
pub mod router_env_flags;
pub mod sandbox_control;
pub mod pre_tool_use_guard;
pub mod stdio_op_registry;
pub mod trace_attach;
pub mod trace_stream_io;
pub mod trace_transport;
pub mod types;

// ── extracted from runtime-core/src/contracts/ ──
pub mod closeout_enforcement;
pub mod execution_contract;

// ── extracted from runtime-core/src/ ──

// ── Convenience re-exports ──
pub use constants::{
    FRAMEWORK_ALIAS_SCHEMA_VERSION, FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION,
    FRAMEWORK_RUNTIME_AUTHORITY, FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION,
    FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY, FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION,
};
pub use codex_hooks_duplicate::eprint_codex_hooks_duplicate_warnings;
pub use closeout_enforcement::closeout_enforcement_contract;
pub use execution_contract::{
    build_execution_contract_bundle, decode_execution_response_value,
    normalize_execution_kernel_contract_value,
    normalize_execution_kernel_metadata_contract_value,
    validate_execution_kernel_steady_state_metadata_value,
};
pub use repo_roots::{
    framework_root_from_executable_path, is_framework_root, resolve_repo_root_arg,
};
pub use pre_tool_use_guard::{
    PRE_TOOL_USE_GUARD_SCHEMA_VERSION, PRE_TOOL_USE_GUARD_STDIO_OP, PreToolUseGuardRequest,
    PreToolUseGuardResponse, PreToolUseGuardVerdict, evaluate_pre_tool_use_guard,
    evaluate_pre_tool_use_guard_value, host_requires_strict_pre_tool_fallback,
    pre_tool_use_guard_contract,
};
pub use sandbox_control::build_sandbox_control_response;
pub use stdio_op_registry::{StdioOpDomain, classify_stdio_op};
pub use trace_attach::{
    attach_runtime_event_transport, cleanup_attached_runtime_event_transport,
    subscribe_attached_runtime_events,
};
pub use trace_stream_io::{
    inspect_trace_stream, replay_trace_stream, sha256_hex, write_trace_compaction_delta,
    write_trace_metadata,
};
pub use types::FrameworkAliasBuildOptions;
