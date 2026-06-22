//! Codex-specific capabilities: install and contract guard.
//! state.rs — hook state persistence required by install.

pub(crate) mod state;
pub mod install;

pub use install::{
    build_codex_hook_manifest, build_codex_hook_projection, build_codex_hooks_readme,
    build_hook_binary_preamble, host_entrypoint_provider, install_codex_cli_hooks,
    resolve_codex_home,
};

// ── Constants ──

pub const CODEX_HOOKS_PATH: &str = ".codex/hooks.json";
pub const CODEX_HOOKS_README_PATH: &str = ".codex/README.md";
pub const CODEX_AGENT_POLICY_PATH: &str = "AGENTS.md";
pub const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str = ".codex/host_entrypoints_sync_manifest.json";
pub const ROUTER_RS_HOOK_PROJECTION_VERSION: &str = "v1.0.0";
pub(crate) const CODEX_HOOK_AUTHORITY: &str = "rust-codex-audit";

/// Build the Codex agent policy.
pub fn build_codex_agent_policy() -> serde_json::Value {
    serde_json::json!([])
}

pub const INSTALL_LIFECYCLE_EVENTS: [&str; 7] = [
    "SessionStart", "PreToolUse", "UserPromptSubmit",
    "PostToolUse", "Stop", "SubagentStart", "SubagentStop",
];
pub(crate) const INSTALL_EVENTS: [&str; 7] = INSTALL_LIFECYCLE_EVENTS;

pub(super) const PROTECTED_GENERATED_PATHS: [&str; 4] = [
    CODEX_AGENT_POLICY_PATH, CODEX_HOOKS_PATH,
    CODEX_HOOKS_README_PATH, HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
];

pub const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 1] = [CODEX_HOOKS_PATH];

// ── Types ──

use core_policy::HookReviewDiskCore;
use std::cell::Cell;

/// Codex lifecycle host kind — always CODEX in production.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CodexLifecycleHostKind {
    pub state_dir_leaf: &'static str,
}

impl CodexLifecycleHostKind {
    pub const CODEX: Self = Self { state_dir_leaf: ".codex" };
    pub fn require_stable_session_key_env(self) -> &'static str {
        "ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY"
    }
    pub fn hook_state_salt_env(self) -> &'static str {
        "ROUTER_RS_CODEX_HOOK_STATE_SALT"
    }
}

thread_local! {
    static LIFECYCLE_HOST: Cell<CodexLifecycleHostKind> =
        const { Cell::new(CodexLifecycleHostKind::CODEX) };
}

pub(crate) fn lifecycle_host() -> CodexLifecycleHostKind {
    LIFECYCLE_HOST.with(|cell| cell.get())
}

/// Codex lifecycle context state (shared by state.rs and install.rs).
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct CodexLifecycleContextState {
    #[serde(flatten)]
    pub(crate) review_gate: HookReviewDiskCore,
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
}

impl crate::hosts::hook_state_common::HookStateVersion for CodexLifecycleContextState {
    const STATE_VERSION: u32 = core_policy::HOOK_REVIEW_DISK_VERSION;
    fn disk_version(&self) -> u32 { self.review_gate.disk_version() }
}

// ── Install mode & merge stat ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode { Apply, Check }

#[derive(Debug, Clone)]
pub struct HooksMergeStat {
    pub status: &'static str,
    pub preserved_existing_entries: usize,
    pub added_entries: usize,
    pub removed_legacy_entries: usize,
}
