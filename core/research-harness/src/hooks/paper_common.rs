//! Shared utilities for paper hooks (adversarial + prose).
//!
//! Extracts the common patterns duplicated between `paper_adversarial.rs`
//! and `paper_prose.rs`: operator inject check, builtin block resolution,
//! and the append/merge template.

use crate::hooks::paper_block_cache::BlockCache;
use core_state::state_manager::merge_hook_nudge_paragraph;
use serde_json::Value;
use std::path::Path;
use std::sync::LazyLock;

/// Check if the global operator inject flag is enabled.
/// This is the shared gate for all paper hooks.
pub fn operator_inject_globally_enabled() -> bool {
    core_policy::env_flags::env_enabled_default_true("ROUTER_RS_OPERATOR_INJECT")
}

/// Append hook context if the hook is requested and the prompt signals relevance.
///
/// Returns `true` if context was appended.
pub fn maybe_append_context(
    hook_requested: bool,
    signal_detected: bool,
    repo_root: &Path,
    block_cache: &BlockCache,
    builtin_lazy: &LazyLock<String>,
    contexts: &mut Vec<String>,
) -> bool {
    if !hook_requested || !signal_detected {
        return false;
    }
    let builtin = (**builtin_lazy).clone();
    let msg = block_cache.resolve(repo_root, move || builtin);
    if msg.trim().is_empty() {
        return false;
    }
    contexts.push(msg);
    true
}

/// Merge hook context into Cursor-compatible JSON output.
///
/// Returns `true` if context was merged.
pub fn maybe_merge_context(
    hook_requested: bool,
    signal_detected: bool,
    repo_root: &Path,
    block_cache: &BlockCache,
    builtin_lazy: &LazyLock<String>,
    output: &mut Value,
    prefix_line: &str,
    use_followup_message: bool,
) -> bool {
    if !hook_requested || !signal_detected {
        return false;
    }
    let builtin = (**builtin_lazy).clone();
    let msg = block_cache.resolve(repo_root, move || builtin);
    if msg.trim().is_empty() {
        return false;
    }
    merge_hook_nudge_paragraph(output, &msg, prefix_line, use_followup_message);
    true
}
