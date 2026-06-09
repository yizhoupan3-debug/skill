#![recursion_limit = "256"]
#![allow(unused_variables, unused_mut)]

mod autopilot_goal;
pub mod goal_drive {
    pub use core_state::state_manager::{
        framework_autopilot_goal, framework_goal_drive, GOAL_STATE_FILENAME,
        read_active_task_id, read_focus_task_id, read_primary_task_id,
    };
}
// Re-exports from runtime-core (migrated modules)
pub use runtime_core::background_state;
pub use runtime_core::runtime_envelope_ids;
pub use runtime_core::runtime_storage;
pub mod session_supervisor;
pub use runtime_core::trace_runtime;

mod browser_mcp;
#[cfg(feature = "codegraph")]
pub mod codegraph_mcp;
#[cfg(feature = "codegraph")]
pub mod mcp_common;
pub mod hosts;
pub(crate) use hosts::mcp_stdio_harness;
pub(crate) use hosts::claude_code_hooks;
pub(crate) use hosts::codex_hooks;
pub(crate) use hosts::cursor_hooks;
pub mod cli;
pub mod types;
mod closeout_enforcement;
mod hook_event_routing;
mod eval_route;
mod execution_contract;
mod kernel_bootstrap;
mod framework_host_targets;
mod framework_maint;
mod framework_profile;
pub mod framework_runtime;
mod framework_skills;
mod harness_context_signals;
mod harness_contract;
mod harness_operator_nudges;
pub use routing_engine;
pub use core_policy::hook_common;
mod hook_outbound_protect;
mod hook_observation_rules;
pub use core_policy::hook_policy;
#[path = "utils/hook_posttool_normalize.rs"]
mod hook_posttool_normalize;
mod hook_timing;
mod host_entrypoint_sync;
mod host_integration;
pub mod framework_host_integration {
    //! Install/sync CLI surface (`framework host-integration`); historical module name `host_integration`.
    pub use crate::host_integration::*;
}
pub use core_policy::lane_normalize;
mod paper_adversarial_hook;
mod paper_prose_hook;
mod runtime_registry;
mod review_gate;
pub use core_policy::review_gate_engine;
pub use core_policy::review_output_lint;
pub use core_policy::review_routing_signals;
mod rfv_loop;
pub mod route;
mod router_env_flags;
mod router_rs_observation;
pub mod router_self;
mod schema_drift;
mod ship_readiness;
mod session_call_tracker;
mod skill_repo;
pub mod stdio_payload_types;
mod stdio_transport;
mod task_command;
mod telemetry_emit;
pub(crate) use core_state::task_state;
pub(crate) use core_state::task_state_aggregate;
pub(crate) use core_state::task_ledger;
pub(crate) use core_state::step_ledger;

mod path_guard {
    pub use core_state::utils::path_guard::*;
}
mod atomic_write {
    pub use core_state::utils::atomic_write::*;
}
mod task_write_lock {
    pub use core_state::utils::task_write_lock::*;
}

mod formal_toolchain {
    pub use core_math::ascii_lower_contains_formal_toolchain_tokens;
}

pub mod web_fetch_guard;

#[cfg(test)]
mod mcp_stdio_test_support;

#[cfg(test)]
mod integration_test_prelude;

pub mod hook_status {
    pub const REVIEW_GATE_CHECKING: &str = "Loading Codex turn context";
    pub const REVIEW_GATE_UPDATING: &str = "Recording Codex tool evidence";
    pub const REVIEW_GATE_ENFORCING: &str = "Enforcing Codex review gate";
}

pub(crate) use cli::route_task_with_manifest_fallback;

#[cfg(test)]
pub(crate) use cli::{
    classify_stdio_op, dispatch_stdio_json_request, is_framework_stdio_op, is_routing_stdio_op,
    is_runtime_stdio_op, is_trace_stdio_op, StdioOpDomain,
};

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

#[cfg(not(test))]
pub(crate) fn touch_test_kernel_bootstrap() {}

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
