//! Shared hook heuristics for prompt/gate classification and small cross-host JSON key merges.
//!
//! **Architecture (2026-06-25 refactor):**
//! - `tool_origin` — tool vs skill namespace isolation, MCP FQN parsing, tool origin classification.
//! - `review_signals` — review prompt detection, gate status, nudge injection, delegation.
//! - `goal_signals` — goal contract recognition, progress/completion detection, structured goals.
//!
//! **不含**宿主 hook 的 stdin 生命周期分发、写盘或出站 JSON 投影；这类逻辑在各宿主的
//! hook 模块中。
//! Dependency direction: 宿主 hook 模块 → `hook_common`；
//! `hook_posttool_normalize` 不在此链上（其依赖 `cursor_hooks` 的字段 helper）。

pub mod goal_signals;
pub mod review_signals;
pub mod tool_origin;

use regex::Regex;
use std::cell::Cell;
use std::sync::OnceLock;

// ────────────────────────────────────────────────────────────────
// Re-exports — preserve backward-compatible flat API
// ────────────────────────────────────────────────────────────────

// Tool origin
pub use tool_origin::{
    ToolOrigin, classify_tool_origin, is_mcp_tool_name, normalize_tool_name, parse_mcp_tool_fqn,
    tool_input_value_from_map,
};

// Review signals
pub use review_signals::{
    REVIEW_GATE_LINE_CLEAR_MARKERS, has_delegation_override, has_override, has_review_override,
    is_framework_non_goal_entrypoint_prompt,
    is_narrow_review_prompt, is_parallel_delegation_prompt, is_review_prompt,
    is_reviewer_lane_normalized, normalize_subagent_type, review_gate_hard_block_disabled,
    review_gate_stop_would_nudge, saw_reject_reason, should_inject_spawn_first_review_nudge,
    should_inject_subagent_model_inherit_nudge,
};

#[cfg(test)]
pub(crate) use review_signals::install_review_prompt_test_deps;

// Goal signals
pub use goal_signals::{
    COMPLETION_DETECT_EN, COMPLETION_DETECT_ZH_PHRASES, GOAL_CHAT_VERIFY_ZH_PHRASES,
    completion_claim_keywords_export, contains_completion_claim_token, has_goal_progress_signal,
    has_goal_verify_or_block_signal, has_structured_goal_contract,
};

// ────────────────────────────────────────────────────────────────
// Shared utilities (used by sub-modules and external callers)
// ────────────────────────────────────────────────────────────────

pub(crate) fn compile_patterns(patterns: &[&str]) -> Result<Vec<Regex>, regex::Error> {
    patterns.iter().map(|p| Regex::new(p)).collect()
}

/// Strip fenced code blocks, inline code, URLs, blockquotes, and double-quoted strings from text.
/// Used by review_signals and goal_signals for signal detection on sanitized input.
///
/// Single combined regex replaces the previous 5 separate `replace_all` passes,
/// reducing string scans from 5× to 1× for texts >= 200 chars.
pub fn strip_quoted_or_codeblock_or_url(text: &str) -> String {
    // Short text fast path: avoid regex scan entirely.
    if text.len() < 200 {
        return text.to_string();
    }
    static RE_COMBINED: OnceLock<Regex> = OnceLock::new();
    RE_COMBINED
        .get_or_init(|| {
            Regex::new(
                r#"(?s)```.*?```|`[^`\n]*`|https?://\S+|(?m)^\s*>\s.*$|"[^"\n]*""#,
            )
            .unwrap_or_else(|e| panic!("invalid combined re: {e}"))
        })
        .replace_all(text, " ")
        .into_owned()
}

// ── Interactive profile (shared by review_signals and hook_dispatch) ──

thread_local! {
    static TEST_TASK_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Test-only override for [`is_task_profile`] (also used by `router-rs` host hook tests).
/// Thread-local so parallel `#[test]` threads do not race.
#[doc(hidden)]
pub fn set_test_task_override(v: Option<bool>) {
    TEST_TASK_OVERRIDE.with(|c| c.set(v));
}

/// Default UTF-8 **char** budget for assistant text on hook signal / lint paths (all hosts).
pub const HOOK_SIGNAL_ASSISTANT_TAIL_CHARS: usize = 2048;

/// Truncate assistant text for hook signal paths (char-based; matches deep-continuation tail style).
/// Single pass via char_indices() — avoids separate .chars().count() traversal.
pub fn hook_assistant_tail_window(raw: &str, max_chars: usize) -> String {
    if raw.is_empty() {
        return String::new();
    }
    // Single pass: count chars and record the byte offset where truncation begins.
    let mut truncate_at: Option<usize> = None;
    let mut total: usize = 0;
    for (byte_idx, _) in raw.char_indices() {
        if total == max_chars {
            truncate_at = Some(byte_idx);
        }
        total += 1;
    }
    if total <= max_chars {
        return raw.to_string();
    }
    let omitted = total.saturating_sub(max_chars);
    let tail = &raw[truncate_at.unwrap_or(raw.len())..];
    format!("[...omitted {omitted} chars...]\n{tail}")
}

/// True when the current session is in the task lifecycle profile.
///
/// Task profiles suppress review-gate hard block,
/// disable spawn-first nudge, and reject being scheduled by the Loop Engine.
///
/// Detection (in priority order):
/// 1. Thread-local `TEST_TASK_OVERRIDE` (testing only)
/// 2. RUNTIME_REGISTRY.json context/lifecycle_profiles config (via repo_root)
///
/// Cf. docs/README.md §1.2 (hook model)
pub fn is_task_profile(repo_root: Option<&std::path::Path>, _text: &str) -> bool {
    if let Some(v) = TEST_TASK_OVERRIDE.with(|c| c.get()) {
        return v;
    }
    let Some(root) = repo_root else {
        return false;
    };
    // Check for active GOAL_STATE.json with lifecycle_profile == "task"
    // to activate goal-drive mode.
    if let Ok(pointers) = std::fs::read_to_string(root.join("artifacts/current/TASK_POINTERS.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&pointers) {
            if let Some(active) = v.get("active_task_id").and_then(|a| a.as_str()) {
                let goal_path = root
                    .join("artifacts/current")
                    .join(active)
                    .join("GOAL_STATE.json");
                if let Ok(gs) = std::fs::read_to_string(&goal_path) {
                    if let Ok(gv) = serde_json::from_str::<serde_json::Value>(&gs) {
                        return gv
                            .get("lifecycle_profile")
                            .and_then(|p| p.as_str())
                            .is_some_and(|p| p == "task");
                    }
                }
            }
        }
    }
    false
}
