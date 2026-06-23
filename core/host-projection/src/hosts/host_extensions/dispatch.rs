//! Unified hook dispatcher for all 4 closed-set hosts.
//!
//! All hosts use a single `RegistryDispatcher` struct configured from
//! RUNTIME_REGISTRY.json fields. No per-host hardcoded structs remain.

use crate::hosts::hook_dispatch::{extract_session_key, stop_signal_text_from_payload, value_to_hook_output, HookEvent, HookOutput, HostHookConfig, HostHookDispatcher};
use crate::hosts::stop_dispatch::{StopHostOps, run_unified_stop};
use crate::hosts::generic_config::GenericHostConfig;
use super::pretool;
use serde_json::Value;

/// Data-driven dispatcher for all hosts. Configured from RUNTIME_REGISTRY.json fields.
/// Replaces the previous 4 per-host structs (ClaudeDispatcher, CursorDispatcher, etc.).
pub struct RegistryDispatcher {
    pub config: GenericHostConfig,
}

impl HostHookConfig for RegistryDispatcher {
    fn host_id(&self) -> &'static str { self.config.id }
    fn state_dir_leaf(&self) -> &'static str { self.config.id }
    fn hook_state_unreadable_tag(&self) -> &'static str { self.config.unreadable_tag }
    fn session_namespace_env(&self) -> &'static str { self.config.namespace_env }
    fn log_label(&self) -> &'static str { self.config.label }
    fn additional_context_max_bytes(&self) -> usize { self.config.max_context_bytes }
    fn supports_session_start(&self) -> bool { self.config.session_start }
    fn supports_subagent_start(&self) -> bool { self.config.subagent_start }
    fn supports_subagent_stop(&self) -> bool { self.config.subagent_stop }
}

impl StopHostOps for RegistryDispatcher {
    fn host_id(&self) -> &'static str { self.config.id }
    fn log_label(&self) -> &'static str { self.config.label }
    fn session_key(&self, repo_root: &std::path::Path, payload: &Value) -> String {
        let fallback = repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
        extract_session_key(payload, "", fallback, self.config.scan_tool_input)
    }
    fn stop_signal_text(&self, payload: &Value) -> String { stop_signal_text_from_payload(payload) }
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
            Err(err) => Some(HookOutput::Deny { reason: err }),
        }
    }
    fn handle_stop(&self, event: &HookEvent) -> Option<HookOutput> {
        value_to_hook_output(
            &run_unified_stop(event.repo_root, event.payload, self).unwrap_or_default()
        )
    }
}
