#![recursion_limit = "256"]

mod autopilot_goal;
mod background_state;
mod browser_mcp;
mod cli;
mod closeout_enforcement;
mod codex_hooks;
mod cursor_hooks;
mod eval_route;
mod execution_contract;
mod framework_profile;
mod framework_runtime;
mod harness_operator_nudges;
mod hook_policy;
mod host_integration;
mod rfv_loop;
mod route;
mod router_env_flags;
mod router_self;
mod runtime_envelope_ids;
mod runtime_storage;
mod session_supervisor;
mod skill_repo;
mod stdio_transport;
mod task_command;
mod task_state;
mod task_state_aggregate;
mod task_write_lock;
mod trace_runtime;

#[cfg(test)]
mod integration_test_prelude;

pub mod hook_status {
    pub const REVIEW_GATE_CHECKING: &str = "Checking review/subagent gate";
    pub const REVIEW_GATE_UPDATING: &str = "Updating review/subagent gate state";
    pub const REVIEW_GATE_ENFORCING: &str = "Enforcing review/subagent gate";
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
use route::{ROUTE_POLICY_SCHEMA_VERSION, ROUTE_REPORT_SCHEMA_VERSION};

use clap::Parser;

fn main() -> Result<(), String> {
    let args = cli::Cli::parse();
    cli::run(&args)
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
