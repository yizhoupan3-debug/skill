//! Framework contracts: pure data types, env flags, and observation rules.
//!
//! These are leaf modules in the runtime-core dependency graph — they depend
//! on external crates (core-state, framework-kernel, etc.) but no other
//! runtime-core module depends on them except through this re-export hub.
//! Other runtime-core modules import from `contracts::*` via `pub use`.

pub mod atomic_write;
pub mod autopilot_goal;
pub mod closeout_enforcement;
pub mod execution_contract;
pub mod formal_toolchain;
pub mod framework_skills;
pub mod harness_contract;
pub mod hook_event_routing;
pub mod hook_observation_rules;
pub mod hook_outbound_protect;
pub mod harness_context_signals;
pub mod harness_operator_nudges;
pub mod hook_timing;
pub mod kernel_bootstrap;
pub mod mcp_pre_guard;
pub mod path_guard;
pub mod review_gate;
pub mod router_env_flags;
pub mod router_rs_observation;
pub mod session_call_tracker;
pub mod step_ledger;
pub mod task_command;
pub mod task_ledger;
pub mod task_state;
pub mod task_state_aggregate;
pub mod task_write_lock;
pub mod web_fetch_guard;
