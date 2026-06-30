//! Generic host configuration (data-driven, no hardcoded host names).
//!
//! Extracted from `hook_dispatch.rs` as part of the modularization
//! of the shared hook dispatch infrastructure.

use crate::hosts::hook_dispatch::HostHookConfig;

/// Generic host configuration derived from host_id.
/// All values are computed from the host_id string — no hardcoded host names.
pub struct GenericHostConfig {
    pub id: &'static str,
    pub label: &'static str,
    pub state_dir: &'static str,
    pub namespace_env: &'static str,
    pub unreadable_tag: &'static str,
    pub max_context_bytes: usize,
    pub session_start: bool,
    pub subagent_start: bool,
    pub subagent_stop: bool,
    pub pretool_path_protection: bool,
    pub pretool_entrypoint_hint: &'static str,
    pub scan_tool_input: bool,
}

impl GenericHostConfig {
    pub const fn new(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            state_dir: "",
            namespace_env: "",
            unreadable_tag: "",
            max_context_bytes: 640,
            session_start: false,
            subagent_start: false,
            subagent_stop: false,
            pretool_path_protection: false,
            pretool_entrypoint_hint: "",
            scan_tool_input: false,
        }
    }
}

impl HostHookConfig for GenericHostConfig {
    fn host_id(&self) -> &'static str {
        self.id
    }
    fn state_dir_leaf(&self) -> &'static str {
        self.state_dir
    }
    fn hook_state_unreadable_tag(&self) -> &'static str {
        self.unreadable_tag
    }
    fn session_namespace_env(&self) -> &'static str {
        self.namespace_env
    }
    fn log_label(&self) -> &'static str {
        self.label
    }
    fn additional_context_max_bytes(&self) -> usize {
        self.max_context_bytes
    }
    fn supports_session_start(&self) -> bool {
        self.session_start
    }
    fn supports_subagent_start(&self) -> bool {
        self.subagent_start
    }
    fn supports_subagent_stop(&self) -> bool {
        self.subagent_stop
    }
}

/// Macro to generate a complete `HostHookConfig` implementation from just a host_id.
/// All config values are derived from the host_id string — no hardcoded host names in code.
///
/// Usage: `impl_host_config!("claude", "Claude");`
#[macro_export]
macro_rules! impl_host_config {
    ($id:expr, $label:expr) => {
        fn host_id(&self) -> &'static str {
            $id
        }
        fn state_dir_leaf(&self) -> &'static str {
            concat!(".", $id)
        }
        fn hook_state_unreadable_tag(&self) -> &'static str {
            framework_core::runtime_registry::hook_state_unreadable_tag($id)
        }
        fn session_namespace_env(&self) -> &'static str {
            framework_core::runtime_registry::session_namespace_env($id)
        }
        fn log_label(&self) -> &'static str {
            $label
        }
    };
}
