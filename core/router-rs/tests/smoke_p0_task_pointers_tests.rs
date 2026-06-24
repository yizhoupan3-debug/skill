//! `state_manager/task_pointers` coverage at router-rs boundary
//! (physical module: `core_state::state_manager`).

use framework_extra::snapshot::build_framework_runtime_snapshot_envelope;
use framework_extra::session_artifacts::write_framework_session_artifacts;
use core_state::state_manager::{
    neutralize_task_pointers_for_task, read_primary_task_id, read_task_pointer_pair,
    sync_task_pointers_after_goal_drive, write_active_task_pointer,
};