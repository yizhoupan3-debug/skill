use crate::hook_common::{has_override, is_framework_goal_entry_prompt, is_review_prompt};
use serde_json::Value;
use tracing::debug;

/// Cursor `REVIEW_GATE` evidence path: multiset (strict) vs id-only pending vec (lite).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorReviewGateMode {
    Strict,
    Lite,
}

/// `ROUTER_RS_REVIEW_GATE_MODE=lite` enables lite; unset or `strict` → strict.
/// Falls back to legacy `ROUTER_RS_CURSOR_REVIEW_GATE_MODE` for backward compat.
pub fn cursor_review_gate_mode() -> CursorReviewGateMode {
    match std::env::var("ROUTER_RS_REVIEW_GATE_MODE")
        .or_else(|_| std::env::var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE"))
    {
        Ok(v) if v.trim().eq_ignore_ascii_case("lite") => CursorReviewGateMode::Lite,
        _ => CursorReviewGateMode::Strict,
    }
}

pub fn cycle_key_eligible_for_lite(key: &str) -> bool {
    key.starts_with("id:")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewGateFacts {
    pub review_required: bool,
    pub review_override: bool,
    pub independent_reviewer_seen: bool,
}

impl ReviewGateFacts {
    pub fn from_prompt(prompt: &str) -> Self {
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
/// See `env_flags.rs` §5 for operator table.
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

pub fn fork_context_from_values(primary: &Value, secondary: Option<&Value>) -> Option<bool> {
    primary
        .get("fork_context")
        .or_else(|| primary.get("forkContext"))
        .or_else(|| secondary.and_then(|value| value.get("fork_context")))
        .or_else(|| secondary.and_then(|value| value.get("forkContext")))
        .and_then(fork_context_value_as_bool)
}

/// 是否将子代理视为「独立 fork」：`fork_context`/`forkContext` **仅当**可解析为布尔 **`false`**（即 `Some(false)`）时为真。
///
/// **`None`（字段缺失）不为真**（本函数）。缺字段推断见 [`review_independent_fork`]（env 默认关，Claude 语义）。
pub fn independent_context_fork(fork: Option<bool>) -> bool {
    matches!(fork, Some(false))
}

/// Canonical cross-host: independent fork when `fork_context` parses as **`false`**, or when
/// missing on a reviewer lane and [`router_rs_review_fork_context_missing_infer_false_enabled`].
pub fn review_independent_fork(fork: Option<bool>, reviewer_lane: bool) -> bool {
    if !reviewer_lane {
        return false;
    }
    if independent_context_fork(fork) {
        return true;
    }
    if fork == Some(true) {
        return false;
    }
    if !crate::env_flags::router_rs_review_fork_context_missing_infer_false_enabled() {
        return false;
    }
    fork.is_none()
}

/// Canonical cross-host independent reviewer evidence (Claude semantics).
pub fn review_independent_reviewer_evidence(review_lane: bool, fork: Option<bool>) -> bool {
    review_lane && review_independent_fork(fork, review_lane)
}

#[tracing::instrument(level = "debug", skip_all, fields(required, override_seen))]
pub fn review_gate_armed(required: bool, override_seen: bool) -> bool {
    let result = required && !override_seen;
    debug!(result, "review gate armed decision");
    result
}

/// Metrics / advisory detection: armed review without independent reviewer evidence.
/// L4 Stop must not hard-block on this alone when [`hook_common::review_gate_advisory_only`] is true.
pub fn review_gate_blocks_stop(facts: ReviewGateFacts) -> bool {
    review_gate_armed(facts.review_required, facts.review_override)
        && !facts.independent_reviewer_seen
}

/// Canonical Stop satisfaction: armed gate cleared by override or independent reviewer evidence.
pub fn review_gate_satisfied(
    review_required: bool,
    review_override: bool,
    independent_reviewer_seen: bool,
) -> bool {
    !review_gate_blocks_stop(ReviewGateFacts {
        review_required,
        review_override,
        independent_reviewer_seen,
    })
}

/// Codex telemetry: countable PostTool evidence before Stop compact (not a gate condition).
pub fn codex_countable_review_subagent_evidence(
    subagent_start_count: u32,
    independent_reviewer_seen: bool,
) -> bool {
    subagent_start_count > 0 || independent_reviewer_seen
}

/// Bump Codex review phase to 3 when compact findings appear after countable PostTool evidence.
pub fn maybe_bump_codex_review_phase_for_compact_findings(
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
    use crate::hook_common::install_review_prompt_test_deps;
    use serde_json::json;

    #[test]
    fn cursor_review_gate_mode_respects_lite_env() {
        let _lock = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CURSOR_REVIEW_GATE_MODE");
        unsafe { std::env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "lite") };
        assert_eq!(cursor_review_gate_mode(), CursorReviewGateMode::Lite);
        unsafe { std::env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "LITE") };
        assert_eq!(cursor_review_gate_mode(), CursorReviewGateMode::Lite);
        unsafe { std::env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") };
        assert_eq!(cursor_review_gate_mode(), CursorReviewGateMode::Strict);
        unsafe { std::env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", "strict") };
        assert_eq!(cursor_review_gate_mode(), CursorReviewGateMode::Strict);
        match prev {
            Some(v) => unsafe { std::env::set_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE", v) },
            None => unsafe { std::env::remove_var("ROUTER_RS_CURSOR_REVIEW_GATE_MODE") },
        }
    }

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
    fn review_independent_fork_infers_missing_fork_on_reviewer_lane_when_env_on() {
        let _lock = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
        unsafe { std::env::set_var("ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", "1") };
        assert!(review_independent_fork(None, true));
        assert!(!review_independent_fork(Some(true), true));
        assert!(
            !review_independent_fork(Some(false), false),
            "explore-class lane must not count even with explicit fork_context false"
        );
        match prev {
            Some(v) => unsafe {
                std::env::set_var("ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", v)
            },
            None => unsafe {
                std::env::remove_var("ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
            },
        }
    }

    #[test]
    fn legacy_host_fork_env_still_enables_canonical_infer() {
        let _lock = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
        unsafe { std::env::remove_var("ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE") };
        unsafe {
            std::env::set_var(
                "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                "1",
            );
        }
        assert!(review_independent_fork(None, true));
        assert!(!review_independent_fork(Some(true), true));
        assert!(!review_independent_fork(None, false));
        match prev {
            Some(v) => unsafe {
                std::env::set_var(
                    "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                    v,
                );
            },
            None => unsafe {
                std::env::remove_var("ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
            },
        }
    }

    #[test]
    fn codex_countable_evidence_excludes_start_zero_without_independent() {
        assert!(!codex_countable_review_subagent_evidence(0, false));
        assert!(codex_countable_review_subagent_evidence(1, false));
        assert!(codex_countable_review_subagent_evidence(0, true));
    }

    #[test]
    fn review_independent_fork_respects_infer_env_off() {
        let _lock = crate::test_env_sync::process_env_lock();
        let prev = std::env::var_os("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE");
        unsafe { std::env::remove_var("ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE") };
        unsafe {
            std::env::set_var(
                "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                "0",
            );
        }
        assert!(
            !review_independent_fork(None, true),
            "infer off: missing fork must not count on reviewer lane"
        );
        assert!(
            review_independent_fork(Some(false), true),
            "explicit fork_context false still counts"
        );
        match prev {
            Some(v) => unsafe {
                std::env::set_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE", v)
            },
            None => unsafe {
                std::env::remove_var("ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE")
            },
        }
    }

    #[test]
    fn codex_wave2_compact_bump_requires_countable_evidence() {
        let finding = "[P1] foo.rs:1 — substantive compact finding for gate";
        assert!(
            maybe_bump_codex_review_phase_for_compact_findings(true, false, 0, 0, false, finding,)
                .is_none()
        );
        assert!(
            maybe_bump_codex_review_phase_for_compact_findings(true, false, 2, 0, false, finding,)
                .is_none(),
            "legacy phase>=2 alone must not count as countable evidence"
        );
        assert_eq!(
            maybe_bump_codex_review_phase_for_compact_findings(true, false, 2, 1, true, finding,),
            Some(3)
        );
    }

    #[test]
    fn review_gate_satisfied_matches_blocks_stop() {
        assert!(!review_gate_satisfied(true, false, false));
        assert!(review_gate_satisfied(true, false, true));
        assert!(review_gate_satisfied(true, true, false));
        assert!(review_gate_satisfied(false, false, false));
    }

    #[test]
    fn review_gate_facts_suppresses_review_when_goal_drive_in_same_prompt() {
        install_review_prompt_test_deps();
        let dual =
            ReviewGateFacts::from_prompt("请全面review这个仓库 /implementx 修复刚发现的问题");
        assert!(
            !dual.review_required,
            "goal drive entry must suppress review arming in facts"
        );
        let review_only = ReviewGateFacts::from_prompt("全面review这个仓库");
        assert!(review_only.review_required);
    }
}
