//! `review_gate` lane sets from embedded [`RUNTIME_REGISTRY.json`](../../../configs/framework/RUNTIME_REGISTRY.json).
//! - `deep_gate_lanes`: Cursor / Codex countable deep-review lanes (`hook_common::is_deep_review_gate_lane_normalized`).
//! - `claude_reviewer_lanes`: Claude Code reviewer admission (`claude_hooks::reviewer_lane`).

use crate::lane_normalize::normalize_subagent_lane;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;

const REGISTRY_EMBED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/framework/RUNTIME_REGISTRY.json"
));

fn lane_set_from_registry(field: &str) -> HashSet<String> {
    let root: Value =
        serde_json::from_str(REGISTRY_EMBED).expect("RUNTIME_REGISTRY.json embed parse");
    let lanes = root
        .get("review_gate")
        .and_then(|v| v.get(field))
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!("RUNTIME_REGISTRY.review_gate.{field} must be a non-empty array")
        });
    let mut out = HashSet::new();
    for item in lanes {
        let s = item
            .as_str()
            .unwrap_or_else(|| panic!("review_gate.{field} entry must be string"));
        out.insert(normalize_subagent_lane(s));
    }
    assert!(
        !out.is_empty(),
        "RUNTIME_REGISTRY.review_gate.{field} must not be empty"
    );
    out
}

fn deep_gate_lane_set() -> &'static HashSet<String> {
    static CELL: OnceLock<HashSet<String>> = OnceLock::new();
    CELL.get_or_init(|| lane_set_from_registry("deep_gate_lanes"))
}

fn claude_reviewer_lane_set() -> &'static HashSet<String> {
    static CELL: OnceLock<HashSet<String>> = OnceLock::new();
    CELL.get_or_init(|| lane_set_from_registry("claude_reviewer_lanes"))
}

pub(crate) fn is_deep_review_gate_lane_from_registry(lane: &str) -> bool {
    let key = normalize_subagent_lane(lane);
    deep_gate_lane_set().contains(&key)
}

pub(crate) fn is_claude_reviewer_lane_from_registry(lane: &str) -> bool {
    let key = normalize_subagent_lane(lane);
    claude_reviewer_lane_set().contains(&key)
}

/// Shared Cursor/Codex countable deep-review lane smoke matrix (registry embed + normalization).
#[cfg(test)]
pub(crate) fn assert_deep_review_gate_lane_matrix() {
    assert!(is_deep_review_gate_lane_from_registry("general-purpose"));
    assert!(is_deep_review_gate_lane_from_registry("generalpurpose"));
    assert!(is_deep_review_gate_lane_from_registry("best-of-n-runner"));
    assert!(is_deep_review_gate_lane_from_registry("bestofnrunner"));
    assert!(!is_deep_review_gate_lane_from_registry("explore"));
    assert!(!is_deep_review_gate_lane_from_registry("ci-investigator"));
    assert!(!is_deep_review_gate_lane_from_registry("review"));
    assert!(!is_deep_review_gate_lane_from_registry("reviewer"));
    assert!(!is_deep_review_gate_lane_from_registry("critic"));
    assert!(!is_deep_review_gate_lane_from_registry("code-review"));
    assert!(is_deep_review_gate_lane_from_registry("General_Purpose"));
    assert!(is_deep_review_gate_lane_from_registry("Best_Of_N_Runner"));
}

/// Claude Code reviewer lane smoke matrix (superset of deep_gate_lanes).
#[cfg(test)]
pub(crate) fn assert_claude_reviewer_lane_matrix() {
    assert_deep_review_gate_lane_matrix();
    assert!(is_claude_reviewer_lane_from_registry("review"));
    assert!(is_claude_reviewer_lane_from_registry("reviewer"));
    assert!(is_claude_reviewer_lane_from_registry("critic"));
    assert!(is_claude_reviewer_lane_from_registry("code-review"));
    assert!(!is_claude_reviewer_lane_from_registry("explore"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_review_gate_lane_matrix() {
        assert_deep_review_gate_lane_matrix();
    }

    #[test]
    fn claude_reviewer_lane_matrix() {
        assert_claude_reviewer_lane_matrix();
    }
}
