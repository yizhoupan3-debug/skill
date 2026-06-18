mod state;
pub use state::CodexHookError;
use state::*;

pub(crate) mod handlers;
pub mod dispatcher;
pub use handlers::run_codex_audit_hook;
pub use handlers::run_codex_lifecycle_context_hook_for_state_dir;

mod install;
pub use install::{
    build_codex_hook_manifest, build_codex_hook_projection, build_codex_hooks_readme,
    build_hook_binary_preamble, codex_host_entrypoint_provider, install_codex_cli_hooks,
    resolve_codex_home,
};

mod drift;

pub(crate) mod pretool;

mod contract_guard;

mod policy_embed;
pub use policy_embed::build_codex_agent_policy;

use crate::hooks;
use core_policy::HookReviewDiskCore;
use serde_json::Value;
use std::cell::Cell;
use std::path::Path;

// ---------------------------------------------------------------------------
// Constants (public API surface)
// ---------------------------------------------------------------------------

const CODEX_HOOK_AUTHORITY: &str = "rust-codex-audit";
pub const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str = ".codex/host_entrypoints_sync_manifest.json";
pub(super) const HOST_ENTRYPOINT_SYNC_HINT: &str = "cargo run --manifest-path core/router-rs/Cargo.toml -- framework sync-entrypoints --host-id codex --repo-root \"$PWD\"";
pub const CODEX_AGENT_POLICY_PATH: &str = "AGENTS_CODEX.md";
pub const CODEX_HOOKS_PATH: &str = ".codex/hooks.json";
pub const CODEX_HOOKS_README_PATH: &str = ".codex/README.md";
pub const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 1] = [CODEX_HOOKS_PATH];
pub(super) const PROTECTED_GENERATED_PATHS: [&str; 6] = [
    CODEX_AGENT_POLICY_PATH,
    "AGENTS.md",
    "AGENTS_CURSOR.md",
    CODEX_HOOKS_PATH,
    CODEX_HOOKS_README_PATH,
    HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
];
pub(super) const PROTECTED_GENERATED_PREFIXES: [&str; 3] = [
    "skills/SKILL_",
    "configs/framework/RUNTIME_REGISTRY.json",
    "core/router-rs/",
];
pub(super) const CODEX_REVIEW_SUBAGENT_TYPES: &[&str] = &[
    "default",
    "explore",
    "explorer",
    "general-purpose",
    "generalpurpose",
    "shell",
    "worker",
    "browser-use",
    "browseruse",
    "ci-investigator",
    "ciinvestigator",
    "best-of-n-runner",
    "bestofnrunner",
    "cursor-guide",
    "cursorguide",
];
pub const INSTALL_LIFECYCLE_EVENTS: [&str; 7] = [
    "SessionStart",
    "PreToolUse",
    "UserPromptSubmit",
    "PostToolUse",
    "Stop",
    "SubagentStart",
    "SubagentStop",
];
pub(crate) const INSTALL_EVENTS: [&str; 7] = INSTALL_LIFECYCLE_EVENTS;

pub(super) const CODEX_ADDITIONAL_CONTEXT_MAX_BYTES: usize = 640;

// ---------------------------------------------------------------------------
// Host strings & lifecycle kind
// ---------------------------------------------------------------------------

struct HostStrings {
    review_gate_tag: &'static str,
    stop_hook_active_bypass_env: &'static str,
    require_stable_session_key_env: &'static str,
    hook_state_salt_env: &'static str,
    hook_state_unreadable_tag: &'static str,
    lifecycle_label: &'static str,
    spawn_first_host_id: &'static str,
}

const CODEX_STRINGS: HostStrings = HostStrings {
    review_gate_tag: "CODEX_REVIEW_GATE",
    stop_hook_active_bypass_env: "ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS",
    require_stable_session_key_env: "ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY",
    hook_state_salt_env: "ROUTER_RS_CODEX_HOOK_STATE_SALT",
    hook_state_unreadable_tag: "CODEX_HOOK_STATE_UNREADABLE",
    lifecycle_label: "Codex",
    spawn_first_host_id: "codex",
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CodexLifecycleHostKind {
    state_dir_leaf: &'static str,
}

impl CodexLifecycleHostKind {
    pub const CODEX: Self = Self {
        state_dir_leaf: ".codex",
    };

    fn strings(self) -> &'static HostStrings {
        &CODEX_STRINGS
    }

    fn review_gate_tag(self) -> &'static str {
        self.strings().review_gate_tag
    }

    fn stop_hook_active_bypass_env(self) -> &'static str {
        self.strings().stop_hook_active_bypass_env
    }

    fn require_stable_session_key_env(self) -> &'static str {
        self.strings().require_stable_session_key_env
    }

    fn hook_state_salt_env(self) -> &'static str {
        self.strings().hook_state_salt_env
    }

    fn hook_state_unreadable_tag(self) -> &'static str {
        self.strings().hook_state_unreadable_tag
    }

    fn lifecycle_label(self) -> &'static str {
        self.strings().lifecycle_label
    }

    fn spawn_first_host_id(self) -> &'static str {
        self.strings().spawn_first_host_id
    }

    fn paper_prose_hook_host(self) -> hooks::PaperProseHookHost {
        hooks::PaperProseHookHost::from_codex_lifecycle_state_dir(self.state_dir_leaf)
    }
}

thread_local! {
    static LIFECYCLE_HOST: Cell<CodexLifecycleHostKind> =
        const { Cell::new(CodexLifecycleHostKind::CODEX) };
}

pub(super) fn lifecycle_host() -> CodexLifecycleHostKind {
    LIFECYCLE_HOST.with(|cell| cell.get())
}

pub const ROUTER_RS_HOOK_PROJECTION_VERSION: &str = "v1.0.0";

// ---------------------------------------------------------------------------
// Install mode & merge stat (public API types)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    Apply,
    Check,
}

#[derive(Debug, Clone)]
pub struct HooksMergeStat {
    pub status: &'static str,
    pub preserved_existing_entries: usize,
    pub added_entries: usize,
    pub removed_legacy_entries: usize,
}

// ---------------------------------------------------------------------------
// Lifecycle context state (shared by state.rs and handlers.rs)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct CodexLifecycleContextState {
    #[serde(flatten)]
    review_gate: HookReviewDiskCore,
    #[serde(default)]
    seq: i64,
    #[serde(default)]
    review_subagent_seen: bool,
    #[serde(default)]
    generic_subagent_seen: bool,
    #[serde(default)]
    review_lane_seen: bool,
    #[serde(default)]
    parallel_lane_seen: bool,
    #[serde(default)]
    phase: u32,
    #[serde(default)]
    subagent_start_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_subagent_tool: Option<String>,
}

impl crate::hosts::hook_state_common::HookStateVersion for CodexLifecycleContextState {
    const STATE_VERSION: u32 = core_policy::HOOK_REVIEW_DISK_VERSION;
    fn disk_version(&self) -> u32 {
        self.review_gate.disk_version()
    }
}

impl CodexLifecycleContextState {
    fn review_gate_fields(&self) -> core_policy::HookReviewGateFields {
        self.review_gate.gate_fields()
    }
}

fn codex_reset_hook_state(repo_root: &Path, event: &Value) {
    let _ = with_codex_state_lock(repo_root, event, |_loaded| {
        let reset = CodexLifecycleContextState {
            seq: 0,
            ..CodexLifecycleContextState::default()
        };
        Ok((Some(reset), ()))
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
