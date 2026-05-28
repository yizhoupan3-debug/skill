#![recursion_limit = "256"]
#![allow(unused_variables, unused_mut)]

mod autopilot_goal;
pub mod goal_drive {
    pub use antigravity_core::state_manager::{
        framework_autopilot_goal, framework_goal_drive, GOAL_STATE_FILENAME,
        read_active_task_id, read_focus_task_id, read_primary_task_id,
    };
}
mod background_state;
mod browser_mcp;
pub mod hosts;
pub(crate) use hosts::claude_desktop_hooks;
pub(crate) use hosts::claude_hooks;
pub(crate) use hosts::antigravity_cli_hooks;
pub(crate) use hosts::codex_hooks;
pub(crate) use hosts::cursor_hooks;
pub mod cli;
mod closeout_enforcement;
mod eval_route;
mod execution_contract;
mod formal_toolchain;
mod framework_host_targets;
mod framework_maint;
mod framework_profile;
mod framework_runtime;
mod framework_skills;
mod harness_context_signals;
mod harness_contract;
mod harness_operator_nudges;
mod hook_common;
mod hook_outbound_protect;
mod hook_observation_rules;
mod hook_policy;
#[path = "utils/hook_posttool_normalize.rs"]
mod hook_posttool_normalize;
mod hook_timing;
mod host_entrypoint_sync;
mod host_integration;
pub mod framework_host_integration {
    //! Install/sync CLI surface (`framework host-integration`); historical module name `host_integration`.
    pub use crate::host_integration::*;
}
#[path = "utils/lane_normalize.rs"]
mod lane_normalize;
mod paper_adversarial_hook;
mod paper_prose_hook;
mod runtime_registry;
mod review_gate;
mod review_gate_engine;
mod review_output_lint;
mod review_routing_signals;
mod rfv_loop;
mod route;
mod router_env_flags;
mod router_rs_observation;
mod router_self;
mod schema_drift;
mod ship_readiness;
mod runtime_envelope_ids;
mod runtime_storage;
mod session_call_tracker;
mod session_supervisor;
mod skill_repo;
mod stdio_transport;
mod task_command;
pub(crate) use antigravity_core::task_state;
pub(crate) use antigravity_core::task_state_aggregate;
pub(crate) use antigravity_core::task_ledger;
pub(crate) use antigravity_core::step_ledger;

mod path_guard {
    pub use antigravity_core::utils::path_guard::*;
}
mod atomic_write {
    pub use antigravity_core::utils::atomic_write::*;
}
mod task_write_lock {
    pub use antigravity_core::utils::task_write_lock::*;
}

mod trace_runtime;

#[cfg(test)]
mod claude_desktop_test_support;

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
mod test_env_sync;

#[cfg(test)]
#[path = "../tests/main_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/claude_desktop_hooks_tests.rs"]
mod claude_desktop_hooks_tests;
