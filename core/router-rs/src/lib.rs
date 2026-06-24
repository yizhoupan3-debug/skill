#![recursion_limit = "256"]

// ── Re-exports from runtime-core (B3 migration: single source of truth) ──
// `route` is consumed externally (by router-rs-cli).
// `cli` is now defined locally in router-rs (moved from runtime-core per ADR §10.3).
// All test-only re-exports live in `tests/common/prelude.rs` included via #[path].
pub mod cli;
pub use runtime_core::route;

#[cfg(test)]
#[path = "../tests/common/prelude.rs"]
mod test_prelude;
#[cfg(test)]
pub(crate) use test_prelude::*;

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
    
}

// ── hook_status (inline, test-only) ──
#[cfg(test)]
pub(crate) mod hook_status {
    pub const REVIEW_GATE_CHECKING: &str = "Loading Codex turn context";
    pub const REVIEW_GATE_UPDATING: &str = "Recording Codex tool evidence";
    pub const REVIEW_GATE_ENFORCING: &str = "Enforcing Codex review gate";
}

// ── crate-level re-exports ──

// ── cli re-exports (from framework_runtime public API, not cli cfg(test) items) ──
#[cfg(test)]
pub(crate) use framework_runtime::{StdioOpDomain, classify_stdio_op, dispatch_stdio_json_request};
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
#[path = "../tests/smoke_p0_qg_state_tests.rs"]
mod smoke_p0_qg_state_tests;

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
