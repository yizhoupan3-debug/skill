//! Runtime-core contracts: extracted pure-data modules from runtime-core.
//!
//! Leaf modules in the runtime-core dependency graph that can be extracted
//! without creating circular dependencies. review_gate.rs, task_command.rs,
//! and hook_timing.rs remain in runtime-core due to deeper internal coupling.

pub mod formal_toolchain;
pub mod framework_skills;
pub mod harness_contract;
pub mod harness_context_signals;
pub mod harness_operator_nudges;
pub mod hook_event_routing;
pub mod hook_observation_rules;
pub mod hook_outbound_protect;
pub mod kernel_bootstrap;
pub mod mcp_pre_guard;
pub mod router_env_flags;
pub mod router_rs_observation;
pub mod session_call_tracker;
pub mod web_fetch_guard;
