#![cfg(test)]

use super::*;
use crate::hosts::codex_hooks::handlers;
use crate::hosts::codex_hooks::handlers::{codex_additional_context_max_bytes, codex_compact_contexts, read_codex_stdin_limited, run_codex_review_subagent_gate, saw_subagent_codex, handle_codex_session_start, handle_codex_userpromptsubmit, handle_codex_stop, run_codex_lifecycle_context_hook};
use crate::hosts::codex_hooks::install;
use crate::hosts::codex_hooks::install::{build_install_hook_command, install_codex_cli_hooks, merge_hooks_json, write_atomic_text};
use crate::hosts::codex_hooks::install::FORCE_ATOMIC_WRITE_FAIL;
use crate::hosts::codex_hooks::pretool::{run_pre_tool_use, normalize_repo_relative_path, classify_protected_generated_path};
use crate::hosts::codex_hooks::state;
use crate::hosts::codex_hooks::state::{codex_load_state, codex_state_path, codex_session_key, codex_save_state_to_path, acquire_codex_state_lock, with_codex_state_lock, prune_stale_hook_state_files};
use crate::hosts::codex_hooks::{InstallMode, ROUTER_RS_HOOK_PROJECTION_VERSION, INSTALL_EVENTS};
use serial_test::serial;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;
use std::time::Duration;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static TEST_DEPS_ONCE: Once = Once::new();
pub(super) fn ensure_test_deps() {
    TEST_DEPS_ONCE.call_once(|| {
        crate::hooks::install_test_deps();
    });
}

#[test]
fn codex_first_nonempty_prompt_line_skips_leading_blank_lines() {
    assert_eq!(
        handlers::codex_first_nonempty_prompt_line("\n  \nreal task\nmore"),
        "real task"
    );
}

#[test]
fn protected_generated_paths_match_lexical_variants() {
    assert_eq!(normalize_repo_relative_path("./AGENTS.md"), "AGENTS.md");
    assert_eq!(
        normalize_repo_relative_path(".codex/../.codex/host_entrypoints_sync_manifest.json"),
        ".codex/host_entrypoints_sync_manifest.json"
    );
    assert!(classify_protected_generated_path("./AGENTS.md").is_some());
    assert!(classify_protected_generated_path(
        ".codex/../.codex/host_entrypoints_sync_manifest.json"
    )
    .is_some());
    assert!(classify_protected_generated_path("./.codex/prompts/gitx.md").is_none());
}

#[test]
fn pre_tool_use_blocks_normalized_direct_paths() {
    let payload = json!({"tool_input": {"file_path": "./AGENTS.md"}});
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({"tool_input": {"file_path": ".codex/../.codex/host_entrypoints_sync_manifest.json"}});
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({"tool_input": {"file_path": ".codex/../.codex/prompts/autopilot.md"}});
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());
}

#[test]
fn pre_tool_use_blocks_normalized_bash_write_targets() {
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x > ./AGENTS.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x | tee .codex/../.codex/host_entrypoints_sync_manifest.json"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x | tee .codex/prompts/gitx.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());

    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "printf x >| ./AGENTS.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_some());
}

#[test]
fn pre_tool_use_allows_read_only_bash_commands_on_protected_paths() {
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cat ./AGENTS.md"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());

    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "rg contract_digest .codex/host_entrypoints_sync_manifest.json"}
    });
    assert!(run_pre_tool_use(Path::new("."), &payload)
        .unwrap()
        .is_none());
}

mod install_codex_cli_hooks_tests;
mod lifecycle_context_tests;
mod lifecycle_context_tests_2;
