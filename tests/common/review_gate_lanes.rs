#![allow(dead_code)]

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

pub fn reviewer_lanes_from_registry(v: &Value) -> HashSet<String> {
    review_gate_lane_set(v, "reviewer_lanes")
}

pub fn expected_reviewer_lanes() -> HashSet<String> {
    [
        "general-purpose",
        "generalpurpose",
        "best-of-n-runner",
        "bestofnrunner",
        "deep-reviewer",
        "deepreviewer",
        "review",
        "reviewer",
        "critic",
        "code-review",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub fn assert_reviewer_lanes_closed(lanes: &HashSet<String>) {
    let expected = expected_reviewer_lanes();
    assert_eq!(
        *lanes, expected,
        "reviewer_lanes must match cross-host canonical closed-set"
    );

    for forbidden in ["explore", "ci-investigator", "architecture-review"] {
        assert!(
            !lanes.contains(forbidden),
            "reviewer_lanes must not contain {forbidden}"
        );
    }
}
