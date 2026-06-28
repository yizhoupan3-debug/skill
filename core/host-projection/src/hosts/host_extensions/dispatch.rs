//! Unified hook dispatcher for all supported hosts (registry-driven).
//!
//! All hosts use a single `RegistryDispatcher` struct configured from
//! RUNTIME_REGISTRY.json fields. No per-host hardcoded structs remain.

use super::pretool;
use crate::hosts::generic_config::GenericHostConfig;
use crate::hosts::hook_dispatch::{
    HookEvent, HookOutput, HostHookConfig, HostHookDispatcher, extract_session_key,
    stop_signal_text_from_payload, value_to_hook_output,
};
use crate::hosts::stop_dispatch::{StopHostOps, run_unified_stop};
use serde_json::Value;

/// Data-driven dispatcher for all hosts. Configured from RUNTIME_REGISTRY.json fields.
/// Replaces the previous 4 per-host structs (ClaudeDispatcher, CursorDispatcher, etc.).
pub struct RegistryDispatcher {
    pub config: GenericHostConfig,
}

impl HostHookConfig for RegistryDispatcher {
    fn host_id(&self) -> &'static str {
        self.config.id
    }
    fn state_dir_leaf(&self) -> &'static str {
        self.config.id
    }
    fn hook_state_unreadable_tag(&self) -> &'static str {
        self.config.unreadable_tag
    }
    fn session_namespace_env(&self) -> &'static str {
        self.config.namespace_env
    }
    fn log_label(&self) -> &'static str {
        self.config.label
    }
    fn additional_context_max_bytes(&self) -> usize {
        self.config.max_context_bytes
    }
    fn supports_session_start(&self) -> bool {
        self.config.session_start
    }
    fn supports_subagent_start(&self) -> bool {
        self.config.subagent_start
    }
    fn supports_subagent_stop(&self) -> bool {
        self.config.subagent_stop
    }
}

impl StopHostOps for RegistryDispatcher {
    fn host_id(&self) -> &'static str {
        self.config.id
    }
    fn log_label(&self) -> &'static str {
        self.config.label
    }
    fn session_key(&self, repo_root: &std::path::Path, payload: &Value) -> String {
        let fallback = repo_root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        extract_session_key(payload, "", fallback, self.config.scan_tool_input)
    }
    fn stop_signal_text(&self, payload: &Value) -> String {
        stop_signal_text_from_payload(payload)
    }

    /// Hydrate goal gate from disk: when GOAL_STATE shows the goal is no longer
    /// driving (completed / blocked / drive_until_done=false), clear the stale
    /// `goal_drive_entry_active` flag so the goal gate doesn't block unrelated
    /// prompts (e.g. entering plan mode).
    fn hydrate_goal_gate_from_disk(
        &self,
        repo_root: &std::path::Path,
        state: &mut core_policy::hook_review_disk_state::HookReviewDiskCore,
        _goal_drive_entrypoint: bool,
    ) {
        if !state.goal.goal_drive_entry_active {
            return; // nothing to clear
        }
        let goal = match core_state::state_manager::read_goal_state(repo_root, None) {
            Ok(Some(g)) => g,
            _ => return,
        };
        let driving = goal
            .get("drive_until_done")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let status = goal.get("status").and_then(Value::as_str).unwrap_or("");
        let running = status == "running";
        // Clear the stale flag when the goal is no longer driving the session.
        if !driving || !running {
            state.goal.goal_drive_entry_active = false;
            state.goal.goal_contract_seen = false;
            state.goal.goal_progress_seen = false;
            state.goal.goal_verify_or_block_seen = false;
        }
    }
}

impl HostHookDispatcher for RegistryDispatcher {
    fn handle_pre_tool_use(&self, event: &HookEvent) -> Option<HookOutput> {
        if !self.config.pretool_path_protection {
            return None;
        }
        match pretool::run_pre_tool_use(
            event.repo_root,
            event.payload,
            &std::collections::HashSet::<String>::new(),
            &["configs/framework/", "core/", "AGENTS.md"],
            self.config.pretool_entrypoint_hint,
        ) {
            Ok(Some(val)) => Some(HookOutput::Raw(val)),
            Ok(None) => None,
            Err(err) => Some(HookOutput::Deny {
                reason: err.to_string(),
            }),
        }
    }
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        value_to_hook_output(
            &run_unified_stop(event.repo_root, event.payload, self).unwrap_or_default(),
        )
    }
}
