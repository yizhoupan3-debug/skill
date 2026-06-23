//! framework-runtime: backward-compatible facade over fr-utils(L1), fr-contracts(L2), fr-exec(L3).
//!
//! All pub modules re-export from the appropriate sub-crate.
//! Downstream code using `framework_runtime::*` continues to work unchanged.

#![recursion_limit = "256"]

// hooks re-exported from L5 crate (framework-runtime-hooks).
pub use framework_runtime_hooks as hooks;

// ── L1 re-exports (fr-utils) ──
pub mod constants { pub use fr_utils::constants::*; }
pub mod env_flags { pub use fr_utils::env_flags::*; }
pub mod io_utils { pub use fr_utils::io_utils::*; }
pub mod json_io { pub use fr_utils::json_io::*; }
pub mod json_value { pub use fr_utils::json_value::*; }
pub mod stdio_op_registry { pub use fr_utils::stdio_op_registry::*; }
pub mod types { pub use fr_utils::types::*; }
pub mod util { pub use fr_utils::util::*; }

// ── L2 re-exports (fr-contracts) ──
pub mod closeout_enforcement { pub use fr_contracts::closeout_enforcement::*; }
pub mod execution_contract { pub use fr_contracts::execution_contract::*; }
pub mod pre_tool_use_guard { pub use fr_contracts::pre_tool_use_guard::*; }

// ── L3 re-exports (fr-exec) ──
pub mod evolution_observer { pub use fr_exec::evolution_observer::*; }
pub mod live_execute { pub use fr_exec::live_execute::*; }
pub mod router_env_flags { pub use fr_exec::router_env_flags::*; }
pub mod runtime_view { pub use fr_exec::runtime_view::*; }
pub mod sandbox_control { pub use fr_exec::sandbox_control::*; }
pub mod trace_attach { pub use fr_exec::trace_attach::*; }
pub mod trace_stream_io { pub use fr_exec::trace_stream_io::*; }
pub mod trace_transport { pub use fr_exec::trace_transport::*; }

// ── Convenience re-exports (preserved from original) ──
pub use constants::{
    FRAMEWORK_ALIAS_SCHEMA_VERSION, FRAMEWORK_CONTRACT_SUMMARY_SCHEMA_VERSION,
    FRAMEWORK_RUNTIME_AUTHORITY, FRAMEWORK_RUNTIME_SNAPSHOT_SCHEMA_VERSION,
    FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY, FRAMEWORK_SESSION_ARTIFACT_WRITE_SCHEMA_VERSION,
};
pub use closeout_enforcement::closeout_enforcement_contract;
pub use execution_contract::{
    build_execution_contract_bundle, decode_execution_response_value,
    normalize_execution_kernel_contract_value,
    normalize_execution_kernel_metadata_contract_value,
    validate_execution_kernel_steady_state_metadata_value,
};
pub use framework_kernel::repo_roots::{
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
    inspect_trace_stream, replay_trace_stream, write_trace_compaction_delta,
    write_trace_metadata,
};
pub use trace_runtime::sha256_hex;
pub use types::FrameworkAliasBuildOptions;
