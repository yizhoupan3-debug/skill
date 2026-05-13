//! Single source for subagent / review-gate lane token normalization (trim, lowercase, `_` → `-`).

/// Normalize a subagent lane or type string for comparison against registry / hook policy.
#[must_use]
pub fn normalize_subagent_lane(input: &str) -> String {
    input.trim().to_lowercase().replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_subagent_lane_vectors() {
        assert_eq!(
            normalize_subagent_lane("  General_Purpose  "),
            "general-purpose"
        );
        assert_eq!(
            normalize_subagent_lane("best-of-n-runner"),
            "best-of-n-runner"
        );
        assert_eq!(normalize_subagent_lane(""), "");
    }
}
