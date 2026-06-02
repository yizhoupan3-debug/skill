use crate::hook_common::{has_override, is_framework_goal_entry_prompt, is_review_prompt};
use serde_json::Value;

/// Cursor `REVIEW_GATE` evidence path: multiset (strict) vs id-only pending vec (lite).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorReviewGateMode {
    Strict,
    Lite,
}

/// `ROUTER_RS_CURSOR_REVIEW_GATE_MODE=lite` enables lite; unset or `strict` → strict.
pub fn cursor_review_gate_mode() -> CursorReviewGateMode {
    match std::env::var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") {
        Ok(v) if v.trim().eq_ignore_ascii_case("lite") => CursorReviewGateMode::Lite,
        _ => CursorReviewGateMode::Strict,
    }
}

pub fn cycle_key_eligible_for_lite(key: &str) -> bool {
    key.starts_with("id:")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewGateFacts {
    pub review_required: bool,
    pub review_override: bool,
    pub independent_reviewer_seen: bool,
}

impl ReviewGateFacts {
    pub(crate) fn from_prompt(prompt: &str) -> Self {
        Self {
            review_required: is_review_prompt(prompt) && !is_framework_goal_entry_prompt(prompt),
            review_override: has_override(prompt),
            independent_reviewer_seen: false,
        }
    }
}

/// Parse `fork_context` / `forkContext` for subagent-style payloads.
///
/// Accepts JSON **boolean**, string spellings：`true`/`1`/`yes`/`y` vs `false`/`0`/`no`/`n`（trim + ASCII 小写），
/// or JSON **integer** `0` / `1` only as `false` / `true` for host interop (other numeric types / values → `None`).
/// See `docs/harness_architecture.md` §5.0.
fn fork_context_value_as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
            _ => None,
        },
        Value::Number(n) => match n.as_i64() {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn fork_context_from_values(primary: &Value, secondary: Option<&Value>) -> Option<bool> {
    primary
        .get("fork_context")
        .or_else(|| primary.get("forkContext"))
        .or_else(|| secondary.and_then(|value| value.get("fork_context")))
        .or_else(|| secondary.and_then(|value| value.get("forkContext")))
        .and_then(fork_context_value_as_bool)
}

/// 是否将子代理视为「独立 fork」：`fork_context`/`forkContext` **仅当**可解析为布尔 **`false`**（即 `Some(false)`）时为真。
///
/// **`None`（字段缺失）不为真**（本函数）。Cursor 可数深度 lane 另有
/// [`cursor_review_independent_fork`] 缺字段推断路径（env 默认开）；Claude 用
/// [`claude_review_independent_fork`]。勿与本函数注释混读。
pub(crate) fn independent_context_fork(fork: Option<bool>) -> bool {
    matches!(fork, Some(false))
}

#[allow(dead_code)]
pub(crate) fn independent_reviewer_evidence(review_lane: bool, fork: Option<bool>) -> bool {
    review_lane && cursor_review_independent_fork(fork, review_lane)
}

/// Claude Code: independent reviewer evidence; does **not** read Cursor fork-infer env (ADR-006).
pub(crate) fn claude_independent_reviewer_evidence(review_lane: bool, fork: Option<bool>) -> bool {
    review_lane && claude_review_independent_fork(fork, review_lane)
}

/// Claude-only: optional infer when `fork_context` is absent on a reviewer lane payload.
pub(crate) fn claude_review_independent_fork(fork: Option<bool>, deep_review_lane: bool) -> bool {
    if independent_context_fork(fork) {
        return true;
    }
    if fork == Some(true) {
        return false;
    }
    if !deep_review_lane {
        return false;
    }
    if !crate::router_env_flags::router_rs_claude_review_fork_context_missing_infer_false_enabled()
    {
        return false;
    }
    matches!(fork, None)
}

/// Cursor-only: when `fork_context` is absent on a deep review lane payload, optionally treat as
/// independent fork (`false`). Explicit `fork_context: true` never infers. Off when
/// `ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE` is disabled.
pub(crate) fn cursor_review_independent_fork(fork: Option<bool>, deep_review_lane: bool) -> bool {
    if independent_context_fork(fork) {
        return true;
    }
    if fork == Some(true) {
        return false;
    }
    if !deep_review_lane {
        return false;
    }
    if !crate::router_env_flags::router_rs_cursor_review_fork_context_missing_infer_false_enabled()
    {
        return false;
    }
    matches!(fork, None)
}

/// Codex CLI: missing `fork_context` on a deep review lane payload → independent fork when env enabled.
pub(crate) fn codex_review_independent_fork(fork: Option<bool>, deep_review_lane: bool) -> bool {
    if !deep_review_lane {
        return false;
    }
    if independent_context_fork(fork) {
        return true;
    }
    if fork == Some(true) {
        return false;
    }
    if !crate::router_env_flags::router_rs_codex_review_fork_context_missing_infer_false_enabled()
    {
        return false;
    }
    matches!(fork, None)
}

pub(crate) fn review_gate_armed(required: bool, override_seen: bool) -> bool {
    required && !override_seen
}

pub(crate) fn review_gate_blocks_stop(facts: ReviewGateFacts) -> bool {
    review_gate_armed(facts.review_required, facts.review_override)
        && !facts.independent_reviewer_seen
}

/// Codex wave-2 (partial): countable PostTool evidence before Stop compact may clear the gate.
/// Excludes legacy `phase>=2` alone (aligned with Cursor wave-2 / P0-4).
pub(crate) fn codex_countable_review_subagent_evidence(
    subagent_start_count: u32,
    independent_reviewer_seen: bool,
) -> bool {
    subagent_start_count > 0 || independent_reviewer_seen
}

/// Codex wave-2 Stop satisfaction: phase≥3 with PostTool evidence, or explicit reject/rg_clear.
pub(crate) fn codex_review_gate_satisfied(
    review_required: bool,
    review_override: bool,
    reject_reason_seen: bool,
    independent_reviewer_seen: bool,
    phase: u32,
) -> bool {
    if !review_gate_armed(review_required, review_override) {
        return true;
    }
    if review_override || reject_reason_seen {
        return true;
    }
    independent_reviewer_seen && phase >= 3
}

/// Bump Codex review phase to 3 when compact findings appear after countable PostTool evidence.
pub(crate) fn maybe_bump_codex_review_phase_for_compact_findings(
    review_required: bool,
    review_override: bool,
    phase: u32,
    subagent_start_count: u32,
    independent_reviewer_seen: bool,
    assistant_tail: &str,
) -> Option<u32> {
    if !review_gate_armed(review_required, review_override) || phase >= 3 {
        return None;
    }
    if !codex_countable_review_subagent_evidence(subagent_start_count, independent_reviewer_seen) {
        return None;
    }
    if !crate::review_output_lint::assistant_has_substantive_compact_review_finding_line(
        assistant_tail,
    ) {
        return None;
    }
    Some(phase.max(3))
}

#[cfg(test)]
mod fork_context_parse_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fork_context_from_values_accepts_integer_zero_and_one() {
        assert_eq!(
            fork_context_from_values(&json!({"fork_context": 0}), None),
            Some(false)
        );
        assert_eq!(
            fork_context_from_values(&json!({"fork_context": 1}), None),
            Some(true)
        );
        assert_eq!(
            fork_context_from_values(&json!({"fork_context": 2}), None),
            None
        );
    }

    #[test]
    fn fork_context_from_values_accepts_bool_false_and_string_false() {
        assert_eq!(
            fork_context_from_values(&json!({"fork_context": false}), None),
            Some(false)
        );
        assert_eq!(
            fork_context_from_values(&json!({"fork_context": "false"}), None),
            Some(false)
        );
    }

    #[test]
    fn codex_review_independent_fork_infers_missing_fork_on_deep_lane() {
        let _lock = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
        std::env::set_var(
            "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            "1",
        );
        assert!(codex_review_independent_fork(None, true));
        assert!(!codex_review_independent_fork(Some(true), true));
        assert!(
            !codex_review_independent_fork(Some(false), false),
            "explore-class lane must not count even with explicit fork_context false"
        );
        match prev {
            Some(v) => std::env::set_var(
                "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                v,
            ),
            None => {
                std::env::remove_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
            }
        }
    }

    #[test]
    fn cursor_review_independent_fork_infers_missing_fork_on_deep_lane() {
        let _lock = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
        std::env::set_var(
            "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            "1",
        );
        assert!(cursor_review_independent_fork(None, true));
        assert!(!cursor_review_independent_fork(Some(true), true));
        assert!(!cursor_review_independent_fork(None, false));
        match prev {
            Some(v) => std::env::set_var(
                "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                v,
            ),
            None => {
                std::env::remove_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
            }
        }
    }

    #[test]
    fn codex_countable_evidence_excludes_start_zero_without_independent() {
        assert!(!codex_countable_review_subagent_evidence(0, false));
        assert!(codex_countable_review_subagent_evidence(1, false));
        assert!(codex_countable_review_subagent_evidence(0, true));
    }

    #[test]
    fn codex_review_independent_fork_respects_infer_env_off() {
        let _lock = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
        std::env::set_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", "0");
        assert!(
            !codex_review_independent_fork(None, true),
            "infer off: missing fork must not count on deep lane"
        );
        assert!(
            codex_review_independent_fork(Some(false), true),
            "explicit fork_context false still counts"
        );
        match prev {
            Some(v) => std::env::set_var(
                "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                v,
            ),
            None => std::env::remove_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE"),
        }
    }

    #[test]
    fn codex_wave2_compact_bump_requires_countable_evidence() {
        let finding = "[P1] foo.rs:1 — substantive compact finding for gate";
        assert!(maybe_bump_codex_review_phase_for_compact_findings(
            true,
            false,
            0,
            0,
            false,
            finding,
        )
        .is_none());
        assert!(
            maybe_bump_codex_review_phase_for_compact_findings(
                true,
                false,
                2,
                0,
                false,
                finding,
            )
            .is_none(),
            "legacy phase>=2 alone must not count as countable evidence"
        );
        assert_eq!(
            maybe_bump_codex_review_phase_for_compact_findings(
                true,
                false,
                2,
                1,
                true,
                finding,
            ),
            Some(3)
        );
    }

    #[test]
    fn codex_review_gate_satisfied_wave2() {
        assert!(codex_review_gate_satisfied(true, false, false, true, 3));
        assert!(!codex_review_gate_satisfied(true, false, false, true, 2));
        assert!(codex_review_gate_satisfied(true, false, true, false, 0));
    }

    #[test]
    fn review_gate_facts_suppresses_review_when_goal_drive_in_same_prompt() {
        let dual = ReviewGateFacts::from_prompt("请全面review这个仓库 /implementx 修复刚发现的问题");
        assert!(
            !dual.review_required,
            "goal drive entry must suppress review arming in facts"
        );
        let review_only = ReviewGateFacts::from_prompt("全面review这个仓库");
        assert!(review_only.review_required);
    }
}
