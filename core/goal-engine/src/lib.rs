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
    take_kill_signal, write_kill_signal, write_signal, take_signal, write_pause_signal,
    write_resume_signal, write_redirect_signal, write_pause_state, read_pause_state,
    clear_pause_state, is_pause_state_active,
};
pub use report::{render_loop_report, write_loop_report};
pub use runner::{
    preflight_profile_check, run_loop_pause, run_loop_pause_status, run_loop_redirect,
    run_loop_resume,
};
pub use safety::{assign_safety_for_action, assign_safety_for_file, parse_safety_level};
pub use state::{
    closeout_path, create_initial_state, generate_run_id, kill_signal_path, lock_path,
    loop_artifacts_dir, loop_state_path, now_iso, pause_state_path, read_loop_state,
    write_loop_state,
};
pub use types::{
    KillSignalAction, KillSignalPayload, LoopAction, LoopActionRecord, LoopCloseoutAggregate,
    LoopError, LoopPhase, LoopProfileConfig, LoopRegistryEntry, LoopRegistryRoot, LoopRunState,
    PauseState, SafetyLevel, PAUSE_STATE_SCHEMA_VERSION,
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn smoke_goal_engine_types_accessible() {
        // Verify crate types are importable and constructible
        let phase = LoopPhase::Running;
        assert_eq!(phase.as_str(), "running");
        assert!(phase.valid_transitions().contains(&LoopPhase::Verifying));
    }
}
