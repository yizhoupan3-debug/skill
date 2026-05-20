//! Shared hook gate helpers (fork_context, lane sets, spoof scrub). Host adapters stay thin (ADR-007).

pub(crate) use crate::registry_loader::{
    is_claude_reviewer_lane_from_registry, is_deep_review_gate_lane_from_registry,
};
pub(crate) use crate::review_gate_engine::fork_context_from_values;
pub(crate) use crate::autopilot_goal::{
    scrub_followup_fields_in_hook_output, scrub_spoof_host_followup_lines,
};
