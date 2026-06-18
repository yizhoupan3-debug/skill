//! Framework contracts: pure data types, env flags, and observation rules.
//!
//! Most modules extracted to `runtime-core-contracts` crate. This module
//! re-exports from that crate for backward compatibility.
//! review_gate, task_command, and hook_timing remain here due to deeper
//! internal coupling with runtime-core internals.

// --- Re-exports from runtime-core-contracts crate ---
pub use rt_core_contracts::formal_toolchain as formal_toolchain;
pub use rt_core_contracts::framework_skills as framework_skills;
pub use rt_core_contracts::harness_contract as harness_contract;
pub use rt_core_contracts::harness_context_signals as harness_context_signals;
pub use rt_core_contracts::harness_operator_nudges as harness_operator_nudges;
pub use rt_core_contracts::hook_event_routing as hook_event_routing;
pub use rt_core_contracts::hook_observation_rules as hook_observation_rules;
pub use rt_core_contracts::hook_outbound_protect as hook_outbound_protect;
pub use rt_core_contracts::kernel_bootstrap as kernel_bootstrap;
pub use rt_core_contracts::mcp_pre_guard as mcp_pre_guard;
pub use rt_core_contracts::router_env_flags as router_env_flags;
pub use rt_core_contracts::router_rs_observation as router_rs_observation;
pub use rt_core_contracts::session_call_tracker as session_call_tracker;
pub use rt_core_contracts::web_fetch_guard as web_fetch_guard;

// --- Modules that remain in runtime-core ---
pub mod hook_timing;
pub mod review_gate;
pub mod task_command;

// --- Pure re-exports from core-state (unchanged) ---
pub use core_state::utils::atomic_write as atomic_write;
pub use core_state::utils::path_guard as path_guard;
pub use core_state::step_ledger as step_ledger;
pub use core_state::task_ledger as task_ledger;
pub use core_state::task_state as task_state;
pub use core_state::task_state_aggregate as task_state_aggregate;
pub use core_state::utils::task_write_lock as task_write_lock;
pub use core_state::state_manager as goal_drive;
