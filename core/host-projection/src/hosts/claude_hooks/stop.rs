//! E7 step 2: Stop handler — review gate, touch-state validation, session cleanup.

use router_rs::review_gate_engine::{review_gate_blocks_stop, ReviewGateFacts};
use serde_json::Value;
use std::path::Path;

use super::{
    active_stdio_agent_hook_host, block_stop, claude_review_gate_suppressed,
};
use super::session::{
    clear_review_state, clear_touch_state, load_review_gate_disk, load_touch_state_disk,
    review_state_path, touch_state_path, AgentDiskState, ReviewGateState, TouchState,
};
use super::user_prompt::{
    claude_review_gate_incomplete_stop_reason_for_stop, claude_stop_signal_text,
};

/// Claude Stop: `None` → allow; `Some` → block with stopReason.
pub fn evaluate_claude_stop(repo_root: &Path, payload: &Value) -> Option<Value> {
    let review_load = load_review_gate_disk(repo_root, payload);
    let touch_load = load_touch_state_disk(repo_root, payload);
    if matches!(review_load, AgentDiskState::Unreadable) {
        eprintln!(
            "[router-rs] {} review_gate state unreadable on Stop: {}",
            active_stdio_agent_hook_host().log_label(),
            review_state_path(repo_root, payload).display()
        );
        return block_stop(active_stdio_agent_hook_host().hook_state_unreadable());
    }
    if matches!(touch_load, AgentDiskState::Unreadable) {
        eprintln!(
            "[router-rs] {} hook_state unreadable on Stop: {}",
            active_stdio_agent_hook_host().log_label(),
            touch_state_path(repo_root, payload).display()
        );
        return block_stop(active_stdio_agent_hook_host().hook_state_unreadable());
    }

    let review_state = match review_load {
        AgentDiskState::Absent => ReviewGateState::default(),
        AgentDiskState::Ok(s) => s,
        AgentDiskState::Unreadable => {
            return block_stop(active_stdio_agent_hook_host().hook_state_unreadable());
        }
    };
    if !claude_review_gate_suppressed(repo_root, &claude_stop_signal_text(payload))
        && review_gate_blocks_stop(ReviewGateFacts {
            review_required: review_state.review_required,
            review_override: review_state.review_override,
            independent_reviewer_seen: review_state.independent_reviewer_seen,
        })
    {
        return block_stop(&claude_review_gate_incomplete_stop_reason_for_stop());
    }
    let state = match touch_load {
        AgentDiskState::Absent => TouchState::default(),
        AgentDiskState::Ok(s) => s,
        AgentDiskState::Unreadable => {
            return block_stop(active_stdio_agent_hook_host().hook_state_unreadable());
        }
    };
    if state.settings && !state.settings_validated {
        return block_stop(active_stdio_agent_hook_host().validate_settings_stop_reason());
    }
    if state.framework && !state.framework_tested {
        return block_stop("cargo test --lib -p router-rs");
    }
    clear_review_state(repo_root, payload);
    clear_touch_state(repo_root, payload);
    // I6: 关键任务 RFV 默认启用时注入自动续跑提示
    if let Some(nudge) = router_rs::framework_runtime::rfv_nudge_for_key_task(repo_root) {
        return block_stop(&nudge);
    }
    None
}
