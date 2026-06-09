mod audit;
mod install;
mod lifecycle;
mod policy_embed;
mod post_tool;
mod state;
mod stop;

#[cfg(test)]
mod tests;

pub use audit::*;
pub use install::*;
pub use lifecycle::*;
pub use post_tool::evaluate_codex_post_tool_use;
#[allow(unused_imports)] // re-exported for `codex_hooks/tests` and external hook callers
pub use stop::{evaluate_codex_stop, handle_codex_stop};
#[allow(unused_imports)]
pub use state::{
    acquire_codex_state_lock, codex_load_state, codex_save_state_to_path, codex_session_key,
    codex_state_path, prune_stale_hook_state_files, with_codex_state_lock,
};

use std::cell::Cell;
use std::sync::atomic::AtomicU64;

pub const CODEX_HOOK_AUTHORITY: &str = "rust-codex-audit";
pub const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str =
    router_rs::hook_common::path_guard::HOST_ENTRYPOINT_SYNC_MANIFEST_PATH;
pub const HOST_ENTRYPOINT_SYNC_HINT: &str =
    "cargo run --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root \"$PWD\"";
pub const CODEX_AGENT_POLICY_PATH: &str =
    router_rs::hook_common::path_guard::CODEX_AGENT_POLICY_PATH;
pub const CODEX_HOOKS_PATH: &str = router_rs::hook_common::path_guard::CODEX_HOOKS_PATH;
pub const CODEX_HOOKS_README_PATH: &str =
    router_rs::hook_common::path_guard::CODEX_HOOKS_README_PATH;
pub const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 1] = [CODEX_HOOKS_PATH];
pub const CODEX_REVIEW_SUBAGENT_TOOL_NAMES: [&str; 6] = [
    "task",
    "functions.task",
    "functions.subagent",
    "functions.spawn_agent",
    "subagent",
    "spawn_agent",
];
pub const CODEX_REVIEW_SUBAGENT_TYPES: &[&str] = &[
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
pub const INSTALL_EVENTS: [&str; 7] = INSTALL_LIFECYCLE_EVENTS;

struct HostStrings {
    review_gate_tag: &'static str,
    review_gate_disable_env: &'static str,
    stop_hook_active_bypass_env: &'static str,
    require_stable_session_key_env: &'static str,
    hook_state_salt_env: &'static str,
    hook_state_unreadable_tag: &'static str,
    lifecycle_label: &'static str,
    spawn_first_host_id: &'static str,
}

const CODEX_STRINGS: HostStrings = HostStrings {
    review_gate_tag: "CODEX_REVIEW_GATE",
    review_gate_disable_env: "ROUTER_RS_CODEX_REVIEW_GATE_DISABLE",
    stop_hook_active_bypass_env: "ROUTER_RS_CODEX_STOP_HOOK_ACTIVE_BYPASS",
    require_stable_session_key_env: "ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY",
    hook_state_salt_env: "ROUTER_RS_CODEX_HOOK_STATE_SALT",
    hook_state_unreadable_tag: "CODEX_HOOK_STATE_UNREADABLE",
    lifecycle_label: "Codex",
    spawn_first_host_id: "codex",
};

const ANTIGRAVITY_CLI_STRINGS: HostStrings = HostStrings {
    review_gate_tag: "ANTIGRAVITY_CLI_REVIEW_GATE",
    review_gate_disable_env: "ROUTER_RS_ANTIGRAVITY_CLI_REVIEW_GATE_DISABLE",
    stop_hook_active_bypass_env: "ROUTER_RS_ANTIGRAVITY_CLI_STOP_HOOK_ACTIVE_BYPASS",
    require_stable_session_key_env: "ROUTER_RS_ANTIGRAVITY_CLI_REQUIRE_STABLE_SESSION_KEY",
    hook_state_salt_env: "ROUTER_RS_ANTIGRAVITY_CLI_HOOK_STATE_SALT",
    hook_state_unreadable_tag: "ANTIGRAVITY_CLI_HOOK_STATE_UNREADABLE",
    lifecycle_label: "Antigravity CLI",
    spawn_first_host_id: "antigravity",
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CodexLifecycleHostKind {
    state_dir_leaf: &'static str,
}

impl CodexLifecycleHostKind {
    pub const CODEX: Self = Self {
        state_dir_leaf: ".codex",
    };
    pub const ANTIGRAVITY_CLI: Self = Self {
        state_dir_leaf: ".antigravitycli",
    };

    fn strings(self) -> &'static HostStrings {
        match self.state_dir_leaf {
            ".antigravitycli" => &ANTIGRAVITY_CLI_STRINGS,
            _ => &CODEX_STRINGS,
        }
    }

    pub fn review_gate_tag(self) -> &'static str {
        self.strings().review_gate_tag
    }

    pub fn review_gate_disable_env(self) -> &'static str {
        self.strings().review_gate_disable_env
    }

    pub fn stop_hook_active_bypass_env(self) -> &'static str {
        self.strings().stop_hook_active_bypass_env
    }

    pub fn require_stable_session_key_env(self) -> &'static str {
        self.strings().require_stable_session_key_env
    }

    pub fn hook_state_salt_env(self) -> &'static str {
        self.strings().hook_state_salt_env
    }

    pub fn hook_state_unreadable_tag(self) -> &'static str {
        self.strings().hook_state_unreadable_tag
    }

    pub fn lifecycle_label(self) -> &'static str {
        self.strings().lifecycle_label
    }

    pub fn spawn_first_host_id(self) -> &'static str {
        self.strings().spawn_first_host_id
    }

    pub fn paper_prose_hook_host(self) -> router_rs::paper_prose_hook::PaperProseHookHost {
        router_rs::paper_prose_hook::PaperProseHookHost::from_codex_lifecycle_state_dir(
            self.state_dir_leaf,
        )
    }
}

thread_local! {
    pub static LIFECYCLE_HOST: Cell<CodexLifecycleHostKind> =
        const { Cell::new(CodexLifecycleHostKind::CODEX) };
}

pub fn lifecycle_host() -> CodexLifecycleHostKind {
    LIFECYCLE_HOST.with(|cell| cell.get())
}

pub const ROUTER_RS_HOOK_PROJECTION_VERSION: &str = "v1.0.0";
pub const INSTALL_STATUS_USER_PROMPT: &str = "Loading Codex turn context";
pub const INSTALL_STATUS_SESSION_START: &str = "Loading Codex live state";
pub const INSTALL_STATUS_PRE_TOOL: &str = "Checking generated-surface guard";
pub const INSTALL_STATUS_POST_TOOL: &str = "Recording Codex tool evidence";
pub const INSTALL_STATUS_STOP: &str = "Enforcing Codex review gate";
pub const INSTALL_STATUS_SUBAGENT_START: &str = "Recording Codex subagent start";
pub const INSTALL_STATUS_SUBAGENT_STOP: &str = "Recording Codex subagent stop";

pub const INSTALL_STATUS_ANTIGRAVITY_SESSION_START: &str =
    "Loading Antigravity CLI session state";
pub const INSTALL_STATUS_ANTIGRAVITY_PRE_TOOL: &str =
    "Checking Antigravity CLI generated-surface guard";
pub const INSTALL_STATUS_ANTIGRAVITY_USER_PROMPT: &str =
    "Loading Antigravity CLI turn context";
pub const INSTALL_STATUS_ANTIGRAVITY_POST_TOOL: &str =
    "Recording Antigravity CLI tool evidence";
pub const INSTALL_STATUS_ANTIGRAVITY_STOP: &str = "Enforcing Antigravity CLI review gate";
pub const INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_START: &str =
    "Recording Antigravity CLI subagent start";
pub const INSTALL_STATUS_ANTIGRAVITY_SUBAGENT_STOP: &str =
    "Recording Antigravity CLI subagent stop";
/// Default UTF-8 **byte** budget for merged Codex `additionalContext` (SessionStart / UserPromptSubmit).
pub const CODEX_ADDITIONAL_CONTEXT_MAX_BYTES: usize = 640;
pub static ATOMIC_WRITE_NONCE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
thread_local! {
    pub static FORCE_ATOMIC_WRITE_FAIL: Cell<bool> = const { Cell::new(false) };
}
