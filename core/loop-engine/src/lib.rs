#![deny(clippy::unwrap_used, clippy::expect_used)]
pub mod closeout;
pub mod dispatcher;
pub mod drift;
pub mod env_flags;
pub mod kill_switch;
pub mod report;
pub mod runner;
pub mod safety;
pub mod state;
pub mod types;

pub use closeout::{
    aggregate_passes, build_aggregate, read_action_record, verify_closeout_value,
    verify_closeout_with_evidence, verify_evidence_index,
};
pub use dispatcher::{
    SubagentResult, build_handoff, check_scope_compliance, resolve_subagent_binary,
    run_action_dry_run, run_action_sync,
};
pub use kill_switch::{
    acquire_lock, clear_kill_signal, is_kill_signal_active, read_lock_info, release_lock,
    take_kill_signal, write_kill_signal,
};
pub use report::{render_loop_report, write_loop_report};
pub use runner::preflight_profile_check;
pub use safety::{assign_safety_for_action, assign_safety_for_file, parse_safety_level};
pub use state::{
    closeout_path, create_initial_state, generate_run_id, kill_signal_path, lock_path,
    loop_artifacts_dir, loop_state_path, now_iso, read_loop_state, write_loop_state,
};
pub use types::{
    LoopAction, LoopActionRecord, LoopCloseoutAggregate, LoopError, LoopPhase, LoopProfileConfig,
    LoopRegistryEntry, LoopRegistryRoot, LoopRunState, SafetyLevel,
};
