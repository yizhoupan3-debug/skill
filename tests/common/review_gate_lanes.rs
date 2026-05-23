//! Shared REVIEW_GATE lane closed-set assertions for policy + host_integration tests.
//! Normalization matches `router-rs` `lane_normalize::normalize_subagent_lane`.

use serde_json::Value;
use std::collections::HashSet;

pub fn normalize_review_gate_lane(s: &str) -> String {
    s.trim().to_lowercase().replace('_', "-")
}

pub fn review_gate_lane_set(v: &Value, field: &str) -> HashSet<String> {
    v["review_gate"][field]
        .as_array()
        .unwrap_or_else(|| panic!("review_gate.{field} must be array"))
        .iter()
        .map(|item| {
            normalize_review_gate_lane(
                item.as_str()
                    .unwrap_or_else(|| panic!("review_gate.{field} entry must be string")),
            )
        })
        .collect()
}

pub fn review_gate_lane_sets_from_registry(v: &Value) -> (HashSet<String>, HashSet<String>) {
    (
        review_gate_lane_set(v, "deep_gate_lanes"),
        review_gate_lane_set(v, "claude_reviewer_lanes"),
    )
}

pub fn expected_deep_gate_lanes() -> HashSet<String> {
    [
        "general-purpose",
        "generalpurpose",
        "best-of-n-runner",
        "bestofnrunner",
        "deep-reviewer",
        "deepreviewer",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub fn assert_review_gate_lane_sets_closed(deep: &HashSet<String>, claude: &HashSet<String>) {
    let expected_deep = expected_deep_gate_lanes();
    assert_eq!(
        *deep, expected_deep,
        "deep_gate_lanes must be exactly GP/bon spellings for Cursor/Codex"
    );

    for forbidden in [
        "review",
        "reviewer",
        "critic",
        "code-review",
        "explore",
        "architecture-review",
    ] {
        assert!(
            !deep.contains(forbidden),
            "deep_gate_lanes must not contain {forbidden}"
        );
    }

    assert!(
        !claude.is_empty(),
        "claude_reviewer_lanes must be non-empty"
    );
    for lane in &expected_deep {
        assert!(
            claude.contains(lane),
            "claude_reviewer_lanes must superset deep_gate_lanes ({lane})"
        );
    }
    for extra in ["review", "reviewer", "critic", "code-review"] {
        assert!(
            claude.contains(extra),
            "claude_reviewer_lanes must include Claude-only lane {extra}"
        );
    }
}
