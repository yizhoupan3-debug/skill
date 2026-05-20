//! `review_gate` lane sets from disk [`RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json).
//! See [`crate::registry_loader`] (ADR-005).

pub(crate) use crate::registry_loader::{
    assert_claude_reviewer_lane_matrix, assert_deep_review_gate_lane_matrix,
    is_claude_reviewer_lane_from_registry, is_deep_review_gate_lane_from_registry,
};
