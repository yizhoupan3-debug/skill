//! Re-export autopilot goal functions from core-state.
pub use core_state::state_manager::{
    deactivate_goal_for_conflict_with_rfv, evidence_index_entry_implies_success,
    framework_goal_drive, goal_state_requests_continuation, merge_hook_nudge_paragraph,
    read_active_task_id, read_focus_task_id, read_goal_state, read_task_pointer_pair,
    scrub_followup_fields_in_hook_output, scrub_spoof_host_followup_lines,
    task_evidence_artifacts_summary_for_task, task_evidence_success_only_self_attested,
    validate_external_research_strict, validate_external_research_structured,
};

#[cfg(test)]
pub use core_state::state_manager::goal_state_path_for_task;
