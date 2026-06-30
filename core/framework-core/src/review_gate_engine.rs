//! Review Gate Engine — code review workflow gate.
//!
//! **This module controls code review workflow logic** (whether a review is required,
//! whether an independent reviewer has been seen, whether Stop should be blocked).
//!
//! **This is NOT the same as `quality-gate/`** (the QG Route crate).
//! - `review_gate_engine` = "does this code review session satisfy the review requirement?"
//! - `quality-gate` crate = "does this task's completion pass quality checkers?"
//!
//! The two systems are independent and serve different purposes:
//! - Review gate: gates the **Stop action** in code review flows (Cursor/Claude hosts)
//! - Quality gate: gates **task completion** in the goal engine (QG Route checker chain)

use crate::hook_common::{has_override, is_review_prompt};
use serde_json::Value;
use tracing::debug;

/// Cursor `REVIEW_GATE` evidence path: multiset (strict) vs id-only pending vec (lite).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewGateMode {
    Strict,
    Lite,
}

/// `ROUTER_RS_REVIEW_GATE_MODE=lite` enables lite; unset or `strict` → strict.
pub fn review_gate_mode() -> ReviewGateMode {
    match std::env::var("ROUTER_RS_REVIEW_GATE_MODE") {
        Ok(v) if v.trim().eq_ignore_ascii_case("lite") => ReviewGateMode::Lite,
        _ => ReviewGateMode::Strict,
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
            review_required: is_review_prompt(prompt),
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

/// 检查 `fork_context` 是否表示「独立 fork」。
///
/// # ⚠️ 命名反转说明
/// Claude Code 生态中 `fork_context` 的语义是反直觉的：
/// - `fork_context=false` → "此 subagent 已 fork 出独立上下文" → **是独立 review**
/// - `fork_context=true`  → "此 subagent 未 fork，共享主上下文" → 不是独立 review
///
/// 这是因为 `fork_context` 字段最初的含义是"是否需要从主上下文 fork 出来"：
/// 已经独立运行的 subagent 不需要再 fork，所以设为 `false`。
/// 这是一个历史命名错误，为保持向后兼容保留此约定。
pub fn fork_context_false_means_independent(fork: Option<bool>) -> bool {
    matches!(fork, Some(false))
}

/// Canonical cross-host: independent fork when `fork_context` parses as **`false`**, or when
/// missing on a reviewer lane and [`router_rs_review_fork_context_missing_infer_false_enabled`].
///
/// 参考 [`fork_context_false_means_independent`] 的命名反转说明。
pub fn review_independent_fork(fork: Option<bool>, reviewer_lane: bool) -> bool {
    if !reviewer_lane {
        return false;
    }
    if fork_context_false_means_independent(fork) {
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
pub fn review_independent_reviewer_evidence(fork: Option<bool>, review_lane: bool) -> bool {
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

/// Countable review subagent evidence (PostTool evidence before Stop compact).
pub fn countable_review_subagent_evidence(
    subagent_start_count: u32,
    independent_reviewer_seen: bool,
) -> bool {
    subagent_start_count > 0 || independent_reviewer_seen
}

/// Bump review phase to 3 when compact findings appear after countable PostTool evidence.
pub fn maybe_bump_review_phase_for_compact_findings(
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
    if !countable_review_subagent_evidence(subagent_start_count, independent_reviewer_seen) {
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::hook_common::install_review_prompt_test_deps;
    use serde_json::json;

    #[test]
    fn review_gate_mode_respects_lite_env() {
        let _lock = crate::test_env_sync::process_env_lock();
        crate::test_env_sync::with_env_var("ROUTER_RS_REVIEW_GATE_MODE", "lite", || {
            assert_eq!(review_gate_mode(), ReviewGateMode::Lite)
        });
        crate::test_env_sync::with_env_var("ROUTER_RS_REVIEW_GATE_MODE", "LITE", || {
            assert_eq!(review_gate_mode(), ReviewGateMode::Lite)
        });
        crate::test_env_sync::with_env_var_removed("ROUTER_RS_REVIEW_GATE_MODE", || {
            assert_eq!(review_gate_mode(), ReviewGateMode::Strict)
        });
        crate::test_env_sync::with_env_var("ROUTER_RS_REVIEW_GATE_MODE", "strict", || {
            assert_eq!(review_gate_mode(), ReviewGateMode::Strict)
        });
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
        crate::test_env_sync::with_env_var(
            "ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            "1",
            || {
                assert!(review_independent_fork(None, true));
                assert!(!review_independent_fork(Some(true), true));
                assert!(
                    !review_independent_fork(Some(false), false),
                    "explore-class lane must not count even with explicit fork_context false"
                );
            },
        );
    }

    #[test]
    fn legacy_host_fork_env_still_enables_canonical_infer() {
        let _lock = crate::test_env_sync::process_env_lock();
        crate::test_env_sync::with_env_var_removed(
            "ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            || {
                crate::test_env_sync::with_env_var(
                    "ROUTER_RS_CURSOR_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                    "1",
                    || {
                        assert!(review_independent_fork(None, true));
                        assert!(!review_independent_fork(Some(true), true));
                        assert!(!review_independent_fork(None, false));
                    },
                );
            },
        );
    }

    #[test]
    fn countable_evidence_excludes_start_zero_without_independent() {
        assert!(!countable_review_subagent_evidence(0, false));
        assert!(countable_review_subagent_evidence(1, false));
        assert!(countable_review_subagent_evidence(0, true));
    }

    #[test]
    fn review_independent_fork_respects_infer_env_off() {
        let _lock = crate::test_env_sync::process_env_lock();
        crate::test_env_sync::with_env_var_removed(
            "ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            || {
                crate::test_env_sync::with_env_var(
                    "ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
                    "0",
                    || {
                        assert!(
                            !review_independent_fork(None, true),
                            "infer off: missing fork must not count on reviewer lane"
                        );
                        assert!(
                            review_independent_fork(Some(false), true),
                            "explicit fork_context false still counts"
                        );
                    },
                );
            },
        );
    }

    #[test]
    fn wave2_compact_bump_requires_countable_evidence() {
        let finding = "[P1] foo.rs:1 — substantive compact finding for gate";
        assert!(
            maybe_bump_review_phase_for_compact_findings(true, false, 0, 0, false, finding,)
                .is_none()
        );
        assert!(
            maybe_bump_review_phase_for_compact_findings(true, false, 2, 0, false, finding,)
                .is_none(),
            "legacy phase>=2 alone must not count as countable evidence"
        );
        assert_eq!(
            maybe_bump_review_phase_for_compact_findings(true, false, 2, 1, true, finding,),
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
    fn review_gate_facts_from_review_prompt_snapshot() {
        install_review_prompt_test_deps();
        let facts = ReviewGateFacts::from_prompt("全面review这个仓库");
        insta::assert_debug_snapshot!(facts);
    }

    #[test]
    fn review_gate_blocks_stop_matrix_snapshot() {
        let scenarios = vec![
            (
                "armed-no-evidence",
                ReviewGateFacts {
                    review_required: true,
                    review_override: false,
                    independent_reviewer_seen: false,
                },
            ),
            (
                "cleared-by-evidence",
                ReviewGateFacts {
                    review_required: true,
                    review_override: false,
                    independent_reviewer_seen: true,
                },
            ),
            (
                "cleared-by-override",
                ReviewGateFacts {
                    review_required: true,
                    review_override: true,
                    independent_reviewer_seen: false,
                },
            ),
            (
                "not-required",
                ReviewGateFacts {
                    review_required: false,
                    review_override: false,
                    independent_reviewer_seen: false,
                },
            ),
        ];
        let results: Vec<(&str, bool, bool)> = scenarios
            .into_iter()
            .map(|(label, facts)| {
                let blocks_stop = review_gate_blocks_stop(facts);
                let satisfied = review_gate_satisfied(
                    facts.review_required,
                    facts.review_override,
                    facts.independent_reviewer_seen,
                );
                (label, blocks_stop, satisfied)
            })
            .collect();
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn review_independent_fork_matrix_snapshot() {
        let _lock = crate::test_env_sync::process_env_lock();
        crate::test_env_sync::with_env_var(
            "ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE",
            "1",
            || {
                let matrix: Vec<(&str, bool)> = vec![
                    (
                        "None, reviewer_lane=true",
                        review_independent_fork(None, true),
                    ),
                    (
                        "Some(true), reviewer_lane=true",
                        review_independent_fork(Some(true), true),
                    ),
                    (
                        "Some(false), reviewer_lane=true",
                        review_independent_fork(Some(false), true),
                    ),
                    (
                        "Some(false), reviewer_lane=false",
                        review_independent_fork(Some(false), false),
                    ),
                    (
                        "None, reviewer_lane=false",
                        review_independent_fork(None, false),
                    ),
                ];
                insta::assert_debug_snapshot!(matrix);
            },
        );
    }
}
