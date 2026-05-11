use crate::hook_common::{has_override, has_review_override, is_review_prompt};
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
            review_override: has_review_override(prompt) || has_override(prompt),
            independent_reviewer_seen: false,
        }
    }
}

pub(crate) fn json_value_as_boolish(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_i64().map(|i| i != 0),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
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
        .and_then(json_value_as_boolish)
}

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
