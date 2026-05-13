use crate::hook_common::{has_override, is_review_prompt};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewGateFacts {
    pub review_required: bool,
    pub review_override: bool,
    pub independent_reviewer_seen: bool,
}

impl ReviewGateFacts {
    pub(crate) fn from_prompt(prompt: &str) -> Self {
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
/// **`None`（字段缺失）不为真**：与仓库根 `docs/harness_architecture.md` §5.0「排障」一致；缺省不能当作独立上下文证据（REVIEW_GATE 相位 / pre-goal）。
pub(crate) fn independent_context_fork(fork: Option<bool>) -> bool {
    matches!(fork, Some(false))
}

pub(crate) fn independent_reviewer_evidence(review_lane: bool, fork: Option<bool>) -> bool {
    review_lane && independent_context_fork(fork)
}

pub(crate) fn review_gate_armed(required: bool, override_seen: bool) -> bool {
    required && !override_seen
}

pub(crate) fn review_gate_blocks_stop(facts: ReviewGateFacts) -> bool {
    review_gate_armed(facts.review_required, facts.review_override)
        && !facts.independent_reviewer_seen
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
}
