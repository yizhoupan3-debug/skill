pub mod runner;
pub mod types;
pub mod state;
pub mod safety;
pub mod kill_switch;
pub mod closeout;
pub mod dispatcher;
pub mod env_flags;
pub mod report;

pub use runner::preflight_profile_check;
pub use types::{
    LoopAction, LoopActionRecord, LoopCloseoutAggregate, LoopError, LoopPhase, LoopProfileConfig,
    LoopRegistryEntry, LoopRegistryRoot, LoopRunState, SafetyLevel,
};
pub use state::{
    read_loop_state, write_loop_state, create_initial_state, generate_run_id,
    lock_path, kill_signal_path, closeout_path, now_iso, loop_state_path, loop_artifacts_dir,
};
pub use safety::{parse_safety_level, assign_safety_for_action, assign_safety_for_file};
pub use kill_switch::{
    acquire_lock, release_lock, read_lock_info,
    write_kill_signal, clear_kill_signal, is_kill_signal_active, take_kill_signal,
};
pub use closeout::{verify_closeout_value, verify_closeout_with_evidence, verify_evidence_index, read_action_record, build_aggregate, aggregate_passes};
pub use dispatcher::{build_handoff, resolve_subagent_binary, run_action_sync, run_action_dry_run, check_scope_compliance, SubagentResult};
pub use report::{render_loop_report, write_loop_report};
